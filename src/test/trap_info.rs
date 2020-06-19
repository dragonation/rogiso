use super::super::base::Value;
use super::super::trap::TrapInfo;

pub struct TestTrapInfo {
    subject: Value,
    parameters: Vec<Value>
}

impl TestTrapInfo {

    pub fn new(subject: Value, parameters: Vec<Value>) -> TestTrapInfo {
        TestTrapInfo {
            subject: subject,
            parameters: parameters
        }
    }

}

impl TrapInfo for TestTrapInfo {

    fn get_subject(&self) -> Value {
        self.subject
    }

    fn get_parameters_count(&self) -> usize {
        self.parameters.len()
    }

    fn get_parameter(&self, index: usize) -> Value {
        if index < self.parameters.len() {
            self.parameters[index]
        } else {
            Value::make_undefined()
        }
    }

}
