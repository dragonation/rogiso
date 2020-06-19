use std::cell::Cell;
use std::sync::Arc;

use super::super::base::Value;
use super::super::root::DropListener;

pub struct TestDropListener {
    value: Arc<Cell<Value>>
}

impl TestDropListener {

    pub fn new(value: Arc<Cell<Value>>) -> TestDropListener {
        TestDropListener {
            value: value
        }
    }

}

impl DropListener for TestDropListener {

    fn notify_drop(&self) {
        self.value.as_ref().set(Value::make_null());
    }

}