use super::base::Error;
use super::base::Value;

/// Barrier for the garbage collector in isolate
pub trait Barrier {

    fn preremove_value_reference(&self, value: Value) -> Result<(), Error>;

    fn postgain_value(&self, value: Value) -> Result<(), Error>;

}
