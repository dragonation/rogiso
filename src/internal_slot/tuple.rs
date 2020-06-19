use std::any::Any;
use std::cell::Cell;

use super::internal_slot::InternalSlot;

use super::super::base::Error;
use super::super::base::Value;
use super::super::context::Context;
use super::super::util::RwLock;


pub struct Tuple {
    rw_lock: RwLock,
    subject: Cell<Value>,
    id: u32,
    values: Vec<Cell<Value>>
}

// Tuple constructor
impl Tuple {

    pub fn new(subject: Value, id: u32, values: Vec<Value>) -> Tuple {
        let mut new_values = Vec::new();
        for value in values.iter() {
            new_values.push(Cell::new(*value));
        }
        Tuple {
            rw_lock: RwLock::new(),
            subject: Cell::new(subject),
            id: id,
            values: new_values
        }
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

}

impl InternalSlot for Tuple {

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn refresh_subject(&self, subject: Value) {

        let _guard = self.rw_lock.lock_write();

        self.subject.set(subject);

    }

    fn list_and_autorefresh_referenced_values(&self, self_id: Value, context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {

        let _guard = self.rw_lock.lock_write();

        let mut result = Vec::with_capacity(self.values.len());
        for value in self.values.iter() {
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

        for value in self.values.iter() {
            if value.get() == old_value {
                value.set(new_value);
            }
        }

    }
    
}

// Tuple basic properties
impl Tuple {

    pub fn get_length(&self) -> usize {

        let _guard = self.rw_lock.lock_read();

        self.values.len()

    }

    pub fn get_element(&self, index: usize) -> Value {

        let _guard = self.rw_lock.lock_read();

        if index >= self.values.len() {
            return Value::make_undefined();
        }

        self.values[index].get()

    }

}

impl Tuple {

    pub fn get_value_list(&self) -> Vec<Value> {

        let _guard = self.rw_lock.lock_read();

        let mut result = Vec::with_capacity(self.values.len());
        for value in self.values.iter() {
            result.push(value.get());
        }

        result

    }

}

#[test]
fn test_creation() {

    let _tuple = Tuple::new(Value::make_null(), 0, [Value::make_cardinal(23), Value::make_cardinal(34)].to_vec());

}

#[test]
fn test_tuple_references() {

    let tuple = Tuple::new(Value::make_null(), 0, [Value::make_cardinal(23), Value::make_cardinal(34)].to_vec());

    assert_eq!(tuple.list_referenced_values().len(), 2);
    assert_eq!(tuple.list_referenced_values()[0], Value::make_cardinal(23));
    assert_eq!(tuple.list_referenced_values()[1], Value::make_cardinal(34));

}

#[test]
fn test_refresh_reference() {

    let tuple = Tuple::new(Value::make_null(), 0, [Value::make_cardinal(23), Value::make_cardinal(34)].to_vec());

    tuple.refresh_referenced_value(Value::make_cardinal(34), Value::make_float(3.14));

    assert_eq!(tuple.get_length(), 2);
    assert_eq!(tuple.get_element(0), Value::make_cardinal(23));
    assert_eq!(tuple.get_element(1), Value::make_float(3.14));

}

#[test]
fn test_length() {

    let tuple = Tuple::new(Value::make_null(), 0, [Value::make_cardinal(23), Value::make_cardinal(34)].to_vec());

    assert_eq!(tuple.get_length(), 2);

}

#[test]
fn test_get_element() {

    let tuple = Tuple::new(Value::make_null(), 0, [Value::make_cardinal(23), Value::make_cardinal(34)].to_vec());

    assert_eq!(tuple.get_element(0), Value::make_cardinal(23));
    assert_eq!(tuple.get_element(1), Value::make_cardinal(34));
    assert_eq!(tuple.get_element(2), Value::make_undefined());

}
