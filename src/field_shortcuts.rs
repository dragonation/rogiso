use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use super::base::Error;
use super::base::ErrorType::*;
use super::base::Symbol;
use super::base::Value;
use super::util::RwLock;

const MAX_SHORTCUTS_SIZE: usize = 26; // make the field shortcuts 256 byte size

pub struct FieldToken {
    rw_lock: RwLock,
    template: u32,
    version: Cell<u16>,
    index: Cell<u8>,
    symbol: Symbol
}

impl FieldToken {

    pub fn get_template(&self) -> u32 {
        self.template
    }

    pub fn get_symbol(&self) -> Symbol {
        self.symbol
    }

    pub fn get_version(&self) -> u16 {
        let _guard = self.rw_lock.lock_read();
        self.version.get()
    }

    pub fn get_index(&self) -> u8 {
        let _guard = self.rw_lock.lock_read();
        self.index.get()
    }

    pub fn get_field(&self, field_shortcuts: &Arc<FieldShortcuts>) -> Option<Value> {

        let (result, need_update) = {
            let _guard = self.rw_lock.lock_read();
            field_shortcuts.get_field(self.template, self.version.get(), self.index.get())
        };

        if need_update {
            field_shortcuts.refresh_field_token(self);
        }

        result

    }

    pub fn set_field(&self, field_shortcuts: &Arc<FieldShortcuts>, value: Value) {

        let need_update = {
            let _guard = self.rw_lock.lock_read();
            field_shortcuts.set_field(self.template, self.version.get(), self.index.get(), value)
        };

        if need_update {
            field_shortcuts.refresh_field_token(self);
        }

    }

}

pub struct FieldTemplate {
    rw_lock: RwLock,
    id: u32, 
    version: Cell<u16>,
    bitmap: Cell<u64>,
    fields: RefCell<HashMap<Symbol, u8>>
}

impl FieldTemplate {

    pub fn new(id: u32) -> FieldTemplate {
        FieldTemplate {
            rw_lock: RwLock::new(),
            id: id,
            version: Cell::new(1u16),
            bitmap: Cell::new(0u64),
            fields: RefCell::new(HashMap::new())
        }
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_version(&self) -> u16 {
        self.version.get()
    }

    pub fn get_field_token(&self, symbol: Symbol) -> Option<FieldToken> {

        let _guard = self.rw_lock.lock_read();

        match self.fields.borrow().get(&symbol) {
            None => None,
            Some(index) => Some(FieldToken {
                rw_lock: RwLock::new(),
                template: self.id,
                version: Cell::new(self.version.get()),
                index: Cell::new(*index),
                symbol: symbol
            })
        }
        
    }

    pub fn refresh_field_token(&self, field_token: &FieldToken) {

        let _guard = self.rw_lock.lock_read();

        match self.fields.borrow().get(&field_token.symbol) {
            Some(index) => {
                let _guard = field_token.rw_lock.lock_write();
                field_token.version.set(self.version.get());
                field_token.index.set(*index);
            },
            None => {}
        }

    }

    pub fn get_symbol_index(&self, symbol: Symbol) -> Option<u8> {

        let _guard = self.rw_lock.lock_read();

        match self.fields.borrow().get(&symbol) {
            None => None,
            Some(index) => Some(*index)
        }

    }

    pub fn get_symbol_count(&self) -> u8 {

        let _guard = self.rw_lock.lock_read();

        self.fields.borrow().len() as u8

    } 

    pub fn add_symbol(&self, symbol: Symbol) -> Result<u8, Error> {

        let _guard = self.rw_lock.lock_write();

        if self.fields.borrow().len() >= MAX_SHORTCUTS_SIZE {
            return Err(Error::new(FatalError, "Fields overflow"));
        }

        if self.fields.borrow().get(&symbol).is_some() {
            return Err(Error::new(FatalError, "Fields duplicated"));
        }

        let bitmap = self.bitmap.get();
        let mut index = 0;
        while ((bitmap >> index) & 0b1 == 1) && (index < 64) {
            index += 1;
        }

        if index >= 64 {
            return Err(Error::new(FatalError, "Fields overflow"));
        }

        self.bitmap.set(bitmap | (1 << index));

        self.fields.borrow_mut().insert(symbol, index);

        Ok(index)

    }

    pub fn has_symbol(&self, symbol: Symbol) -> bool {

        let _guard = self.rw_lock.lock_write();

        self.fields.borrow().get(&symbol).is_some()

    }

    pub fn remove_symbol(&self, symbol: Symbol) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let index = match self.fields.borrow().get(&symbol) {
            None => {
                return Err(Error::new(FatalError, "Fields not found"));
            },
            Some(index) => *index
        };

        self.version.set(self.version.get() + 1);
        self.bitmap.set(self.bitmap.get() & !(1u64 << index));

        self.fields.borrow_mut().remove(&symbol);

        Ok(())

    }

}

