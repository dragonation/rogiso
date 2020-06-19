use std::collections::HashMap;
use std::collections::hash_map::Keys;
use std::cell::RefCell;
use std::sync::Arc;

use super::base::Error;
use super::base::ErrorType::*;
use super::base::PrimitiveType;
use super::base::PrimitiveType::*;
use super::base::Symbol;
use super::base::Value;
use super::context::Context;
use super::field_shortcuts::FieldShortcuts;
use super::field_shortcuts::FieldToken;
use super::internal_slot::InternalSlot;
use super::internal_slot::ProtectedInternalSlot;
use super::reference_map::ReferenceMap;
use super::storage::Pinned;
use super::trap::PropertyTrap;
use super::trap::ProtectedPropertyTrap;
use super::trap::FieldPropertyTrap;
use super::trap::SlotTrap;
use super::trap::SlotTrapResult::*;
use super::trap::ProtectedSlotTrap;
use super::util::RwLock;
use super::util::ReentrantLockReadGuard;

const LIVE_FLAG: u32 = 0b1;
const SEAL_FLAG: u32 = 0b10;

pub const BASE_WHITE: u8 = 0b00_u8;
pub const BASE_BLACK: u8 = 0b11_u8;
const BASE_GRAY: u8 = 0b01_u8;

struct InternalSlotIterator<'a> {
    keys: Option<Keys<'a, u64, Arc<dyn InternalSlot>>>
}

impl<'a> Iterator for InternalSlotIterator<'a> {

    type Item = &'a u64;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {

        match self.keys.as_mut() {
            None => None,
            Some(keys) => keys.next()
        }

    }

}

struct OwnPropertySymbolIterator<'a> {
    keys: Keys<'a, Symbol, Arc<dyn PropertyTrap>>
}

impl<'a> Iterator for OwnPropertySymbolIterator<'a> {

    type Item = &'a Symbol;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.keys.next()
    }

}

#[allow(dead_code)]
union AtomicSlotOptimizationData {
    u8_data: [u8; 16],
    u16_data: [u16; 8],
    u32_data: [u32; 4],
    u64_data: [u64; 2],
    u128_data: [u128; 1],
    i8_data: [i8; 16],
    i16_data: [i16; 8],
    i32_data: [i32; 4],
    i64_data: [i64; 2],
    i128_data: [i128; 1],
    f32_data: [f32; 4],
    f64_data: [f64; 2]
}

#[allow(dead_code)]
impl AtomicSlotOptimizationData {

    fn new() -> AtomicSlotOptimizationData {
        AtomicSlotOptimizationData {
            u8_data: [0u8; 16]
        }
    }

    #[allow(unused_unsafe)]
    fn reset(&mut self) {
        unsafe {
            self.u8_data = [0u8; 16];
        };
    }

    #[inline]
    unsafe fn get_u8_data<'a>(&'a self) -> &'a [u8; 16] {
        &self.u8_data
    }

    #[inline]
    unsafe fn get_u16_data<'a>(&'a self) -> &'a [u16; 8] {
        &self.u16_data
    }

    #[inline]
    unsafe fn get_u32_data<'a>(&'a self) -> &'a [u32; 4] {
        &self.u32_data
    }

    #[inline]
    unsafe fn get_u64_data<'a>(&'a self) -> &'a [u64; 2] {
        &self.u64_data
    }

    #[inline]
    unsafe fn get_u128_data<'a>(&'a self) -> &'a [u128; 1] {
        &self.u128_data
    }

    #[inline]
    unsafe fn get_i8_data<'a>(&'a self) -> &'a [i8; 16] {
        &self.i8_data
    }

    #[inline]
    unsafe fn get_i16_data<'a>(&'a self) -> &'a [i16; 8] {
        &self.i16_data
    }

    #[inline]
    unsafe fn get_i32_data<'a>(&'a self) -> &'a [i32; 4] {
        &self.i32_data
    }

    #[inline]
    unsafe fn get_i64_data<'a>(&'a self) -> &'a [i64; 2] {
        &self.i64_data
    }

    #[inline]
    unsafe fn get_i128_data<'a>(&'a self) -> &'a [i128; 1] {
        &self.i128_data
    }

    #[inline]
    unsafe fn get_f32_data<'a>(&'a self) -> &'a [f32; 4] {
        &self.f32_data
    }

    #[inline]
    unsafe fn get_f64_data<'a>(&'a self) -> &'a [f64; 2] {
        &self.f64_data
    }

    #[inline]
    unsafe fn set_u8_data(&mut self, u8_data: &[u8; 16]) {
        self.u8_data = *u8_data;
    }

    #[inline]
    unsafe fn set_u16_data(&mut self, u16_data: &[u16; 8]) {
        self.u16_data = *u16_data;
    }


    #[inline]
    unsafe fn set_u32_data(&mut self, u32_data: &[u32; 4]) {
        self.u32_data = *u32_data;
    }

    #[inline]
    unsafe fn set_u64_data(&mut self, u64_data: &[u64; 2]) {
        self.u64_data = *u64_data;
    }

    #[inline]
    unsafe fn set_u128_data(&mut self, u128_data: &[u128; 1]) {
        self.u128_data = *u128_data;
    }

    #[inline]
    unsafe fn set_i8_data(&mut self, i8_data: &[i8; 16]) {
        self.i8_data = *i8_data;
    }

    #[inline]
    unsafe fn set_i16_data(&mut self, i16_data: &[i16; 8]) {
        self.i16_data = *i16_data;
    }

    #[inline]
    unsafe fn set_i32_data(&mut self, i32_data: &[i32; 4]) {
        self.i32_data = *i32_data;
    }

    #[inline]
    unsafe fn set_i64_data(&mut self, i64_data: &[i64; 2]) {
        self.i64_data = *i64_data;
    }

    #[inline]
    unsafe fn set_i128_data(&mut self, i128_data: &[i128; 1]) {
        self.i128_data = *i128_data;
    }

   #[inline]
    unsafe fn set_f32_data(&mut self, f32_data: &[f32; 4]) {
        self.f32_data = *f32_data;
    }

    #[inline]
    unsafe fn set_f64_data(&mut self, f64_data: &[f64; 2]) {
        self.f64_data = *f64_data;
    }

}

struct AtomicSlot {

    flags: u32,

    primitive_type: PrimitiveType,

    prototype: Value,

    slot_trap: Option<Arc<dyn SlotTrap>>,

    own_property_traps: HashMap<Symbol, Arc<dyn PropertyTrap>>,

    field_shortcuts: Option<Arc<FieldShortcuts>>,

    internal_slots: Option<Box<HashMap<u64, Arc<dyn InternalSlot>>>>,

    #[allow(dead_code)]
    optimization_flags: u32,
    #[allow(dead_code)]
    optimization_type: u32,
    #[allow(dead_code)]
    optimization_data: AtomicSlotOptimizationData

}

// TODO: add direct prototype support
// TODO: add optimization supports

/// Slot managements
impl AtomicSlot {

    pub fn new() -> AtomicSlot {

        AtomicSlot {
            flags: 0,
            primitive_type: Undefined,
            prototype: Value::make_undefined(),
            slot_trap: None,
            own_property_traps: HashMap::new(),
            field_shortcuts: None,
            internal_slots: None,
            optimization_flags: 0,
            optimization_type: 0,
            optimization_data: AtomicSlotOptimizationData::new()
        }

    }

    pub fn reset(&mut self) -> (Vec<Value>, Vec<Symbol>) {

        self.optimization_flags = 0;
        self.primitive_type = Undefined;

        let (values, symbols) = self.list_self_references_without_autorefresh();

        self.prototype = Value::make_undefined();
        self.slot_trap = None;
        self.own_property_traps = HashMap::new();
        self.internal_slots = None;

        self.field_shortcuts = None;

        self.flags = 0;

        (values, symbols)

    }

}

/// Slot states and basic information
impl AtomicSlot {

    pub fn is_sealed(&self) -> bool {

        (self.flags & SEAL_FLAG) != 0

    }

    pub fn seal_slot(&mut self) {

        self.flags |= SEAL_FLAG;

    }

    pub fn is_alive(&self) -> bool {

        (self.flags & LIVE_FLAG) != 0

    }

    pub fn mark_as_alive(&mut self) {

        self.flags |= LIVE_FLAG;

    }

    pub fn get_primitive_type(&self) -> PrimitiveType {

        self.primitive_type

    }

    pub fn overwrite_primitive_type(&mut self, primitive_type: PrimitiveType) {

        self.primitive_type = primitive_type;

    }

    pub fn list_self_references_without_autorefresh(&self) -> (Vec<Value>, Vec<Symbol>) {

        let mut values = Vec::new();
        let mut symbols = Vec::new();

        values.push(self.prototype);

        if let Some(slot_trap) = &self.slot_trap {
            for value in slot_trap.list_internal_referenced_values() {
                values.push(value);
            }
            for symbol in slot_trap.list_internal_referenced_symbols() {
                symbols.push(symbol);
            }
        }
        for (_, property_trap) in self.own_property_traps.iter() {
            for value in property_trap.list_referenced_values() {
                values.push(value);
            }
            for symbol in property_trap.list_internal_referenced_symbols() {
                symbols.push(symbol);
            }
        }
        if let Some(internal_slots) = &self.internal_slots {
            for (_, internal_slot) in internal_slots.iter() {
                for value in internal_slot.list_referenced_values() {
                    values.push(value);
                }
                for symbol in internal_slot.list_referenced_symbols() {
                    symbols.push(symbol);
                }
            }
        }

        (values, symbols)

    }

