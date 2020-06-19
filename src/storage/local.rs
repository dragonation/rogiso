use std::collections::HashSet;
use std::sync::Arc;

use super::super::base::Error;
use super::super::base::ErrorType::*;
use super::super::base::Symbol;
use super::super::base::Value;
use super::super::context::Context;
use super::super::field_shortcuts::FieldToken;
use super::super::internal_slot::InternalSlot;
use super::super::internal_slot::ProtectedInternalSlot;
use super::super::isolate::Isolate;
use super::super::storage::Pinned;
use super::super::root::Root;
use super::super::trap::PropertyTrap;
use super::super::trap::SlotTrap;
use super::super::util::ReentrantToken;

/// Value visitor for native stack 
pub struct Local<'a> {
    context: &'a Box<dyn Context>,
    root: Arc<Root>
}

impl<'a> Local<'a> {

    /// Create a new local
    pub fn new(context: &'a Box<dyn Context>, value: Value) -> Result<Local<'a>, Error> {

        if !value.is_slotted() {
            return Err(Error::new(FatalError, "Value not slotted"));
        }

        let root = context.add_root(value)?;

        Ok(Local {
            context: context,
            root: root 
        })
        
    }

    pub fn for_symbol(context: &'a Box<dyn Context>, symbol: Symbol) -> Result<Local<'a>, Error> {

        Local::new(context, Value::make_symbol(symbol))

    }

    pub fn from_pinned(context: &'a Box<dyn Context>, pinned: Pinned) -> Result<Local<'a>, Error> {

        Local::new(context, pinned.get_value())

    }

    /// Get the isolate of the local
    pub fn get_isolate(&'a self) -> &'a Arc<Isolate> {
        self.context.get_isolate()
    }

    pub fn get_slot_layout_token(&'a self) -> &'a ReentrantToken {
        self.context.get_slot_layout_token()
    }

    /// Get the value of the local
    pub fn get_value(&self) -> Value {
        self.root.get_value()
    }

    /// Get prototype of a value
    pub fn get_prototype(&self) -> Result<Pinned, Error> {
        self.context.get_prototype(self.root.get_value(), self.context)
    }

    /// Set prototype of a value
    pub fn set_prototype(&self, prototype: Value) -> Result<(), Error> {
        self.context.set_prototype(self.root.get_value(), prototype, self.context)
    }

    /// Set a trap for specified slot
    pub fn set_slot_trap(&self, slot_trap: Arc<dyn SlotTrap>) -> Result<(), Error> {
        self.context.set_slot_trap(self.root.get_value(), slot_trap, self.context)
    }

    /// Test whether a slot has own some properties
    pub fn has_own_property(&self, symbol: Symbol) -> Result<bool, Error> {
        self.context.has_own_property(self.root.get_value(), symbol, self.context)
    }

    /// Get own property of a value
    pub fn get_own_property(&self, symbol: Symbol, field_token: Option<&FieldToken>) -> Result<Pinned, Error> {
        self.context.get_own_property(self.root.get_value(), symbol, field_token, self.context)
    }

    /// Set own property of a value
    pub fn set_own_property(&self, symbol: Symbol, value: Value) -> Result<(), Error> {
        self.context.set_own_property(self.root.get_value(), symbol, value, self.context)
    }

    /// Define own property of a value
    pub fn define_own_property(&self, symbol: Symbol, property_trap: Arc<dyn PropertyTrap>) -> Result<(), Error> {
        self.context.define_own_property(self.root.get_value(), symbol, property_trap, self.context)
    }

    /// List own property symbols in a value
    pub fn list_own_property_symbols(&self) -> Result<HashSet<Symbol>, Error> {
        self.context.list_own_property_symbols(self.root.get_value(), self.context)
    }

    /// List property symbols in a value
    pub fn list_property_symbols(&self) -> Result<HashSet<Symbol>, Error> {
        self.context.list_property_symbols(self.root.get_value(), self.context)
    }

    /// Test whether a slot has some properties
    pub fn has_property(&self, symbol: Symbol) -> Result<bool, Error> {
        self.context.has_property(self.root.get_value(), symbol, self.context)
    }

    /// Get property of a value
    pub fn get_property(&self, symbol: Symbol, field_token: Option<&FieldToken>) -> Result<Pinned, Error> {
        self.context.get_property(self.root.get_value(), symbol, field_token, self.context)
    }
   
    /// Get a specified internal slot from a value
    pub fn get_internal_slot(&'a self, index: u64) -> Result<Option<ProtectedInternalSlot<'a>>, Error> {
        self.context.get_internal_slot(self.root.get_value(), index, self.context)
    }

    /// Set a specified internal slot of a value
    pub fn set_internal_slot(&self, index: u64, internal_slot: Arc<dyn InternalSlot>, context: &Box<dyn Context>) -> Result<(), Error> {
        self.context.set_internal_slot(self.root.get_value(), index, internal_slot, context)
    }

    /// Clear a specified internal slot of a value
    pub fn clear_internal_slot(&self, index: u64, context: &Box<dyn Context>) -> Result<(), Error> {
        self.context.clear_internal_slot(self.root.get_value(), index, context)
    }

}

impl<'a> Drop for Local<'a> {
    fn drop(&mut self) {
        match self.context.remove_root(&self.root) {
            Err(_) => {
                panic!("Failed to drop root");
            },
            _ => {}
        }
    }
}
