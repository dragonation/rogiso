use std::any::Any;
use std::sync::Arc;
use std::cell::Cell;
use std::ops::Deref;

use super::super::base::Error;
use super::super::base::ErrorType::*;
use super::super::base::Symbol;
use super::super::base::Value;
use super::super::context::Context;
use super::super::storage::Pinned;
use super::super::trap::TrapInfo;
use super::super::util::RwLock;

/// Property trap on specified slotted object, usually used with a symbol
/// 
/// A property trap is a trap for property getter and setter
///
/// Usually the property info will be recorded as a `TrapInfo` object during 
/// getting and setting property
pub trait PropertyTrap {

    /// Convert the property trap into `Any` to make it support downcast
    ///
    /// Sometimes we need to extract the implementation of the property trap 
    /// for specific functions or optimizations
    ///
    /// You can return `self` directly while implementing it
    fn as_any(&self) -> &dyn Any;

    /// Check whether the property trap is a simple-field one
    ///
    /// A simple-field property trap means the property has all of the features
    /// below:
    /// * No side-effects, only explicit `property_trap.set_property(trap_info)` 
    ///   call will make the value changes
    /// * No more references except the value itself, only the property value is 
    ///   referenced by the property trap
    /// * Fixed and predictable execution, no more context touch except 
    ///   references, and there is no more script internal execution related, 
    ///   too
    /// * Cacheable, the property value is stable all the time, no volatile 
    ///   changes will happen.
    /// 
    /// With the features above, it makes the property trap value cacheable in 
    /// the isolate if the field shortcuts is set and a prepared field token is 
    /// provided during the operation of property with specific symbol
    ///
    /// **Default** return `false`, that means it is a complex field and 
    /// non-cacheable
    fn is_simple_field(&self) -> bool {
        false
    }

    /// Get the property value with specified symbol from the object
    ///
    /// The `trap_info` object records the information of the object and symbol
    /// * `trap_info.get_subject()` to get the the object value
    /// * `trap_info.get_parameters(0)` to get the symbol value
    ///
    /// Symbol info could be resolved from symbol value by following APIs:
    /// * `environment.extract_key(value) -> Key` to extract the real key from 
    ///   the symbol value in the isolate
    /// * `environment.resolve_symbol_info(key) -> SymbolInfo` to resolve the 
    ///   related symbol info
    ///
    /// **Default** return `Ok(Value::make_undefined())`, that means the
    /// property is missing, and the isolate will try to get the property from 
    /// the prototype of the object at next
    fn get_property(&self, _trap_info: Box<dyn TrapInfo>, context: &Box<dyn Context>) -> Result<Pinned, Error> {
        Pinned::new(context, Value::make_undefined())
    }

    /// Set the property in object a new value
    ///
    /// Result is two vectors containing reference changes `(removed_references, added_references)`
    fn set_property(&self, _trap_info: Box<dyn TrapInfo>, _context: &Box<dyn Context>) -> Result<(Vec<Value>, Vec<Value>, Vec<Symbol>, Vec<Symbol>), Error> {
        Err(Error::new(MutatingReadOnlyProperty, "Property immutable"))
    }

    fn list_and_autorefresh_referenced_values(&self, _self_id: Value, _context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {
        Ok(vec!())
    }

    /// List all referenced values to keep them from garbage collection
    fn list_referenced_values(&self) -> Vec<Value> {
        vec!()
    }

    /// List all referenced symbols to keep them from garbage collection
    fn list_internal_referenced_symbols(&self) -> Vec<Symbol> {
        vec!()
    }

    /// Refresh referenced value for memory refragment
    fn refresh_referenced_value(&self, _old_value: Value, _new_value: Value) {
        // Do nothing
    }
    
}

pub struct ProtectedPropertyTrap<'a> {
    context: &'a Box<dyn Context>,
    property_trap: Arc<dyn PropertyTrap>,
    protected_id: u64
}

