use std::cell::RefCell;
use std::collections::HashSet;
use std::ptr::NonNull;
use std::sync::Arc;

use super::base::Error;
use super::base::Symbol;
use super::base::PrimitiveType;
use super::base::Value;
use super::barrier::Barrier;
use super::context::Context;
use super::field_shortcuts::FieldToken;
use super::internal_slot::InternalSlot;
use super::internal_slot::ProtectedInternalSlot;
use super::isolate::Isolate;
use super::isolate::SymbolInfo;
use super::root::DropListener;
use super::root::Root;
use super::root::WeakRoot;
use super::storage::Pinned;
use super::trap::PropertyTrap;
use super::trap::SlotTrap;
use super::trap::TrapInfo;
use super::util::ReentrantToken;
use super::util::RwLock;
use super::util::SpinLock;

const MAX_SLICE_SIZE: usize = 128;

enum CollectorState {
    Free,
    Pending,
    MarkingRoots,
    MarkingGrays,
    RemarkingGrays,
    Sweeping,
    Refragmenting
}

struct ValueSlice {
    values: RefCell<Vec<Value>>
}

struct CollectorBarrier {
    collector: NonNull<Collector>
}

impl Barrier for CollectorBarrier {

    fn preremove_value_reference(&self, value: Value) -> Result<(), Error> { 

        let collector = unsafe {
            self.collector.as_ref() 
        };

        collector.preremove_value_reference(value)

    }

    fn postgain_value(&self, value: Value) -> Result<(), Error> {

        let collector = unsafe {
            self.collector.as_ref() 
        };

        collector.postgain_value(value)

    }

}

struct CollectorContext {
    isolate: Arc<Isolate>,
    slot_layout_token: ReentrantToken
}

impl Context for CollectorContext {

    fn resolve_real_value(&self, value: Value) -> Result<Value, Error> {
        self.isolate.resolve_real_value(value, &self.slot_layout_token)
    }

    fn add_value_reference(&self, from: Value, to: Value) -> Result<(), Error> {
        self.isolate.add_value_reference(from, to, &self.slot_layout_token)
    }

    fn remove_value_reference(&self, from: Value, to: Value) -> Result<(), Error> {
        self.isolate.remove_value_reference(from, to, &self.slot_layout_token)?;
        Ok(())
    }
    
    fn add_symbol_reference(&self, symbol: Symbol) -> Result<(), Error> {
        self.isolate.add_symbol_reference(symbol)
    }

    fn remove_symbol_reference(&self, symbol: Symbol) -> Result<(), Error> {
        self.isolate.remove_symbol_reference(symbol)
    }

    fn notify_slot_drop(&self, value: Value) -> Result<(), Error> {
        self.isolate.notify_slot_drop(value)
    }

