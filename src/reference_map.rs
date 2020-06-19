use std::collections::HashMap;
use std::cell::Cell;
use std::cell::RefCell;

use super::base::Error;
use super::base::ErrorType::*;
use super::base::Value;
use super::util::SpinLock;

pub struct ReferenceMap {
    spin_lock: SpinLock,
    count: Cell<u32>,
    counts: RefCell<HashMap<Value, u32>>
}

impl ReferenceMap {

    pub fn new() -> ReferenceMap {
        ReferenceMap {
            spin_lock: SpinLock::new(),
            count: Cell::new(0),
            counts: RefCell::new(HashMap::new())
        }
    }

    pub fn is_empty(&self) -> bool {
        self.count.get() == 0
    }

    pub fn add_reference(&self, value: Value) -> Result<(), Error> {

        let _guard = self.spin_lock.lock();

        let count = match self.counts.borrow().get(&value) {
            None => 0,
            Some(count) => *count
        };

        self.counts.borrow_mut().insert(value, count + 1);
        self.count.set(self.count.get() + 1);

        Ok(())

    }

    pub fn remove_reference(&self, value: Value) -> Result<(), Error> {

        let _guard = self.spin_lock.lock();

        let count = match self.counts.borrow().get(&value) {
            None => {
                return Err(Error::new(FatalError, "No references recorded"));
            },
            Some(count) => *count
        };

        if count == 0 {
            return Err(Error::new(FatalError, "Reference count should always greater than or equals to zero"))
        }

        if count > 1 {
            self.counts.borrow_mut().insert(value, count - 1);
        } else {
            self.counts.borrow_mut().remove(&value);
        }

        self.count.set(self.count.get() - 1);

        Ok(())

    }

}

#[test]
fn test_add_reference() -> Result<(), Error> {

    let reference_map = ReferenceMap::new();

    assert!(reference_map.is_empty());
    reference_map.add_reference(Value::make_undefined())?;

    assert!(!reference_map.is_empty());

    Ok(())

}

#[test]
fn test_multiple_references() -> Result<(), Error> {

    let reference_map = ReferenceMap::new();

    assert!(reference_map.is_empty());
    reference_map.add_reference(Value::make_undefined())?;

    assert!(!reference_map.is_empty());
    reference_map.add_reference(Value::make_boolean(true))?;

    assert!(!reference_map.is_empty());
    reference_map.remove_reference(Value::make_undefined())?;

    assert!(!reference_map.is_empty());
    reference_map.remove_reference(Value::make_boolean(true))?;

    assert!(reference_map.is_empty());

    Ok(())
}

#[test]
fn test_remove_reference_not_found() -> Result<(), Error> {

    let reference_map = ReferenceMap::new();

    assert!(reference_map.is_empty());
    reference_map.add_reference(Value::make_undefined())?;

    assert!(!reference_map.is_empty());
    assert!(reference_map.remove_reference(Value::make_boolean(true)).is_err());

    assert!(!reference_map.is_empty());
    reference_map.remove_reference(Value::make_undefined())?;

    assert!(reference_map.is_empty());

    Ok(())
}

#[test]
fn test_remove_reference() -> Result<(), Error> {

    let reference_map = ReferenceMap::new();

    assert!(reference_map.is_empty());

    reference_map.add_reference(Value::make_undefined())?;
    reference_map.add_reference(Value::make_undefined())?;

    assert!(!reference_map.is_empty());
    reference_map.remove_reference(Value::make_undefined())?;

    assert!(!reference_map.is_empty());
    reference_map.remove_reference(Value::make_undefined())?;
    
    assert!(reference_map.is_empty());

    Ok(())

}