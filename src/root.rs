use std::cell::Cell;
use std::cell::RefCell;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use super::base::Error;
use super::base::ErrorType::*;
use super::base::Value;
use super::util::RwLock;

pub struct Root {
    rw_lock: RwLock,
    value: Cell<Value>,
    references: Cell<u32>
}

impl Root {

    pub fn new(value: Value) -> Root {
        Root {
            rw_lock: RwLock::new(),
            value: Cell::new(value),
            references: Cell::new(0)
        }
    }

    pub fn get_value(&self) -> Value {

        let _guard = self.rw_lock.lock_read();

        self.value.get()

    }

    pub fn refresh_value(&self, old_value: Value, new_value: Value) {

        let _guard = self.rw_lock.lock_write();

        if self.value.get() != old_value {
            return;
        }

        self.value.set(new_value);

    }

    pub fn increase_reference(&self) -> Result<u32, Error> {

        let _guard = self.rw_lock.lock_write();

        let references = self.references.get();

        self.references.set(references + 1);

        Ok(references + 1)

    }

    pub fn decrease_reference(&self) -> Result<u32, Error> {

        let _guard = self.rw_lock.lock_write();

        let references = self.references.get();
        if references == 0 {
            return Err(Error::new(FatalError, "Reference count over released"))
        }

        self.references.set(references - 1);

        Ok(references - 1)

    }

    pub fn is_alone(&self) -> bool {

        let _guard = self.rw_lock.lock_read();

        self.references.get() == 0

    }

}

pub struct Roots {
    rw_lock: RwLock,
    value: Cell<Value>,
    roots: RefCell<Vec<Arc<Root>>>
}

impl Roots {

    pub fn new(value: Value) -> Roots {
        Roots {
            rw_lock: RwLock::new(),
            value: Cell::new(value),
            roots: RefCell::new(Vec::new())
        }
    }

    pub fn get_any_root(&self) -> Arc<Root> {

        {
            let _guard = self.rw_lock.lock_read();
            let roots = self.roots.borrow();
            if roots.len() > 0 {
                return roots[0].clone();
            }
        }

        {

            let _guard = self.rw_lock.lock_write();

            let mut roots = self.roots.borrow_mut();
            if roots.len() > 0 {
                return roots[0].clone();
            }

            let root = Arc::new(Root::new(self.value.get()));
            roots.push(root.clone());

            root
        }

    }

    // TODO: check whether the code below is needed
    #[allow(dead_code)]
    pub fn get_value(&self) -> Value {

        let _guard = self.rw_lock.lock_read();

        self.value.get()

    }

    pub fn refresh_value(&self, old_value: Value, new_value: Value) {

        let _guard = self.rw_lock.lock_write();

        if self.value.get() != old_value {
            return;
        }

        self.value.set(new_value);

        for root in self.roots.borrow().iter() {
            root.refresh_value(old_value, new_value);
        }

    }

    pub fn merge_roots(&self, roots: Arc<Roots>) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        let _guard_2 = roots.rw_lock.lock_read();

        if self.value != roots.value {
            return Err(Error::new(FatalError, "Root value different"));
        }

        for root in roots.roots.borrow().iter() {
            self.roots.borrow_mut().push(root.clone());
        }

        Ok(())

    }

    pub fn is_alone(&self) -> bool {

        let _guard = self.rw_lock.lock_read();

        for root in self.roots.borrow().iter() {
            if !root.is_alone() {
                return false;
            }
        }

        true

    }

}

pub trait DropListener {

    fn notify_drop(&self);

}

pub struct WeakIdGenerator {
    next_id: AtomicU32
}

impl WeakIdGenerator {

    #[inline]
    pub fn new() -> WeakIdGenerator {
        WeakIdGenerator {
            next_id: AtomicU32::new(1)
        }
    }

    #[inline]
    pub fn generate(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

}

pub struct WeakRoot {
    rw_lock: RwLock,
    weak_id: u32,
    value: Cell<Option<Value>>,
    drop_listener: RefCell<Option<Box<dyn DropListener>>>
}

impl WeakRoot {

    pub fn new(weak_id_generator: &WeakIdGenerator, value: Value, drop_listener: Option<Box<dyn DropListener>>) -> WeakRoot {

        WeakRoot {
            weak_id: weak_id_generator.generate(),
            rw_lock: RwLock::new(),
            value: Cell::new(Some(value)),            
            drop_listener: RefCell::new(drop_listener)
        }

    }

    pub fn is_dropped(&self) -> bool {

        let _guard = self.rw_lock.lock_read();

        self.value.get().is_none()

    }

    pub fn notify_drop(&self) -> Result<(), Error> {

        let _guard = self.rw_lock.lock_write();

        if self.value.get().is_none() {
            return Err(Error::new(FatalError, "Value already dropped"));
        }

        self.value.set(None);

        let mut drop_listener = self.drop_listener.borrow_mut();
        if drop_listener.is_some() {
            drop_listener.as_ref().unwrap().notify_drop();
            *drop_listener = None;
        }

        Ok(())

    }

    pub fn get_value(&self) -> Option<Value> {

        let _guard = self.rw_lock.lock_read();

        self.value.get()

    }

    pub fn refresh_value(&self, old_value: Value, new_value: Value) {

        let _guard = self.rw_lock.lock_write();

        match self.value.get() {
            None => { 
                return;
            },
            Some(value) => {
                if value != old_value {
                    return;
                }
            }
        }

        self.value.set(Some(new_value));

    }

}

impl Eq for WeakRoot {}

impl PartialEq for WeakRoot {

    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.weak_id == other.weak_id
    }

}