    fn get_slot_layout_token<'a>(&'a self) -> &'a ReentrantToken {
        &self.slot_layout_token
    }

    fn get_isolate<'a>(&'a self) -> &'a Arc<Isolate> {
        &self.isolate
    }

    fn create_trap_info(&self, _subject: Value, _parameters: Vec<Value>, _context: &Box<dyn Context>) -> Box<dyn TrapInfo> {
        panic!("Collector context only support reference operations");
    }

    fn gain_slot(&self, _primitive_type: PrimitiveType, _prototype: Value) -> Result<Value, Error> {
        panic!("Collector context only support reference operations");
    }

    fn get_text_symbol(&self, _scope: &str, _text: &str) -> Symbol {
        panic!("Collector context only support reference operations");
    }

    fn get_value_symbol(&self, _scope: &str, _value: Value) -> Symbol {
        panic!("Collector context only support reference operations");
    }
    
    fn resolve_symbol_info(&self, _symbol: Symbol) -> Result<SymbolInfo, Error> {
        panic!("Collector context only support reference operations");
    }

    fn get_prototype(&self, _value: Value, _context: &Box<dyn Context>) -> Result<Pinned, Error> {
        panic!("Collector context only support reference operations");
    }

    fn set_prototype(&self, _value: Value, _prototype: Value, _context: &Box<dyn Context>) -> Result<(), Error> {
        panic!("Collector context only support reference operations");
    }

    fn set_slot_trap(&self, _value: Value, _slot_trap: Arc<dyn SlotTrap>, _context: &Box<dyn Context>) -> Result<(), Error> {
        panic!("Collector context only support reference operations");
    }

    fn has_own_property(&self, _subject: Value, _symbol: Symbol, _context: &Box<dyn Context>) -> Result<bool, Error> {
        panic!("Collector context only support reference operations");
    }

    fn get_own_property(&self, _subject: Value, _symbol: Symbol, _field_token: Option<&FieldToken>, _context: &Box<dyn Context>) -> Result<Pinned, Error> {
        panic!("Collector context only support reference operations");
    }

    fn delete_own_property(&self, _subject: Value, _symbol: Symbol, _context: &Box<dyn Context>) -> Result<(), Error> {
        panic!("Collector context only support reference operations");
    }

    fn set_own_property(&self, _subject: Value, _symbol: Symbol, _value: Value, _context: &Box<dyn Context>) -> Result<(), Error> {
        panic!("Collector context only support reference operations");
    }

    fn define_own_property(&self, _subject: Value, _symbol: Symbol, _property_trap: Arc<dyn PropertyTrap>, _context: &Box<dyn Context>) -> Result<(), Error> {
        panic!("Collector context only support reference operations");
    }

    fn list_own_property_symbols(&self, _subject: Value, _context: &Box<dyn Context>) -> Result<HashSet<Symbol>, Error> {
        panic!("Collector context only support reference operations");
    }

    fn get_own_property_ignore_slot_trap(&self, _subject: Value, _symbol: Symbol, _context: &Box<dyn Context>) -> Result<Pinned, Error> {
        panic!("Collector context only support reference operations");
    }

    fn set_own_property_ignore_slot_trap(&self, _subject: Value, _symbol: Symbol, _value: Value, _context: &Box<dyn Context>) -> Result<(), Error> {
        panic!("Collector context only support reference operations");
    }

    fn delete_own_property_ignore_slot_trap(&self, _subject: Value, _symbol: Symbol, _context: &Box<dyn Context>) -> Result<(), Error> {
        panic!("Collector context only support reference operations");
    }

    fn define_own_property_ignore_slot_trap(&self, _subject: Value, _symbol: Symbol, _property_trap: Arc<dyn PropertyTrap>, _context: &Box<dyn Context>) -> Result<(), Error> {
        panic!("Collector context only support reference operations");
    }

    fn list_own_property_symbols_ignore_slot_trap(&self, _subject: Value, _context: &Box<dyn Context>) -> Result<HashSet<Symbol>, Error> {
        panic!("Collector context only support reference operations");
    }

    fn get_internal_slot<'a>(&self, _subject: Value, _index: u64, _context: &'a Box<dyn Context>) -> Result<Option<ProtectedInternalSlot<'a>>, Error> {
        panic!("Collector context only support reference operations");
    }

    fn set_internal_slot(&self, _subject: Value, _index: u64, _internal_slot: Arc<dyn InternalSlot>, _context: &Box<dyn Context>) -> Result<(), Error> {
        panic!("Collector context only support reference operations");
    }

    fn clear_internal_slot(&self, _subject: Value, _index: u64, _context: &Box<dyn Context>) -> Result<(), Error> {
        panic!("Collector context only support reference operations");
    }

    fn list_property_symbols(&self, _subject: Value, _context: &Box<dyn Context>) -> Result<HashSet<Symbol>, Error> {
        panic!("Collector context only support reference operations");
    }

    fn has_property(&self, _subject: Value, _symbol: Symbol, _context: &Box<dyn Context>) -> Result<bool, Error> {
        panic!("Collector context only support reference operations");
    }

    fn get_property(&self, _subject: Value, _symbol: Symbol, _field_token: Option<&FieldToken>, _context: &Box<dyn Context>) -> Result<Pinned, Error> {
        panic!("Collector context only support reference operations");
    }
    
    fn make_text(&self, _text: &str, _context: &Box<dyn Context>) -> Result<Pinned, Error> {
        panic!("Collector context only support reference operations");
    }

    fn make_list(&self, _list: Vec<Value>, _context: &Box<dyn Context>) -> Result<Pinned, Error> {
        panic!("Collector context only support reference operations");
    }

    fn make_tuple(&self, _prototype: Value, _id: u32, _values: Vec<Value>, _context: &Box<dyn Context>) -> Result<Pinned, Error> {
        panic!("Collector context only support reference operations");
    }

    fn extract_text(&self, _value: Value, _context: &Box<dyn Context>) -> String {
        panic!("Collector context only support reference operations");
    }

    fn extract_list(&self, _value: Value, _context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {
        panic!("Collector context only support reference operations");
    }

    fn make_property_trap_value(&self, _property_trap: Arc<dyn PropertyTrap>, _context: &Box<dyn Context>) -> Result<Value, Error> {
        panic!("Collector context only support reference operations");
    }

    fn extract_property_trap(&self, _value: Value, _context: &Box<dyn Context>) -> Result<Arc<dyn PropertyTrap>, Error> {
        panic!("Collector context only support reference operations");
    }

    fn add_root(&self, _value: Value) -> Result<Arc<Root>, Error> {
        panic!("Collector context only support reference operations");
    }

    fn remove_root(&self, _root: &Arc<Root>) -> Result<(), Error> {
        panic!("Collector context only support reference operations");
    }

    fn add_weak_root(&self, _value: Value, _drop_listener: Option<Box<dyn DropListener>>) -> Result<Arc<WeakRoot>, Error> {
        panic!("Collector context only support reference operations");
    }

    fn remove_weak_root(&self, _root: &Arc<WeakRoot>) -> Result<(), Error> {
        panic!("Collector context only support reference operations");
    }

}

