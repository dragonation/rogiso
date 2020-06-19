use std::fmt;
use std::hash::Hash; 
use std::hash::Hasher;

use super::error::Error;
use super::error::ErrorType::*;

use super::primitive_type::PrimitiveType;
use super::primitive_type::PrimitiveType::*;

use super::symbol::Symbol;

const NAN_PREFIX: u64 = 0x7ff8;
const NIL_OR_BOOLEAN_PREFIX: u64 = NAN_PREFIX | 0b001;
const INTEGER_PREFIX: u64 = NAN_PREFIX | 0b010;
const TEXT_PREFIX: u64 = NAN_PREFIX | 0b011;
const SYMBOL_PREFIX: u64 = NAN_PREFIX | 0b100;
const TUPLE_PREFIX: u64 = NAN_PREFIX | 0b101;
const LIST_PREFIX: u64 = NAN_PREFIX | 0b110;
const OBJECT_PREFIX: u64 = NAN_PREFIX | 0b111;

const UNDEFINED_SUFFIX: u64 = 0x0;
const NULL_SUFFIX: u64 = 0x1;
const NO_SUFFIX: u64 = 0x2;
const YES_SUFFIX: u64 = 0x3;

/// A 64-bit data representing all kinds of values in rogic memory
/// 
/// According to the IEEE-754 specification, there are lots of `NaN`s in the 
/// 64-bit float ranges. We just use a special predefined `NaN` as the real 
/// `NaN` in the value system, which make us to be able to regard the rest 
/// `NaN`s as various values with different types within just a 64-bit data.
#[derive(Copy, Clone)]
pub struct Value {
    data: f64
}

impl fmt::Debug for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.get_primitive_type() {
            Undefined => formatter.debug_tuple("Value:undefined").finish(),
            Null => formatter.debug_tuple("Value:null").finish(),
            Boolean => formatter.debug_tuple("Value:boolean").field(&self.as_boolean()).finish(),
            Integer => {
                if self.is_cardinal() {
                    formatter.debug_tuple("Value:cardinal").field(&self.extract_cardinal(0)).finish()
                } else {
                    formatter.debug_tuple("Value:integer").field(&self.extract_integer(0)).finish()
                }
            },
            Float => formatter.debug_tuple("Value:float").field(&self.extract_float(0.0)).finish(),
            Text => formatter.debug_tuple("Value:text").finish(),
            Symbol => formatter.debug_tuple("Value::symbol").field(&self.extract_symbol(Symbol::new(0)).get_id()).finish(),
            List => {
                let region = self.get_region_id();
                let slot = self.get_region_slot();
                if region.is_ok() && slot.is_ok() {
                    formatter.debug_tuple("Value::list").field(&region.unwrap()).field(&slot.unwrap()).finish()
                } else {
                    formatter.debug_tuple("Value::list").finish()
                }
            },
            Tuple => {
                let region = self.get_region_id();
                let slot = self.get_region_slot();
                if region.is_ok() && slot.is_ok() {
                    formatter.debug_tuple("Value::tuple").field(&region.unwrap()).field(&slot.unwrap()).finish()
                } else {
                    formatter.debug_tuple("Value:tuple").finish()
                }
            },
            Object => {
                let region = self.get_region_id();
                let slot = self.get_region_slot();
                if region.is_ok() && slot.is_ok() {
                    formatter.debug_tuple("Value::object").field(&region.unwrap()).field(&slot.unwrap()).finish()
                } else {
                    formatter.debug_tuple("Value:object").finish()
                }
            }
        }
    }
}

impl PartialEq for Value {

    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let data = unsafe { std::mem::transmute::<f64, u64>(self.data) };
        let other_data = unsafe { std::mem::transmute::<f64, u64>(other.data) };
        data == other_data 
    }

}

impl Eq for Value {}

impl Hash for Value {

    fn hash<H: Hasher>(&self, state: &mut H) {
        let data = unsafe { std::mem::transmute::<f64, u64>(self.data) };
        data.hash(state);
    }
    
}

/// Basic data
impl Value {

    /// Get the 64-bit data of the value
    #[inline]
    fn get_data(&self) -> u64 {
        unsafe {
            std::mem::transmute::<f64, u64>(self.data)
        }
    }

