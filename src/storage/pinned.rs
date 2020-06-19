use std::fmt;

use std::ops::Deref;
use std::ptr::NonNull;
use std::sync::Arc;

use super::super::base::Error;
use super::super::base::Symbol;
use super::super::base::Value;
use super::super::context::Context;
use super::super::isolate::Isolate;
use super::super::root::Root;

pub struct Slotted {
    isolate: NonNull<Isolate>,
    root: Arc<Root>,
    origin_value: Value
}

pub struct Pinned {
    slotted: Option<Slotted>,
    value: Value
}

impl Pinned {

    /// Create a new pinned value 
    pub fn new(context: &Box<dyn Context>, value: Value) -> Result<Pinned, Error> {

        if value.is_slotted() {
            Ok(Pinned {
                slotted: Some(Slotted {
                    isolate: NonNull::from(context.get_isolate().as_ref()),
                    root: context.add_root(value)?,
                    origin_value: value,
                }),
                value: value
            })
        } else {
            Ok(Pinned {
                slotted: None,
                value: value
            })
        }
               
    }

    pub fn for_symbol(context: &Box<dyn Context>, symbol: Symbol) -> Result<Pinned, Error> {

        Pinned::new(context, Value::make_symbol(symbol))

    }

    pub fn get_value(&self) -> Value {
        match &self.slotted {
            Some(slotted) => slotted.root.get_value(),
            None => self.value
        }
    }

    pub fn get_origin_value(&self) -> Value {
        match &self.slotted {
            Some(slotted) => slotted.origin_value,
            None => self.value
        }
    }

}

impl Drop for Pinned {
    fn drop(&mut self) {
        if let Some(slotted) = &self.slotted {
            let isolate = unsafe { slotted.isolate.as_ref() };
            if isolate.remove_root(&slotted.root).is_err() {
                panic!("Failed to drop root");
            }
        }
    }
}

impl fmt::Debug for Pinned {

    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.get_value().fmt(formatter)
    }

}

impl Eq for Pinned { }

impl PartialEq for Pinned {

    fn eq(&self, other: &Self) -> bool {
        self.get_value() == other.get_value()
    }

}

impl Deref for Pinned {
    type Target = Value;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
