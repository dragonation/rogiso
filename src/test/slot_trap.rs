use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use super::super::base::Error;
use super::super::base::Symbol;
use super::super::base::Value;
use super::super::context::Context;
use super::super::storage::Pinned;
use super::super::trap::PropertyTrap;
use super::super::trap::SlotTrap;
use super::super::trap::SlotTrapResult;
use super::super::trap::TrapInfo;
use super::super::util::RwLock;

pub struct TestSlotTrap {
    rw_lock: RwLock,
    reference: Cell<Value>,
}

impl TestSlotTrap {
    pub fn new(reference: Value) -> TestSlotTrap {
        TestSlotTrap {
            rw_lock: RwLock::new(),
            reference: Cell::new(reference),
        }
    }
}

impl SlotTrap for TestSlotTrap {

    fn list_and_autorefresh_internal_referenced_values(&self, self_id: Value, context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {

        let _guard = self.rw_lock.lock_write();
        
        let value = self.reference.get();
        let new_value = context.resolve_real_value(value)?;
        if value != new_value {
            context.add_value_reference(self_id, new_value)?;
            self.reference.set(new_value);
            context.remove_value_reference(self_id, value)?;
        }

        Ok(vec!(new_value))

    }

    fn list_internal_referenced_values(&self) -> Vec<Value> {

        let _guard = self.rw_lock.lock_read();
        
        [self.reference.get()].to_vec()

    }

    fn refresh_referenced_value(&self, old_value: Value, new_value: Value) {

        {
            let _guard = self.rw_lock.lock_read();
            if self.reference.get() != old_value {
                return;
            }
        }

        {
            let _guard = self.rw_lock.lock_write();
            if self.reference.get() != old_value {
                return;
            }
            self.reference.set(new_value);
        }

    }
 
}


pub struct TestSlotTrap2 {
    rw_lock: RwLock,
    subject: Cell<Value>,
    prototype: Cell<Value>,
    own_properties: RefCell<HashMap<Symbol, Value>>,
    property_traps: RefCell<HashMap<Symbol, Arc<dyn PropertyTrap>>>
}

impl TestSlotTrap2 {
    pub fn new(subject: Value) -> TestSlotTrap2 {
        TestSlotTrap2 {
            rw_lock: RwLock::new(),
            subject: Cell::new(subject),
            prototype: Cell::new(Value::make_null()),
            own_properties: RefCell::new(HashMap::new()),
            property_traps: RefCell::new(HashMap::new())
        }
    }
}

impl SlotTrap for TestSlotTrap2 {

