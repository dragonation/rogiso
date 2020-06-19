use std::cell::Cell;
use std::any::Any;

use super::super::base::Error;
use super::super::base::Value;
use super::super::context::Context;
use super::super::internal_slot::InternalSlot;
use super::super::util::RwLock;

pub struct TestInternalSlot {
    rw_lock: RwLock,
    reference: Cell<Value>
}

impl TestInternalSlot {
    pub fn new(reference: Value) -> TestInternalSlot {
        TestInternalSlot {
            rw_lock: RwLock::new(),
            reference: Cell::new(reference)
        }
    }
}

impl InternalSlot for TestInternalSlot {

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn list_and_autorefresh_referenced_values(&self, self_id: Value, context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {

        let _guard = self.rw_lock.lock_write();
        
        let reference = self.reference.get();
        let new_reference = context.resolve_real_value(reference)?;
        if reference != new_reference {
            context.add_value_reference(self_id, new_reference)?;
            self.reference.set(new_reference);
            context.remove_value_reference(self_id, reference)?;
        }

        Ok(vec!(new_reference))

    }

    fn list_referenced_values(&self) -> Vec<Value> {

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
