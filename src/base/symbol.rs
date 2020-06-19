use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use super::error::Error;
use super::error::ErrorType::*;
use super::super::util::RwLock;
use super::value::Value;


pub struct SymbolIdGenerator {
    next_id: AtomicU32
}

impl SymbolIdGenerator {

    #[inline]
    pub fn new() -> SymbolIdGenerator {
        SymbolIdGenerator {
            next_id: AtomicU32::new(1)
        }
    }

    #[inline]
    pub fn generate(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

}

#[derive(Clone)]
pub enum SymbolRecord {
    TextSymbol(Arc<String>),
    ValueSymbol(Value)
}

/// Property symbol scope for object properties
pub struct SymbolScope {
    id: Arc<String>,
    rw_lock: RwLock,
    generator: Arc<SymbolIdGenerator>,
    text_symbols: RefCell<HashMap<String, Symbol>>,
    value_symbols: RefCell<HashMap<Value, Symbol>>,
    symbol_records: RefCell<HashMap<Symbol, SymbolRecord>>,
    symbol_references: RefCell<HashMap<Symbol, u32>>,
    symbol_nursery: RefCell<HashSet<Symbol>>
}

impl SymbolScope {

    /// Create a new symbol scope 
    pub fn new(generator: Arc<SymbolIdGenerator>, id: &str) -> SymbolScope {
        SymbolScope {
            id: Arc::new(id.to_owned()),
            rw_lock: RwLock::new(),
            generator: generator,
            text_symbols: RefCell::new(HashMap::new()),
            value_symbols: RefCell::new(HashMap::new()),
            symbol_records: RefCell::new(HashMap::new()),
            symbol_references: RefCell::new(HashMap::new()),
            symbol_nursery: RefCell::new(HashSet::new())
        }
    }

    /// Get the id of the symbol scope
    pub fn get_id(&self) -> Arc<String> {
        self.id.clone()
    }

    /// Get symbol recorded info from a symbol
    pub fn get_symbol_record(&self, symbol: Symbol) -> Option<SymbolRecord> {
        let _guard = self.rw_lock.lock_read();
        match self.symbol_records.borrow().get(&symbol) {
            Some(symbol_record) => Some(symbol_record.clone()),
            None => None
        }
    }

    /// Get a text property symbol
    pub fn get_text_symbol(&self, text: &str) -> Symbol {

        {
            let _guard = self.rw_lock.lock_read();
            if let Some(result) = self.text_symbols.borrow().get(text) {
                return *result;
            }
        }

        {
            let _guard = self.rw_lock.lock_write();
            if let Some(result) = self.text_symbols.borrow().get(text) {
                return *result;
            }
            let result = Symbol::new(self.generator.generate());
            self.text_symbols.borrow_mut().insert(text.to_owned(), result);
            self.symbol_records.borrow_mut().insert(result, SymbolRecord::TextSymbol(Arc::new(text.to_owned())));
            self.symbol_nursery.borrow_mut().insert(result);
            result
        }

    }

    /// Get a value property symbol
    pub fn get_value_symbol(&self, value: Value) -> Symbol {

        { 
            let _guard = self.rw_lock.lock_read();
            if let Some(result) = self.value_symbols.borrow().get(&value) {
                return *result;
            }
        }

        {
            let _guard = self.rw_lock.lock_write();
            if let Some(result) = self.value_symbols.borrow().get(&value) {
                return *result;
            }
            let result = Symbol::new(self.generator.generate());
            self.value_symbols.borrow_mut().insert(value, result);
            self.symbol_records.borrow_mut().insert(result, SymbolRecord::ValueSymbol(value));
            self.symbol_nursery.borrow_mut().insert(result);
            result
        }

    }

    pub fn add_symbol_reference(&self, symbol: Symbol) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let mut references = self.symbol_references.borrow_mut();
        let count = match references.get(&symbol) {
            None => 0,
            Some(count) => *count
        };

        references.insert(symbol, count + 1);

        self.symbol_nursery.borrow_mut().remove(&symbol);

        Ok(()) 
    }

    pub fn remove_symbol_reference(&self, symbol: Symbol) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();
        
        let mut references = self.symbol_references.borrow_mut();
        let count = match references.get(&symbol) {
            None => {
                return Err(Error::new(FatalError, "Symbol has no references"));
            },
            Some(count) => *count
        };