    pub fn list_and_autorefresh_self_references(&mut self, self_id: Value, context: &Box<dyn Context>) -> Result<(Vec<Value>, Vec<Symbol>), Error> {

        let mut values = Vec::new();
        let mut symbols = Vec::new();

        let prototype = context.resolve_real_value(self.prototype)?;
        if prototype != self.prototype {
            context.add_value_reference(self_id, prototype)?;
            let old_prototype = self.prototype;
            self.prototype = prototype;
            context.remove_value_reference(self_id, old_prototype)?;
        }

        if let Some(slot_trap) = &self.slot_trap {
            for value in slot_trap.list_and_autorefresh_internal_referenced_values(self_id, context)? {
                values.push(value);
            }
            for symbol in slot_trap.list_internal_referenced_symbols() {
                symbols.push(symbol);
            }
        }
        for (_, property_trap) in self.own_property_traps.iter() {
            for value in property_trap.list_and_autorefresh_referenced_values(self_id, context)? {
                values.push(value);
            }
            for symbol in property_trap.list_internal_referenced_symbols() {
                symbols.push(symbol);
            }
        }
        if let Some(internal_slots) = &self.internal_slots {
            for (_, internal_slot) in internal_slots.iter() {
                for value in internal_slot.list_and_autorefresh_referenced_values(self_id, context)? {
                    values.push(value);
                }
                for symbol in internal_slot.list_referenced_symbols() {
                    symbols.push(symbol);
                }
            }
        }

        Ok((values, symbols))

    }

}

/// Slot trap
impl AtomicSlot {

    pub fn set_slot_trap(&mut self, slot_trap: Arc<dyn SlotTrap>) -> Option<Arc<dyn SlotTrap>> {

        self.slot_trap.replace(slot_trap)

    }

    pub fn clear_slot_trap(&mut self) -> Option<Arc<dyn SlotTrap>> {

        self.slot_trap.take()

    }

    pub fn get_slot_trap<'a>(&'a self) -> Option<&'a Arc<dyn SlotTrap>> {

        (&self.slot_trap).as_ref()

    }

}

/// Slot internal slot
impl AtomicSlot {

    pub fn set_internal_slot(&mut self, id: u64, internal_slot: Arc<dyn InternalSlot>) -> Option<Arc<dyn InternalSlot>> {

        let internal_slots = self.internal_slots.get_or_insert_with(|| Box::new(HashMap::new()));

        internal_slots.insert(id, internal_slot)

    }

    pub fn clear_internal_slot(&mut self, id: u64) -> Option<Arc<dyn InternalSlot>> {

        match self.internal_slots.as_mut() {
            None => None,
            Some(internal_slots) => internal_slots.remove(&id)
        }

    }

    pub fn get_internal_slot<'a>(&'a self, id: u64) -> Option<&'a Arc<dyn InternalSlot>> {

        match &self.internal_slots {
            None => None,
            Some(internal_slots) => internal_slots.get(&id)
        }

    }

    pub fn iterate_internal_slot_ids(&self) -> InternalSlotIterator {

        InternalSlotIterator { 
            keys: self.internal_slots.as_ref().map(|internal_slots| internal_slots.keys())
        }

    }

}

/// Slot own property traps
impl AtomicSlot {

    pub fn get_own_property_trap<'a>(&'a self, symbol: Symbol) -> Option<&'a Arc<dyn PropertyTrap>> {

        self.own_property_traps.get(&symbol)

    }

    pub fn define_own_property_trap(&mut self, symbol: Symbol, property_trap: Arc<dyn PropertyTrap>) -> Option<Arc<dyn PropertyTrap>> {

        self.own_property_traps.insert(symbol, property_trap)

    }

    pub fn clear_own_property_trap(&mut self, symbol: Symbol) -> Option<Arc<dyn PropertyTrap>> {

        self.own_property_traps.remove(&symbol)

    }

    pub fn iterate_own_property_symbols(&self) -> OwnPropertySymbolIterator {

        OwnPropertySymbolIterator { 
            keys: self.own_property_traps.keys()
        }

    }

}

/// Slot field shortcuts
impl AtomicSlot {

    pub fn get_field_shortcuts<'a>(&'a self) -> Option<&'a Arc<FieldShortcuts>> {

        self.field_shortcuts.as_ref()

    }

    pub fn set_field_shortcuts(&mut self, field_shortcuts: Arc<FieldShortcuts>) -> Option<Arc<FieldShortcuts>> {

        self.field_shortcuts.replace(field_shortcuts)

    }

    pub fn clear_field_shortcuts(&mut self) -> Option<Arc<FieldShortcuts>> {

        self.field_shortcuts.take()

    }

}


/// Snapshot of slot record
pub struct SlotRecordSnapshot {
    atomic_slot: Box<AtomicSlot>
}


/// Record for slot stored in region
struct SlotRecord {
    region_id: u32,
    slot_index: u32,
    color: u8,
    outer_reference_map: Option<Box<ReferenceMap>>,
    atomic_slot: Box<AtomicSlot>
}

/// Slot constructor, snapshot and initialization
impl SlotRecord {

    pub fn new(region_id: u32, slot_index: u32) -> SlotRecord {
        SlotRecord {
            region_id: region_id,
            slot_index: slot_index,
            color: 0,
            outer_reference_map: None,
            atomic_slot: Box::new(AtomicSlot::new())
        }
    }

    pub fn reset(&mut self) -> (Vec<Value>, Vec<Symbol>) {

        self.color = 0;
        self.outer_reference_map = None;

        self.atomic_slot.as_mut().reset()

    }

    pub fn freeze(&mut self) -> (SlotRecordSnapshot, Option<Box<ReferenceMap>>, Vec<Value>, Vec<Symbol>) {

        self.color = 0;

        let (removed_values, remove_symbols) = self.atomic_slot.list_self_references_without_autorefresh();

        let mut atomic_slot = Box::new(AtomicSlot::new());
        std::mem::swap(&mut atomic_slot, &mut self.atomic_slot);

        let outer_reference_map = self.outer_reference_map.take();
        let slot_record_snapshot = SlotRecordSnapshot {
            atomic_slot: atomic_slot
        };

        (slot_record_snapshot, outer_reference_map, removed_values, remove_symbols)

    }

    pub fn sweep_outer_reference_map(&mut self) -> Option<Box<ReferenceMap>> {

        self.outer_reference_map.take()

    }

    pub fn restore(&mut self, mut snapshot: SlotRecordSnapshot) {

        self.color = 0;
        self.outer_reference_map = None;
        std::mem::swap(&mut self.atomic_slot, &mut snapshot.atomic_slot);

    }

}

/// Slot record basic information
impl SlotRecord {

    pub fn get_id(&self) -> Result<Value, Error> {
        match self.atomic_slot.get_primitive_type() {
            Undefined => Err(Error::new(FatalError, "Slot is not supported for undefined value")),
            Null => Err(Error::new(FatalError, "Slot is not supported for null value")),
            Boolean => Err(Error::new(FatalError, "Slot is not supported for boolean value")),
            Integer => Err(Error::new(FatalError, "Slot is not supported for integer value")),
            Float => Err(Error::new(FatalError, "Slot is not supported for float value")),
            Symbol => Err(Error::new(FatalError, "Slot is not supported for symbol value")),
            Text => Ok(Value::make_text(self.region_id, self.slot_index)),
            List => Ok(Value::make_list(self.region_id, self.slot_index)),
            Tuple => Ok(Value::make_tuple(self.region_id, self.slot_index)),
            Object => Ok(Value::make_object(self.region_id, self.slot_index)),
        }
    }

    pub fn is_sealed(&self) -> bool {
        self.atomic_slot.is_sealed()
    }

    pub fn seal_slot(&mut self) {
        self.atomic_slot.as_mut().seal_slot();
    }

    pub fn is_alive(&self) -> bool {
        self.atomic_slot.is_alive()
    }

    pub fn mark_as_alive(&mut self) {
        self.atomic_slot.as_mut().mark_as_alive();
    }

    pub fn overwrite_primitive_type(&mut self, primitive_type: PrimitiveType) -> Result<(), Error> {
        match primitive_type {
            Undefined => Err(Error::new(FatalError, "Slot is not supported for undefined value")),
            Null => Err(Error::new(FatalError, "Slot is not supported for null value")),
            Boolean => Err(Error::new(FatalError, "Slot is not supported for boolean value")),
            Integer => Err(Error::new(FatalError, "Slot is not supported for integer value")),
            Float => Err(Error::new(FatalError, "Slot is not supported for float value")),
            Symbol => Err(Error::new(FatalError, "Slot is not supported for symbol value")),
            _ => {
                self.atomic_slot.overwrite_primitive_type(primitive_type);
                Ok(())
            }
        }
    }

    pub fn list_self_references_without_autorefresh(&self) -> (Vec<Value>, Vec<Symbol>) {
        self.atomic_slot.list_self_references_without_autorefresh()
    }

    pub fn list_and_autorefresh_self_references(&mut self, context: &Box<dyn Context>) -> Result<(Vec<Value>, Vec<Symbol>), Error> {
        self.atomic_slot.list_and_autorefresh_self_references(self.get_id()?, context)
    }

}

/// Slot trap
impl SlotRecord {

