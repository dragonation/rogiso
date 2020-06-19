use std::ops::Deref;
use std::sync::Arc;

use super::super::base::Error;
use super::super::base::Symbol;
use super::super::base::Value;
use super::super::context::Context;
use super::super::storage::Pinned;
use super::super::trap::TrapInfo;

/// The result of slot trap call
pub enum SlotTrapResult {

    /// The trap has response with the value
    Trapped(Pinned),

    /// An error has happened in the trap
    Thrown(Pinned),

    /// Skip the trap
    Skipped,

}

/// Slot trap for interrupt slot operations
pub trait SlotTrap {

    /// Get prototype of a slot
    fn get_prototype(&self, 
                     _trap_info: Box<dyn TrapInfo>, 
                     _context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {
        Ok(SlotTrapResult::Skipped)
    }

    /// Set prototype of a slot
    fn set_prototype(&self, 
                     _trap_info: Box<dyn TrapInfo>, 
                     _context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {
        Ok(SlotTrapResult::Skipped)
    }

    /// Test whether a slot has some properties
    fn has_own_property(&self, 
                        _trap_info: Box<dyn TrapInfo>, 
                        _context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {
        Ok(SlotTrapResult::Skipped)
    }

    /// Get own property of a value
    fn get_own_property(&self, 
                        _trap_info: Box<dyn TrapInfo>, 
                        _context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {
        Ok(SlotTrapResult::Skipped)
    }

    /// Set own property of a value
    fn set_own_property(&self, 
                        _trap_info: Box<dyn TrapInfo>, 
                        _context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {
        Ok(SlotTrapResult::Skipped)
    }

    /// Define own property of a value
    fn define_own_property(&self, 
                           _trap_info: Box<dyn TrapInfo>, 
                           _context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {
        Ok(SlotTrapResult::Skipped)
    }

    /// Delete own property from a value
    fn delete_own_property(&self, 
                           _trap_info: Box<dyn TrapInfo>, 
                           _context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {
        Ok(SlotTrapResult::Skipped)
    }

    /// List all own properties of value
    fn list_own_property_symbols(&self, 
                                 _trap_info: Box<dyn TrapInfo>, 
                                 _context: &Box<dyn Context>) -> Result<SlotTrapResult, Error> {
        Ok(SlotTrapResult::Skipped)
    }

    /// Notify when the value is dropped
    fn notify_drop(&self) -> Result<SlotTrapResult, Error> {
        Ok(SlotTrapResult::Skipped)
    } 

    /// List all internal referenced symbols to keep them from garbage collection
    fn list_internal_referenced_symbols(&self) -> Vec<Symbol> {
        [].to_vec()
    }

    /// List all internal referenced values to keep them from garbage collection
    fn list_internal_referenced_values(&self) -> Vec<Value> {
        vec!()
    }

    fn list_and_autorefresh_internal_referenced_values(&self, _self_id: Value, _context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {
        Ok(vec!())
    }

    /// Refresh referenced value for memory refragment
    fn refresh_referenced_value(&self, _old_value: Value, _new_value: Value) {
        // Do nothing
    }
    
}

pub struct ProtectedSlotTrap<'a> {
    context: &'a Box<dyn Context>,
    slot_trap: Arc<dyn SlotTrap>,
    protected_id: u64
}

impl<'a> ProtectedSlotTrap<'a> {
    pub fn new(slot_trap: &Arc<dyn SlotTrap>, context: &'a Box<dyn Context>) -> Result<ProtectedSlotTrap<'a>, Error> {
        let (protected_id, slot_trap) = context.protect_slot_trap(slot_trap)?;
        Ok(ProtectedSlotTrap {
            context: context,
            slot_trap: slot_trap,
            protected_id: protected_id
        })
    }
}

impl<'a> Drop for ProtectedSlotTrap<'a> {
    fn drop(&mut self) {
        if self.context.unprotect_slot_trap(self.protected_id).is_err() {
            panic!("Failed to unprotect slot trap");
        }
    }
}

impl<'a> Deref for ProtectedSlotTrap<'a> {
    type Target = Arc<dyn SlotTrap>;
    fn deref(&self) -> &Self::Target {
        &self.slot_trap
    }
}

