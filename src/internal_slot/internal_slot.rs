use std::any::Any;
use std::ops::Deref;
use std::sync::Arc;

use super::super::base::Error;
use super::super::base::Symbol;
use super::super::base::Value;

use super::super::context::Context;

/// Native internal slot for slotteds
pub trait InternalSlot: Any {

    /// Cast internal slot into any to make it available to other specified codes
    fn as_any(&self) -> &dyn Any;

    /// Get the subject value of the internal slot
    fn get_subject(&self) -> Value {
        Value::make_undefined()
    }

    /// Refresh the subject value of the internal slot for refragment
    fn refresh_subject(&self, _subject: Value) {
        // Do nothing
    }

    /// List all referenced values to keep them from garbage collection
    fn list_referenced_values(&self) -> Vec<Value> {
        [].to_vec()
    }

    fn list_and_autorefresh_referenced_values(&self, _self_id: Value, _context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {
        Ok(vec!())
    }

    /// List all referenced symbols to keep them from garbage collection
    fn list_referenced_symbols(&self) -> Vec<Symbol> {
        vec!()
    }

    /// Refresh referenced value for memory refragment
    fn refresh_referenced_value(&self, _old_value: Value, _new_value: Value) {
        // Do nothing
    }

}

pub struct ProtectedInternalSlot<'a> {
    context: &'a Box<dyn Context>,
    internal_slot: Arc<dyn InternalSlot>,
    protected_id: u64
}

impl<'a> ProtectedInternalSlot<'a> {
    pub fn new(internal_slot: &Arc<dyn InternalSlot>, context: &'a Box<dyn Context>) -> Result<ProtectedInternalSlot<'a>, Error> {
        let (protected_id, internal_slot) = context.protect_internal_slot(internal_slot)?;
        Ok(ProtectedInternalSlot {
            context: context,
            internal_slot: internal_slot,
            protected_id: protected_id
        })
    }
}

impl<'a> Drop for ProtectedInternalSlot<'a> {
    fn drop(&mut self) {
        if self.context.unprotect_internal_slot(self.protected_id).is_err() {
            panic!("Failed to unprotect internal slot");
        }
    }
}

impl<'a> Deref for ProtectedInternalSlot<'a> {
    type Target = Arc<dyn InternalSlot>;
    fn deref(&self) -> &Self::Target {
        &self.internal_slot
    }
}
