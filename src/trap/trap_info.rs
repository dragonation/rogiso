use super::super::base::Value;

/// Information of trap bridge calling 
pub trait TrapInfo {

    /// Get subject of the trap
    fn get_subject(&self) -> Value;

    /// Get parameters count 
    fn get_parameters_count(&self) -> usize;

    /// Get parameter at specified index
    fn get_parameter(&self, index: usize) -> Value;

}