pub struct FieldShortcuts {
    rw_lock: RwLock,
    version: Cell<u16>,
    template: RefCell<Arc<FieldTemplate>>,
    bitmap: Cell<u64>,
    fields: RefCell<[Value; MAX_SHORTCUTS_SIZE]>
}

impl FieldShortcuts {

    pub fn new(template: Arc<FieldTemplate>) -> FieldShortcuts {

        FieldShortcuts {
            rw_lock: RwLock::new(),
            version: Cell::new(template.get_version()),
            template: RefCell::new(template),
            bitmap: Cell::new(0u64),
            fields: RefCell::new([Value::make_undefined(); MAX_SHORTCUTS_SIZE])
        }

    }

    pub fn reset(&self) {

        let _guard = self.rw_lock.lock_write();

        self.bitmap.set(0u64);

    }

    pub fn get_field_token(&self, symbol: Symbol) -> Option<FieldToken> {

        let _guard = self.rw_lock.lock_read();

        self.template.borrow().get_field_token(symbol)

    }

    pub fn refresh_field_token(&self, field_token: &FieldToken) {

        let _guard = self.rw_lock.lock_read();

        self.template.borrow().refresh_field_token(field_token);

    }

    pub fn get_field_template(&self) -> Arc<FieldTemplate> {

        let _guard = self.rw_lock.lock_read();

        self.template.borrow().clone()

    }

    pub fn get_field_template_id(&self) -> u32 {

        let _guard = self.rw_lock.lock_read();

        self.template.borrow().get_id()

    }

    pub fn update_field_template(&self, template: Arc<FieldTemplate>) {

        let _guard = self.rw_lock.lock_write();

        {
            let _guard_2 = template.rw_lock.lock_read();
            let version = template.version.get();
            self.version.set(version);
        }

        *self.template.borrow_mut() = template;

        self.bitmap.set(0u64);

    }

    pub fn get_field_index(&self, symbol: Symbol) -> Option<u8> {

        let _guard = self.rw_lock.lock_read();

        self.template.borrow().get_symbol_index(symbol)

    }

    pub fn get_field(&self, template: u32, version: u16, index: u8) -> (Option<Value>, bool) {

        let _guard = self.rw_lock.lock_read();

        let template_version = {
            let self_template = self.template.borrow();
            if self_template.get_id() != template {
                return (None, false);
            }
            let _guard_2 = self_template.rw_lock.lock_read();
            let template_version = self_template.version.get();
            if self.version.get() != template_version {
                self.bitmap.set(0u64);
                self.version.set(template_version);
                return (None, true);
            }
            template_version
        };

        if (template_version == version) &&
           ((self.bitmap.get() >> index) & 0b1 == 1) {
            (Some(self.fields.borrow()[index as usize]), false)
        } else {
            (None, false)
        }

    }

    pub fn set_symbol_field(&self, symbol: Symbol, value: Value) {

        let _guard = self.rw_lock.lock_write();

        let self_template = self.template.borrow();

        let _guard_2 = self_template.rw_lock.lock_read();

        let template_version = self_template.version.get();
        if self.version.get() != template_version {
            self.bitmap.set(0u64);
            self.version.set(template_version);
        }

        if let Some(index) = self_template.get_symbol_index(symbol) {
            self.bitmap.set(self.bitmap.get() | (1 << index));
            self.fields.borrow_mut()[index as usize] = value;
        }

    }

    pub fn set_field(&self, template: u32, version: u16, index: u8, value: Value) -> bool {

        let _guard = self.rw_lock.lock_write();

        let mut need_update = false;
        let template_version = {
            let self_template = self.template.borrow();
            if self_template.get_id() != template {
                return false;
            }
            let _guard_2 = self_template.rw_lock.lock_read();
            let template_version = self_template.version.get();
            if self.version.get() != template_version {
                need_update = true;
                self.bitmap.set(0u64);
                self.version.set(template_version);
            }
            template_version
        };

        if version == template_version {
            self.bitmap.set(self.bitmap.get() | (1 << index));
            self.fields.borrow_mut()[index as usize] = value;
        }

        need_update
    }

    pub fn clear_field(&self, symbol: Symbol) {

        let _guard = self.rw_lock.lock_write();

        let index = self.template.borrow().get_symbol_index(symbol);
        match index {
            Some(index) => {
                let self_template = self.template.borrow();
                let _guard_2 = self_template.rw_lock.lock_read();
                let template_version = self_template.version.get();
                if self.version.get() != template_version {
                    self.bitmap.set(0u64);
                    self.version.set(template_version);
                } else {
                    self.bitmap.set(self.bitmap.get() & (!(1 << index)));
                }
            },
            None => {}
        }

    }

}