pub struct Collector {

    context: Box<dyn Context>,

    barrier_remarking_lock: SpinLock,
    barrier_remarking_slice: ValueSlice,

    state: CollectorState,
    requested_to_collect: bool,

    gray_slices: Arc<RefCell<Vec<Vec<Value>>>>,

    symbol_rw_lock: RwLock,
    symbol_marks: RefCell<HashSet<Symbol>>

}

impl Collector {

    pub fn new(isolate: &Arc<Isolate>) -> Collector {

        Collector {
            context: Box::new(CollectorContext {
                isolate: isolate.clone(),
                slot_layout_token: isolate.create_slot_layout_token()
            }),
            barrier_remarking_lock: SpinLock::new(),
            barrier_remarking_slice: ValueSlice {
                values: RefCell::new(Vec::new())
            },
            state: CollectorState::Free,
            requested_to_collect: false,
            gray_slices: Arc::new(RefCell::new(Vec::new())),
            symbol_rw_lock: RwLock::new(),
            symbol_marks: RefCell::new(HashSet::new())
        }

    }

}

impl Collector {

    fn preremove_value_reference(&self, value: Value) -> Result<(), Error> { 

        let value = self.context.get_isolate().resolve_real_value(value, self.context.get_slot_layout_token())?;

        match self.state {
            CollectorState::MarkingGrays => {
                let _guard = self.barrier_remarking_lock.lock();
                self.mark_as_gray(value, &self.barrier_remarking_slice)
            },
            _ => Ok(())
        }

    }

    fn postgain_value(&self, value: Value) -> Result<(), Error> {

        let value = self.context.get_isolate().resolve_real_value(value, self.context.get_slot_layout_token())?;

        match self.state {
            CollectorState::MarkingGrays => {
                let _guard = self.barrier_remarking_lock.lock();
                self.mark_as_gray(value, &self.barrier_remarking_slice)
            },
            _ => Ok(())
        }

    }

}

impl Collector {

    pub fn request_to_collect(&mut self, context: &Box<dyn Context>) {

        self.requested_to_collect = true;

        match self.state {
            CollectorState::Free => {
                self.state = CollectorState::Pending;
                if self.full_collect_garbages(0.4, context).is_err() {
                    panic!("Failed to collect garbages");
                }
            },
            _ => {}
        }

    }

    fn full_collect_garbages(&mut self, refragment_ratio: f32, context: &Box<dyn Context>) -> Result<(), Error> {

        self.requested_to_collect = false;

        self.mark_roots()?;
        self.full_mark_grays()?;
        self.remark_grays()?;
        self.full_sweep_values(context)?;
        self.full_refragment_slots(refragment_ratio, context)?;

        self.context.get_isolate().flip_base_color();

        self.state = CollectorState::Free;

        Ok(())

    }

}

impl Collector {

    fn mark_roots(&mut self) -> Result<(), Error> {

        self.state = CollectorState::MarkingRoots;

        let _guard = self.context.get_slot_layout_token().lock_write();

        let slice = self.create_value_slice();

        let isolate = self.context.get_isolate();
        for value in isolate.list_buitins() {
            self.mark_as_gray(value, &slice)?;
        }

        for value in isolate.list_roots() {
            self.mark_as_gray(value, &slice)?;
        }

        for value in isolate.list_values_in_nursery() {
            self.mark_as_gray(value, &slice)?;
        }

        self.flush_slice(&slice)?;

        let barrier: Box<dyn Barrier> = Box::new(CollectorBarrier {
            collector: NonNull::from(&*self)
        });

        isolate.overwrite_barrier(barrier)?;

        Ok(())

    }

    fn full_mark_grays(&mut self) -> Result<(), Error> {

        self.state = CollectorState::MarkingGrays;

        // TODO: make it multithreading

        let slice = self.create_value_slice();

        let isolate = self.context.get_isolate();

        loop {
            let values = self.list_grays(MAX_SLICE_SIZE);
            if values.len() == 0 {
                break;
            }
            for value in values {
                self.mark_as_black(value)?;
                let (values, _symbols) = isolate.list_and_autorefresh_referenced_values(value, &self.context)?;
                for value in values {
                    self.mark_as_gray(value, &slice)?;
                }
            }
            self.flush_slice(&slice)?;
        }

        Ok(())

    }

