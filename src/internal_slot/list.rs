use std::any::Any;
use std::cell::Cell;
use std::cell::RefCell;

use super::internal_slot::InternalSlot;

use super::super::base::Error;
use super::super::base::Value;
use super::super::context::Context;
use super::super::util::RwLock;


pub struct List {
    subject: Cell<Value>,
    rw_lock: RwLock,
    values: RefCell<Vec<Cell<Value>>>
}

// List constructor
impl List {

    pub fn new(subject: Value, values: Vec<Value>) -> List {
        let mut new_values = Vec::new();
        for value in values {
            new_values.push(Cell::new(value));
        }
        List {
            subject: Cell::new(subject),
            rw_lock: RwLock::new(),
            values: RefCell::new(new_values)
        }
    }

}

impl InternalSlot for List {

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn refresh_subject(&self, subject: Value) {

        let _guard = self.rw_lock.lock_write();

        self.subject.set(subject);

    }

    fn list_and_autorefresh_referenced_values(&self, self_id: Value, context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {

        let _guard = self.rw_lock.lock_write();

        let values = self.values.borrow();
        let mut result = Vec::with_capacity(values.len());
        for value in values.iter() {
            let old_value = value.get();
            let new_value = context.resolve_real_value(old_value)?;
            if old_value != new_value {
                context.add_value_reference(self_id, new_value)?;
                value.set(new_value);
                context.remove_value_reference(self_id, old_value)?;
            }
            result.push(new_value);
        }

        Ok(result)

    }
    
    fn list_referenced_values(&self) -> Vec<Value> {

        self.get_value_list()

    }

    fn refresh_referenced_value(&self, old_value: Value, new_value: Value) {

        let _guard = self.rw_lock.lock_write();

        let values = self.values.borrow();

        for value in values.iter() {
            if value.get() == old_value {
                value.set(new_value);
            }
        }

    }
    
}

// List basic properties
impl List {

    pub fn get_length(&self) -> usize {

        let _guard = self.rw_lock.lock_read();

        self.values.borrow().len()

    }

    pub fn get_element(&self, index: usize) -> Value {

        let _guard = self.rw_lock.lock_read();

        let values = self.values.borrow();
        if index >= values.len() {
            return Value::make_undefined();
        }

        values[index].get()

    }

    pub fn set_element(&self, index: usize, value: Value) -> (Vec<Value>, Vec<Value>) {

        let _guard = self.rw_lock.lock_write();

        let mut values = self.values.borrow_mut();
        while index >= values.len() {
            values.push(Cell::new(Value::make_undefined()));
        }

        let old_value = values[index].get();

        values[index].set(value);

        ([old_value].to_vec(), [value].to_vec())

    }

}

impl List {

    pub fn get_value_list(&self) -> Vec<Value> {

        let _guard = self.rw_lock.lock_read();

        let values = self.values.borrow();

        let mut result = Vec::with_capacity(values.len());
        for value in values.iter() {
            result.push(value.get());
        }

        result

    }

    // TODO: rest apis
    // push
    // pop
    // shift
    // unshift


}

#[test]
fn test_creation() {

    let _list = List::new(Value::make_null(), [Value::make_cardinal(23), Value::make_cardinal(34)].to_vec());

}

#[test]
fn test_list_references() {

    let list = List::new(Value::make_null(), [Value::make_cardinal(23), Value::make_cardinal(34)].to_vec());

    assert_eq!(list.list_referenced_values().len(), 2);
    assert_eq!(list.list_referenced_values()[0], Value::make_cardinal(23));
    assert_eq!(list.list_referenced_values()[1], Value::make_cardinal(34));

}

#[test]
fn test_refresh_reference() {

    let list = List::new(Value::make_null(), [Value::make_cardinal(23), Value::make_cardinal(34)].to_vec());

    list.refresh_referenced_value(Value::make_cardinal(34), Value::make_float(3.14));

    assert_eq!(list.get_length(), 2);
    assert_eq!(list.get_element(0), Value::make_cardinal(23));
    assert_eq!(list.get_element(1), Value::make_float(3.14));

}

#[test]
fn test_length() {

    let list = List::new(Value::make_null(), [Value::make_cardinal(23), Value::make_cardinal(34)].to_vec());

    assert_eq!(list.get_length(), 2);

}

#[test]
fn test_elements() {

    let list = List::new(Value::make_null(), [Value::make_cardinal(23), Value::make_cardinal(34)].to_vec());

    assert_eq!(list.get_element(0), Value::make_cardinal(23));
    assert_eq!(list.get_element(1), Value::make_cardinal(34));
    assert_eq!(list.get_element(2), Value::make_undefined());

    let (removes, adds) = list.set_element(0, Value::make_float(3.14));
    assert_eq!(list.get_element(0), Value::make_float(3.14));
    assert_eq!(removes.len(), 1);
    assert_eq!(removes[0], Value::make_cardinal(23));
    assert_eq!(adds.len(), 1);
    assert_eq!(adds[0], Value::make_float(3.14));

    let (removes, adds) = list.set_element(4, Value::make_float(6.14));
    assert_eq!(list.get_element(4), Value::make_float(6.14));
    assert_eq!(removes.len(), 1);
    assert_eq!(removes[0], Value::make_undefined());
    assert_eq!(adds.len(), 1);
    assert_eq!(adds[0], Value::make_float(6.14));

    assert_eq!(list.get_element(3), Value::make_undefined());
    assert_eq!(list.get_element(5), Value::make_undefined());

}
