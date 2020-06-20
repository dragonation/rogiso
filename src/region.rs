use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::mem::MaybeUninit;
use std::sync::Arc;

use super::base::Error;
use super::base::ErrorType::*;
use super::base::PrimitiveType;
use super::base::PrimitiveType::*;
use super::base::Symbol;
use super::base::Value;
use super::context::Context;
use super::field_shortcuts::FieldToken;
use super::field_shortcuts::FieldShortcuts;
use super::internal_slot::InternalSlot;
use super::internal_slot::ProtectedInternalSlot;
use super::reference_map::ReferenceMap;
use super::storage::Pinned;
use super::slot::RegionSlot;
use super::slot::SlotRecordSnapshot;
use super::trap::PropertyTrap;
use super::trap::SlotTrap;
use super::util::RwLock;
use super::util::ReentrantLockReadGuard;


/// Make region size equals `8 * 4 = 32 KiB`
const REGION_SLOT_SIZE: usize = 578; 

/// Make the region bitmap match the slot size
/// 
/// `ceil(REGION_SLOT_SIZE / 64)`
const REGION_BITMAP_SIZE: usize = 10; 

struct RegionRedirectionReference {
    redirection: Value,
    reference_map: RefCell<Box<ReferenceMap>>
}

impl RegionRedirectionReference {

    fn is_empty(&self) -> bool {
        self.reference_map.borrow().is_empty()
    }

    fn add_reference(&self, value: Value) -> Result<(), Error> {
        self.reference_map.borrow().add_reference(value)
    }

    fn remove_reference(&self, value: Value) -> Result<(), Error> {
        self.reference_map.borrow().remove_reference(value)
    }

}

// Region definition
pub struct Region {

    id: u32,

    rw_lock: RwLock,

    occupied: Cell<u16>,
    next_empty_slot_index: Cell<u16>,

    bitmap: RefCell<[u64; REGION_BITMAP_SIZE]>,
    empties: RefCell<[u64; REGION_BITMAP_SIZE]>,

    redirection_rw_lock: RwLock,
    redirections: RefCell<HashMap<Value, RegionRedirectionReference>>,
    redirection_froms: RefCell<HashMap<Value, HashSet<Value>>>,

    nursery: RefCell<HashSet<Value>>,
    slots: [RegionSlot; REGION_SLOT_SIZE] 

    // TODO: add more fields
    // base_color: u8

}

// Region constructor
impl Region {

    pub fn new(id: u32) -> Region {

        let region = Region {

            id: id,

            rw_lock: RwLock::new(),

            occupied: Cell::new(0),
            next_empty_slot_index: Cell::new(0),

            bitmap: RefCell::new([0; REGION_BITMAP_SIZE]),
            empties: RefCell::new([!0; REGION_BITMAP_SIZE]),

            redirection_rw_lock: RwLock::new(),
            redirections: RefCell::new(HashMap::new()),
            redirection_froms: RefCell::new(HashMap::new()),

            slots: {
                let mut array: [MaybeUninit<RegionSlot>; REGION_SLOT_SIZE] = unsafe { 
                    MaybeUninit::uninit().assume_init() 
                };
                for (index, slot) in array.iter_mut().enumerate() {
                    let region_slot = RegionSlot::new(id, index as u32);
                    *slot = MaybeUninit::new(region_slot);
                }
                unsafe { 
                    std::mem::transmute::<_, [RegionSlot; REGION_SLOT_SIZE]>(array) 
                }
            },
            nursery: RefCell::new(HashSet::new())

        };

        region

    }

}

// Region basic properties
impl Region {

    #[inline]
    pub fn is_full(&self) -> bool {
        let _guard = self.rw_lock.lock_read();
        self.is_full_without_lock()
    }

    #[inline]
    fn is_full_without_lock(&self) -> bool {
        (self.occupied.get() as usize) == REGION_SLOT_SIZE
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        let _guard = self.rw_lock.lock_read();
        self.is_empty_without_lock()
    }

    #[inline]
    pub fn is_empty_without_lock(&self) -> bool {
        (self.occupied.get() == 0) && self.redirections.borrow().is_empty()
    }

    #[inline]
    pub fn need_refragment(&self) -> f32 {
        let _guard = self.rw_lock.lock_read();
        self.need_refragment_without_lock()
    }

    #[inline]
    pub fn need_refragment_without_lock(&self) -> f32 {
        let next_empty_slot_index = self.next_empty_slot_index.get();
        if next_empty_slot_index == 0 {
            0.0
        } else if next_empty_slot_index < REGION_SLOT_SIZE as u16 {
            1.0 - self.occupied.get() as f32 / next_empty_slot_index as f32
        } else {
            1.0 - self.occupied.get() as f32 / REGION_SLOT_SIZE as f32
        }
    }

    #[inline]
    pub fn could_gain_slot_quickly(&self) -> bool {
        let _guard = self.rw_lock.lock_read();
        self.could_gain_slot_quickly_without_lock()
    }

    #[inline]
    pub fn could_gain_slot_quickly_without_lock(&self) -> bool {
        self.next_empty_slot_index.get() != REGION_SLOT_SIZE as u16
    }

}

// Region slot checkers
impl Region {