    pub fn set_slot_trap(&mut self, slot_trap: Arc<dyn SlotTrap>) -> Option<Arc<dyn SlotTrap>> {
        self.atomic_slot.as_mut().set_slot_trap(slot_trap)
    }

    pub fn clear_slot_trap(&mut self) -> Option<Arc<dyn SlotTrap>> {
        self.atomic_slot.as_mut().clear_slot_trap()
    }

    pub fn get_slot_trap<'a>(&'a self) -> Option<&'a Arc<dyn SlotTrap>> {
        self.atomic_slot.get_slot_trap()
    }

}

/// Slot internal slot
impl SlotRecord {

    pub fn set_internal_slot(&mut self, id: u64, internal_slot: Arc<dyn InternalSlot>) -> Option<Arc<dyn InternalSlot>> {
        self.atomic_slot.as_mut().set_internal_slot(id, internal_slot)
    }

    pub fn clear_internal_slot(&mut self, id: u64) -> Option<Arc<dyn InternalSlot>> {
        self.atomic_slot.as_mut().clear_internal_slot(id)
    }

    pub fn get_internal_slot<'a>(&'a self, id: u64) -> Option<&'a Arc<dyn InternalSlot>> {
        self.atomic_slot.get_internal_slot(id)
    }

    pub fn iterate_internal_slot_ids(&self) -> InternalSlotIterator {
        self.atomic_slot.iterate_internal_slot_ids()
    }

}

/// Slot own property trap
impl SlotRecord {

    pub fn get_own_property_trap<'a>(&'a self, symbol: Symbol) -> Option<&'a Arc<dyn PropertyTrap>> {
        self.atomic_slot.get_own_property_trap(symbol)
    }

    pub fn define_own_property_trap(&mut self, symbol: Symbol, property_trap: Arc<dyn PropertyTrap>) -> Option<Arc<dyn PropertyTrap>> {
        self.atomic_slot.as_mut().define_own_property_trap(symbol, property_trap)
    }

    pub fn clear_own_property_trap(&mut self, symbol: Symbol) -> Option<Arc<dyn PropertyTrap>> {
        self.atomic_slot.as_mut().clear_own_property_trap(symbol)
    }

    pub fn iterate_own_property_symbols(&self) -> OwnPropertySymbolIterator {
        self.atomic_slot.iterate_own_property_symbols()
    }

}

/// Slot field shortcuts
impl SlotRecord {

    pub fn get_field_shortcuts<'a>(&'a self) -> Option<&'a Arc<FieldShortcuts>> {
        self.atomic_slot.get_field_shortcuts()
    }

    pub fn set_field_shortcuts(&mut self, field_shortcuts: Arc<FieldShortcuts>) -> Option<Arc<FieldShortcuts>> {
        self.atomic_slot.as_mut().set_field_shortcuts(field_shortcuts)
    }

    pub fn clear_field_shortcuts(&mut self) -> Option<Arc<FieldShortcuts>> {
        self.atomic_slot.as_mut().clear_field_shortcuts()
    }

}

/// Slot value references
impl SlotRecord {

    pub fn has_no_outer_references(&self) -> bool {
        match &self.outer_reference_map {
            Some(map) => map.is_empty(),
            None => true
        }
    }

    pub fn add_outer_reference(&mut self, value: Value) -> Result<(), Error> {

        let reference_map = self.outer_reference_map.get_or_insert_with(|| Box::new(ReferenceMap::new()));

        reference_map.add_reference(value)

    }

    pub fn remove_outer_reference(&mut self, value: Value) -> Result<(), Error> {

        match &mut self.outer_reference_map {
            None => {
                return Err(Error::new(FatalError, "No reference available"));
            },
            Some(reference_map) => {
                reference_map.remove_reference(value)?;
            }
        }

        if self.outer_reference_map.iter().next().unwrap().is_empty() {
            self.outer_reference_map = None;
        }

        Ok(())

    }

}

/// Slot color
impl SlotRecord {

    pub fn mark_as_white(&mut self, base: u8) {
        self.color = (BASE_WHITE ^ base) & 0b11;
    }

    pub fn mark_as_black(&mut self, base: u8) {
        self.color = (BASE_BLACK ^ base) & 0b11;
    }

    pub fn mark_as_gray(&mut self, _base: u8) {
        self.color = BASE_GRAY;
    }

    pub fn is_white(&self, base: u8) -> bool {
        (self.color ^ base) & 0b11 == BASE_WHITE
    }

    pub fn is_black(&self, base: u8) -> bool {
        (self.color ^ base) & 0b11 == BASE_BLACK
    }

    pub fn is_gray(&self, _base: u8) -> bool {
        self.color == BASE_GRAY
    }

}


/// Slot in region with a lock
/// 
/// Generally a slot may take `24 + 32 + 128 = 184 bytes`
///  * `24` bytes rw lock based on spin lock (stored in region)
///  * `32` bytes garbage collection info (store in region)
///  * `128` bytes slot value info
/// Rest `256 - 184 = 72 bytes` for more properties, about `3` fields
/// 
/// So we could consider `1 KiB` may store `4` slots
/// 
/// `4` GiB may store 16M slots at most

pub struct RegionSlot {
    rw_lock: RwLock,
    record: RefCell<SlotRecord>
}

/// Slot constructor, snapshot and initialization
impl RegionSlot {

    pub fn new(region_id: u32, slot_index: u32) -> RegionSlot {
        RegionSlot {
            rw_lock: RwLock::new(),
            record: RefCell::new(SlotRecord::new(region_id, slot_index))
        }
    }

    pub fn recycle(&self, drop_value: bool, context: &Box<dyn Context>) -> Result<(), Error> {

        let (id, slot_trap, removed_values, removed_symbols) = {
            let _guard = self.rw_lock.lock_write();
            let mut record = self.record.borrow_mut();
            if !record.is_alive() {
                return Ok(());
            }
            let id = record.get_id()?;
            let slot_trap = record.get_slot_trap().map(|arc| arc.clone());
            let (removed_values, removed_symbols) = record.reset();
            (id, slot_trap, removed_values, removed_symbols)
        };

        for value in removed_values {
            context.remove_value_reference(id, value)?;
        }
        for symbol in removed_symbols {
            context.remove_symbol_reference(symbol)?;
        }

        if drop_value {
            if slot_trap.is_some() {
                slot_trap.unwrap().notify_drop()?;
            }
            context.notify_slot_drop(id)?;
        }

        Ok(())

    }

    pub fn freeze(&self) -> Result<(SlotRecordSnapshot, Option<Box<ReferenceMap>>, Vec<Value>, Vec<Symbol>), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot is not alive"));
        }

        Ok(record.freeze())

    }

    pub fn restore(&self, snapshot: SlotRecordSnapshot) -> Result<(Value, Vec<Value>, Vec<Symbol>), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if record.is_alive() {
            return Err(Error::new(FatalError, "Slot is alive"));
        }

        record.restore(snapshot);

        let (added_values, added_symbols) = record.list_self_references_without_autorefresh();

        Ok((record.get_id()?, added_values, added_symbols))

    }

}

/// Slot basic information
impl RegionSlot {

    pub fn get_id(&self) -> Result<Value, Error> {

        let _guard = self.rw_lock.lock_read();

        let record = self.record.borrow();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        record.get_id()

    }

    pub fn is_sealed(&self) -> Result<bool, Error> {

        let _guard = self.rw_lock.lock_read();

        let record = self.record.borrow();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.is_sealed())

    }

    pub fn seal_slot(&self) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        record.seal_slot();

        Ok(())

    }

    pub fn is_alive(&self) -> bool {

        let _guard = self.rw_lock.lock_read();

        self.record.borrow().is_alive()

    }

    pub fn mark_as_alive(&self) {

        let _guard = self.rw_lock.lock_write();

        self.record.borrow_mut().mark_as_alive()

    }

    pub fn overwrite_primitive_type(&self, primitive_type: PrimitiveType) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        self.record.borrow_mut().overwrite_primitive_type(primitive_type)

    }

}

/// Slot trap
impl RegionSlot {

    pub fn set_slot_trap(&self, slot_trap: Arc<dyn SlotTrap>, context: &Box<dyn Context>) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }
        if record.is_sealed() {
            return Err(Error::new(MutatingSealedProperty, "Slot is sealed"));
        }

        let id = record.get_id()?;

        for value in slot_trap.list_internal_referenced_values() {
            context.add_value_reference(id, value)?;
        }

        for symbol in slot_trap.list_internal_referenced_symbols() {
            context.add_symbol_reference(symbol)?;
        }

        let old_slot_trap = record.set_slot_trap(slot_trap);

        if let Some(old_slot_trap) = old_slot_trap {
            for symbol in old_slot_trap.list_internal_referenced_symbols() {
                context.remove_symbol_reference(symbol)?;
            }
            for value in old_slot_trap.list_internal_referenced_values() {
                context.remove_value_reference(id, value)?;
            }
        }

        Ok(())

    }

    pub fn clear_slot_trap(&self, context: &Box<dyn Context>) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }
        if record.is_sealed() {
            return Err(Error::new(MutatingSealedProperty, "Slot is sealed"));
        }

        let id = record.get_id()?;

        let old_slot_trap = record.clear_slot_trap();

        if let Some(old_slot_trap) = old_slot_trap {
            for symbol in old_slot_trap.list_internal_referenced_symbols() {
                context.remove_symbol_reference(symbol)?;
            }
            for value in old_slot_trap.list_internal_referenced_values() {
                context.remove_value_reference(id, value)?;
            }
        }

        Ok(())

    }

    pub fn has_slot_trap(&self) -> Result<bool, Error> {

        let _guard = self.rw_lock.lock_read();

        let record = self.record.borrow();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.get_slot_trap().is_some())

    }

}