    /// Get the primitive type of the value
    #[inline]
    pub fn get_primitive_type(&self) -> PrimitiveType {

        if !self.data.is_nan() {
            return Float;
        }

        let data = self.get_data();
        match data >> 48 {
            NAN_PREFIX => Float,
            NIL_OR_BOOLEAN_PREFIX => {
                match data & 0xff {
                    UNDEFINED_SUFFIX => Undefined,
                    NULL_SUFFIX => Null,
                    NO_SUFFIX => Boolean,
                    YES_SUFFIX => Boolean,
                    _ => Undefined
                }
            },
            INTEGER_PREFIX => Integer,
            TEXT_PREFIX => Text,
            SYMBOL_PREFIX => Symbol,
            LIST_PREFIX => List,
            TUPLE_PREFIX => Tuple,
            OBJECT_PREFIX => Object,
            _ => Float
        }

    }

}

/// Number equal
impl Value {
    /// Check whether two number values are equal
    #[inline]
    pub fn number_eq(&self, other: &Self) -> bool {
        if self.is_number() && other.is_number() {
            self.extract_float(0.0) == other.extract_float(0.0)
        } else {
            false
        }
    }
}

/// Value makers
impl Value {

    /// Make a null value
    #[inline]
    pub fn make_null() -> Value {
        Value { 
            data: unsafe { std::mem::transmute::<u64, f64>(NIL_OR_BOOLEAN_PREFIX << 48 | NULL_SUFFIX) }
        }
    }

    /// Make an undefined value
    #[inline]
    pub fn make_undefined() -> Value {
        Value { 
            data: unsafe { std::mem::transmute::<u64, f64>(NIL_OR_BOOLEAN_PREFIX << 48 | UNDEFINED_SUFFIX) }
        }
    }

    /// Make a boolean value
    #[inline]
    pub fn make_boolean(value: bool) -> Value {
        if value {
            Value { 
                data: unsafe { std::mem::transmute::<u64, f64>(NIL_OR_BOOLEAN_PREFIX << 48 | YES_SUFFIX) }
            }
        } else {
            Value { 
                data: unsafe { std::mem::transmute::<u64, f64>(NIL_OR_BOOLEAN_PREFIX << 48 | NO_SUFFIX) }
            }
        }
    }

    /// Make a symbol value
    pub fn make_symbol(symbol: super::symbol::Symbol) -> Value {
        Value { 
            data: unsafe { std::mem::transmute::<u64, f64>(SYMBOL_PREFIX << 48 | (symbol.get_id() as u64)) }
        }
    }

    /// Make a 32-bit cardinal value
    #[inline]
    pub fn make_cardinal(value: u32) -> Value {
        Value { 
            data: unsafe { std::mem::transmute::<u64, f64>((INTEGER_PREFIX << 48) | (value as u64)) }
        }
    }

    /// Make an 32-bit integer value
    #[inline]
    pub fn make_integer(value: i32) -> Value {

        let mut uvalue = unsafe { 
            std::mem::transmute::<i32, u32>(value) 
        } as u64;
        if value < 0 {
            uvalue |= 1 << 32;
        }

        Value { 
            data: unsafe { std::mem::transmute::<u64, f64>((INTEGER_PREFIX << 48) | uvalue) }
        }

    }

    /// Make a 64-bit float value
    #[inline]
    pub fn make_float(value: f64) -> Value {
        if value.is_nan() {
            Value { 
                data: unsafe { std::mem::transmute::<u64, f64>(NAN_PREFIX << 48) }
            }
        } else {
            Value { data: value }
        }
    }

    /// Make a list value
    #[inline]
    pub fn make_list(region: u32, slot: u32) -> Value {

        let mut data = LIST_PREFIX << 48;
        data |= (region as u64) << 16;
        data |= slot as u64;

        Value { 
            data: unsafe { std::mem::transmute::<u64, f64>(data) }
        }

    }

    /// Make a tuple value
    #[inline]
    pub fn make_tuple(region: u32, slot: u32) -> Value {

        let mut data = TUPLE_PREFIX << 48;
        data |= (region as u64) << 16;
        data |= slot as u64;

        Value { 
            data: unsafe { std::mem::transmute::<u64, f64>(data) }
        }

    }

    /// Make a text value
    #[inline]
    pub fn make_text(region: u32, slot: u32) -> Value {

        let mut data = TEXT_PREFIX << 48;
        data |= (region as u64) << 16;
        data |= slot as u64;

        Value { 
            data: unsafe { std::mem::transmute::<u64, f64>(data) }
        }

    }

    /// Make a object value
    #[inline]
    pub fn make_object(region: u32, slot: u32) -> Value {

        let mut data = OBJECT_PREFIX << 48;
        data |= (region as u64) << 16;
        data |= slot as u64;

        Value { 
            data: unsafe { std::mem::transmute::<u64, f64>(data) }
        }

    }

}