    #[inline]
    fn ensure_slot_referencable(&self, slot: Value) -> Result<u32, Error> {

        if self.id != slot.get_region_id()? {
            return Err(Error::new(FatalError, "Incorrect region ID"));
        }

        let slot = slot.get_region_slot()?;

        let offset = (slot >> 6) as usize;
        let shift = slot & 0x3f;

        if (self.empties.borrow()[offset] >> shift) & 0b1 != 0 {
            return Err(Error::new(FatalError, "Incorrect slot state"));
        }

        Ok(slot)

    }

    #[inline]
    fn ensure_slot_available(&self, slot: Value) -> Result<u32, Error> {

        if self.id != slot.get_region_id()? {
            return Err(Error::new(FatalError, "Incorrect region ID"));
        }

        let slot = slot.get_region_slot()?;

        let offset = (slot >> 6) as usize;
        let shift = slot & 0x3f;

        if (self.bitmap.borrow()[offset] >> shift) & 0b1 == 0 {
            return Err(Error::new(FatalError, "Incorrect slot state"));
        }

        if (self.empties.borrow()[offset] >> shift) & 0b1 != 0 {
            return Err(Error::new(FatalError, "Incorrect slot state"));
        }

        Ok(slot)

    }

}

// Region slot managments 
impl Region {

    pub fn gain_slot(&self, primitive_type: PrimitiveType) -> Result<Value, Error> {

        match primitive_type {
            Undefined => { return Err(Error::new(FatalError, "Region slot is not available for undefined type")); },
            Null => { return Err(Error::new(FatalError, "Region slot is not available for null type")); },
            Boolean => { return Err(Error::new(FatalError, "Region slot is not available for boolean type")); },
            Integer => { return Err(Error::new(FatalError, "Region slot is not available for integer type")); },
            Float => { return Err(Error::new(FatalError, "Region slot is not available for float type")); },
            Symbol => { return Err(Error::new(FatalError, "Region slot is not available for symbol type")); },
            Text => {},
            List => {},
            Tuple => {},
            Object => {}
        }

        let (id, record) = {

            let _guard = self.rw_lock.lock_write();

            if self.is_full_without_lock() {
                return Err(Error::new(OutOfSpace, "Out of slots"));
            }

            if !self.could_gain_slot_quickly_without_lock() {
                return Err(Error::new(OutOfSpace, "Out of slots"));
            }

            let slot = self.next_empty_slot_index.get();

            let offset = (slot >> 6) as usize;
            let shift = slot & 0x3f;

            if (self.bitmap.borrow()[offset] >> shift) & 0b1 != 0 {
                return Err(Error::new(FatalError, "Incorrect slot state"));
            }
            if (self.empties.borrow()[offset] >> shift) & 0b1 == 0 {
                return Err(Error::new(FatalError, "Incorrect slot state"));
            }

            self.bitmap.borrow_mut()[offset] |= 0b1 << shift;
            self.empties.borrow_mut()[offset] &= !(0b1 << shift);

            self.occupied.set(self.occupied.get() + 1);
            self.next_empty_slot_index.set(self.next_empty_slot_index.get() + 1);

            let id = match primitive_type {
                Undefined => { return Err(Error::new(FatalError, "Region slot is not available for undefined type")); },
                Null => { return Err(Error::new(FatalError, "Region slot is not available for null type")); },
                Boolean => { return Err(Error::new(FatalError, "Region slot is not available for boolean type")); },
                Integer => { return Err(Error::new(FatalError, "Region slot is not available for integer type")); },
                Float => { return Err(Error::new(FatalError, "Region slot is not available for float type")); },
                Symbol => { return Err(Error::new(FatalError, "Region slot is not available for symbol type")); },
                Text => { Value::make_text(self.id, slot as u32) },
                List => { Value::make_list(self.id, slot as u32) },
                Tuple => { Value::make_tuple(self.id, slot as u32) },
                Object => { Value::make_object(self.id, slot as u32) }
            };

            self.nursery.borrow_mut().insert(id);

            (id, &self.slots[slot as usize])

        };

        record.mark_as_alive();
        record.overwrite_primitive_type(primitive_type)?;

        Ok(id)
        
    }

    pub fn recycle_slot(&self, value: Value, drop_value: bool, context: &Box<dyn Context>) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_write();

            if self.id != value.get_region_id()? {
                return Err(Error::new(FatalError, "Incorrect region ID"));
            }

            let slot = value.get_region_slot()?;

            let offset = (slot >> 6) as usize;
            let shift = slot & 0x3f;

            if (self.bitmap.borrow()[offset] >> shift) & 0b1 == 0 {
                return Err(Error::new(FatalError, "Incorrect slot state"));
            }

            if self.nursery.borrow().get(&value).is_some() {
                return Err(Error::new(FatalError, "Value in nursery"));
            }

            let record = &self.slots[slot as usize];

            if record.is_alive() && (!record.has_no_outer_references()?) {
                return Err(Error::new(FatalError, "Slot has outer references"));
            }

            {
                let _guard = self.redirection_rw_lock.lock_read();
                if self.redirection_froms.borrow().get(&value).is_some() {
                    return Err(Error::new(FatalError, "Slot has outer references"));
                }
            }

            if drop_value {
                self.empties.borrow_mut()[offset] |= 1 << shift;
                self.occupied.set(self.occupied.get() - 1);
            }