/// Slot internal slot
impl RegionSlot {

    pub fn set_internal_slot(&self, id: u64, internal_slot: Arc<dyn InternalSlot>, context: &Box<dyn Context>) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }
        if record.is_sealed() {
            return Err(Error::new(MutatingSealedProperty, "Slot is sealed"));
        }

        let slot_id = record.get_id()?;

        for value in internal_slot.list_referenced_values() {
            context.add_value_reference(slot_id, value)?;
        }
        for symbol in internal_slot.list_referenced_symbols() {
            context.add_symbol_reference(symbol)?;
        }

        let old_internal_slot = record.set_internal_slot(id, internal_slot);

        if let Some(old_internal_slot) = old_internal_slot {
            for symbol in old_internal_slot.list_referenced_symbols() {
                context.remove_symbol_reference(symbol)?;
            }
            for value in old_internal_slot.list_referenced_values() {
                context.remove_value_reference(slot_id, value)?;
            }
        }

        Ok(())

    }

    pub fn clear_internal_slot(&self, id: u64, context: &Box<dyn Context>) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }
        if record.is_sealed() {
            return Err(Error::new(MutatingSealedProperty, "Slot is sealed"));
        }

        let slot_id = record.get_id()?;

        let old_internal_slot = record.clear_internal_slot(id);

        if let Some(old_internal_slot) = old_internal_slot {
            for symbol in old_internal_slot.list_referenced_symbols() {
                context.remove_symbol_reference(symbol)?;
            }
            for value in old_internal_slot.list_referenced_values() {
                context.remove_value_reference(slot_id, value)?;
            }
        }

        Ok(())

    }

    pub fn get_internal_slot<'a>(&self, id: u64, context: &'a Box<dyn Context>) -> Result<Option<ProtectedInternalSlot::<'a>>, Error> {

        let _guard = self.rw_lock.lock_read();

        let record = self.record.borrow();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        match record.get_internal_slot(id) {
            None => Ok(None),
            Some(internal_slot) => Ok(Some(ProtectedInternalSlot::<'a>::new(internal_slot, context)?))
        }

    }

    pub fn has_internal_slot(&self, id: u64) -> Result<bool, Error> {

        let _guard = self.rw_lock.lock_read();

        let record = self.record.borrow();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.get_internal_slot(id).is_some())

    }

    pub fn list_internal_slot_ids(&self) -> Result<Vec<u64>, Error> {

        let _guard = self.rw_lock.lock_read();

        let record = self.record.borrow();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.iterate_internal_slot_ids().map(|value| *value).collect())

    }

}

/// Slot own properties
impl RegionSlot {

    pub fn has_own_property(&self, symbol: Symbol, context: &Box<dyn Context>) -> Result<bool, Error> {

        let (id, slot_trap, has_property_trap) = {
            let _guard = self.rw_lock.lock_read();
            let record = self.record.borrow();
            if !record.is_alive() {
                return Err(Error::new(FatalError, "Slot not alive"));
            }
            let id = record.get_id()?;
            let slot_trap = record.get_slot_trap();
            let property_trap = record.get_own_property_trap(symbol); 
            match slot_trap {
                None => {
                    return Ok(property_trap.is_some());
                },
                Some(slot_trap) => (id, ProtectedSlotTrap::new(slot_trap, context)?, property_trap.is_some())
            }
        };

        let symbol_value = Value::make_symbol(symbol);
        slot_trap.list_and_autorefresh_internal_referenced_values(id, context)?;
        let trap_info = context.create_trap_info(id, [symbol_value].to_vec(), context);
        let result = slot_trap.has_own_property(trap_info, context)?;
        match result {
            Trapped(value) => Ok(value.as_boolean()),
            Thrown(value) => Err(Error::new(RogicError(value), "Rogic error happened")),
            Skipped => Ok(has_property_trap)
        }

    }