/// Type checks
impl Value {

    /// Check whether a value is null
    #[inline]
    pub fn is_null(&self) -> bool {
        match self.get_primitive_type() {
            Null => true,
            _ => false
        }
    }

    /// Check whether a value is undefined
    #[inline]
    pub fn is_undefined(&self) -> bool {
        match self.get_primitive_type() {
            Undefined => true,
            _ => false
        }
    }

    /// Check whether a value is undefined or null
    #[inline]
    pub fn is_nil(&self) -> bool {
        match self.get_primitive_type() {
            Undefined => true,
            Null => true,
            _ => false
        }
    }

    /// Check whether a value is a boolean
    #[inline]
    pub fn is_boolean(&self) -> bool {
        match self.get_primitive_type() {
            Boolean => true,
            _ => false
        }
    }

    /// Check whether a value is a float
    #[inline]
    pub fn is_float(&self) -> bool {
        match self.get_primitive_type() {
            Float => true,
            _ => false
        }
    }

    /// Check whether a value is a symbol
    #[inline]
    pub fn is_symbol(&self) -> bool {
        match self.get_primitive_type() {
            Symbol => true,
            _ => false
        }
    }

    /// Check whether a value is an list
    #[inline]
    pub fn is_list(&self) -> bool {
        match self.get_primitive_type() {
            Boolean => true,
            List => true,
            _ => false
        }
    }

    /// Check whether a value is a NaN
    #[inline]
    pub fn is_nan(&self) -> bool {
        match self.get_primitive_type() {
            Integer => false,
            Float => self.data.is_nan(),
            _ => true 
        }
    }

    /// Check whether a value is a infinity float
    #[inline]
    pub fn is_infinite(&self) -> bool {
        match self.get_primitive_type() {
            Float => self.data.is_infinite(),
            _ => false
        }
    }

    /// Check whether a value is a finite number
    #[inline]
    pub fn is_finite(&self) -> bool {
        match self.get_primitive_type() {
            Integer => true,
            Float => self.data.is_finite(),
            _ => false
        }
    }

    /// Check whether a value is an integer 
    #[inline]
    pub fn is_integer(&self) -> bool {
        match self.get_primitive_type() {
            Integer => true,
            _ => false
        }
    }

    /// Check whether a value is a number
    #[inline]
    pub fn is_number(&self) -> bool {
        match self.get_primitive_type() {
            Integer => true,
            Float => true,
            _ => false
        }
    }

    /// Check whether a value is a positive number or zero
    #[inline]
    pub fn is_sign_negative(&self) -> bool {
        match self.get_primitive_type() {
            Integer => {
                let data = self.get_data();
                ((data >> 32) & 0b1 == 1) && ((data >> 31) & 0b1 == 1)
            },
            Float => (!self.data.is_nan()) && self.data.is_sign_negative(),
            _ => false
        }
    }

    /// Check whether a value is a negative number
    #[inline]
    pub fn is_sign_positive(&self) -> bool {
        match self.get_primitive_type() {
            Integer => {
                let data = self.get_data();
                ((data >> 32) & 0b1 == 0) || ((data >> 31) & 0b1 == 0)
            },
            Float => (!self.data.is_nan()) && self.data.is_sign_positive(),
            _ => false
        }
    }

    /// Check whether a value is a cardinal
    #[inline]
    pub fn is_cardinal(&self) -> bool {
        match self.get_primitive_type() {
            Integer => {
                let data = self.get_data();
                ((data >> 32) & 0b1 == 0) || ((data >> 31) & 0b1 == 0)
            },
            _ => false
        }
    }

    /// Check whether a value is a text
    #[inline]
    pub fn is_text(&self) -> bool {
        match self.get_primitive_type() {
            Text => true,
            _ => false
        }
    }

    /// Check whether a value is a tuple
    #[inline]
    pub fn is_tuple(&self) -> bool {
        match self.get_primitive_type() {
            Tuple => true,
            _ => false
        }
    }

    /// Check whether a value is an object
    #[inline]
    pub fn is_object(&self) -> bool {
        match self.get_primitive_type() {
            Object => true,
            _ => false
        }
    }

    /// Check whether a value is a slot
    #[inline]
    pub fn is_slotted(&self) -> bool {
        match self.get_primitive_type() {
            Text => true,
            List => true,
            Tuple => true,
            Object => true,
            _ => false
        }
    }

}

/// Extract primitive values
impl Value {