            self.bitmap.borrow_mut()[offset] &= !(1 << shift);
            self.nursery.borrow_mut().remove(&value);

            record

        };

        record.recycle(drop_value, context)?;

        Ok(())

    }

    pub fn recalculate_next_empty_slot_index(&self) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        if self.is_full_without_lock() {
            return Ok(());
        }

        let mut slot = self.next_empty_slot_index.get();
        loop {
            let offset = (slot >> 6) as usize;
            let shift = slot & 0x3f;
            if ((self.bitmap.borrow()[offset] >> shift) & 0b1 == 1) ||
                ((self.empties.borrow()[offset] >> shift) & 0b1 == 0) {
                slot += 1;
                break;
            }
            if slot == 0 {
                break;
            }
            slot -= 1;
        }

        self.next_empty_slot_index.set(slot);

        Ok(())

    }

}

// Region slot redirections
impl Region {

    #[inline]
    pub fn resolve_redirection(&self, value: Value) -> Result<Value, Error> {

        let _guard = self.rw_lock.lock_read();

        if self.id != value.get_region_id()? {
            return Err(Error::new(FatalError, "Incorrect region ID"));
        }

        let _guard_2 = self.redirection_rw_lock.lock_read();

        let slot = value.get_region_slot()?;

        let offset = (slot >> 6) as usize;
        let shift = slot & 0x3f;

        match self.redirections.borrow().get(&value) {
            None => {
                if (self.bitmap.borrow()[offset] >> shift) & 0b1 == 0 {
                    return Err(Error::new(FatalError, "Incorrect slot state"));
                }
                Ok(value)
            },
            Some(reference) => Ok(reference.redirection)
        }

    }

    pub fn redirect_slot(&self, value: Value, redirection: Value, reference_map: Option<Box<ReferenceMap>>) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        self.redirect_slot_without_lock(value, redirection, reference_map)

    }

    pub fn redirect_slot_without_lock(&self, value: Value, redirection: Value, reference_map: Option<Box<ReferenceMap>>) -> Result<(), Error> {

        if self.id != value.get_region_id()? {
            return Err(Error::new(FatalError, "Incorrect region ID"));
        }

        let slot = value.get_region_slot()?;

        let offset = (slot >> 6) as usize;
        let shift = slot & 0x3f;

        if (self.bitmap.borrow()[offset] >> shift) & 0b1 == 0 {
            return Err(Error::new(FatalError, "Incorrect slot state"));
        }

        let record = &self.slots[slot as usize];
        if record.is_alive() {
            return Err(Error::new(FatalError, "Incorrect slot state"));
        }

        if reference_map.is_some() {
            let _guard = self.redirection_rw_lock.lock_write();
            self.redirections.borrow_mut().insert(value, RegionRedirectionReference {
                redirection: redirection, 
                reference_map: RefCell::new(reference_map.unwrap())
            });
        }

        self.nursery.borrow_mut().remove(&value);

        Ok(())

    }

    pub fn move_out_from_nursery(&self, value: Value) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        if self.id != value.get_region_id()? {
            return Err(Error::new(FatalError, "Incorrect region ID"));
        }

        let slot = value.get_region_slot()?;

        let offset = (slot >> 6) as usize;
        let shift = slot & 0x3f;

        if (self.bitmap.borrow()[offset] >> shift) & 0b1 == 0 {
            return Err(Error::new(FatalError, "Incorrect slot state"));
        }

        self.nursery.borrow_mut().remove(&value);

        Ok(())

    }

    pub fn is_value_alive(&self, value: Value) -> Result<bool, Error> {

        let _guard = self.rw_lock.lock_read();

        if self.id != value.get_region_id()? {
            return Err(Error::new(FatalError, "Incorrect region ID"));
        }

        let slot = value.get_region_slot()?;

        let offset = (slot >> 6) as usize;
        let shift = slot & 0x3f;

        if (self.bitmap.borrow()[offset] >> shift) & 0b1 == 0 {
            return Ok(false);
        }

        Ok(self.slots[slot as usize].is_alive())

    }

    pub fn is_value_occupied(&self, value: Value) -> Result<bool, Error> {

        let _guard = self.rw_lock.lock_read();

        if self.id != value.get_region_id()? {
            return Err(Error::new(FatalError, "Incorrect region ID"));
        }

        let slot = value.get_region_slot()?;

        let offset = (slot >> 6) as usize;
        let shift = slot & 0x3f;

        Ok((self.empties.borrow()[offset] >> shift) & 0b1 == 0)

    }

    pub fn remove_redirection_from(&self, from: Value, to: Value) -> Result<bool, Error> {

        let _guard = self.rw_lock.lock_write();
        let _guard_2 = self.redirection_rw_lock.lock_write();

        let mut redirection_froms = self.redirection_froms.borrow_mut();
        if redirection_froms.get(&to).is_none() {
            return Err(Error::new(FatalError, "No redirection from found"));
        }

        redirection_froms.get_mut(&to).unwrap().remove(&from);
        if redirection_froms.get(&to).unwrap().is_empty() {
            redirection_froms.remove(&to);
            Ok(true)
        } else {
            Ok(false)
        }

    }

}

// Region slot snapshots
impl Region {

