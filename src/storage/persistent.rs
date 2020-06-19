use std::sync::Arc;

use super::super::base::Error;
use super::super::base::ErrorType::*;
use super::super::context::Context;
use super::super::isolate::Isolate;
use super::super::storage::Local;
use super::super::root::Root;

/// Persistent record of object
pub struct Persistent {
    isolate: Arc<Isolate>,
    root: Arc<Root>
}

impl Persistent {

    /// Create persistent from a local object
    pub fn from_local<'a>(local: &Local<'a>) -> Result<Persistent, Error> {

        let isolate = local.get_isolate().clone();
        let root = isolate.add_root(local.get_value(), local.get_slot_layout_token())?;

        Ok(Persistent {
            isolate: isolate,
            root: root
        })

    }

    /// Create local object
    pub fn to_local<'a>(&self, context: &'a Box<dyn Context>) -> Result<Local<'a>, Error> {

        if !Arc::ptr_eq(context.get_isolate(), &self.isolate) {
            return Err(Error::new(FatalError, "Invalid context with different isolate"));
        }

        Local::new(context, self.root.get_value())

    }

}

impl Drop for Persistent {
    fn drop(&mut self) {
        match self.isolate.remove_root(&self.root) {
            Err(_) => {
                panic!("Failed to drop root");
            },
            _ => {}
        }
    }
}