    /// Cast the value into a boolean
    /// 
    /// * `null` is regarded as `false`
    /// * `undefined` is regarded as `false`
    /// * `false` is regarded as `false`
    /// * `0` is regarded as `false`
    /// * `0.0` is regarded as `false`
    /// * `""` is regarded as `false`
    /// * Otherwise is regarded as `true`
    #[inline]
    pub fn as_boolean(&self) -> bool {
        match self.get_primitive_type() {
            Null => false,
            Undefined => false,
            Boolean => self.get_data() & 0xff == YES_SUFFIX,
            Integer => self.get_data() & 0xffff_ffff_ffff != 0,
            Float => self.data != 0.0,
            Symbol => true,
            List => true,
            Text => self.get_data() & 0xffff_ffff_ffff != 0,
            Tuple => true,
            Object => true
        }
    }

    /// Extract 32-bit integer from the value
    ///
    /// * `null` will output `default`
    /// * `undefined` will output `default`
    /// * `false` will output `0`
    /// * `true` will output `1`
    /// * Integer in range[`-0x80000000`, `0x7fffffff`] will output itself
    /// * Integer out of range[`-0x80000000`, `0x7fffffff`] will output `default`
    /// * Float in range[`-0x80000000`, `0x7fffffff`] will output truncated integer
    /// * Float out of range[`-0x80000000`, `0x7fffffff`] will output `default`
    /// * Otherwise will output `default`
    #[inline]
    pub fn extract_integer(&self, default: i32) -> i32 {
        match self.get_primitive_type() {
            Null => default,
            Undefined => default,
            Boolean => match self.get_data() & 0xff == YES_SUFFIX {
                false => 0,
                _ => 1
            },
            Integer => {
                let data = self.get_data();
                if ((data >> 32) & 0b1 == 1) || ((data >> 31) & 0b1 == 0) {
                    unsafe {
                        std::mem::transmute::<u32, i32>((data & 0xffff_ffff) as u32)
                    }
                } else {
                    default
                }
            },
            Float => {
                if self.data.is_nan() || self.data.is_infinite() || 
                   (self.data > (0x7fff_ffff as f64)) || 
                   (self.data < (-0x8000_0000 as f64)) {
                    default
                } else {
                    self.data as i32
                }
            },
            Symbol => default,
            List => default,
            Text => default,
            Tuple => default,
            Object => default
        }
    }

    /// Extract 32-bit cardinal from the value
    ///
    /// * `null` will output `default`
    /// * `undefined` will output `default`
    /// * `false` will output `0`
    /// * `true` will output `1`
    /// * Positive integer will output itself
    /// * Negative integer will output `default`
    /// * Float in range[`0`, `0xffffffff`] will output truncated integer
    /// * Float out of range[`0`, `0xffffffff`] will output `default`
    /// * Otherwise will output `default`
    #[inline]
    pub fn extract_cardinal(&self, default: u32) -> u32 {
        match self.get_primitive_type() {
            Null => default,
            Undefined => default,
            Boolean => match self.get_data() & 0xff == YES_SUFFIX {
                false => 0,
                _ => 1
            },
            Integer => {
                let data = self.get_data();
                if ((data >> 32) & 0b1 == 1) && ((data >> 31) & 0b1 == 1) {
                    default
                } else {
                    (data & 0xffff_ffff) as u32
                }
            },
            Float => {
                if self.data.is_nan() || self.data.is_infinite() || 
                   (self.data > ((0xffff_ffff as u32) as f64)) || 
                   (self.data < 0.0) {
                    default
                } else {
                    self.data as u32
                }
            },
            Symbol => default,
            List => default,
            Text => default,
            Tuple => default,
            Object => default
        }
    }

    /// Extract 64-bit float from the value
    ///
    /// * `null` will output `default`
    /// * `undefined` will output `default`
    /// * `false` will output `0.0`
    /// * `true` will output `1.0`
    /// * Integer will output itself as a 64-bit float
    /// * Float will output itself
    /// * Otherwise will output `default`
    #[inline]
    pub fn extract_float(&self, default: f64) -> f64 {
        match self.get_primitive_type() {
            Null => default,
            Undefined => default,
            Boolean => match self.get_data() & 0xff == YES_SUFFIX {
                false => 0.0,
                _ => 1.0
            },
            Integer => {
                let data = self.get_data();
                if ((data >> 32) & 0b1 == 1) && ((data >> 31) & 0b1 == 1) {
                    unsafe {
                        std::mem::transmute::<u32, i32>((data & 0xffff_ffff) as u32) as f64
                    }
                } else {
                    (data & 0xffff_ffff) as f64
                }
            },
            Float => self.data,
            Symbol => default,
            List => default,
            Text => default,
            Tuple => default,
            Object => default
        }
    }