    pub fn freeze_slot(&self, slot: Value) 
        -> Result<(SlotRecordSnapshot, bool, Option<Box<ReferenceMap>>, Vec<Value>, Vec<Symbol>), Error> {

        let _guard = self.rw_lock.lock_write();

        let slot = self.ensure_slot_available(slot)?;

        let record = &self.slots[slot as usize];

        let in_nursery = self.nursery.borrow().get(&record.get_id()?).is_some();

        let (snapshot, reference_map, removed_values, removed_symbols) = record.freeze()?;

        Ok((snapshot, in_nursery, reference_map, removed_values, removed_symbols))

    }

    pub fn restore_slot(&self, 
                        from: Value, snapshot: SlotRecordSnapshot, 
                        in_nursery: bool, reference_map: &Option<Box<ReferenceMap>>) 
        -> Result<(Value, Vec<Value>, Vec<Symbol>), Error> {

        let record = {

            let _guard = self.rw_lock.lock_write();

            if self.is_full_without_lock() {
                return Err(Error::new(OutOfSpace, "Out of slots"));
            }

            let mut slot = 0;
            let (offset, shift) = loop {
                let offset = (slot >> 6) as usize;
                let shift = slot & 0x3f;
                if ((self.bitmap.borrow()[offset] >> shift) & 0b1 == 0) &&
                    ((self.empties.borrow()[offset] >> shift) & 0b1 == 1) {
                    break (offset, shift);
                }
                slot += 1;
                if slot >= REGION_SLOT_SIZE {
                    return Err(Error::new(OutOfSpace, "No empty slot is available"));
                }
            };

            if slot >= self.next_empty_slot_index.get() as usize {
                self.next_empty_slot_index.set((slot + 1) as u16);
            }

            self.bitmap.borrow_mut()[offset] |= 0b1u64 << shift;
            self.empties.borrow_mut()[offset] &= !(0b1u64 << shift);

            self.occupied.set(self.occupied.get() + 1);

            &self.slots[slot as usize]

        };

        let (id, added_values, added_symbols) = record.restore(snapshot)?;

        if in_nursery {
            let _guard = self.rw_lock.lock_write();
            self.nursery.borrow_mut().insert(id);
        }

        if reference_map.is_some() {
            let _guard = self.rw_lock.lock_write();
            let _guard_2 = self.redirection_rw_lock.lock_write();
            let mut redirection_froms = self.redirection_froms.borrow_mut();
            if redirection_froms.get(&id).is_none() {
                redirection_froms.insert(id, HashSet::new());
            }
            redirection_froms.get_mut(&id).unwrap().insert(from);
        }

        Ok((id, added_values, added_symbols))

    }

}

// Region slot references
impl Region {

    pub fn add_reference(&self, reference: Value, from: Value) -> Result<(), Error> {

        let (record, removing_nursery) = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_referencable(reference)?;

            {
                let _guard = self.redirection_rw_lock.lock_read();
                if let Some(reference_map) = self.redirections.borrow().get(&reference) {
                    return reference_map.add_reference(from);
                }
            }

            (&self.slots[slot as usize], self.nursery.borrow().get(&reference).is_some())

        };

        record.add_outer_reference(from)?;

        if removing_nursery {
            let _guard = self.rw_lock.lock_write();
            self.nursery.borrow_mut().remove(&reference);
        }

        Ok(())

    }

    pub fn remove_reference(&self, reference: Value, from: Value) -> Result<(bool, Value), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_referencable(reference)?;

            {
                let _guard = self.redirection_rw_lock.lock_read();
                let mut has_reference_map = false;
                let mut reference_map_is_empty = false;
                let mut redirection = Value::make_undefined();
                let offset = (slot >> 6) as usize;
                let shift = slot & 0x3f;
                if let Some(reference_map) = self.redirections.borrow().get(&reference) {
                    has_reference_map = true;
                    redirection = reference_map.redirection;
                    if (self.empties.borrow()[offset] >> shift) & 0b1 == 1 {
                        return Err(Error::new(FatalError, "Invalid slot state"));
                    }
                    reference_map.remove_reference(from)?;
                    reference_map_is_empty = reference_map.is_empty();
                }
                if has_reference_map {
                    if reference_map_is_empty {
                        self.redirections.borrow_mut().remove(&reference);
                        self.empties.borrow_mut()[offset] |= 0b1 << shift;
                        self.occupied.set(self.occupied.get() - 1);
                    }
                    return Ok((reference_map_is_empty, redirection));
                }
            }

            &self.slots[slot as usize]
        };

        record.remove_outer_reference(from)?;

        Ok((false, Value::make_undefined()))

    }

}

// Region slot seal
impl Region {

    pub fn is_sealed(&self, value: Value) -> Result<bool, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.is_sealed()

    }

    pub fn seal_slot(&self, value: Value) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.seal_slot()

    }

}

// Region slot trap
impl Region {

    pub fn has_slot_trap(&self, value: Value) -> Result<bool, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.has_slot_trap()

    }

    pub fn set_slot_trap(&self, value: Value, slot_trap: Arc<dyn SlotTrap>, context: &Box<dyn Context>) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.set_slot_trap(slot_trap, context)

    }

    pub fn clear_slot_trap(&self, value: Value, context: &Box<dyn Context>) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.clear_slot_trap(context)

    }

}

// Region field shortcuts
impl Region {