    fn get_prototype(&self, 
                     _trap_info: Box<dyn TrapInfo>, 
                     context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {
        let _guard = self.rw_lock.lock_read();
        Ok(SlotTrapResult::Trapped(Pinned::new(context, self.prototype.get())?))
    }

    fn set_prototype(&self, 
                     trap_info: Box<dyn TrapInfo>, 
                     context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {

        let old_prototype = {
            let _guard = self.rw_lock.lock_write();
            let old_prototype = self.prototype.get();
            self.prototype.set(trap_info.get_parameter(0));
            old_prototype
        };

        context.add_value_reference(self.subject.get(), self.prototype.get())?;
        context.remove_value_reference(self.subject.get(), old_prototype)?;

        Ok(SlotTrapResult::Trapped(Pinned::new(context, self.prototype.get())?))

    }

    fn has_own_property(&self, 
                        trap_info: Box<dyn TrapInfo>, 
                        context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {
        let _guard = self.rw_lock.lock_read();
        let symbol_value = trap_info.get_parameter(0);
        match self.own_properties.borrow().get(&symbol_value.extract_symbol(Symbol::new(0))) {
            None => Ok(SlotTrapResult::Trapped(Pinned::new(context, Value::make_boolean(false))?)),
            Some(_) => Ok(SlotTrapResult::Trapped(Pinned::new(context, Value::make_boolean(true))?))
        }
    }

    fn get_own_property(&self, 
                        trap_info: Box<dyn TrapInfo>, 
                        context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {

        let symbol_value = trap_info.get_parameter(0);
        let symbol = symbol_value.extract_symbol(Symbol::new(0));

        let property_trap = {
            let _guard = self.rw_lock.lock_read();
            match self.property_traps.borrow().get(&symbol) {
                Some(property_trap) => property_trap.clone(),
                None => {
                    match self.own_properties.borrow().get(&symbol) {
                        Some(value) => {
                            return Ok(SlotTrapResult::Trapped(Pinned::new(context, *value)?));
                        },
                        None => {
                            return Ok(SlotTrapResult::Skipped);
                        }
                    }
                }
            }
        };

        let trap_info = context.create_trap_info(self.subject.get(), [symbol_value].to_vec(), context);

        Ok(SlotTrapResult::Trapped(property_trap.get_property(trap_info, context)?))

    }

    fn set_own_property(&self, 
                        trap_info: Box<dyn TrapInfo>, 
                        context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {

        let symbol_value = trap_info.get_parameter(0);
        let symbol = symbol_value.extract_symbol(Symbol::new(0));
        let value = trap_info.get_parameter(1);

        let mut removes = Vec::new();
        let mut adds = Vec::new();
        let mut defined = false;

        let property_trap = {
            let _guard = self.rw_lock.lock_write();
            match self.property_traps.borrow().get(&symbol) {
                Some(property_trap) => Some(property_trap.clone()),
                None => {
                    adds.push(value);
                    match self.own_properties.borrow().get(&symbol) {
                        None => {
                            defined = true;
                        },
                        Some(old_value) => {
                            removes.push(*old_value);
                        }
                    };
                    self.own_properties.borrow_mut().insert(symbol, value);
                    None
                }
            }
        };

        match property_trap {
            None => {
                if defined {
                    context.add_symbol_reference(symbol)?;
                }
                for value in adds {
                    context.add_value_reference(self.subject.get(), value)?;
                }
                for value in removes {
                    context.remove_value_reference(self.subject.get(), value)?;
                }
            },
            Some(property_trap) => {
                let trap_info = context.create_trap_info(self.subject.get(), [symbol_value, value].to_vec(), context);
                let (removes, adds, _, _) = property_trap.set_property(trap_info, context)?;
                for value in adds {
                    context.add_value_reference(self.subject.get(), value)?;
                }
                for value in removes {
                    context.remove_value_reference(self.subject.get(), value)?;
                }
            }
        }

        Ok(SlotTrapResult::Trapped(Pinned::new(context, Value::make_undefined())?))
    }

    fn define_own_property(&self, 
                           trap_info: Box<dyn TrapInfo>, 
                           context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {

        let symbol_value = trap_info.get_parameter(0);
        let symbol = symbol_value.extract_symbol(Symbol::new(0));
        let property_trap_value = trap_info.get_parameter(1);
        let property_trap = context.extract_property_trap(property_trap_value, context)?;

        let mut defined = false;

        let adds = property_trap.list_referenced_values();
        let mut removes = Vec::new();

        {
            let _guard = self.rw_lock.lock_write();
            match self.property_traps.borrow_mut().insert(symbol, property_trap) {
                Some(property_trap) => {
                    for value in property_trap.list_referenced_values().iter() {
                        removes.push(*value);
                    }
                },
                None => {
                    match self.own_properties.borrow_mut().remove(&symbol) {
                        Some(old_value) => {
                            removes.push(old_value);
                        },
                        None => {
                            defined = true;
                        }
                    }
                }
            }
        }

        if defined {
            context.add_symbol_reference(symbol)?;
        }

        for value in adds {
            context.add_value_reference(self.subject.get(), value)?;
        }

        for value in removes {
            context.remove_value_reference(self.subject.get(), value)?;
        }

        Ok(SlotTrapResult::Trapped(Pinned::new(context, Value::make_undefined())?))
    }

    fn delete_own_property(&self, 
                           trap_info: Box<dyn TrapInfo>, 
                           context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {

        let _guard = self.rw_lock.lock_write();

        let mut removes = Vec::new();

        let symbol_value = trap_info.get_parameter(0);
        let symbol = symbol_value.extract_symbol(Symbol::new(0));
        match self.property_traps.borrow_mut().remove(&symbol) {
            Some(property_trap) => {
                for value in property_trap.list_referenced_values().iter() {
                    removes.push(*value);
                }
            },
            None => {
                match self.own_properties.borrow_mut().remove(&symbol) {
                    None => {
                        return Ok(SlotTrapResult::Thrown(Pinned::new(context, Value::make_undefined())?));
                    },
                    Some(old_value) => {
                        removes.push(old_value);
                    }
                }
            }
        }

        context.remove_symbol_reference(symbol)?;

        for value in removes.iter() {
            context.remove_value_reference(self.subject.get(), *value)?;
        }

        Ok(SlotTrapResult::Trapped(Pinned::new(context, Value::make_undefined())?))

    }

    fn list_own_property_symbols(&self, 
                                 _trap_info: Box<dyn TrapInfo>, 
                                 context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {
        let _guard = self.rw_lock.lock_write();
        let mut values = Vec::new();
        for symbol in self.own_properties.borrow().keys() {
            values.push(Value::make_symbol(*symbol));
        }
        for symbol in self.property_traps.borrow().keys() {
            values.push(Value::make_symbol(*symbol));
        }
        Ok(SlotTrapResult::Trapped(context.make_list(values, context)?))
    }

    fn notify_drop(&self) -> Result<SlotTrapResult, Error> {
        Ok(SlotTrapResult::Skipped)
    }

    fn list_internal_referenced_symbols(&self) -> Vec<Symbol> {

        let _guard = self.rw_lock.lock_write();

        let mut values = Vec::new();
        for symbol in self.own_properties.borrow().keys() {
            values.push(*symbol);
        }
        for symbol in self.property_traps.borrow().keys() {
            values.push(*symbol);
        }

        values

    }

    fn list_and_autorefresh_internal_referenced_values(&self, self_id: Value, context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {

        let _guard = self.rw_lock.lock_write();
        
        let mut values = Vec::new();
        let prototype = self.prototype.get();
        let new_prototype = context.resolve_real_value(prototype)?;
        if prototype != new_prototype {
            context.add_value_reference(self_id, new_prototype)?;
            self.prototype.set(new_prototype);
            context.remove_value_reference(self_id, prototype)?;
        }
        values.push(new_prototype);

        let mut updates = HashMap::new();

        for (key, value) in self.own_properties.borrow().iter() {
            let new_value = context.resolve_real_value(*value)?;
            if *value != new_value {
                context.add_value_reference(self_id, new_value)?;
                updates.insert(*key, new_value);
                context.remove_value_reference(self_id, *value)?;
            }
            values.push(new_value);
        }

        let mut own_properties = self.own_properties.borrow_mut();
        for (key, new_value) in updates.iter() {
            own_properties.insert(*key, *new_value);
        }

        Ok(values)

    }

    fn list_internal_referenced_values(&self) -> Vec<Value> {

        let _guard = self.rw_lock.lock_read();
        
        let mut values = Vec::new();
        values.push(self.prototype.get());
        for (_key, value) in self.own_properties.borrow().iter() {
            values.push(*value);
        }

        values

    }

    fn refresh_referenced_value(&self, old_value: Value, new_value: Value) {

        let _guard = self.rw_lock.lock_write();

        if self.prototype.get() == old_value {
            self.prototype.set(new_value);
        }

        let mut keys = Vec::new();
        for (key, value) in self.own_properties.borrow().iter() {
            if *value == old_value {
                keys.push(*key);
            }
        }

        for key in keys.iter() {
            self.own_properties.borrow_mut().insert(*key, new_value);
        }

    }
 
}