    pub fn get_own_property_with_layout_guard<'a>(&self, symbol: Symbol, field_token: Option<&FieldToken>, context: &Box<dyn Context>, mut layout_guard: ReentrantLockReadGuard<'a>, no_redirection: bool) -> Result<Pinned, Error> {

        if let Some(field_token) = field_token {
            if field_token.get_symbol() != symbol {
                return Err(Error::new(FatalError, "Field token not match the symbol expected"));
            }
        }

        let (id, slot_trap) = {
            let _guard = self.rw_lock.lock_read();
            let record = self.record.borrow();
            if !record.is_alive() {
                return Err(Error::new(FatalError, "Slot not alive"));
            }
            let id = record.get_id()?;
            let slot_trap = record.get_slot_trap();
            let property_trap = record.get_own_property_trap(symbol); 
            let field_shortcuts = record.get_field_shortcuts();
            if slot_trap.is_none() && property_trap.is_some() && 
               field_token.is_some() && field_shortcuts.is_some() {
                let field_token = field_token.iter().next().unwrap();
                let field_shortcuts = field_shortcuts.unwrap();
                let field_value = field_token.get_field(field_shortcuts);
                match field_value {
                    Some(field_value) => {
                        let new_value = context.resolve_real_value(field_value)?;
                        if new_value != field_value {
                            context.add_value_reference(id, new_value)?;
                            field_token.set_field(field_shortcuts, new_value);
                            property_trap.unwrap().refresh_referenced_value(field_value, new_value);
                            context.remove_value_reference(id, field_value)?;
                        }
                        return Pinned::new(context, new_value);           
                    },
                    None => {
                        let property_trap = property_trap.iter().next().unwrap();
                        if property_trap.is_simple_field() {
                            let symbol_value = Value::make_symbol(symbol);
                            let trap_info = context.create_trap_info(id, vec!(symbol_value), context);
                            let field_value = property_trap.get_property(trap_info, context)?;
                            let origin_value = field_value.get_origin_value();
                            let new_value = context.resolve_real_value(origin_value)?;
                            if new_value != origin_value {
                                context.add_value_reference(id, new_value)?;
                                field_token.set_field(&field_shortcuts, new_value);
                                property_trap.refresh_referenced_value(origin_value, new_value);
                                context.remove_value_reference(id, origin_value)?;
                            } else {
                                field_token.set_field(&field_shortcuts, new_value);
                            }
                            return Pinned::new(context, new_value);           
                        }
                    }
                }
            }
            match slot_trap {
                None => (id, None),
                Some(slot_trap) => (id, Some(ProtectedSlotTrap::new(slot_trap, context)?))
            }
        };

        layout_guard.unlock();

        let symbol_value = Value::make_symbol(symbol);
        if let Some(slot_trap) = slot_trap {
            slot_trap.list_and_autorefresh_internal_referenced_values(id, context)?;
            let trap_info = context.create_trap_info(id, vec!(symbol_value), context);
            let result = slot_trap.get_own_property(trap_info, context)?;
            match result {
                Trapped(value) => { return Ok(value); },
                Thrown(value) => { return Err(Error::new(RogicError(value), "Rogic error happened")); },
                Skipped => {}
            }
        }

        if no_redirection {
            self.get_own_property_ignore_slot_trap(symbol, context)
        } else {
            context.get_own_property_ignore_slot_trap(id, symbol, context)
        }

    }

    pub fn get_own_property_ignore_slot_trap(&self, symbol: Symbol, context: &Box<dyn Context>) -> Result<Pinned, Error> {

        let (id, property_trap) = {
            let _guard = self.rw_lock.lock_read();
            let record = self.record.borrow();
            if !record.is_alive() {
                return Err(Error::new(FatalError, "Slot not alive"));
            }
            let id = record.get_id()?;
            let property_trap = record.get_own_property_trap(symbol); 
            match property_trap {
                None => {
                    return Pinned::new(context, Value::make_undefined());
                }, 
                Some(property_trap) => (id, ProtectedPropertyTrap::new(property_trap, context)?)
            }
        };

        let symbol_value = Value::make_symbol(symbol);

        property_trap.list_and_autorefresh_referenced_values(id, context)?;

        let trap_info = context.create_trap_info(id, vec!(symbol_value), context);

        property_trap.get_property(trap_info, context)

    } 

    pub fn overwrite_own_property(&self, 
        symbol: Symbol, 
        value: Value) -> Result<(Vec<Value>, Vec<Symbol>, Vec<Value>, Vec<Symbol>), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();
        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }
        if record.is_sealed() {
            return Err(Error::new(MutatingSealedProperty, "Slot is sealed"));
        }

        let property_trap = record.get_own_property_trap(symbol); 
        
        let (removed_values, removed_symbols, added_symbols) = match property_trap {
            None => (Vec::new(), Vec::new(), Vec::new()),
            Some(property_trap) => (property_trap.list_referenced_values(), 
                                    property_trap.list_internal_referenced_symbols(),
                                    [symbol].to_vec())
        };

        if let Some(field_shortcuts) = record.get_field_shortcuts() {
            field_shortcuts.set_symbol_field(symbol, value);
        }

        let new_property_trap: Arc<dyn PropertyTrap> = Arc::new(FieldPropertyTrap::new(value));

        record.define_own_property_trap(symbol, new_property_trap);

        Ok((removed_values, removed_symbols, [value].to_vec(), added_symbols))
 
    }

    pub fn set_own_property_with_layout_guard<'a>(&self, 
        symbol: Symbol, 
        value: Value, 
        context: &Box<dyn Context>, 
        mut layout_guard: ReentrantLockReadGuard<'a>, 
        no_redirection: bool) -> Result<(), Error> {

        let value = context.resolve_real_value(value)?;

        let (id, slot_trap) = {
            let _guard = self.rw_lock.lock_write();
            let mut record = self.record.borrow_mut();
            if !record.is_alive() {
                return Err(Error::new(FatalError, "Slot not alive"));
            }
            if record.is_sealed() {
                return Err(Error::new(MutatingSealedProperty, "Slot is sealed"));
            }
            let id = record.get_id()?;
            let slot_trap = record.get_slot_trap();
            let property_trap = record.get_own_property_trap(symbol); 
            let field_shortcuts = record.get_field_shortcuts();
            if slot_trap.is_none() {
                match property_trap {
                    None => {
                        let property_trap: Arc<dyn PropertyTrap> = Arc::new(FieldPropertyTrap::new(value));
                        for value in property_trap.list_referenced_values() {
                            context.add_value_reference(id, value)?;
                        }
                        for symbol in property_trap.list_internal_referenced_symbols() {
                            context.add_symbol_reference(symbol)?;
                        }
                        context.add_symbol_reference(symbol)?;
                        record.define_own_property_trap(symbol, property_trap);
                        return Ok(());
                    },
                    Some(property_trap) => {
                        if let Some(field_shortcuts) = field_shortcuts {
                            if property_trap.is_simple_field() {
                                let symbol_value = Value::make_symbol(symbol);
                                let trap_info = context.create_trap_info(id, [symbol_value, value].to_vec(), context);
                                let (removed_values, added_values, removed_symbols, added_symbols) = property_trap.set_property(trap_info, context)?;
                                for value in added_values {
                                    context.add_value_reference(id, value)?;
                                }
                                for symbol in added_symbols {
                                    context.add_symbol_reference(symbol)?;
                                }
                                field_shortcuts.set_symbol_field(symbol, value);
                                for symbol in removed_symbols {
                                    context.remove_symbol_reference(symbol)?;
                                }
                                for value in removed_values {
                                    context.remove_value_reference(id, value)?;
                                }
                                return Ok(());           
                            } else {
                                field_shortcuts.clear_field(symbol);
                            }
                        }
                    }
                }
            }
            match slot_trap {
                None => (id, None),
                Some(slot_trap) => (id, Some(ProtectedSlotTrap::new(slot_trap, context)?))
            }
        };

        layout_guard.unlock();

        let symbol_value = Value::make_symbol(symbol);
        if let Some(slot_trap) = slot_trap {
            slot_trap.list_and_autorefresh_internal_referenced_values(id, context)?;
            let trap_info = context.create_trap_info(id, [symbol_value, value].to_vec(), context);
            let result = slot_trap.set_own_property(trap_info, context)?;
            match result {
                Trapped(_) => { return Ok(()); },
                Thrown(value) => { return Err(Error::new(RogicError(value), "Rogic error happened")); },
                Skipped => {}
            }
        }

        if no_redirection {
            self.set_own_property_ignore_slot_trap(symbol, value, context)
        } else {
            context.set_own_property_ignore_slot_trap(id, symbol, value, context)
        }

    }

    pub fn set_own_property_ignore_slot_trap(&self, 
        symbol: Symbol, 
        value: Value, 
        context: &Box<dyn Context>) -> Result<(), Error> {

        let value = context.resolve_real_value(value)?;

        let (id, property_trap) = {
            let _guard = self.rw_lock.lock_write();
            let mut record = self.record.borrow_mut();
            if !record.is_alive() {
                return Err(Error::new(FatalError, "Slot not alive"));
            }
            if record.is_sealed() {
                return Err(Error::new(MutatingSealedProperty, "Slot is sealed"));
            }
            let id = record.get_id()?;
            let property_trap = record.get_own_property_trap(symbol); 
            let field_shortcuts = record.get_field_shortcuts();
            match property_trap {
                None => {
                    let property_trap: Arc<dyn PropertyTrap> = Arc::new(FieldPropertyTrap::new(value));
                    for value in property_trap.list_referenced_values() {
                        context.add_value_reference(id, value)?;
                    }
                    for symbol in property_trap.list_internal_referenced_symbols() {
                        context.add_symbol_reference(symbol)?;
                    }
                    context.add_symbol_reference(symbol)?;
                    record.define_own_property_trap(symbol, property_trap);
                    return Ok(());
                },
                Some(property_trap) => {
                    if let Some(field_shortcuts) = field_shortcuts {
                        if property_trap.is_simple_field() {
                            let symbol_value = Value::make_symbol(symbol);
                            let trap_info = context.create_trap_info(id, [symbol_value, value].to_vec(), context);
                            let (removed_values, added_values, removed_symbols, added_symbols) = property_trap.set_property(trap_info, context)?;
                            for value in added_values {
                                context.add_value_reference(id, value)?;
                            }
                            for symbol in added_symbols {
                                context.add_symbol_reference(symbol)?;
                            }
                            field_shortcuts.set_symbol_field(symbol, value);
                            for symbol in removed_symbols {
                                context.remove_symbol_reference(symbol)?;
                            }
                            for value in removed_values {
                                context.remove_value_reference(id, value)?;
                            }
                            return Ok(());           
                        } else {
                            field_shortcuts.clear_field(symbol);
                        }
                    }
                    (id, ProtectedPropertyTrap::new(property_trap, context)?)
                }
            }
        };

        let symbol_value = Value::make_symbol(symbol);

        let trap_info = context.create_trap_info(id, [symbol_value, value].to_vec(), context);
        let (removed_values, added_values, removed_symbols, added_symbols) = property_trap.set_property(trap_info, context)?;
        for value in added_values {
            context.add_value_reference(id, value)?;
        }
        for symbol in added_symbols {
            context.add_symbol_reference(symbol)?;
        }

        {
            let _guard = self.rw_lock.lock_write();
            let record = self.record.borrow_mut();
            let field_shortcuts = record.get_field_shortcuts();
            if let Some(field_shortcuts) = field_shortcuts {
                field_shortcuts.clear_field(symbol);
            }
        }

        for symbol in removed_symbols {
            context.remove_symbol_reference(symbol)?;
        }
        for value in removed_values {
            context.remove_value_reference(id, value)?;
        }

        Ok(())

    }

    pub fn define_own_property_with_layout_guard<'a>(&self, 
        symbol: Symbol, 
        property_trap: Arc<dyn PropertyTrap>, 
        context: &Box<dyn Context>, 
        mut layout_guard: ReentrantLockReadGuard<'a>, 
        no_redirection: bool) -> Result<(), Error> {

        let (id, slot_trap) = {
            let _guard = self.rw_lock.lock_write();
            let mut record = self.record.borrow_mut();
            if !record.is_alive() {
                return Err(Error::new(FatalError, "Slot not alive"));
            }
            if record.is_sealed() {
                return Err(Error::new(MutatingSealedProperty, "Slot is sealed"));
            }
            let id = record.get_id()?;
            let slot_trap = record.get_slot_trap();
            let field_shortcuts = record.get_field_shortcuts();
            if slot_trap.is_none() {
                for value in property_trap.list_referenced_values() {
                    context.add_value_reference(id, value)?;
                }
                for symbol in property_trap.list_internal_referenced_symbols() {
                    context.add_symbol_reference(symbol)?;
                }
                if let Some(field_shortcuts) = field_shortcuts {
                    if property_trap.is_simple_field() {
                        let symbol_value = Value::make_symbol(symbol);
                        let trap_info = context.create_trap_info(id, [symbol_value].to_vec(), context);
                        let value = property_trap.get_property(trap_info, context)?;
                        field_shortcuts.set_symbol_field(symbol, value.get_value());
                    } else {
                        field_shortcuts.clear_field(symbol);
                    }
                }
                let old_property_trap = record.define_own_property_trap(symbol, property_trap);
                if let Some(old_property_trap) = old_property_trap {
                    for value in old_property_trap.list_referenced_values() {
                        context.remove_value_reference(id, value)?;
                    }
                    for symbol in old_property_trap.list_internal_referenced_symbols() {
                        context.remove_symbol_reference(symbol)?;
                    }
                } else {
                    context.add_symbol_reference(symbol)?;
                }
                return Ok(());
            }
            match slot_trap {
                None => (id, None),
                Some(slot_trap) => (id, Some(ProtectedSlotTrap::new(slot_trap, context)?))
            }
        };

        layout_guard.unlock();

        let symbol_value = Value::make_symbol(symbol);
        if let Some(slot_trap) = slot_trap {
            slot_trap.list_and_autorefresh_internal_referenced_values(id, context)?;
            let trap_value = context.make_property_trap_value(property_trap.clone(), context)?;
            let trap_info = context.create_trap_info(id, [symbol_value, trap_value].to_vec(), context);
            let result = slot_trap.define_own_property(trap_info, context)?;
            match result {
                Trapped(_) => { return Ok(()); },
                Thrown(value) => { return Err(Error::new(RogicError(value), "Rogic error happened")); },
                Skipped => {}
            }
        }

        if no_redirection {
            self.define_own_property_ignore_slot_trap(symbol, property_trap, context)
        } else {
            context.define_own_property_ignore_slot_trap(id, symbol, property_trap, context)
        }

    }

    pub fn define_own_property_ignore_slot_trap(&self, 
        symbol: Symbol, 
        property_trap: Arc<dyn PropertyTrap>, 
        context: &Box<dyn Context>) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();
        let mut record = self.record.borrow_mut();
        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }
        if record.is_sealed() {
            return Err(Error::new(MutatingSealedProperty, "Slot is sealed"));
        }
        let id = record.get_id()?;
        let field_shortcuts = record.get_field_shortcuts();
        for value in property_trap.list_referenced_values() {
            context.add_value_reference(id, value)?;
        }
        for symbol in property_trap.list_internal_referenced_symbols() {
            context.add_symbol_reference(symbol)?;
        }
        if let Some(field_shortcuts) = field_shortcuts {
            if property_trap.is_simple_field() {
                let symbol_value = Value::make_symbol(symbol);
                let trap_info = context.create_trap_info(id, [symbol_value].to_vec(), context);
                let value = property_trap.get_property(trap_info, context)?;
                field_shortcuts.set_symbol_field(symbol, value.get_value());
            } else {
                field_shortcuts.clear_field(symbol);
            }
        }
        let old_property_trap = record.define_own_property_trap(symbol, property_trap);
        if let Some(old_property_trap) = old_property_trap {
            for value in old_property_trap.list_referenced_values() {
                context.remove_value_reference(id, value)?;
            }
            for symbol in old_property_trap.list_internal_referenced_symbols() {
                context.remove_symbol_reference(symbol)?;
            }
        } else {
            context.add_symbol_reference(symbol)?;
        }

        Ok(())

    }

    pub fn delete_own_property_with_layout_guard<'a>(&self, 
        symbol: Symbol, 
        context: &Box<dyn Context>, 
        mut layout_guard: ReentrantLockReadGuard<'a>,
        no_redirection: bool) -> Result<(), Error> {

        let (id, slot_trap) = {
            let _guard = self.rw_lock.lock_write();
            let mut record = self.record.borrow_mut();
            if !record.is_alive() {
                return Err(Error::new(FatalError, "Slot not alive"));
            }
            if record.is_sealed() {
                return Err(Error::new(MutatingSealedProperty, "Slot is sealed"));
            }
            let id = record.get_id()?;
            let slot_trap = record.get_slot_trap();
            let field_shortcuts = record.get_field_shortcuts();
            if slot_trap.is_none() {
                if let Some(field_shortcuts) = field_shortcuts {
                    field_shortcuts.clear_field(symbol);
                }
                let old_property_trap = record.clear_own_property_trap(symbol);
                if let Some(old_property_trap) = old_property_trap {
                    for value in old_property_trap.list_referenced_values() {
                        context.remove_value_reference(id, value)?;
                    }
                    for symbol in old_property_trap.list_internal_referenced_symbols() {
                        context.remove_symbol_reference(symbol)?;
                    }
                    context.remove_symbol_reference(symbol)?;
                }
                return Ok(());
            }
            match slot_trap {
                None => (id, None),
                Some(slot_trap) => (id, Some(ProtectedSlotTrap::new(slot_trap, context)?))
            }
        };

        layout_guard.unlock();

        let symbol_value = Value::make_symbol(symbol);
        if let Some(slot_trap) = slot_trap {
            slot_trap.list_and_autorefresh_internal_referenced_values(id, context)?;
            let trap_info = context.create_trap_info(id, [symbol_value].to_vec(), context);
            let result = slot_trap.delete_own_property(trap_info, context)?;
            match result {
                Trapped(_) => { return Ok(()); },
                Thrown(value) => { return Err(Error::new(RogicError(value), "Rogic error happened")); },
                Skipped => {}
            }
        }

        if no_redirection {
            self.delete_own_property_ignore_slot_trap(symbol, context)
        } else {
            context.delete_own_property_ignore_slot_trap(id, symbol, context)
        }

    }

    pub fn delete_own_property_ignore_slot_trap(&self, symbol: Symbol, context: &Box<dyn Context>) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();
        let mut record = self.record.borrow_mut();
        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }
        if record.is_sealed() {
            return Err(Error::new(MutatingSealedProperty, "Slot is sealed"));
        }
        let id = record.get_id()?;
        let field_shortcuts = record.get_field_shortcuts();

        if let Some(field_shortcuts) = field_shortcuts {
            field_shortcuts.clear_field(symbol);
        }
        let old_property_trap = record.clear_own_property_trap(symbol);
        if let Some(old_property_trap) = old_property_trap {
            for value in old_property_trap.list_referenced_values() {
                context.remove_value_reference(id, value)?;
            }
            for symbol in old_property_trap.list_internal_referenced_symbols() {
                context.remove_symbol_reference(symbol)?;
            }
            context.remove_symbol_reference(symbol)?;
        }

        Ok(())

    }

    pub fn list_own_property_symbols_with_layout_guard<'a>(&self, context: &Box<dyn Context>, mut layout_guard: ReentrantLockReadGuard<'a>, no_redirection: bool) -> Result<Vec<Symbol>, Error> {

        let (id, slot_trap) = {
            let _guard = self.rw_lock.lock_read();
            let record = self.record.borrow();
            if !record.is_alive() {
                return Err(Error::new(FatalError, "Slot not alive"));
            }
            let id = record.get_id()?;
            let slot_trap = record.get_slot_trap();
            match slot_trap {
                None => {
                    let mut symbols = Vec::new();
                    for value in record.iterate_own_property_symbols() {
                        symbols.push(*value);
                    }
                    return Ok(symbols);
                },
                Some(slot_trap) => (id, ProtectedSlotTrap::new(slot_trap, context)?)
            }
        };

        layout_guard.unlock();

        slot_trap.list_and_autorefresh_internal_referenced_values(id, context)?;
        let trap_info = context.create_trap_info(id, [].to_vec(), context);
        let result = slot_trap.list_own_property_symbols(trap_info, context)?;
        match result {
            Trapped(list_value) => { 
                let mut symbols = Vec::new();
                for value in context.extract_list(list_value.get_value(), context)? {
                    if value.is_symbol() {
                        symbols.push(value.extract_symbol(Symbol::new(0)));
                    } else {
                        return Err(Error::new(RogicRuntimeError, "Invalid symbols"));
                    }
                }
                return Ok(symbols);
            },
            Thrown(value) => { return Err(Error::new(RogicError(value), "Rogic error happened")); },
            Skipped => {}
        }

        if no_redirection {
            self.list_own_property_symbols_ignore_slot_trap(context)
        } else {
            let mut symbols = Vec::new();
            for symbol in context.list_own_property_symbols_ignore_slot_trap(id, context)?.iter() {
                symbols.push(*symbol);
            }
            Ok(symbols)
        }

    }

    pub fn list_own_property_symbols_ignore_slot_trap(&self, _context: &Box<dyn Context>) -> Result<Vec<Symbol>, Error> {

        let _guard = self.rw_lock.lock_read();
        let record = self.record.borrow();
        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        let mut symbols = Vec::new();
        for value in record.iterate_own_property_symbols() {
            symbols.push(*value);
        }
        return Ok(symbols);

    }

}