    pub fn has_field_shortcuts(&self, value: Value) -> Result<bool, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.has_field_shortcuts()
 
    }

    pub fn get_field_shortcuts(&self, value: Value) -> Result<Option<Arc<FieldShortcuts>>, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.get_field_shortcuts()

    }

    pub fn update_field_shortcuts(&self, value: Value, field_shortcuts: Arc<FieldShortcuts>) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.set_field_shortcuts(field_shortcuts)?;

        Ok(())

    }

    pub fn clear_field_shortcuts(&self, value: Value) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.clear_field_shortcuts()?;

        Ok(())
        
    }

}

// Region slot internal slots
impl Region {

    pub fn has_internal_slot(&self, subject: Value, id: u64) -> Result<bool, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(subject)?;

            &self.slots[slot as usize]

        };

        record.has_internal_slot(id)

    }

    pub fn list_internal_slot_ids(&self, subject: Value) -> Result<Vec<u64>, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(subject)?;

            &self.slots[slot as usize]

        };

        record.list_internal_slot_ids()

    }

    pub fn set_internal_slot(&self, subject: Value, id: u64, internal_slot: Arc<dyn InternalSlot>, context: &Box<dyn Context>) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(subject)?;

            &self.slots[slot as usize]

        };

        record.set_internal_slot(id, internal_slot, context)

    }

    pub fn clear_internal_slot(&self, subject: Value, id: u64, context: &Box<dyn Context>) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(subject)?;

            &self.slots[slot as usize]

        };

        record.clear_internal_slot(id, context)

    }

    pub fn get_internal_slot<'a>(&self, subject: Value, id: u64, context: &'a Box<dyn Context>) -> Result<Option<ProtectedInternalSlot::<'a>>, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(subject)?;

            &self.slots[slot as usize]

        };

        record.get_internal_slot(id, context)

    }

}

impl Region {

    pub fn get_prototype_with_layout_guard(&self, subject: Value, context: &Box<dyn Context>, layout_guard: ReentrantLockReadGuard, no_redirection: bool) -> Result<Pinned, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(subject)?;

            &self.slots[slot as usize]

        };

        record.get_prototype_with_layout_guard(context, layout_guard)

    }

    pub fn set_prototype_with_layout_guard(&self, subject: Value, prototype: Value, context: &Box<dyn Context>, layout_guard: ReentrantLockReadGuard, no_redirection: bool) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(subject)?;

            &self.slots[slot as usize]

        };

        record.set_prototype_with_layout_guard(prototype, context, layout_guard, no_redirection)

    }

    pub fn set_prototype_ignore_slot_trap(&self, subject: Value, prototype: Value, context: &Box<dyn Context>) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(subject)?;

            &self.slots[slot as usize]

        };

        record.set_prototype_ignore_slot_trap(prototype, context)

    }

}

// Region slot own properties
impl Region {

    pub fn get_own_property_with_layout_guard(&self, id: Value, subject: Value, symbol: Symbol, field_token: Option<&FieldToken>, context: &Box<dyn Context>, layout_guard: ReentrantLockReadGuard, no_redirection: bool) -> Result<Pinned, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(id)?;

            &self.slots[slot as usize]

        };

        record.get_own_property_with_layout_guard(subject, symbol, field_token, context, layout_guard, no_redirection)

    }

    pub fn get_own_property_ignore_slot_trap(&self, id: Value, subject: Value, symbol: Symbol, context: &Box<dyn Context>) -> Result<Pinned, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(id)?;

            &self.slots[slot as usize]

        };

        record.get_own_property_ignore_slot_trap(subject, symbol, context)

    }

    pub fn overwrite_own_property(&self, id: Value, symbol: Symbol, value: Value) -> Result<(Vec<Value>, Vec<Symbol>, Vec<Value>, Vec<Symbol>), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(id)?;

            &self.slots[slot as usize]

        };

        record.overwrite_own_property(symbol, value)

    }

    pub fn set_own_property_with_layout_guard<'a>(&self, id: Value, subject: Value, symbol: Symbol, value: Value, context: &Box<dyn Context>, layout_guard: ReentrantLockReadGuard<'a>, no_redirection: bool) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(id)?;

            &self.slots[slot as usize]

        };

        record.set_own_property_with_layout_guard(subject, symbol, value, context, layout_guard, no_redirection)
 
    }

    pub fn set_own_property_ignore_slot_trap(&self, id: Value, subject: Value, symbol: Symbol, value: Value, context: &Box<dyn Context>) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(id)?;

            &self.slots[slot as usize]

        };

        record.set_own_property_ignore_slot_trap(subject, symbol, value, context)
 
    }

    pub fn define_own_property_with_layout_guard<'a>(&self, id: Value, subject: Value, symbol: Symbol, property_trap: Arc<dyn PropertyTrap>, context: &Box<dyn Context>, layout_guard: ReentrantLockReadGuard<'a>, no_redirection: bool) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(id)?;

            &self.slots[slot as usize]

        };

        record.define_own_property_with_layout_guard(subject, symbol, property_trap, context, layout_guard, no_redirection)
        
    }

    pub fn define_own_property_ignore_slot_trap(&self, id: Value, subject: Value, symbol: Symbol, property_trap: Arc<dyn PropertyTrap>, context: &Box<dyn Context>) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(id)?;

            &self.slots[slot as usize]

        };

        record.define_own_property_ignore_slot_trap(subject, symbol, property_trap, context)
        
    }

    pub fn delete_own_property_with_layout_guard<'a>(&self, id: Value, subject: Value, symbol: Symbol, context: &Box<dyn Context>, layout_guard: ReentrantLockReadGuard<'a>, no_redirection: bool) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(id)?;

            &self.slots[slot as usize]

        };

        record.delete_own_property_with_layout_guard(subject, symbol, context, layout_guard, no_redirection)
        
    }

    pub fn delete_own_property_ignore_slot_trap(&self, id: Value, subject: Value, symbol: Symbol, context: &Box<dyn Context>) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(id)?;

            &self.slots[slot as usize]

        };

        record.delete_own_property_ignore_slot_trap(subject, symbol, context)
        
    }

    pub fn has_own_property_with_layout_guard(&self, id: Value, subject: Value, symbol: Symbol, context: &Box<dyn Context>, layout_guard: ReentrantLockReadGuard)  -> Result<bool, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(id)?;

            &self.slots[slot as usize]

        };

        record.has_own_property_with_layout_guard(subject, symbol, context, layout_guard)
        
    }

    pub fn list_own_property_symbols_with_layout_guard<'a>(&self, id: Value, subject: Value, context: &Box<dyn Context>, layout_guard: ReentrantLockReadGuard<'a>, no_redirection: bool)  -> Result<Vec<Symbol>, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(id)?;

            &self.slots[slot as usize]

        };

        record.list_own_property_symbols_with_layout_guard(subject, context, layout_guard, no_redirection)
        
    }

    pub fn list_own_property_symbols_ignore_slot_trap(&self, id: Value, subject: Value, context: &Box<dyn Context>)  -> Result<Vec<Symbol>, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(id)?;

            &self.slots[slot as usize]

        };

        record.list_own_property_symbols_ignore_slot_trap(subject, context)
        
    }

}

