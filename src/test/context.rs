use std::any::Any;
use std::cell::Cell;
use std::sync::Arc;

use super::super::base::Error;
use super::super::base::ErrorType::*;
use super::super::base::PrimitiveType;
use super::super::base::PrimitiveType::*;
use super::super::base::Symbol;
use super::super::base::Value;
use super::super::context::Context;
use super::super::internal_slot::InternalSlot;
use super::super::isolate::Isolate;
use super::super::trap::PropertyTrap;
use super::super::trap::TrapInfo;
use super::super::util::ReentrantToken;

use super::trap_info::TestTrapInfo;

struct TestPropertyTrap {
    property_trap: Arc<dyn PropertyTrap>
}

impl InternalSlot for TestPropertyTrap {

    fn as_any(&self) -> &dyn Any {
        self
    }

}

impl TestPropertyTrap {

    fn new(property_trap: Arc<dyn PropertyTrap>) -> TestPropertyTrap {
        TestPropertyTrap {
            property_trap: property_trap
        }
    }

    fn get_property_trap(&self) -> Arc<dyn PropertyTrap> {
        self.property_trap.clone()
    }

}

pub struct TestContext {
    new_born_region_id: Cell<u32>,
    isolate: Arc<Isolate>,
    slot_layout_token: ReentrantToken
}

impl TestContext {

    pub fn new(isolate: Arc<Isolate>) -> TestContext {
        let layout_token = isolate.create_slot_layout_token();
        TestContext {
            new_born_region_id: Cell::new(0),
            isolate: isolate,
            slot_layout_token: layout_token
        }
    }

    fn ensure_new_born_region(&self) -> Result<(), Error> {

        let new_born_region_id = self.new_born_region_id.get();
        if (new_born_region_id != 0) && 
            self.isolate.could_region_gain_slot_quickly(new_born_region_id) {
            return Ok(());
        }

        self.new_born_region_id.set(self.isolate.create_region()?);

        Ok(())
        
    }

}

impl Context for TestContext {

    fn get_isolate<'a>(&'a self) -> &'a Arc<Isolate> {
        &self.isolate
    }

    fn get_slot_layout_token<'a>(&'a self) -> &'a ReentrantToken {
        &self.slot_layout_token
    }

    // The implementation has no considering GC
    fn add_value_reference(&self, _from: Value, _to: Value) -> Result<(), Error> {
        Ok(())
    }

    fn remove_value_reference(&self, _from: Value, _to: Value) -> Result<(), Error> {
        Ok(())
    }
    
    fn add_symbol_reference(&self, _symbol: Symbol) -> Result<(), Error> {
        Ok(())
    }

    fn remove_symbol_reference(&self, _symbol: Symbol) -> Result<(), Error> {
        Ok(())
    }

    fn gain_slot(&self, primitive_type: PrimitiveType, prototype: Value) -> Result<Value, Error> {

        self.ensure_new_born_region()?;

        self.isolate.gain_slot(self.new_born_region_id.get(), primitive_type, prototype, self.get_slot_layout_token())

    }

    fn create_trap_info(&self, subject: Value, parameters: Vec<Value>, _context: &Box<dyn Context>) -> Box<dyn TrapInfo> {
        Box::new(TestTrapInfo::new(subject, parameters))
    }

    fn make_property_trap_value(&self, property_trap: Arc<dyn PropertyTrap>, context: &Box<dyn Context>) -> Result<Value, Error> {

        let value = self.gain_slot(Object, self.isolate.get_list_prototype())?;

        let test_property_trap: Arc<dyn InternalSlot> = Arc::new(TestPropertyTrap::new(property_trap));

        self.set_internal_slot(value, 0, test_property_trap, context)?;

        Ok(value)

    }

    fn extract_property_trap(&self, value: Value, context: &Box<dyn Context>) -> Result<Arc<dyn PropertyTrap>, Error> {

        match context.get_internal_slot(value, 0, context)? {
            Some(internal_slot) => {
                match internal_slot.as_any().downcast_ref::<TestPropertyTrap>() {
                    Some(test_property_trap) => {
                        return Ok(test_property_trap.get_property_trap());
                    },
                    None => {}
                }
            },
            None => {}
        }

        Err(Error::new(FatalError, "No property trap found"))

    }

}

pub struct TestContext2 {
    new_born_region_ready: Cell<bool>,
    new_born_region_id: Cell<u32>,
    isolate: Arc<Isolate>,
    slot_layout_token: ReentrantToken
}

impl TestContext2 {

    pub fn new(isolate: Arc<Isolate>) -> TestContext2 {
        let layout_token = isolate.create_slot_layout_token();
        TestContext2 {
            new_born_region_ready: Cell::new(false),
            new_born_region_id: Cell::new(0),
            isolate: isolate,
            slot_layout_token: layout_token,
        }
    }

    fn ensure_new_born_region(&self) -> Result<(), Error> {

        let new_born_region_ready = self.new_born_region_ready.get();
        let new_born_region_id = self.new_born_region_id.get();
        if new_born_region_ready && 
            self.isolate.could_region_gain_slot_quickly(new_born_region_id) {
            return Ok(());
        }

        self.new_born_region_ready.set(true);
        self.new_born_region_id.set(self.isolate.create_region()?);

        Ok(())
        
    }

}

impl Context for TestContext2 {

    fn get_isolate<'a>(&'a self) -> &'a Arc<Isolate> {
        &self.isolate
    }

    fn get_slot_layout_token<'a>(&'a self) -> &'a ReentrantToken {
        &self.slot_layout_token
    }

    fn gain_slot(&self, primitive_type: PrimitiveType, prototype: Value) -> Result<Value, Error> {

        self.ensure_new_born_region()?;

        self.isolate.gain_slot(self.new_born_region_id.get(), primitive_type, prototype, self.get_slot_layout_token())

    }

    fn create_trap_info(&self, subject: Value, parameters: Vec<Value>, _context: &Box<dyn Context>) -> Box<dyn TrapInfo> {
        Box::new(TestTrapInfo::new(subject, parameters))
    }

    fn make_property_trap_value(&self, property_trap: Arc<dyn PropertyTrap>, context: &Box<dyn Context>) -> Result<Value, Error> {

        let value = self.gain_slot(Object, self.isolate.get_list_prototype())?;

        let test_property_trap: Arc<dyn InternalSlot> = Arc::new(TestPropertyTrap::new(property_trap));

        self.set_internal_slot(value, 0, test_property_trap, context)?;

        Ok(value)

    }

    fn extract_property_trap(&self, value: Value, context: &Box<dyn Context>) -> Result<Arc<dyn PropertyTrap>, Error> {

        match context.get_internal_slot(value, 0, context)? {
            Some(internal_slot) => {
                match internal_slot.as_any().downcast_ref::<TestPropertyTrap>() {
                    Some(test_property_trap) => {
                        return Ok(test_property_trap.get_property_trap());
                    },
                    None => {}
                }
            },
            None => {}
        }

        Err(Error::new(FatalError, "No property trap found"))

    }

}