    #[inline]
    pub fn extract_symbol(&self, default: super::symbol::Symbol) -> super::symbol::Symbol {
        match self.get_primitive_type() {
            Null => default,
            Undefined => default,
            Boolean => default,
            Integer => default,
            Float => default,
            Symbol => {
                let data = self.get_data();
                super::symbol::Symbol::new((data & 0xffff_ffff) as u32)
            },
            List => default,
            Text => default,
            Tuple => default,
            Object => default
        }
    }

}

/// Get specified type data
impl Value {

    /// Get boolean data from the value if type is boolean
    #[inline]
    pub fn get_boolean_data(&self) -> Result<bool, Error> {
        match self.get_primitive_type() {
            Boolean => Ok(self.get_data() & 0xff == YES_SUFFIX),
            _ => Err(Error::new(TypeNotMatch, "Not boolean value"))
        }
    }

    /// Get 32-bit integer data from the value if type is integer
    ///
    /// Cardinal may output `Error` for overflow
    #[inline]
    pub fn get_integer_data(&self) -> Result<i32, Error> {
        match self.get_primitive_type() {
            Integer => {
                let data = self.get_data();
                let value = unsafe {
                    std::mem::transmute::<u32, i32>((data & 0xffff_ffff) as u32) 
                };
                if ((data >> 32) & 0b1 == 1) || (value >= 0) {
                    Ok(value)
                } else {
                    Err(Error::new(IntegerOutOfRange, "Integer out of range"))
                }
            },
            _ => Err(Error::new(TypeNotMatch, "Not integer value"))
        }
    }

    /// Get 32-bit cardinal data from the value if type is integer
    ///
    /// Integer may output `Error` for overflow
    #[inline]
    pub fn get_cardinal_data(&self) -> Result<u32, Error> {
        match self.get_primitive_type() {
            Integer => {
                let data = self.get_data();
                if (data >> 32) & 0b1 == 0 {
                    Ok((data & 0xffff_ffff) as u32)
                } else {
                    Err(Error::new(IntegerOutOfRange, "Cardinal out of range"))
                }
            },
            _ => Err(Error::new(TypeNotMatch, "Not cardinal value"))
        }
    }

    /// Get 64-bit float data from the value if type is float
    #[inline]
    pub fn get_float_data(&self) -> Result<f64, Error> {
        match self.get_primitive_type() {
            Float => Ok(self.data),
            _ => Err(Error::new(TypeNotMatch, "Not float value"))
        }
    }

    /// Get region ID from the value if type is slot
    #[inline]
    pub fn get_region_id(&self) -> Result<u32, Error> {
        if self.is_slotted() {
            let data = self.get_data();
            Ok(((data >> 16) & 0xffff_ffff) as u32)
        } else {
            Err(Error::new(TypeNotMatch, "No slotted value"))
        }
    }

    /// Get index of slot within target region from the value if type is slot
    #[inline]
    pub fn get_region_slot(&self) -> Result<u32, Error> {
        if self.is_slotted() {
            let data = self.get_data();
            Ok((data & 0xffff) as u32)
        } else {
            Err(Error::new(TypeNotMatch, "No slotted value"))
        }
    }

}

#[test]
fn test_null() {
    let value = Value::make_null();
    assert!(value.is_null());
    assert!(!value.is_undefined());
    assert!(value.is_nil());
    assert!(!value.is_boolean());
    assert!(!value.is_integer());
    assert!(!value.is_float());
    assert!(!value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_symbol());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(!value.as_boolean());
    assert!(value.is_nan());
    assert!(!value.is_finite());
    assert!(!value.is_infinite());
    assert!(!value.is_sign_positive());
    assert!(!value.is_sign_negative());
    assert_eq!(value.extract_integer(0), 0);
    assert_eq!(value.extract_integer(10), 10);
    assert_eq!(value.extract_cardinal(0), 0);
    assert_eq!(value.extract_cardinal(10), 10);
    assert_eq!(value.extract_float(0.0), 0.0);
    assert_eq!(value.extract_float(10.0), 10.0);
}