/// Slot color
impl Region {

    pub fn list_alive_values(&self) -> Result<Vec<Value>, Error> {

        let _guard = self.rw_lock.lock_write();

        let mut values = Vec::new();

        let mut slot = 0;
        while slot < REGION_SLOT_SIZE {
            let record = &self.slots[slot];
            let offset = (slot >> 6) as usize;
            let shift = slot & 0x3f;
            if ((self.bitmap.borrow()[offset] >> shift) & 0b1 == 1) && record.is_alive() {
                let id = record.get_id()?;
                values.push(id);
            }
            slot += 1;
        }

        Ok(values)

    }

    pub fn list_and_autorefresh_referenced_values(&self, value: Value, context: &Box<dyn Context>) -> Result<(Vec<Value>, Vec<Symbol>), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.list_and_autorefresh_self_references(context)

    }

    pub fn list_values_in_nursery(&self) -> Vec<Value> {

        let _guard = self.rw_lock.lock_read();

        let mut values = Vec::new();

        for value in self.nursery.borrow().iter() {
            values.push(*value);
        }

        values

    }

    pub fn sweep_values(&self, base: u8, context: &Box<dyn Context>) -> Result<(), Error> {

        let records = {

            let mut records = Vec::new();

            let _guard = self.rw_lock.lock_write();

            let mut slot = 0;
            while slot < REGION_SLOT_SIZE {

                let record = &self.slots[slot];

                let offset = (slot >> 6) as usize;
                let shift = slot & 0x3f;

                if ((self.bitmap.borrow()[offset] >> shift) & 0b1 == 1) && 
                   record.is_alive() && record.is_white(base)? {

                    let id = record.get_id()?;

                    let reference_map = record.sweep_outer_reference_map()?;

                    let reference_map_is_none = reference_map.is_none();
                    self.redirect_slot_without_lock(id, Value::make_undefined(), reference_map)?;
                    if reference_map_is_none {
                        records.push(record);
                    }

                    self.empties.borrow_mut()[offset] |= 1 << shift;
                    self.occupied.set(self.occupied.get() - 1);

                    self.bitmap.borrow_mut()[offset] &= !(1 << shift);
                    self.nursery.borrow_mut().remove(&id);


                }
                slot += 1;
            }

            records
        };

        for record in records {
            record.recycle(true, context)?;
        }

        Ok(())

    }

    pub fn mark_as_white(&self, value: Value, base: u8) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.mark_as_white(base)

    }

    pub fn mark_as_black(&self, value: Value, base: u8) -> Result<(), Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.mark_as_black(base)
 
    }

    pub fn mark_as_gray(&self, value: Value, base: u8) -> Result<bool, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        if record.is_white(base)? {
            record.mark_as_gray(base)?;
            Ok(true)
        } else {
            Ok(false)
        }
 
    }

    pub fn is_white(&self, value: Value, base: u8) -> Result<bool, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.is_white(base)
 
    }

    pub fn is_black(&self, value: Value, base: u8) -> Result<bool, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.is_black(base)
 
    }

    pub fn is_gray(&self, value: Value, base: u8) -> Result<bool, Error> {

        let record = {

            let _guard = self.rw_lock.lock_read();

            let slot = self.ensure_slot_available(value)?;

            &self.slots[slot as usize]

        };

        record.is_gray(base)
 
    }

}