        if count == 1_u32 {
            references.remove(&symbol);
        } else {
            references.insert(symbol, count - 1);
        }
        Ok(())

    }

    pub fn recycle_symbol(&self, symbol: Symbol) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        if self.symbol_nursery.borrow().get(&symbol).is_some() {
            return Err(Error::new(FatalError, "Symbol in nursery"));
        }
        
        let references = self.symbol_references.borrow_mut();
        match references.get(&symbol) {
            None => {
                let mut records = self.symbol_records.borrow_mut();
                match records.get(&symbol) {
                    Some(symbol_record) => {
                        match symbol_record {
                            SymbolRecord::TextSymbol(text) => {
                                self.text_symbols.borrow_mut().remove(text.as_ref());
                            },
                            SymbolRecord::ValueSymbol(value) => {
                                self.value_symbols.borrow_mut().remove(value);
                            }
                        }
                        records.remove(&symbol);
                        Ok(())
                    },
                    None => {
                        Err(Error::new(FatalError, "Symbol not found"))
                    }
                }
            },
            Some(_) => {
                Err(Error::new(FatalError, "Symbol referenced by other objects"))
            }
        }

    }

}


/// Property symbol for objects
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Symbol {
    id: u32
}

impl Symbol {

    /// Create a new property symbol with ID
    #[inline]
    pub fn new(id: u32) -> Symbol {
        Symbol {
            id: id
        }
    }

    /// Get the ID of property symbol
    #[inline]
    pub fn get_id(&self) -> u32 {
        self.id
    }

}

#[test]
fn test_symbol_id_generator() {

    let generator = SymbolIdGenerator::new();

    assert_eq!(generator.generate(), 1);
    assert_eq!(generator.generate(), 2);
    assert_eq!(generator.generate(), 3);
    assert_eq!(generator.generate(), 4);
    
}

#[test]
fn test_text_symbol() {

    let generator = Arc::new(SymbolIdGenerator::new());
    let scope = SymbolScope::new(generator.clone(), "test");

    let test = scope.get_text_symbol("test");
    let test_2 = scope.get_text_symbol("test2");
    let test_3 = scope.get_text_symbol("test2");

    assert_ne!(test, test_2);
    assert_eq!(test_2, test_3);

    let scope_2 = SymbolScope::new(generator.clone(), "test2");
    let test_2_2 = scope_2.get_text_symbol("test");
    assert_ne!(test, test_2_2);
    assert_ne!(test_2, test_2_2);

}

#[test]
fn test_value_symbol() {

    let generator = Arc::new(SymbolIdGenerator::new());
    let scope = SymbolScope::new(generator.clone(), "test");

    let test = scope.get_value_symbol(Value::make_null());
    let test_2 = scope.get_value_symbol(Value::make_integer(1));
    let test_3 = scope.get_value_symbol(Value::make_integer(1));

    assert_ne!(test, test_2);
    assert_eq!(test_2, test_3);

    let scope_2 = SymbolScope::new(generator.clone(), "test2");
    let test_2_2 = scope_2.get_value_symbol(Value::make_null());
    assert_ne!(test, test_2_2);
    assert_ne!(test_2, test_2_2);

}

#[test]
fn test_symbol_recycle() -> Result<(), Error> {

    let generator = Arc::new(SymbolIdGenerator::new());
    let scope = SymbolScope::new(generator.clone(), "test");

    assert!(scope.recycle_symbol(Symbol::new(1)).is_err());

    let test = scope.get_value_symbol(Value::make_null());
    let test_2 = scope.get_text_symbol("test");

    assert!(scope.get_symbol_record(test).is_some());
    assert!(scope.recycle_symbol(test).is_err());
    scope.add_symbol_reference(test)?;
    assert!(scope.recycle_symbol(test).is_err());
    scope.remove_symbol_reference(test)?;
    assert!(scope.recycle_symbol(test).is_ok());
    assert!(scope.get_symbol_record(test).is_none());

    assert!(scope.get_symbol_record(test_2).is_some());
    assert!(scope.recycle_symbol(test_2).is_err());
    scope.add_symbol_reference(test_2)?;
    assert!(scope.recycle_symbol(test_2).is_err());
    scope.remove_symbol_reference(test_2)?;
    assert!(scope.recycle_symbol(test_2).is_ok());
    assert!(scope.get_symbol_record(test_2).is_none());

    Ok(())

}
