use super::super::storage::Pinned;

/// Type of errors
#[derive(Debug)]
pub enum ErrorType {

    /// Fatal error, which means API call logic error
    FatalError,

    /// All slots in the isolate is occupied, no more slots is available
    OutOfSpace,

    /// Visiting the prototype of some undefined values
    VisitingUndefinedPrototype,

    /// Visiting properties of some undefined values
    VisitingUndefinedProperty,

    /// Visiting the prototype of some null values
    VisitingNullPrototype,

    /// Visiting properties of some null values
    VisitingNullProperty,

    /// Mutating the prototype of some undefined values
    MutatingUndefinedPrototype,

    /// Mutating properties of some undefined values
    MutatingUndefinedProperty,

    /// Mutating the prototype of some null values
    MutatingNullPrototype,

    /// Mutating properties of some null values
    MutatingNullProperty,

    /// Mutating prototype of some sealed values
    MutatingSealedPrototype,

    /// Mutating properties of some sealed values
    MutatingSealedProperty,

    /// Mutating read-only properties of some values
    MutatingReadOnlyProperty,

    /// Prototype of some values not found
    PrototypeNotFound,

    /// Property of some values not found
    PropertyNotFound,

    /// The type of value does not match
    TypeNotMatch,

    /// The integer value extracted is out of range
    IntegerOutOfRange,

    /// Internal slot not found
    InternalSlotNotFound,

    /// Slot moved
    SlotMoved,

    /// Rogic runtime error
    RogicRuntimeError,
    
    /// Rogic script error
    RogicError(Pinned)

}

/// Error record with type and message
#[derive(Debug)]
pub struct Error {
    error_type: ErrorType,
    message: String
}

impl Error {
    /// Create error with error type and message
    pub fn new(error_type: ErrorType, message: &str) -> Error {
        Error {
            error_type: error_type,
            message: message.to_owned()
        }
    }
}