#[cfg(test)] use super::field_shortcuts::FieldTemplate;
#[cfg(test)] use super::isolate::Isolate;
#[cfg(test)] use super::test::TestContext;
#[cfg(test)] use super::test::TestInternalSlot;
#[cfg(test)] use super::test::TestPropertyTrap;
#[cfg(test)] use super::test::TestSlotTrap2;

#[test]
fn test_region_size() {
    assert_eq!(REGION_BITMAP_SIZE, (REGION_SLOT_SIZE as f32 / 64.0).ceil() as usize);
    assert_eq!(std::mem::size_of::<Region>() % 4096, 0);
}

#[test]
fn test_region_creation() {

    let _region = Region::new(0);

}

#[test]
fn test_region_basic_states() {

    let region = Region::new(0);

    assert!(!region.is_full());
    assert!(region.is_empty());
    assert_eq!(region.need_refragment(), 0.0);
    assert!(region.could_gain_slot_quickly());

}

#[test]
fn test_region_basic_slot_management() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext::new(isolate));

    let region = Region::new(0);

    let mut slots = Vec::new();
    let mut i = 0;
    while i < REGION_SLOT_SIZE {
        let slot = region.gain_slot(Object)?;
        assert_eq!(region.occupied.get(), (i + 1) as u16);
        assert_eq!(region.is_full(), i == REGION_SLOT_SIZE - 1);
        assert!(!region.is_empty());
        assert_eq!(region.need_refragment(), 0.0);
        assert_eq!(region.could_gain_slot_quickly(), i != REGION_SLOT_SIZE - 1);
        slots.push(slot);
        i += 1;
    }

    let mut i = 1;
    while i < REGION_SLOT_SIZE {
        assert!(region.recycle_slot(slots[i], true, &context).is_err());
        region.move_out_from_nursery(slots[i])?;
        region.recycle_slot(slots[i], true, &context)?;
        assert!(!region.is_full());
        assert!(!region.is_empty());
        assert_eq!(region.occupied.get(), (REGION_SLOT_SIZE - i) as u16);
        assert!(!region.could_gain_slot_quickly());
        assert!(region.need_refragment() > 0.0);
        i += 1;
    }

    assert!(region.need_refragment() > 0.0);

    region.recalculate_next_empty_slot_index()?;

    assert_eq!(region.next_empty_slot_index.get(), 1);

    assert!(region.recycle_slot(slots[0], true, &context).is_err());
    region.move_out_from_nursery(slots[0])?;
    region.recycle_slot(slots[0], true, &context)?;

    assert_eq!(region.need_refragment(), 1.0);
    assert!(region.is_empty());

    region.recalculate_next_empty_slot_index()?;

    assert_eq!(region.need_refragment(), 0.0);
    assert!(region.is_empty());

    Ok(())
}

#[test]
fn test_region_snapshot() -> Result<(), Error> {

    let region = Region::new(0);

    let slot = region.gain_slot(Object)?;

    let (snapshot, in_nursery, reference_map, _removed_values, _removed_symbols) = region.freeze_slot(slot)?;

    let (_slot_2, _added_values, __added_symbols) = region.restore_slot(slot, snapshot, in_nursery, &reference_map)?;

    assert_eq!(region.occupied.get(), 2);
    assert_eq!(region.next_empty_slot_index.get(), 2);

    Ok(())

}

#[test]
fn test_region_references() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext::new(isolate));

    let region = Region::new(0);

    let slot = region.gain_slot(Object)?;

    {
        let slot_2 = region.gain_slot(Object)?;
        region.add_reference(slot_2, slot)?;
        assert!(region.recycle_slot(slot_2, false, &context).is_err());
    }

    {
        let slot_2 = region.gain_slot(Object)?;
        region.add_reference(slot_2, slot)?;
        let (snapshot, in_nursery, reference_map, _removed_values, _removed_symbols) = region.freeze_slot(slot_2)?;
        let (slot_3, _added_values, _added_symbols) = region.restore_slot(slot_2, snapshot, in_nursery, &reference_map)?;
        assert!(region.add_reference(slot_2, slot).is_err());
        region.redirect_slot(slot_2, slot_3, reference_map)?;
        region.add_reference(slot_2, slot)?;
        assert!(region.recycle_slot(slot_3, false, &context).is_err());
    }

    {
        let slot_2 = region.gain_slot(Object)?;
        region.add_reference(slot_2, slot)?;
        let (snapshot, in_nursery, reference_map, _removed_values, _removed_symbols) = region.freeze_slot(slot_2)?;
        let (slot_3, _added_values, _added_symbols) = region.restore_slot(slot_2, snapshot, in_nursery, &reference_map)?;
        assert!(region.add_reference(slot_2, slot).is_err());
        region.redirect_slot(slot_2, slot_3, reference_map)?;
        region.recycle_slot(slot_2, false, &context)?;
        assert!(region.remove_reference(slot_2, slot)?.0);
        assert!(region.add_reference(slot_2, slot).is_err());
        region.remove_redirection_from(slot_2, slot_3)?;
        region.recycle_slot(slot_3, false, &context)?;
    }
   
    Ok(())
}

