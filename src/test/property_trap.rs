use std::any::Any;
use std::cell::Cell;

use super::super::base::Error;
use super::super::base::Symbol;
use super::super::base::Value;
use super::super::context::Context;
use super::super::storage::Pinned;
use super::super::util::RwLock;
use super::super::trap::PropertyTrap;
use super::super::trap::TrapInfo;

pub struct TestPropertyTrap {
    rw_lock: RwLock,
    value: Cell<Value>
}

impl TestPropertyTrap {

    pub fn new(value: Value) -> TestPropertyTrap {
        TestPropertyTrap {
            rw_lock: RwLock::new(),
            value: Cell::new(value)
        }
    }

}

impl PropertyTrap for TestPropertyTrap {

    fn as_any(&self) -> &dyn Any {
        self
    }
    
    fn get_property(&self, _trap_info: Box<dyn TrapInfo>, context: &Box<dyn Context>) -> Result<Pinned, Error> {

        let _guard = self.rw_lock.lock_read();

        Pinned::new(context, self.value.get())
        
    }

    fn set_property(&self, trap_info: Box<dyn TrapInfo>, _context: &Box<dyn Context>) -> Result<(Vec<Value>, Vec<Value>, Vec<Symbol>, Vec<Symbol>), Error> {

        let _guard = self.rw_lock.lock_write();

        let old_value = self.value.get();
        let value = trap_info.get_parameter(2);
        self.value.replace(value);

        if old_value != value {
            Ok(([old_value].to_vec(), [value].to_vec(), Vec::new(), Vec::new()))
        } else {
            Ok((Vec::new(), Vec::new(), Vec::new(), Vec::new()))
        }

    }

    fn list_and_autorefresh_referenced_values(&self, self_id: Value, context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {

        let _guard = self.rw_lock.lock_write();
        
        let reference = self.value.get();
        let new_reference = context.resolve_real_value(reference)?;
        if reference != new_reference {
            context.add_value_reference(self_id, new_reference)?;
            self.value.set(new_reference);
            context.remove_value_reference(self_id, reference)?;
        }

        Ok(vec!(new_reference))

    }

    fn list_referenced_values(&self) -> Vec<Value> {

        let _guard = self.rw_lock.lock_read();

        [self.value.get()].to_vec()

    }

    fn refresh_referenced_value(&self, old_value: Value, new_value: Value) {

        {
            let _guard = self.rw_lock.lock_read();
            if self.value.get() != old_value {
                return;
            }
        }

        {
            let _guard = self.rw_lock.lock_write();
            if self.value.get() != old_value {
                return;
            }
            self.value.set(new_value);
        }
        
    }
    
}
