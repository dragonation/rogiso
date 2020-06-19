use std::collections::HashSet;
use std::sync::Arc;

use super::base::Error;
use super::base::PrimitiveType;
use super::base::PrimitiveType::*;
use super::base::Symbol;
use super::base::Value;
use super::field_shortcuts::FieldToken;
use super::internal_slot::InternalSlot;
use super::internal_slot::ProtectedInternalSlot;
use super::internal_slot::List;
use super::internal_slot::Text;
use super::internal_slot::Tuple;
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

/// Rogic context for API calls
pub trait Context {

    /// Get isolate of the context
    fn get_isolate<'a>(&'a self) -> &'a Arc<Isolate>;

    /// Get the slot layout token to lock slot layouts.
    /// The token could be used to keep your slot got from slot refragmentation
    fn get_slot_layout_token<'a>(&'a self) -> &'a ReentrantToken;

    fn protect_property_trap(&self, property_trap: &Arc<dyn PropertyTrap>) -> Result<(u64, Arc<dyn PropertyTrap>), Error> {
        self.get_isolate().protect_property_trap(property_trap)
    }

    fn unprotect_property_trap(&self, protected_id: u64) -> Result<(), Error> {
        self.get_isolate().unprotect_property_trap(protected_id)
    }

    fn protect_slot_trap(&self, slot_trap: &Arc<dyn SlotTrap>) -> Result<(u64, Arc<dyn SlotTrap>), Error> {
        self.get_isolate().protect_slot_trap(slot_trap)
    }

    fn unprotect_slot_trap(&self, protected_id: u64) -> Result<(), Error> {
        self.get_isolate().unprotect_slot_trap(protected_id)
    }

    fn protect_internal_slot(&self, internal_slot: &Arc<dyn InternalSlot>) -> Result<(u64, Arc<dyn InternalSlot>), Error> {
        self.get_isolate().protect_internal_slot(internal_slot)
    }

    fn unprotect_internal_slot(&self, protected_id: u64) -> Result<(), Error> {
        self.get_isolate().unprotect_internal_slot(protected_id)
    }

    /// Resolve the value, to get the final value usable.
    /// The API helps you to keep the value from slot refragmention redirection
    fn resolve_real_value(&self, value: Value) -> Result<Value, Error> {
        self.get_isolate().resolve_real_value(value, self.get_slot_layout_token())
    }


    /// Add value reference, to inform the isolate that there are some values 
    /// have referenced to the value you concerned
    fn add_value_reference(&self, from: Value, to: Value) -> Result<(), Error> {
        self.get_isolate().add_value_reference(from, to, self.get_slot_layout_token())
    }

    /// Remove value reference, to inform the isolate that a reference to the
    /// value you concerned has disconnected the relationship
    fn remove_value_reference(&self, from: Value, to: Value) -> Result<(), Error> {
        self.get_isolate().remove_value_reference(from, to, self.get_slot_layout_token())?;
        Ok(())
    }
    
    /// Add symbol reference, to inform the isolate that keeps the symbol from recycle
    fn add_symbol_reference(&self, symbol: Symbol) -> Result<(), Error> {
        self.get_isolate().add_symbol_reference(symbol)
    }

    /// Remove symbol reference, to inform the isolate that the symbol could be recycled
    fn remove_symbol_reference(&self, symbol: Symbol) -> Result<(), Error> {
        self.get_isolate().remove_symbol_reference(symbol)
    }


    /// Create a new trap info
    fn create_trap_info(&self, subject: Value, parameters: Vec<Value>, context: &Box<dyn Context>) -> Box<dyn TrapInfo>;


    /// Gain a new slot with prototype preset
    fn gain_slot(&self, primitive_type: PrimitiveType, prototype: Value) -> Result<Value, Error>;


    /// Get a symbol with specified scope and text
    fn get_text_symbol(&self, scope: &str, text: &str) -> Symbol {
        self.get_isolate().get_text_symbol(scope, text)
    }

    /// Get a symbol with specified scope and value
    fn get_value_symbol(&self, scope: &str, value: Value) -> Symbol {
        self.get_isolate().get_value_symbol(scope, value)
    }
    
    /// Resolve symbol info from a symbol
    fn resolve_symbol_info(&self, symbol: Symbol) -> Result<SymbolInfo, Error> {
        self.get_isolate().resolve_symbol_info(symbol)
    }


    /// Get prototype of a value
    fn get_prototype(&self, value: Value, context: &Box<dyn Context>) -> Result<Pinned, Error> {
        self.get_isolate().get_prototype(value, context)
    }

    /// Set prototype of a value
    fn set_prototype(&self, value: Value, prototype: Value, context: &Box<dyn Context>) -> Result<(), Error> {
        self.get_isolate().set_prototype(value, prototype, context)
    }


    /// Set a trap for specified slot
    fn set_slot_trap(&self, value: Value, slot_trap: Arc<dyn SlotTrap>, context: &Box<dyn Context>) -> Result<(), Error> {
        self.get_isolate().set_slot_trap(value, slot_trap, context)
    }


    /// Test whether a slot has own some properties
    fn has_own_property(&self, subject: Value, symbol: Symbol, context: &Box<dyn Context>) -> Result<bool, Error> {
        self.get_isolate().has_own_property(subject, symbol, context)
    }

    /// Get own property of a value
    fn get_own_property(&self, subject: Value, symbol: Symbol, field_token: Option<&FieldToken>, context: &Box<dyn Context>) -> Result<Pinned, Error> {
        self.get_isolate().get_own_property(subject, symbol, field_token, context)
    }

    /// Delete own property of a value
    fn delete_own_property(&self, subject: Value, symbol: Symbol, context: &Box<dyn Context>) -> Result<(), Error> {
        self.get_isolate().delete_own_property(subject, symbol, context)
    }

    /// Set own property of a value
    fn set_own_property(&self, subject: Value, symbol: Symbol, value: Value, context: &Box<dyn Context>) -> Result<(), Error> {
        self.get_isolate().set_own_property(subject, symbol, value, context)
    }

    /// Define own property of a value
    fn define_own_property(&self, subject: Value, symbol: Symbol, property_trap: Arc<dyn PropertyTrap>, context: &Box<dyn Context>) -> Result<(), Error> {
        self.get_isolate().define_own_property(subject, symbol, property_trap, context)
    }

    /// List own property symbols in a value
    fn list_own_property_symbols(&self, subject: Value, context: &Box<dyn Context>) -> Result<HashSet<Symbol>, Error> {
        self.get_isolate().list_own_property_symbols(subject, context)
    }


    /// Get own property of a value
    fn get_own_property_ignore_slot_trap(&self, subject: Value, symbol: Symbol, context: &Box<dyn Context>) -> Result<Pinned, Error> {
        self.get_isolate().get_own_property_ignore_slot_trap(subject, symbol, context)
    }

    /// Set own property of a value
    fn set_own_property_ignore_slot_trap(&self, subject: Value, symbol: Symbol, value: Value, context: &Box<dyn Context>) -> Result<(), Error> {
        self.get_isolate().set_own_property_ignore_slot_trap(subject, symbol, value, context)
    }

    /// Delete own property of a value
    fn delete_own_property_ignore_slot_trap(&self, subject: Value, symbol: Symbol, context: &Box<dyn Context>) -> Result<(), Error> {
        self.get_isolate().delete_own_property_ignore_slot_trap(subject, symbol, context)
    }

    /// Define own property of a value
    fn define_own_property_ignore_slot_trap(&self, subject: Value, symbol: Symbol, property_trap: Arc<dyn PropertyTrap>, context: &Box<dyn Context>) -> Result<(), Error> {
        self.get_isolate().define_own_property_ignore_slot_trap(subject, symbol, property_trap, context)
    }

    /// List own property symbols in a value
    fn list_own_property_symbols_ignore_slot_trap(&self, subject: Value, context: &Box<dyn Context>) -> Result<HashSet<Symbol>, Error> {
        self.get_isolate().list_own_property_symbols_ignore_slot_trap(subject, context)
    }

    /// Get a specified internal slot from a value
    fn get_internal_slot<'a>(&self, subject: Value, index: u64, context: &'a Box<dyn Context>) -> Result<Option<ProtectedInternalSlot<'a>>, Error> {
        self.get_isolate().get_internal_slot(subject, index, context)
    }

    /// Set a specified internal slot of a value
    fn set_internal_slot(&self, subject: Value, index: u64, internal_slot: Arc<dyn InternalSlot>, context: &Box<dyn Context>) -> Result<(), Error> {
        self.get_isolate().set_internal_slot(subject, index, internal_slot, context)
    }

    /// Clear a specified internal slot of a value
    fn clear_internal_slot(&self, subject: Value, index: u64, context: &Box<dyn Context>) -> Result<(), Error> {
        self.get_isolate().clear_internal_slot(subject, index, context)
    }


    /// List property symbols in a value
    fn list_property_symbols(&self, subject: Value, context: &Box<dyn Context>) -> Result<HashSet<Symbol>, Error> {
        self.get_isolate().list_property_symbols(subject, context)
    }

    /// Test whether a slot has some properties
    fn has_property(&self, subject: Value, symbol: Symbol, context: &Box<dyn Context>) -> Result<bool, Error> {
        self.get_isolate().has_property(subject, symbol, context)
    }

    /// Get property of a value
    fn get_property(&self, subject: Value, symbol: Symbol, field_token: Option<&FieldToken>, context: &Box<dyn Context>) -> Result<Pinned, Error> {
        self.get_isolate().get_property(subject, symbol, field_token, context)
    }
    

    /// Make a text value from string
    fn make_text(&self, text: &str, context: &Box<dyn Context>) -> Result<Pinned, Error> {

        let value = self.gain_slot(Text, self.get_isolate().get_text_prototype())?;

        let text: Arc<dyn InternalSlot> = Arc::new(Text::new(text));

        self.set_internal_slot(value, 0, text, context)?;

        Pinned::new(context, value)

    }

    /// Make a list value from values 
    fn make_list(&self, list: Vec<Value>, context: &Box<dyn Context>) -> Result<Pinned, Error> {

        let value = self.gain_slot(List, self.get_isolate().get_list_prototype())?;

        let list: Arc<dyn InternalSlot> = Arc::new(List::new(value, list));

        self.set_internal_slot(value, 0, list, context)?;

        Pinned::new(context, value)

    }

    /// Make a tuple value from values 
    fn make_tuple(&self, prototype: Value, id: u32, values: Vec<Value>, context: &Box<dyn Context>) -> Result<Pinned, Error> {

        let value = self.gain_slot(Tuple, prototype)?;

        let tuple: Arc<dyn InternalSlot> = Arc::new(Tuple::new(value, id, values));

        self.set_internal_slot(value, 0, tuple, context)?;

        Pinned::new(context, value)

    }


    /// Extract text from a value 
    fn extract_text(&self, value: Value, context: &Box<dyn Context>) -> String {
        self.get_isolate().extract_text(value, context)
    }

    fn extract_list(&self, value: Value, context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {
        self.get_isolate().extract_list(value, context)
    }


    /// Get the value respresenting a property trap
    fn make_property_trap_value(&self, property_trap: Arc<dyn PropertyTrap>, context: &Box<dyn Context>) -> Result<Value, Error>;

    /// Extract the property trap
    fn extract_property_trap(&self, value: Value, context: &Box<dyn Context>) -> Result<Arc<dyn PropertyTrap>, Error>;


    /// Add root value for garbage collection
    fn add_root(&self, value: Value) -> Result<Arc<Root>, Error> {
        self.get_isolate().add_root(value, self.get_slot_layout_token())
    }

    /// Remove value from roots for garbage collection
    fn remove_root(&self, root: &Arc<Root>) -> Result<(), Error> {
        self.get_isolate().remove_root(root)
    }

    /// Add weak root value for garbage collection
    fn add_weak_root(&self, value: Value, drop_listener: Option<Box<dyn DropListener>>) -> Result<Arc<WeakRoot>, Error> {
        self.get_isolate().add_weak_root(value, drop_listener, self.get_slot_layout_token())
    }

    /// Remove value from weak roots for garbage collection
    fn remove_weak_root(&self, root: &Arc<WeakRoot>) -> Result<(), Error> {
        self.get_isolate().remove_weak_root(root)
    }

    /// Notify while a value is dropped
    fn notify_slot_drop(&self, value: Value) -> Result<(), Error> {
        self.get_isolate().notify_slot_drop(value)
    }

}