/// Slot field shortcuts
impl RegionSlot {

    pub fn has_field_shortcuts(&self) -> Result<bool, Error> {

        let _guard = self.rw_lock.lock_read();

        let record = self.record.borrow();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.get_field_shortcuts().is_some())

    }

    pub fn get_field_shortcuts(&self) -> Result<Option<Arc<FieldShortcuts>>, Error> {

        let _guard = self.rw_lock.lock_read();

        let record = self.record.borrow();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.get_field_shortcuts().map(|arc| arc.clone()))

    }

    pub fn set_field_shortcuts(&self, field_shortcuts: Arc<FieldShortcuts>) -> Result<Option<Arc<FieldShortcuts>>, Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.set_field_shortcuts(field_shortcuts))

    }

    pub fn clear_field_shortcuts(&self) -> Result<Option<Arc<FieldShortcuts>>, Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.clear_field_shortcuts())

    }

}

/// Slot references
impl RegionSlot {

    pub fn sweep_outer_reference_map(&self) -> Result<Option<Box<ReferenceMap>>, Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.sweep_outer_reference_map())

    }
 
    pub fn has_no_outer_references(&self) -> Result<bool, Error> {

        let _guard = self.rw_lock.lock_read();

        let record = self.record.borrow();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.has_no_outer_references())

    }

    pub fn add_outer_reference(&self, value: Value) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        record.add_outer_reference(value)

    }

    pub fn remove_outer_reference(&self, value: Value) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        record.remove_outer_reference(value)

    }

}

/// Slot colors
impl RegionSlot {

    pub fn list_and_autorefresh_self_references(&self, context: &Box<dyn Context>) -> Result<(Vec<Value>, Vec<Symbol>), Error> {

        let _guard = self.rw_lock.lock_write();

        self.record.borrow_mut().list_and_autorefresh_self_references(context)

    }

    pub fn mark_as_white(&self, base: u8) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        record.mark_as_white(base);

        Ok(())

    }

    pub fn mark_as_black(&self, base: u8) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        record.mark_as_black(base);

        Ok(())

    }

    pub fn mark_as_gray(&self, base: u8) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut record = self.record.borrow_mut();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        record.mark_as_gray(base);

        Ok(())

    }

    pub fn is_white(&self, base: u8) -> Result<bool, Error> {

        let _guard = self.rw_lock.lock_read();

        let record = self.record.borrow();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.is_white(base))

    }

    pub fn is_black(&self, base: u8) -> Result<bool, Error> {

        let _guard = self.rw_lock.lock_read();

        let record = self.record.borrow();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.is_black(base))

    }

    pub fn is_gray(&self, base: u8) -> Result<bool, Error> {

        let _guard = self.rw_lock.lock_read();

        let record = self.record.borrow();

        if !record.is_alive() {
            return Err(Error::new(FatalError, "Slot not alive"));
        }

        Ok(record.is_gray(base))

    }

}