#[test]
fn test_field_template_creation() {

    let _template = FieldTemplate::new(1);

}

#[test]
fn test_field_template_symbol() -> Result<(), Error> {

    let template = FieldTemplate::new(1);

    assert_eq!(template.get_id(), 1);

    assert!(template.get_symbol_index(Symbol::new(0)).is_none());

    let index = template.add_symbol(Symbol::new(0))?;

    assert!(template.has_symbol(Symbol::new(0)));

    assert!(template.add_symbol(Symbol::new(0)).is_err());

    assert_eq!(template.get_symbol_count(), 1);

    let index_2 = template.add_symbol(Symbol::new(1))?;

    assert_eq!(template.get_symbol_count(), 2);
    assert_eq!(template.get_symbol_index(Symbol::new(0)).unwrap(), index);
    assert_eq!(template.get_symbol_index(Symbol::new(1)).unwrap(), index_2);

    template.remove_symbol(Symbol::new(0))?;
    assert_eq!(template.get_symbol_count(), 1);
    assert!(!template.has_symbol(Symbol::new(0)));

    assert_eq!(template.get_symbol_count(), 1);

    Ok(())

}

#[test]
fn test_field_shortcuts_size() {

    assert_eq!(std::mem::size_of::<FieldShortcuts>(), 256);

}

#[test]
fn test_field_shortcuts() -> Result<(), Error> {

    let template = Arc::new(FieldTemplate::new(1));
    let template_2 = Arc::new(FieldTemplate::new(2));

    let index = template.add_symbol(Symbol::new(1))?;

    let fields = FieldShortcuts::new(template.clone());

    assert_eq!(fields.get_field_template_id(), template.get_id());

    assert!(Arc::ptr_eq(&template, &fields.get_field_template()));
    assert_eq!(fields.get_field_index(Symbol::new(1)).unwrap(), index);
    fields.set_field(template.get_id(), template.get_version(), index, Value::make_float(32.0));
    fields.clear_field(Symbol::new(1));
    assert!(fields.get_field(template.get_id(), template.get_version(), index).0.is_none());

    fields.set_field(template.get_id(), template.get_version(), index, Value::make_float(32.0));
    assert_eq!(fields.get_field(template.get_id(), template.get_version(), index).0.unwrap(), Value::make_float(32.0));
    assert!(fields.get_field(template.get_id(), template.get_version(), 32).0.is_none());

    fields.update_field_template(template_2.clone());
    assert!(Arc::ptr_eq(&template_2, &fields.get_field_template()));
    assert!(fields.get_field(template_2.get_id(), template_2.get_version(), index).0.is_none());

    assert_eq!(fields.get_field_template_id(), template_2.get_id());

    Ok(())

}

#[test]
fn test_field_template_version() -> Result<(), Error> {

    let template = Arc::new(FieldTemplate::new(1));

    let version = template.get_version();

    assert!(!template.has_symbol(Symbol::new(1)));

    template.add_symbol(Symbol::new(1))?;
    assert_eq!(template.get_version(), version);
    assert!(template.has_symbol(Symbol::new(1)));

    template.add_symbol(Symbol::new(2))?;
    assert_eq!(template.get_version(), version);

    template.remove_symbol(Symbol::new(1))?;
    assert_ne!(template.get_version(), version);
    assert!(!template.has_symbol(Symbol::new(1)));

    Ok(())

}

#[test]
fn test_field_token() -> Result<(), Error> {

    let template = Arc::new(FieldTemplate::new(1));

    let field_shortcuts = Arc::new(FieldShortcuts::new(template.clone()));

    assert!(template.get_field_token(Symbol::new(1)).is_none());

    template.add_symbol(Symbol::new(1))?;

    let field_token = template.get_field_token(Symbol::new(1)).unwrap();
    let field_token_2 = field_shortcuts.get_field_token(Symbol::new(1)).unwrap();

    assert_eq!(field_token.get_template(), field_token_2.get_template());
    assert_eq!(field_token.get_version(), field_token_2.get_version());
    assert_eq!(field_token.get_index(), field_token_2.get_index());

    assert!(field_token.get_field(&field_shortcuts).is_none());

    field_token.set_field(&field_shortcuts, Value::make_float(23.4));

    assert_eq!(field_token.get_field(&field_shortcuts).unwrap(), Value::make_float(23.4));
    assert_eq!(field_token_2.get_field(&field_shortcuts).unwrap(), Value::make_float(23.4));

    template.add_symbol(Symbol::new(2))?;

    assert_eq!(field_token.get_field(&field_shortcuts).unwrap(), Value::make_float(23.4));

    template.remove_symbol(Symbol::new(2))?;

    assert!(field_token.get_field(&field_shortcuts).is_none());

    Ok(())
}