impl Hash for WeakRoot {

    fn hash<H: Hasher>(&self, state: &mut H) {
        self.weak_id.hash(state)
    }

}

#[cfg(test)] use super::test::TestDropListener;

#[test]
fn test_root_creation() {

    Root::new(Value::make_null());

}

#[test]
fn test_root_get() {

    let root = Root::new(Value::make_float(4.3));

    assert_eq!(root.get_value(), Value::make_float(4.3));

}

#[test]
fn test_root_refresh() {

    let root = Root::new(Value::make_float(4.3));

    assert_eq!(root.get_value(), Value::make_float(4.3));

    root.refresh_value(Value::make_float(5.3), Value::make_float(8.4));

    assert_eq!(root.get_value(), Value::make_float(4.3));

    root.refresh_value(Value::make_float(4.3), Value::make_float(8.4));

    assert_eq!(root.get_value(), Value::make_float(8.4));

}

#[test]
fn test_root_references() -> Result<(), Error> {

    let root = Root::new(Value::make_float(4.3));

    assert!(root.is_alone());

    assert_eq!(root.increase_reference()?, 1);
    assert_eq!(root.increase_reference()?, 2);
    assert!(!root.is_alone());
    assert_eq!(root.decrease_reference()?, 1);
    assert_eq!(root.decrease_reference()?, 0);
    assert!(root.is_alone());

    assert!(root.decrease_reference().is_err());

    Ok(())

}

#[test]
fn test_roots_creation() {

    let roots = Roots::new(Value::make_float(4.3));

    assert_eq!(roots.get_value(), Value::make_float(4.3));

}

#[test]
fn test_roots_refresh_value() {

    let roots = Roots::new(Value::make_float(4.3));

    let root = roots.get_any_root();

    assert_eq!(root.get_value(), Value::make_float(4.3));
    assert_eq!(roots.get_value(), Value::make_float(4.3));

    roots.refresh_value(Value::make_float(5.3), Value::make_float(5.4));

    assert_eq!(roots.get_value(), Value::make_float(4.3));
    assert_eq!(root.get_value(), Value::make_float(4.3));

    roots.refresh_value(Value::make_float(4.3), Value::make_float(5.4));

    assert_eq!(roots.get_value(), Value::make_float(5.4));
    assert_eq!(root.get_value(), Value::make_float(5.4));

}

#[test]
fn test_roots_references() -> Result<(), Error> {

    let roots = Roots::new(Value::make_float(4.3));

    assert!(roots.is_alone());

    let root = roots.get_any_root();

    assert!(roots.is_alone());
    assert_eq!(root.increase_reference()?, 1);

    assert!(!roots.is_alone());

    assert_eq!(root.decrease_reference()?, 0);

    assert!(roots.is_alone());

    Ok(())

}

#[test]
fn test_roots_merge() -> Result<(), Error> {

    let roots = Arc::new(Roots::new(Value::make_float(4.3)));

    let root = roots.get_any_root();

    let root_2 = {

        let roots_2 = Arc::new(Roots::new(Value::make_float(5.3)));

        assert!(roots.merge_roots(roots_2.clone()).is_err());

        root.increase_reference()?;

        let root_2 = roots_2.get_any_root();
        root_2.increase_reference()?;

        roots_2.refresh_value(Value::make_float(5.3), Value::make_float(4.3));

        roots.merge_roots(roots_2)?;

        root_2

    };

    assert_eq!(roots.roots.borrow().len(), 2);
    assert!(!roots.is_alone());
    root.decrease_reference()?;
    assert!(!roots.is_alone());
    assert!(root.is_alone());
    assert!(root.decrease_reference().is_err());

    root_2.decrease_reference()?;
    assert!(root_2.is_alone());
    assert!(roots.is_alone());

    Ok(())

}

#[test]
fn test_weak_id_generator() {

    let weak_id_generator = WeakIdGenerator::new();

    assert_eq!(weak_id_generator.generate(), 1);
    assert_eq!(weak_id_generator.generate(), 2);

}

#[test]
fn test_weak_root() -> Result<(), Error> {

    let weak_id_generator = WeakIdGenerator::new();

    let weak_root = WeakRoot::new(&weak_id_generator, Value::make_float(44.0), None);

    assert!(!weak_root.is_dropped());
    assert_eq!(weak_root.get_value().unwrap(), Value::make_float(44.0));

    weak_root.refresh_value(Value::make_float(4.0), Value::make_float(42.0));
    assert_eq!(weak_root.get_value().unwrap(), Value::make_float(44.0));

    weak_root.refresh_value(Value::make_float(44.0), Value::make_float(42.0));
    assert_eq!(weak_root.get_value().unwrap(), Value::make_float(42.0));

    weak_root.notify_drop()?;

    assert!(weak_root.is_dropped());
    assert!(weak_root.get_value().is_none());

    let drop_value = Arc::new(Cell::new(Value::make_float(22.0)));

    let drop_listener = Box::new(TestDropListener::new(drop_value.clone()));

    let weak_root = WeakRoot::new(&weak_id_generator, Value::make_float(44.0), Some(drop_listener));

    assert_eq!(drop_value.as_ref().get(), Value::make_float(22.0));

    weak_root.notify_drop()?;

    assert_eq!(drop_value.as_ref().get(), Value::make_null());

    Ok(())

}