#[test]
fn test_region_slot_trap() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let layout_token = isolate.create_slot_layout_token();

    let context: Box<dyn Context> = Box::new(TestContext::new(isolate));

    let region = Region::new(0);

    let slot = region.gain_slot(Object)?;

    region.set_own_property_with_layout_guard(slot, slot, Symbol::new(1), Value::make_float(1.0), &context, layout_token.lock_read(), true)?;

    let slot_trap: Arc<dyn SlotTrap> = Arc::new(TestSlotTrap2::new(slot));

    region.set_slot_trap(slot, slot_trap, &context)?;

    region.set_own_property_with_layout_guard(slot, slot, Symbol::new(2), Value::make_float(32.0), &context, layout_token.lock_read(), true)?;
    let test_property_trap: Arc<dyn PropertyTrap> = Arc::new(TestPropertyTrap::new(Value::make_float(64.0)));
    region.define_own_property_with_layout_guard(slot, slot, Symbol::new(3), test_property_trap, &context, layout_token.lock_read(), true)?;

    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(1), None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(1.0));
    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(2), None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(32.0));
    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(3), None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(64.0));

    region.clear_slot_trap(slot, &context)?;

    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(1), None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(1.0));
    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(2), None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_undefined());
    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(3), None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_undefined());

    Ok(())

}

#[test]
fn test_region_own_property() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let layout_token = isolate.create_slot_layout_token();

    let context: Box<dyn Context> = Box::new(TestContext::new(isolate));

    let region = Region::new(0);

    let slot = region.gain_slot(Object)?;

    region.set_own_property_with_layout_guard(slot, slot, Symbol::new(1), Value::make_float(43.0), &context, layout_token.lock_read(), true)?;

    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(1), None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(43.0));

    Ok(())
    
}

#[test]
fn test_region_field_shortcuts() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let layout_token = isolate.create_slot_layout_token();

    let context: Box<dyn Context> = Box::new(TestContext::new(isolate));

    let region = Region::new(0);

    let slot = region.gain_slot(Object)?;

    let field_template = Arc::new(FieldTemplate::new(1));
    field_template.add_symbol(Symbol::new(1))?;

    let field_shortcuts = Arc::new(FieldShortcuts::new(field_template.clone()));

    let field_token = field_shortcuts.get_field_token(Symbol::new(1)).unwrap();
    region.set_own_property_with_layout_guard(slot, slot, Symbol::new(1), Value::make_float(43.0), &context, layout_token.lock_read(), true)?;
    region.set_own_property_with_layout_guard(slot, slot, Symbol::new(2), Value::make_float(63.0), &context, layout_token.lock_read(), true)?;

    assert!(field_token.get_field(&field_shortcuts).is_none());

    region.update_field_shortcuts(slot, field_shortcuts.clone())?;

    assert!(&Arc::ptr_eq(&region.get_field_shortcuts(slot)?.unwrap(), &field_shortcuts));

    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(1), Some(&field_token), &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(43.0));
    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(2), None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(63.0));

    assert_eq!(field_token.get_field(&field_shortcuts).unwrap(), Value::make_float(43.0));
    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(1), Some(&field_token), &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(43.0));

    region.set_own_property_with_layout_guard(slot, slot, Symbol::new(1), Value::make_float(53.0), &context, layout_token.lock_read(), true)?;

    assert_eq!(field_token.get_field(&field_shortcuts).unwrap(), Value::make_float(53.0));
    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(1), Some(&field_token), &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(53.0));
    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(1), None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(53.0));

    region.clear_field_shortcuts(slot)?;
    assert_eq!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(1), Some(&field_token), &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(53.0));
    assert!(region.get_own_property_with_layout_guard(slot, slot, Symbol::new(2), Some(&field_token), &context, layout_token.lock_read(), true).is_err());

    Ok(())

}

#[test]
fn test_region_internal_slot() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext::new(isolate));

    let region = Region::new(0);

    let slot = region.gain_slot(Object)?;

    let internal_slot: Arc<dyn InternalSlot> = Arc::new(TestInternalSlot::new(Value::make_float(32.0)));

    region.set_internal_slot(slot, 0, internal_slot.clone(), &context)?;

    let internal_slot_2 = region.get_internal_slot(slot, 0, &context)?;

    assert!(Arc::ptr_eq(&internal_slot, &internal_slot_2.unwrap()));

    Ok(())
 
}

#[test]
fn test_region_seal() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let layout_token = isolate.create_slot_layout_token();

    let context: Box<dyn Context> = Box::new(TestContext::new(isolate));

    let region = Region::new(0);

    let slot = region.gain_slot(Object)?;

    assert!(!region.is_sealed(slot)?);
    region.seal_slot(slot)?;

    assert!(region.is_sealed(slot)?);

    let internal_slot: Arc<dyn InternalSlot> = Arc::new(TestInternalSlot::new(Value::make_float(32.0)));

    assert!(region.set_internal_slot(slot, 0, internal_slot.clone(), &context).is_err());
    assert!(region.clear_internal_slot(slot, 0, &context).is_err());

    assert!(region.set_own_property_with_layout_guard(slot, slot, Symbol::new(1), Value::make_float(43.0), &context, layout_token.lock_read(), true).is_err());

    let slot_trap: Arc<dyn SlotTrap> = Arc::new(TestSlotTrap2::new(slot));

    assert!(region.set_slot_trap(slot, slot_trap, &context).is_err());
    assert!(region.clear_slot_trap(slot, &context).is_err());

    Ok(())

}
