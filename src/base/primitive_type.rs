
/// Pritimive types supported in rogic memory
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PrimitiveType {

    /// Undefined data type, which means the data is not initialized
    Undefined,

    /// Null data type, which means the slot has no content
    Null,

    /// Boolean data type, `true` or `false`
    Boolean,

    /// 32-bit LE integer data type, range [`-0x80000000`, `0xffffffff`]
    Integer,

    /// 64-bit LE float data type, according to IEEE 754
    ///
    /// `NaN` is not only `NaN`, some of NaN values may be used as other types of value
    Float,

    /// Symbol type
    Symbol,
    
    /// Text data type, UTF-8 LE encoding
    Text,

    /// List data type
    List,

    /// Tuple data type
    Tuple,

    /// Complex object data type stored in slots of memory
    Object

}