#[cfg(test)] use std::collections::HashSet;

#[cfg(test)] use super::field_shortcuts::FieldTemplate;
#[cfg(test)] use super::isolate::Isolate;
#[cfg(test)] use super::test::TestContext;
#[cfg(test)] use super::test::TestInternalSlot;
#[cfg(test)] use super::test::TestSlotTrap;

#[test]
fn test_atomic_slot_size() {
    assert_eq!(std::mem::size_of::<AtomicSlot>(), 128);
}

#[test]
fn test_atomic_slot_flags() -> Result<(), Error> {

    let mut atomic_slot = AtomicSlot::new();

    assert!(!atomic_slot.is_alive());
    assert!(!atomic_slot.is_sealed());

    atomic_slot.seal_slot();

    assert!(!atomic_slot.is_alive());
    assert!(atomic_slot.is_sealed());

    atomic_slot.mark_as_alive();
    assert!(atomic_slot.is_alive());
    assert!(atomic_slot.is_sealed());

    let mut atomic_slot = AtomicSlot::new();

    atomic_slot.mark_as_alive();

    assert!(atomic_slot.is_alive());
    assert!(!atomic_slot.is_sealed());

    atomic_slot.seal_slot();

    assert!(atomic_slot.is_alive());
    assert!(atomic_slot.is_sealed());

    Ok(())

}

#[test]
fn test_atomic_slot_primitive_type() {

    let mut atomic_slot = AtomicSlot::new();

    assert_eq!(atomic_slot.get_primitive_type(), Undefined);

    atomic_slot.overwrite_primitive_type(Null);

    assert_eq!(atomic_slot.get_primitive_type(), Null);

}

#[test]
fn test_slot_record_size() {
    assert_eq!(std::mem::size_of::<SlotRecord>(), 32);
}

#[test]
fn test_slot_record_reset() {

    let mut record = SlotRecord::new(1, 2);

    assert_eq!(record.region_id, 1);
    assert_eq!(record.slot_index, 2);

    record.reset();

    assert_eq!(record.region_id, 1);
    assert_eq!(record.slot_index, 2);

}

#[test]
fn test_slot_record_id() -> Result<(), Error> {

    let mut record = SlotRecord::new(1, 2);

    assert!(record.get_id().is_err());

    assert!(record.overwrite_primitive_type(Null).is_err());

    record.overwrite_primitive_type(List)?;

    assert_eq!(record.get_id()?.get_primitive_type(), List);
    assert_eq!(record.get_id()?.get_region_id()?, 1);
    assert_eq!(record.get_id()?.get_region_slot()?, 2);

    record.overwrite_primitive_type(Object)?;

    assert_eq!(record.get_id()?.get_primitive_type(), Object);
    assert_eq!(record.get_id()?.get_region_id()?, 1);
    assert_eq!(record.get_id()?.get_region_slot()?, 2);

    Ok(())

}

#[test]
fn test_slot_record_color() {

    let mut record = SlotRecord::new(0, 0);

    record.mark_as_gray(BASE_WHITE);
    assert!(!record.is_white(BASE_WHITE));
    assert!(!record.is_black(BASE_WHITE));
    assert!(!record.is_white(BASE_BLACK));
    assert!(!record.is_black(BASE_BLACK));
    assert!(record.is_gray(BASE_WHITE));
    assert!(record.is_gray(BASE_BLACK));

    record.mark_as_gray(BASE_BLACK);
    assert!(!record.is_white(BASE_WHITE));
    assert!(!record.is_black(BASE_WHITE));
    assert!(!record.is_white(BASE_BLACK));
    assert!(!record.is_black(BASE_BLACK));
    assert!(record.is_gray(BASE_WHITE));
    assert!(record.is_gray(BASE_BLACK));

    record.mark_as_white(BASE_WHITE);
    assert!(record.is_white(BASE_WHITE));
    assert!(!record.is_black(BASE_WHITE));
    assert!(!record.is_white(BASE_BLACK));
    assert!(record.is_black(BASE_BLACK));
    assert!(!record.is_gray(BASE_WHITE));
    assert!(!record.is_gray(BASE_BLACK));

    record.mark_as_white(BASE_BLACK);
    assert!(!record.is_white(BASE_WHITE));
    assert!(record.is_black(BASE_WHITE));
    assert!(record.is_white(BASE_BLACK));
    assert!(!record.is_black(BASE_BLACK));
    assert!(!record.is_gray(BASE_WHITE));
    assert!(!record.is_gray(BASE_BLACK));

    record.mark_as_black(BASE_WHITE);
    assert!(!record.is_white(BASE_WHITE));
    assert!(record.is_black(BASE_WHITE));
    assert!(record.is_white(BASE_BLACK));
    assert!(!record.is_black(BASE_BLACK));
    assert!(!record.is_gray(BASE_WHITE));
    assert!(!record.is_gray(BASE_BLACK));

    record.mark_as_black(BASE_BLACK);
    assert!(record.is_white(BASE_WHITE));
    assert!(!record.is_black(BASE_WHITE));
    assert!(!record.is_white(BASE_BLACK));
    assert!(record.is_black(BASE_BLACK));
    assert!(!record.is_gray(BASE_WHITE));
    assert!(!record.is_gray(BASE_BLACK));

}

#[test]
fn test_slot_record_references() -> Result<(), Error> {

    let mut record = SlotRecord::new(0, 0);
    record.overwrite_primitive_type(Object)?;

    let mut record_2 = SlotRecord::new(0, 1);
    record_2.overwrite_primitive_type(Object)?;

    assert!(record.has_no_outer_references());

    record.add_outer_reference(record_2.get_id()?)?;
    assert!(!record.has_no_outer_references());

    record.add_outer_reference(record_2.get_id()?)?;
    assert!(!record.has_no_outer_references());

    record.remove_outer_reference(record_2.get_id()?)?;
    assert!(!record.has_no_outer_references());

    record.remove_outer_reference(record_2.get_id()?)?;
    assert!(record.has_no_outer_references());

    Ok(())

}

#[test]
fn test_region_slot_size() {
    assert_eq!(std::mem::size_of::<RegionSlot>(), 56);
}

#[test]
fn test_region_slot_management() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext::new(isolate));

    let slot = RegionSlot::new(1, 1);

    slot.mark_as_alive();
    slot.overwrite_primitive_type(Object)?;

    assert!(slot.is_alive());
    assert!(!slot.is_sealed()?);
    assert_eq!(slot.get_id()?.get_primitive_type(), Object);

    slot.recycle(true, &context)?;

    assert!(!slot.is_alive());
    assert!(slot.is_sealed().is_err());
    assert!(slot.get_id().is_err());

    Ok(())

}

#[test]
fn test_region_slot_snapshot() -> Result<(), Error> {

    let slot = RegionSlot::new(1, 1);
    let slot_2 = RegionSlot::new(1, 2);

    slot.mark_as_alive();
    slot.overwrite_primitive_type(Object)?;

    assert!(!slot_2.is_alive());

    let (snapshot, _reference_map, _removed_values, _removed_symbols) = slot.freeze()?;

    slot_2.restore(snapshot)?;

    assert!(!slot.is_alive());
    assert!(slot_2.is_alive());

    assert_eq!(slot_2.get_id()?.get_primitive_type(), Object);

    Ok(())

}

#[test]
fn test_region_slot_slot_trap() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext::new(isolate));

    let slot = RegionSlot::new(1, 1);

    assert!(slot.has_slot_trap().is_err());

    slot.mark_as_alive();
    slot.overwrite_primitive_type(Object)?;

    assert!(!slot.has_slot_trap()?);

    let slot_trap: Arc<dyn SlotTrap> = Arc::new(TestSlotTrap::new(Value::make_object(1, 2)));

    slot.set_slot_trap(slot_trap, &context)?;

    assert!(slot.has_slot_trap()?);

    slot.clear_slot_trap(&context)?;

    assert!(!slot.has_slot_trap()?);

    Ok(())

}

#[test]
fn test_region_slot_internal_slot() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext::new(isolate));

    let slot = RegionSlot::new(1, 1);

    assert!(slot.has_internal_slot(1).is_err());

    slot.mark_as_alive();
    slot.overwrite_primitive_type(Object)?;

    assert!(!slot.has_internal_slot(1)?);
    assert!(!slot.has_internal_slot(2)?);

    let internal_slot: Arc<dyn InternalSlot> = Arc::new(TestInternalSlot::new(Value::make_object(1, 3)));

    slot.set_internal_slot(1, internal_slot, &context)?;
    assert!(slot.has_internal_slot(1)?);
    assert!(!slot.has_internal_slot(2)?);
    assert_eq!(slot.list_internal_slot_ids()?.len(), 1);
    assert_eq!(slot.list_internal_slot_ids()?[0], 1);

    slot.clear_internal_slot(1, &context)?;
    assert!(!slot.has_internal_slot(1)?);

    Ok(())

}