impl<'a> ProtectedPropertyTrap<'a> {
    pub fn new(property_trap: &Arc<dyn PropertyTrap>, context: &'a Box<dyn Context>) -> Result<ProtectedPropertyTrap<'a>, Error> {
        let (protected_id, property_trap) = context.protect_property_trap(property_trap)?;
        Ok(ProtectedPropertyTrap {
            context: context,
            property_trap: property_trap,
            protected_id: protected_id
        })
    }
}

impl<'a> Drop for ProtectedPropertyTrap<'a> {
    fn drop(&mut self) {
        if self.context.unprotect_property_trap(self.protected_id).is_err() {
            panic!("Failed to unprotect property trap");
        }
    }
}

impl<'a> Deref for ProtectedPropertyTrap<'a> {
    type Target = Arc<dyn PropertyTrap>;
    fn deref(&self) -> &Self::Target {
        &self.property_trap
    }
}

pub struct FieldPropertyTrap {
    rw_lock: RwLock,
    value: Cell<Value>
}

impl FieldPropertyTrap {
    pub fn new(value: Value) -> FieldPropertyTrap {
        FieldPropertyTrap {
            rw_lock: RwLock::new(),
            value: Cell::new(value)
        }
    }
}

impl PropertyTrap for FieldPropertyTrap {

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_simple_field(&self) -> bool {
        true
    }

    fn get_property(&self, _trap_info: Box<dyn TrapInfo>, context: &Box<dyn Context>) -> Result<Pinned, Error> {

        let _guard = self.rw_lock.lock_read();

        Pinned::new(context, self.value.get())
        
    }

    fn set_property(&self, trap_info: Box<dyn TrapInfo>, _context: &Box<dyn Context>) -> Result<(Vec<Value>, Vec<Value>, Vec<Symbol>, Vec<Symbol>), Error> {

        let _guard = self.rw_lock.lock_write();

        let old_value = self.value.get();
        let value = trap_info.get_parameter(1);
        self.value.replace(value);

        if old_value != value {
            Ok(([old_value].to_vec(), [value].to_vec(), Vec::new(), Vec::new()))
        } else {
            Ok((Vec::new(), Vec::new(), Vec::new(), Vec::new()))
        }

    }

    fn list_and_autorefresh_referenced_values(&self, self_id: Value, context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {
 
        let _guard = self.rw_lock.lock_read();

        let value = self.value.get();
        let new_value = context.resolve_real_value(value)?;

        if value != new_value {
            context.add_value_reference(self_id, new_value)?;    
            self.value.set(new_value);
            context.remove_value_reference(self_id, value)?;
        }

        Ok(vec!(new_value))
       
    }

    fn list_referenced_values(&self) -> Vec<Value> {

        let _guard = self.rw_lock.lock_read();

        vec!(self.value.get())

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

// #[cfg(test)] use super::super::test::TestTrapInfo;

// #[test]
// fn test_field_property_trap() -> Result<(), Error> {

//     let property_trap = FieldPropertyTrap::new(Value::make_float(4534.0));

//     let trap_info = Box::new(TestTrapInfo::new(Value::make_null(), [Value::make_null()].to_vec()));

//     assert_eq!(property_trap.get_property(trap_info)?, Value::make_float(4534.0));

//     let trap_info = Box::new(TestTrapInfo::new(Value::make_null(), [Value::make_null(), Value::make_float(1.0)].to_vec()));
//     property_trap.set_property(trap_info)?;

//     let trap_info = Box::new(TestTrapInfo::new(Value::make_null(), [Value::make_null()].to_vec()));
//     assert_eq!(property_trap.get_property(trap_info)?, Value::make_float(1.0));

//     assert_eq!(property_trap.list_referenced_values().len(), 1);
//     assert_eq!(property_trap.list_referenced_values()[0], Value::make_float(1.0));

//     property_trap.refresh_referenced_value(Value::make_float(2.0), Value::make_float(3.0));
//     assert_eq!(property_trap.list_referenced_values().len(), 1);
//     assert_eq!(property_trap.list_referenced_values()[0], Value::make_float(1.0));

//     property_trap.refresh_referenced_value(Value::make_float(1.0), Value::make_float(3.0));
//     assert_eq!(property_trap.list_referenced_values().len(), 1);
//     assert_eq!(property_trap.list_referenced_values()[0], Value::make_float(3.0));
//     Ok(())
// }