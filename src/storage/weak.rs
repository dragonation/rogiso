use std::sync::Arc;

use super::super::base::Error;
use super::super::base::ErrorType::*;
use super::super::context::Context;
use super::super::isolate::Isolate;
use super::super::storage::Local;
use super::super::root::WeakRoot;
use super::super::root::DropListener;

pub struct Weak {
    isolate: Arc<Isolate>,
    root: Arc<WeakRoot>
}

impl Weak {

    /// Create weak from a local object
    pub fn from_local<'a>(local: &Local<'a>, drop_listener: Option<Box<dyn DropListener>>) -> Result<Weak, Error> {

        let isolate = local.get_isolate().clone();
        let root = isolate.add_weak_root(local.get_value(), drop_listener, local.get_slot_layout_token())?;

        Ok(Weak {
            isolate: isolate,
            root: root
        })

    }

    /// Create local object
    pub fn to_local<'a>(&self, context: &'a Box<dyn Context>) -> Result<Option<Local<'a>>, Error> {

        if self.root.is_dropped() {
            return Ok(None);
        }

        if !Arc::ptr_eq(context.get_isolate(), &self.isolate) {
            return Err(Error::new(FatalError, "Invalid context with different isolate"));
        }

        match self.root.get_value() {
            None => Ok(None),
            Some(value) => Ok(Some(Local::new(context, value)?))
        }

    }

}

impl Drop for Weak {
    fn drop(&mut self) {
        match self.isolate.remove_weak_root(&self.root) {
            Err(_) => {
                panic!("Failed to drop weak root");
            },
            _ => {}
        }
    }
}