#[test]
fn test_undefined() {
    let value = Value::make_undefined();
    assert!(!value.is_null());
    assert!(value.is_undefined());
    assert!(value.is_nil());
    assert!(!value.is_boolean());
    assert!(!value.is_integer());
    assert!(!value.is_float());
    assert!(!value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(!value.as_boolean());
    assert!(value.is_nan());
    assert!(!value.is_finite());
    assert!(!value.is_infinite());
    assert!(!value.is_sign_positive());
    assert!(!value.is_sign_negative());
    assert_eq!(value.extract_integer(0), 0);
    assert_eq!(value.extract_integer(10), 10);
    assert_eq!(value.extract_cardinal(0), 0);
    assert_eq!(value.extract_cardinal(10), 10);
    assert_eq!(value.extract_float(0.0), 0.0);
    assert_eq!(value.extract_float(10.0), 10.0);
}

#[test] 
fn test_boolean_true() {
    let value = Value::make_boolean(true);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(value.is_boolean());
    assert!(!value.is_integer());
    assert!(!value.is_float());
    assert!(!value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(value.is_list());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(value.is_nan());
    assert!(!value.is_finite());
    assert!(!value.is_infinite());
    assert!(!value.is_sign_positive());
    assert!(!value.is_sign_negative());
    assert!(value.as_boolean());
    assert_eq!(value.extract_integer(0), 1);
    assert_eq!(value.extract_cardinal(0), 1);
    assert_eq!(value.extract_float(0.0), 1.0);
    match value.get_boolean_data() {
        Ok(value) => assert!(value),
        _ => panic!("Failed to get boolean data")
    }
}

#[test] 
fn test_boolean_false() {
    let value = Value::make_boolean(false);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(value.is_boolean());
    assert!(!value.is_integer());
    assert!(!value.is_float());
    assert!(!value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(value.is_list());
    assert!(value.is_nan());
    assert!(!value.is_finite());
    assert!(!value.is_infinite());
    assert!(!value.is_sign_positive());
    assert!(!value.is_sign_negative());
    assert!(!value.as_boolean());
    assert_eq!(value.extract_integer(1), 0);
    assert_eq!(value.extract_cardinal(1), 0);
    assert_eq!(value.extract_float(1.0), 0.0);
    match value.get_boolean_data() {
        Ok(value) => assert!(!value),
        _ => panic!("Failed to get boolean data")
    }
}

#[test]
fn test_cardinal() {
    let value = Value::make_cardinal(0xffff_ffff);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(!value.is_boolean());
    assert!(value.is_integer());
    assert!(value.is_cardinal());
    assert!(!value.is_float());
    assert!(value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(!value.is_nan());
    assert!(value.is_finite());
    assert!(!value.is_infinite());
    assert!(value.is_sign_positive());
    assert!(!value.is_sign_negative());
    assert!(value.as_boolean());
    assert_eq!(value.extract_integer(1), 1);
    assert_eq!(value.extract_cardinal(1), 0xffff_ffff);
    assert_eq!(value.extract_float(1.0), (0xffff_ffff as u32) as f64);
    match value.get_cardinal_data() {
        Ok(value) => assert_eq!(value, 0xffff_ffff),
        _ => panic!("Failed to get cardinal data")
    }
    if let Ok(_) = value.get_integer_data() {
        panic!("Cardinal value should be overflow");
    }
}

#[test]
fn test_cardinal_zero() {
    let value = Value::make_cardinal(0);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(!value.is_boolean());
    assert!(value.is_integer());
    assert!(value.is_cardinal());
    assert!(!value.is_float());
    assert!(value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(!value.is_nan());
    assert!(value.is_finite());
    assert!(!value.is_infinite());
    assert!(value.is_sign_positive());
    assert!(!value.is_sign_negative());
    assert!(!value.as_boolean());
    assert_eq!(value.extract_integer(1), 0);
    assert_eq!(value.extract_cardinal(1), 0);
    assert_eq!(value.extract_float(1.0), 0.0);
    match value.get_cardinal_data() {
        Ok(value) => assert_eq!(value, 0),
        _ => panic!("Failed to get cardinal data")
    }
    match value.get_integer_data() {
        Ok(value) => assert_eq!(value, 0),
        _ => panic!("Failed to get integer data")
    }
}

#[test]
fn test_integer() {
    let value = Value::make_integer(0x7fff_ffff);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(!value.is_boolean());
    assert!(value.is_integer());
    assert!(value.is_cardinal());
    assert!(!value.is_float());
    assert!(value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(!value.is_nan());
    assert!(value.is_finite());
    assert!(!value.is_infinite());
    assert!(value.is_sign_positive());
    assert!(!value.is_sign_negative());
    assert!(value.as_boolean());
    assert_eq!(value.extract_integer(1), 0x7fff_ffff);
    assert_eq!(value.extract_cardinal(1), 0x7fff_ffff);
    assert_eq!(value.extract_float(1.0), 0x7fff_ffff as f64);
    match value.get_cardinal_data() {
        Ok(value) => assert_eq!(value, 0x7fff_ffff),
        _ => panic!("Failed to get cardinal data")
    }
    match value.get_integer_data() {
        Ok(value) => assert_eq!(value, 0x7fff_ffff),
        _ => panic!("Failed to get integer data")
    }
}

#[test]
fn test_integer_zero() {
    let value = Value::make_integer(0);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(!value.is_boolean());
    assert!(value.is_integer());
    assert!(value.is_cardinal());
    assert!(!value.is_float());
    assert!(value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(!value.is_nan());
    assert!(value.is_finite());
    assert!(!value.is_infinite());
    assert!(value.is_sign_positive());
    assert!(!value.is_sign_negative());
    assert!(!value.as_boolean());
    assert_eq!(value.extract_integer(1), 0);
    assert_eq!(value.extract_cardinal(1), 0);
    assert_eq!(value.extract_float(1.0), 0.0);
    match value.get_cardinal_data() {
        Ok(value) => assert_eq!(value, 0),
        _ => panic!("Failed to get cardinal data")
    }
    match value.get_integer_data() {
        Ok(value) => assert_eq!(value, 0),
        _ => panic!("Failed to get integer data")
    }
}

#[test]
fn test_integer_negative() {
    let value = Value::make_integer(-1);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(!value.is_boolean());
    assert!(value.is_integer());
    assert!(!value.is_cardinal());
    assert!(!value.is_float());
    assert!(value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(!value.is_nan());
    assert!(value.is_finite());
    assert!(!value.is_infinite());
    assert!(!value.is_sign_positive());
    assert!(value.is_sign_negative());
    assert!(value.as_boolean());
    assert_eq!(value.extract_integer(1), -1);
    assert_eq!(value.extract_cardinal(1), 1);
    assert_eq!(value.extract_float(1.0), -1.0);
    if let Ok(_) = value.get_cardinal_data() {
        panic!("Cardinal value should be overflow");
    }
    match value.get_integer_data() {
        Ok(value) => assert_eq!(value, -1),
        _ => panic!("Failed to get integer data")
    }
}

#[test]
fn test_float() {
    let value = Value::make_float(1.0);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(!value.is_boolean());
    assert!(!value.is_integer());
    assert!(!value.is_cardinal());
    assert!(value.is_float());
    assert!(value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(!value.is_nan());
    assert!(value.is_finite());
    assert!(!value.is_infinite());
    assert!(value.is_sign_positive());
    assert!(!value.is_sign_negative());
    assert!(value.as_boolean());
    assert_eq!(value.extract_integer(1), 1);
    assert_eq!(value.extract_cardinal(1), 1);
    assert_eq!(value.extract_float(0.0), 1.0);
    match value.get_float_data() {
        Ok(value) => assert_eq!(value, 1.0),
        _ => panic!("Failed to get float data")
    }
}

#[test]
fn test_float_zero() {
    let value = Value::make_float(0.0);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(!value.is_boolean());
    assert!(!value.is_integer());
    assert!(!value.is_cardinal());
    assert!(value.is_float());
    assert!(value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(!value.is_nan());
    assert!(value.is_finite());
    assert!(!value.is_infinite());
    assert!(value.is_sign_positive());
    assert!(!value.is_sign_negative());
    assert!(!value.as_boolean());
    assert_eq!(value.extract_integer(1), 0);
    assert_eq!(value.extract_cardinal(1), 0);
    assert_eq!(value.extract_float(1.0), 0.0);
    match value.get_float_data() {
        Ok(value) => assert_eq!(value, 0.0),
        _ => panic!("Failed to get float data")
    }
}

#[test]
fn test_float_negative() {
    let value = Value::make_float(-1.0);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(!value.is_boolean());
    assert!(!value.is_integer());
    assert!(!value.is_cardinal());
    assert!(value.is_float());
    assert!(value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(!value.is_nan());
    assert!(value.is_finite());
    assert!(!value.is_infinite());
    assert!(!value.is_sign_positive());
    assert!(value.is_sign_negative());
    assert!(value.as_boolean());
    assert_eq!(value.extract_integer(1), -1);
    assert_eq!(value.extract_cardinal(1), 1);
    assert_eq!(value.extract_float(0.0), -1.0);
    match value.get_float_data() {
        Ok(value) => assert_eq!(value, -1.0),
        _ => panic!("Failed to get float data")
    }
}

#[test]
fn test_float_nan() {
    let value = Value::make_float(f64::NAN);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(!value.is_boolean());
    assert!(!value.is_integer());
    assert!(!value.is_cardinal());
    assert!(value.is_float());
    assert!(value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(value.is_nan());
    assert!(!value.is_finite());
    assert!(!value.is_infinite());
    assert!(!value.is_sign_positive());
    assert!(!value.is_sign_negative());
    assert!(value.as_boolean());
    assert_eq!(value.extract_integer(1), 1);
    assert_eq!(value.extract_cardinal(1), 1);
    assert!(value.extract_float(0.0).is_nan());
    match value.get_float_data() {
        Ok(value) => assert!(value.is_nan()),
        _ => panic!("Failed to get float data")
    }
}

#[test]
fn test_float_infinity() {
    let value = Value::make_float(f64::INFINITY);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(!value.is_boolean());
    assert!(!value.is_integer());
    assert!(!value.is_cardinal());
    assert!(value.is_float());
    assert!(value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(!value.is_nan());
    assert!(!value.is_finite());
    assert!(value.is_infinite());
    assert!(value.is_sign_positive());
    assert!(!value.is_sign_negative());
    assert!(value.as_boolean());
    assert_eq!(value.extract_integer(1), 1);
    assert_eq!(value.extract_cardinal(1), 1);
    assert!(value.extract_float(0.0).is_infinite());
    match value.get_float_data() {
        Ok(value) => assert!(value.is_infinite()),
        _ => panic!("Failed to get float data")
    }
}

#[test]
fn test_float_negative_infinity() {
    let value = Value::make_float(-f64::INFINITY);
    assert!(!value.is_null());
    assert!(!value.is_undefined());
    assert!(!value.is_nil());
    assert!(!value.is_boolean());
    assert!(!value.is_integer());
    assert!(!value.is_cardinal());
    assert!(value.is_float());
    assert!(value.is_number());
    assert!(!value.is_slotted());
    assert!(!value.is_tuple());
    assert!(!value.is_object());
    assert!(!value.is_text());
    assert!(!value.is_list());
    assert!(!value.is_nan());
    assert!(!value.is_finite());
    assert!(value.is_infinite());
    assert!(!value.is_sign_positive());
    assert!(value.is_sign_negative());
    assert!(value.as_boolean());
    assert_eq!(value.extract_integer(1), 1);
    assert_eq!(value.extract_cardinal(1), 1);
    assert!(value.extract_float(0.0).is_infinite());
    match value.get_float_data() {
        Ok(value) => assert!(value.is_infinite()),
        _ => panic!("Failed to get float data")
    }
}

#[test]
fn test_equal() {
    let null_value = Value::make_null();
    let undefined_value = Value::make_undefined();
    let true_value = Value::make_boolean(true);
    let false_value = Value::make_boolean(false);
    let zero_value = Value::make_integer(0);
    let zero_value_2 = Value::make_integer(0);
    let zero_value_3 = Value::make_cardinal(0);
    let zero_float_value = Value::make_float(0.0);
    let one_value = Value::make_integer(1);
    let one_value_2 = Value::make_integer(1);
    let one_value_3 = Value::make_cardinal(1);
    let negative_value = Value::make_integer(-1);
    let negative_float_value = Value::make_float(-1.0);
    let negative_float_value_2 = Value::make_float(-1.0);
    assert_ne!(null_value, undefined_value);
    assert_ne!(null_value, true_value);
    assert_ne!(null_value, false_value);
    assert_ne!(null_value, zero_value);
    assert_ne!(null_value, zero_value_2);
    assert_ne!(null_value, zero_value_3);
    assert_ne!(null_value, zero_float_value);
    assert_ne!(null_value, one_value);
    assert_ne!(null_value, one_value_2);
    assert_ne!(null_value, one_value_3);
    assert_ne!(null_value, negative_value);
    assert_ne!(null_value, negative_float_value);
    assert_ne!(true_value, false_value);
    assert_ne!(true_value, zero_value);
    assert_ne!(zero_value, one_value);
    assert_ne!(zero_value, negative_value);
    assert_ne!(zero_value, negative_float_value);
    assert_ne!(zero_value, zero_float_value);
    assert_ne!(zero_float_value, negative_float_value);
    assert_ne!(one_value, negative_value);
    assert_ne!(one_value, negative_float_value);
    assert_ne!(one_value, zero_float_value);
    assert_ne!(negative_value, negative_float_value);
    assert_eq!(zero_value, zero_value_2);
    assert_eq!(one_value, one_value_2);
    assert_eq!(negative_float_value, negative_float_value_2);
    assert!(zero_value.number_eq(&zero_float_value));
    assert!(negative_value.number_eq(&negative_float_value));
}