#[test]
fn test_region_slot_field_shortcuts() -> Result<(), Error> {

    let slot = RegionSlot::new(1, 1);

    assert!(slot.has_field_shortcuts().is_err());

    slot.mark_as_alive();
    slot.overwrite_primitive_type(Object)?;

    assert!(!slot.has_field_shortcuts()?);

    let template = Arc::new(FieldTemplate::new(1));

    let field_shortcuts = Arc::new(FieldShortcuts::new(template));

    slot.set_field_shortcuts(field_shortcuts)?;
    assert!(slot.has_field_shortcuts()?);

    slot.clear_field_shortcuts()?;
    assert!(!slot.has_field_shortcuts()?);

    Ok(())

}

#[test]
fn test_region_slot_color() -> Result<(), Error> {

    let record = RegionSlot::new(0, 0);

    assert!(record.mark_as_gray(BASE_WHITE).is_err());
    assert!(record.is_white(BASE_WHITE).is_err());

    record.mark_as_alive();
    record.overwrite_primitive_type(Object)?;

    record.mark_as_gray(BASE_WHITE)?;
    assert!(!record.is_white(BASE_WHITE)?);
    assert!(!record.is_black(BASE_WHITE)?);
    assert!(!record.is_white(BASE_BLACK)?);
    assert!(!record.is_black(BASE_BLACK)?);
    assert!(record.is_gray(BASE_WHITE)?);
    assert!(record.is_gray(BASE_BLACK)?);

    record.mark_as_gray(BASE_BLACK)?;
    assert!(!record.is_white(BASE_WHITE)?);
    assert!(!record.is_black(BASE_WHITE)?);
    assert!(!record.is_white(BASE_BLACK)?);
    assert!(!record.is_black(BASE_BLACK)?);
    assert!(record.is_gray(BASE_WHITE)?);
    assert!(record.is_gray(BASE_BLACK)?);

    record.mark_as_white(BASE_WHITE)?;
    assert!(record.is_white(BASE_WHITE)?);
    assert!(!record.is_black(BASE_WHITE)?);
    assert!(!record.is_white(BASE_BLACK)?);
    assert!(record.is_black(BASE_BLACK)?);
    assert!(!record.is_gray(BASE_WHITE)?);
    assert!(!record.is_gray(BASE_BLACK)?);

    record.mark_as_white(BASE_BLACK)?;
    assert!(!record.is_white(BASE_WHITE)?);
    assert!(record.is_black(BASE_WHITE)?);
    assert!(record.is_white(BASE_BLACK)?);
    assert!(!record.is_black(BASE_BLACK)?);
    assert!(!record.is_gray(BASE_WHITE)?);
    assert!(!record.is_gray(BASE_BLACK)?);

    record.mark_as_black(BASE_WHITE)?;
    assert!(!record.is_white(BASE_WHITE)?);
    assert!(record.is_black(BASE_WHITE)?);
    assert!(record.is_white(BASE_BLACK)?);
    assert!(!record.is_black(BASE_BLACK)?);
    assert!(!record.is_gray(BASE_WHITE)?);
    assert!(!record.is_gray(BASE_BLACK)?);

    record.mark_as_black(BASE_BLACK)?;
    assert!(record.is_white(BASE_WHITE)?);
    assert!(!record.is_black(BASE_WHITE)?);
    assert!(!record.is_white(BASE_BLACK)?);
    assert!(record.is_black(BASE_BLACK)?);
    assert!(!record.is_gray(BASE_WHITE)?);
    assert!(!record.is_gray(BASE_BLACK)?);

    Ok(())

}

#[test]
fn test_region_slot_references() -> Result<(), Error> {

    let record = RegionSlot::new(0, 0);
    record.mark_as_alive();
    record.overwrite_primitive_type(Object)?;

    let record_2 = RegionSlot::new(0, 1);
    record_2.mark_as_alive();
    record_2.overwrite_primitive_type(Object)?;

    assert!(record.has_no_outer_references()?);

    assert!(record.is_alive());
    record.add_outer_reference(record_2.get_id()?)?;
    assert!(!record.has_no_outer_references()?);

    record.add_outer_reference(record_2.get_id()?)?;
    assert!(!record.has_no_outer_references()?);

    record.remove_outer_reference(record_2.get_id()?)?;
    assert!(!record.has_no_outer_references()?);

    record.remove_outer_reference(record_2.get_id()?)?;
    assert!(record.has_no_outer_references()?);

    Ok(())

}

#[test]
fn test_region_slot_own_properties() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let layout_token = isolate.create_slot_layout_token();

    let context: Box<dyn Context> = Box::new(TestContext::new(isolate));

    let region_slot = RegionSlot::new(1, 1);
    region_slot.mark_as_alive();
    region_slot.overwrite_primitive_type(Object)?;

    let symbol = Symbol::new(1);
    assert!(!region_slot.has_own_property(symbol, &context)?);
    assert_eq!(region_slot.get_own_property_with_layout_guard(symbol, None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_undefined());

    region_slot.set_own_property_with_layout_guard(symbol, Value::make_integer(100), &context, layout_token.lock_read(), true)?;
    assert!(region_slot.has_own_property(symbol, &context)?);

    assert_eq!(region_slot.get_own_property_with_layout_guard(symbol, None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_integer(100));

    let symbol_2 = Symbol::new(2);

    region_slot.define_own_property_with_layout_guard(symbol_2, Arc::new(FieldPropertyTrap::new(Value::make_boolean(true))), &context, layout_token.lock_read(), true)?;
    assert_eq!(region_slot.get_own_property_with_layout_guard(symbol_2, None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_boolean(true));

    let mut symbols = HashSet::new();
    for value in region_slot.list_own_property_symbols_with_layout_guard(&context, layout_token.lock_read(), true)? {
        symbols.insert(value);
    }
    assert_eq!(symbols.len(), 2);
    assert!(symbols.get(&symbol).is_some());
    assert!(symbols.get(&symbol_2).is_some());
    assert!(symbols.get(&Symbol::new(3)).is_none());

    region_slot.delete_own_property_with_layout_guard(symbol, &context, layout_token.lock_read(), true)?;

    let mut symbols = HashSet::new();
    for value in region_slot.list_own_property_symbols_with_layout_guard(&context, layout_token.lock_read(), true)? {
        symbols.insert(value);
    }
    assert_eq!(symbols.len(), 1);
    assert!(symbols.get(&symbol).is_none());
    assert!(symbols.get(&symbol_2).is_some());
    assert!(!region_slot.has_own_property(symbol, &context)?);
    assert_eq!(region_slot.get_own_property_with_layout_guard(symbol, None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_undefined());

    region_slot.delete_own_property_with_layout_guard(symbol_2, &context, layout_token.lock_read(), true)?;
    assert_eq!(region_slot.list_own_property_symbols_with_layout_guard(&context, layout_token.lock_read(), true)?.len(), 0);

    Ok(())
}

#[test]
fn test_region_slot_own_property_with_field_shortcuts() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let layout_token = isolate.create_slot_layout_token();

    let context: Box<dyn Context> = Box::new(TestContext::new(isolate));

    let region_slot = RegionSlot::new(1, 1);
    region_slot.mark_as_alive();
    region_slot.overwrite_primitive_type(Object)?;

    let field_template = Arc::new(FieldTemplate::new(1));

    field_template.add_symbol(Symbol::new(1))?;

    let field_shortcuts = Arc::new(FieldShortcuts::new(field_template.clone()));

    let field_token = field_shortcuts.get_field_token(Symbol::new(1)).unwrap();

    region_slot.set_own_property_with_layout_guard(Symbol::new(1), Value::make_float(43.0), &context, layout_token.lock_read(), true)?;
    region_slot.set_own_property_with_layout_guard(Symbol::new(2), Value::make_float(63.0), &context, layout_token.lock_read(), true)?;

    assert!(field_token.get_field(&field_shortcuts).is_none());

    region_slot.set_field_shortcuts(field_shortcuts.clone())?;

    assert_eq!(region_slot.get_own_property_with_layout_guard(Symbol::new(1), Some(&field_token), &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(43.0));
    assert_eq!(region_slot.get_own_property_with_layout_guard(Symbol::new(2), None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(63.0));

    let field_token = field_shortcuts.get_field_token(Symbol::new(1)).unwrap();

    assert_eq!(field_token.get_field(&field_shortcuts).unwrap(), Value::make_float(43.0));

    assert_eq!(region_slot.get_own_property_with_layout_guard(Symbol::new(1), Some(&field_token), &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(43.0));

    region_slot.set_own_property_with_layout_guard(Symbol::new(1), Value::make_float(53.0), &context, layout_token.lock_read(), true)?;

    let field_token = field_shortcuts.get_field_token(Symbol::new(1)).unwrap();

    assert_eq!(field_token.get_field(&field_shortcuts).unwrap(), Value::make_float(53.0));
    assert_eq!(region_slot.get_own_property_with_layout_guard(Symbol::new(1), Some(&field_token), &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(53.0));
    assert_eq!(region_slot.get_own_property_with_layout_guard(Symbol::new(1), None, &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(53.0));

    region_slot.clear_field_shortcuts()?;
    let field_token = field_shortcuts.get_field_token(Symbol::new(1)).unwrap();
    assert_eq!(region_slot.get_own_property_with_layout_guard(Symbol::new(1), Some(&field_token), &context, layout_token.lock_read(), true)?.get_value(), Value::make_float(53.0));
    assert!(region_slot.get_own_property_with_layout_guard(Symbol::new(2), Some(&field_token), &context, layout_token.lock_read(), true).is_err());

    Ok(())

}