    fn remark_grays(&mut self) -> Result<(), Error> {

        self.state = CollectorState::RemarkingGrays;

        let _guard = self.context.get_slot_layout_token().lock_write();

        let isolate = self.context.get_isolate();

        isolate.clear_barrier()?;

        self.flush_slice(&self.barrier_remarking_slice)?;

        let slice = self.create_value_slice();
        loop {
            let values = self.list_grays(MAX_SLICE_SIZE);
            if values.len() == 0 {
                break;
            }
            for value in values {
                self.mark_as_black(value)?;
                let (values, _symbols) = isolate.list_and_autorefresh_referenced_values(value, &self.context)?;
                for value in values {
                    self.mark_as_gray(value, &slice)?;
                }
            }
            self.flush_slice(&slice)?;
        }

        Ok(())

    }

    fn full_sweep_values(&mut self, context: &Box<dyn Context>) -> Result<(), Error> {

        self.state = CollectorState::Sweeping;

        // TODO: make it multithreading

        let isolate = self.context.get_isolate();

        for id in isolate.list_region_ids()? {
            isolate.sweep_region(id, context)?;
        }

        Ok(())

    }

    fn full_refragment_slots(&mut self, refragment_ratio: f32, context: &Box<dyn Context>) -> Result<(), Error> {

        self.state = CollectorState::Refragmenting;

        // TODO: make it multithreading
        let isolate = self.context.get_isolate();

        let ids = isolate.list_region_ids()?;

        let mut max_alive_region_id = 0;

        let mut target_id: u32 = 0;
        let mut source_id: u32 = ids.len() as u32 - 1;

        let next_region_id = isolate.peek_next_region_id();

        while target_id <= source_id {
            if isolate.need_region_refragment(source_id)? > refragment_ratio {
                loop {
                    let all_finished = isolate.refragment_region(source_id, target_id, context)?;
                    if all_finished {
                        break;
                    }
                    while (target_id < source_id) && isolate.is_region_full(target_id)? {
                        target_id += 1;
                    }
                    if target_id > source_id {
                        break;
                    }
                }
                let protected = isolate.is_region_protected(source_id)?;
                if (!protected) && isolate.is_region_empty(source_id)? {
                    isolate.recycle_region(source_id)?;
                } else {
                    if source_id > max_alive_region_id {
                        max_alive_region_id = source_id;
                    }
                }
            }
            source_id -= 1;
        }

        isolate.shrink_next_region_id(next_region_id, max_alive_region_id + 1);

        Ok(())

    }

}

impl Collector {

    fn create_value_slice(&self) -> ValueSlice {
        ValueSlice {
            values: RefCell::new(Vec::new())
        }
    }

    fn mark_as_black(&self, value: Value) -> Result<(), Error> {

        self.context.get_isolate().mark_as_black(value)

    }

    fn mark_as_gray(&self, value: Value, slice: &ValueSlice) -> Result<(), Error> {

        if value.is_symbol() {
            let _guard = self.symbol_rw_lock.lock_write();
            self.symbol_marks.borrow_mut().insert(value.extract_symbol(Symbol::new(0)));
            return Ok(());
        }

        if self.context.get_isolate().mark_as_gray(value)? {
            let need_flush = {
                let mut values = slice.values.borrow_mut();
                values.push(value);
                values.len() == MAX_SLICE_SIZE
            };
            if need_flush {
                self.flush_slice(slice)?;
            }
        }

        Ok(())

    }

    fn flush_slice(&self, slice: &ValueSlice) -> Result<(), Error> {

        if slice.values.borrow().len() > 0 {
            let values = slice.values.replace(Vec::new());
            self.gray_slices.borrow_mut().push(values);
        }

        Ok(())

    }

    fn list_grays(&self, count: usize) -> Vec<Value> {

        let mut grays = Vec::with_capacity(count);

        let mut gray_slices = self.gray_slices.borrow_mut();

        loop {
            if grays.len() >= count {
                return grays;
            }
            let values = gray_slices.pop();
            match values {
                None => { return grays; },
                Some(values) => {
                    if grays.len() + values.len() > count {
                        let mut values: Vec<_> = values.iter().map(|value| *value).collect();
                        let split_position = count - grays.len();
                        let new_values: Vec<_> = values.splice(..split_position, Vec::new()).collect();
                        gray_slices.push(values);
                        for value in new_values {
                            grays.push(value);
                        }
                    } else {
                        for value in values {
                            grays.push(value);
                        }
                    }
                }
            }
        }

    }

}