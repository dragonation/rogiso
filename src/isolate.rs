use std::any::Any;
use std::cell::{ Cell, RefCell };
use std::collections::{ HashMap, HashSet };
use std::sync::Arc;
use std::sync::atomic::{ AtomicU64, Ordering };

use super::base::Error;
use super::base::ErrorType::*;
use super::base::PrimitiveType;
use super::base::PrimitiveType::*;
use super::base::Symbol;
use super::base::SymbolInfo;
use super::base::SymbolIdGenerator;
use super::base::SymbolScope;
use super::base::Value;
use super::barrier::Barrier;
use super::context::Context;
use super::field_shortcuts::FieldShortcuts;
use super::field_shortcuts::FieldToken;
use super::internal_slot::InternalSlot;
use super::internal_slot::ProtectedInternalSlot;
use super::internal_slot::List;
use super::internal_slot::Text;
use super::region::Region;
use super::storage::Pinned;
use super::root::Root;
use super::root::Roots;
use super::root::WeakRoot;
use super::root::WeakIdGenerator;
use super::root::DropListener;
use super::trap::PropertyTrap;
use super::trap::SlotTrap;
use super::util::ReentrantLock;
use super::util::ReentrantToken;
use super::util::RwLock;
use super::util::PageMap;
use super::util::PageItemFactory;

use super::slot::BASE_BLACK;
use super::slot::BASE_WHITE;



pub struct RegionFactory {}

impl PageItemFactory<Arc<Region>> for RegionFactory {

    fn create_item(&self, id: usize) -> Box<Arc<Region>> {
        Box::new(Arc::new(Region::new(id as u32)))
    }

}


/// Isolated storage for slotted values
pub struct Isolate {

    barrier: RefCell<Option<Box<dyn Barrier>>>,

    region_rw_lock: RwLock,
    regions: RefCell<PageMap<Arc<Region>, RegionFactory>>,
    protected_region_ids: RefCell<HashSet<u32>>,

    base_color: Cell<u8>,
    next_internal_slot_id: AtomicU64,

    slot_layout_lock: Arc<ReentrantLock>,

    symbol_rw_lock: RwLock,
    symbol_id_generator: Arc<SymbolIdGenerator>,
    symbol_scopes: RefCell<HashMap<String, Arc<SymbolScope>>>,
    symbol_lut: RefCell<HashMap<Symbol, Arc<SymbolScope>>>,

    boolean_prototype: Value,
    integer_prototype: Value,
    float_prototype: Value,
    text_prototype: Value,
    symbol_prototype: Value,
    list_prototype: Value,
    tuple_prototype: Value,
    object_prototype: Value,

    prototype_symbol: Symbol,

    roots_rw_lock: RwLock,
    roots: RefCell<HashMap<Value, Arc<Roots>>>,
    weak_id_generator: WeakIdGenerator,
    weak_roots: RefCell<HashMap<Value, RefCell<HashSet<Arc<WeakRoot>>>>>,

    next_protected_id: AtomicU64,
    protection_rw_lock: RwLock,
    protected_internal_slots: RefCell<HashMap<u64, Arc<dyn InternalSlot>>>,
    protected_slot_traps: RefCell<HashMap<u64, Arc<dyn SlotTrap>>>,
    protected_property_traps: RefCell<HashMap<u64, Arc<dyn PropertyTrap>>>,

    outlets_rw_lock: RwLock,
    next_outlet_id: AtomicU64,
    outlets: RefCell<HashMap<u64, Arc<dyn Any>>>

}

/// Isolate constructors
impl Isolate {

    /// Create an isolate
    pub fn create() -> Result<Isolate, Error> {

        let mut isolate = Isolate {

            barrier: RefCell::new(None),

            region_rw_lock: RwLock::new(),
            regions: RefCell::new(PageMap::new(RegionFactory {})),
            protected_region_ids: RefCell::new(HashSet::new()),

            base_color: Cell::new(BASE_WHITE),
            next_internal_slot_id: AtomicU64::new(0),

            slot_layout_lock: Arc::new(ReentrantLock::new()),

            symbol_rw_lock: RwLock::new(),
            symbol_id_generator: Arc::new(SymbolIdGenerator::new()),
            symbol_scopes: RefCell::new(HashMap::new()),
            symbol_lut: RefCell::new(HashMap::new()),

            boolean_prototype: Value::make_undefined(),
            integer_prototype: Value::make_undefined(),
            float_prototype: Value::make_undefined(),
            symbol_prototype: Value::make_undefined(),
            text_prototype: Value::make_undefined(),
            list_prototype: Value::make_undefined(),
            tuple_prototype: Value::make_undefined(),
            object_prototype: Value::make_undefined(),

            prototype_symbol: Symbol::new(0),

            roots_rw_lock: RwLock::new(),
            roots: RefCell::new(HashMap::new()),
            weak_id_generator: WeakIdGenerator::new(),
            weak_roots: RefCell::new(HashMap::new()),

            next_protected_id: AtomicU64::new(0),
            protection_rw_lock: RwLock::new(),
            protected_internal_slots: RefCell::new(HashMap::new()),
            protected_slot_traps: RefCell::new(HashMap::new()),
            protected_property_traps: RefCell::new(HashMap::new()),

            outlets_rw_lock: RwLock::new(),
            next_outlet_id: AtomicU64::new(0),
            outlets: RefCell::new(HashMap::new())

        };

        let region_id = isolate.create_region()?;

        let layout_token = ReentrantToken::new(isolate.slot_layout_lock.clone());

        isolate.object_prototype = isolate.gain_slot(region_id, Object, Value::make_null(), &layout_token)?;
        isolate.boolean_prototype = isolate.gain_slot(region_id, Object, isolate.object_prototype, &layout_token)?;
        isolate.integer_prototype = isolate.gain_slot(region_id, Object, isolate.object_prototype, &layout_token)?;
        isolate.float_prototype = isolate.gain_slot(region_id, Object, isolate.object_prototype, &layout_token)?;
        isolate.symbol_prototype = isolate.gain_slot(region_id, Object, isolate.object_prototype, &layout_token)?;
        isolate.text_prototype = isolate.gain_slot(region_id, Object, isolate.object_prototype, &layout_token)?;
        isolate.list_prototype = isolate.gain_slot(region_id, Object, isolate.object_prototype, &layout_token)?;
        isolate.tuple_prototype = isolate.gain_slot(region_id, Object, isolate.object_prototype, &layout_token)?;

        isolate.prototype_symbol = isolate.get_text_symbol("isolate.prototype", "prototype");

        isolate.unprotect_region(region_id)?;

        Ok(isolate)
    }

}

/// Isolate barrier and layout locks
impl Isolate {

    pub fn overwrite_barrier(&self, barrier: Box<dyn Barrier>) -> Result<(), Error> {

        let layout_token = self.create_slot_layout_token();

        let _layout_guard = layout_token.lock_write();

        if self.barrier.borrow().is_some() {
            return Err(Error::new(FatalError, "Barrier already exists"));
        }

        self.barrier.borrow_mut().replace(barrier);

        Ok(())

    }

    pub fn clear_barrier(&self) -> Result<Box<dyn Barrier>, Error> {

        let layout_token = self.create_slot_layout_token();

        let _layout_guard = layout_token.lock_write();

        if self.barrier.borrow().is_none() {
            return Err(Error::new(FatalError, "No barrier available"));
        }

        Ok(self.barrier.borrow_mut().take().unwrap())

    }

    pub fn create_slot_layout_token(&self) -> ReentrantToken {
        ReentrantToken::new(self.slot_layout_lock.clone())
    }

}

/// Isolate states and basic properties
impl Isolate {

    /// Check whether a region could gain slot quickly
    pub fn could_region_gain_slot_quickly(&self, region_id: u32) -> bool {

        let _guard = self.region_rw_lock.lock_read();
        self.regions.borrow()[region_id as usize].could_gain_slot_quickly()

    }

    pub fn gain_internal_slot_id(&self) -> u64 {
        self.next_internal_slot_id.fetch_add(1, Ordering::SeqCst)
    }

}

/// Isolate predefined symbols
impl Isolate {

    // Get the symbol for prototype
    pub fn get_prototype_symbol(&self) -> Symbol {
        self.prototype_symbol
    }

}

/// Isolate predefined prototypes
impl Isolate {

    /// Get the prototype of object
    pub fn get_object_prototype(&self) -> Value {
        self.object_prototype
    }

    /// Get the prototype of boolean
    pub fn get_boolean_prototype(&self) -> Value {
        self.boolean_prototype
    }

    /// Get the prototype of integer
    pub fn get_integer_prototype(&self) -> Value {
        self.integer_prototype
    }

    /// Get the prototype of float
    pub fn get_float_prototype(&self) -> Value {
        self.float_prototype
    }

    /// Get the prototype of symbol
    pub fn get_symbol_prototype(&self) -> Value {
        self.symbol_prototype
    }

    /// Get the prototype of text
    pub fn get_text_prototype(&self) -> Value {
        self.text_prototype
    }

    /// Get the prototype of list
    pub fn get_list_prototype(&self) -> Value {
        self.list_prototype
    }

    /// Get the prototype of tuple
    pub fn get_tuple_prototype(&self) -> Value {
        self.tuple_prototype
    }

}

/// Isolate value information extraction
impl Isolate {

    /// Extract text from a value 
    pub fn extract_text(&self, value: Value, context: &Box<dyn Context>) -> String {

        match value.get_primitive_type() {
            Undefined => { 
                return "<undefined>".to_owned(); 
            },
            Null => { 
                return "<null>".to_owned(); 
            },
            Boolean => { 
                return match value.as_boolean() {
                    true => "<yes>".to_owned(),
                    _ => "<no>".to_owned()
                }; 
            },
            Integer => { 
                if value.is_cardinal() {
                    return value.extract_cardinal(0).to_string()
                } else {
                    return value.extract_integer(0).to_string()
                }
            },
            Float => { 
                return value.extract_float(0.0).to_string(); 
            },
            Symbol => { 
                match self.resolve_symbol_info(value.extract_symbol(Symbol::new(0))) {
                    Ok(symbol_info) => {
                        let mut result = String::new();
                        result.push_str("<symbol:");
                        result.push_str(symbol_info.get_symbol_scope());
                        result.push_str("#");
                        if symbol_info.is_text_symbol() {
                            result.push_str(symbol_info.get_text().unwrap())
                        } else {
                            result.push_str("<value>");
                        }
                        result.push_str(">");
                        return result;
                    },
                    Err(_) => {
                        return "<symbol>".to_owned();
                    }
                }
            },
            Text => {
                match self.get_internal_slot(value, 0, context) {
                    Ok(Some(internal_slot)) => {
                        match internal_slot.as_any().downcast_ref::<Text>() {
                            Some(text) => {
                                return text.to_string();
                            },
                            None => {
                                return "<text>".to_owned();
                            }
                        }
                    },
                    Ok(None) => {
                        return "<text>".to_owned();
                    },
                    Err(_) => {
                        return "<text>".to_owned();
                    }
                }
            },
            List => {
                return "<list>".to_owned();
            },
            Tuple => {
                return "<tuple>".to_owned();
            },
            Object => {
                return "<object>".to_owned();
            } 
        }

    }

    pub fn extract_list(&self, value: Value, context: &Box<dyn Context>) -> Result<Vec<Value>, Error> {
        
        match value.get_primitive_type() {
            Undefined => { return Err(Error::new(FatalError, "Undefined could not converted to list")); },
            Null => { return Err(Error::new(FatalError, "Null could not converted to list")); },
            Boolean => { return Err(Error::new(FatalError, "Boolean could not converted to list")); },
            Integer => { return Err(Error::new(FatalError, "Integer could not converted to list")); },
            Float => { return Err(Error::new(FatalError, "Float could not converted to list")); },
            Symbol => { return Err(Error::new(FatalError, "Symbol could not converted to list")); },
            Text => { return Err(Error::new(FatalError, "Text could not converted to list")); },
            List => {
                match self.get_internal_slot(value, 0, context) {
                    Ok(Some(internal_slot)) => {
                        match internal_slot.as_any().downcast_ref::<List>() {
                            Some(list) => { return Ok(list.get_value_list()); },
                            None => { return Ok(Vec::new()); }
                        }
                    },
                    Ok(None) => { return Ok(Vec::new()); },
                    Err(_) => { return Ok(Vec::new()); }
                }
            },
            Tuple => { return Err(Error::new(FatalError, "Tuple could not converted to list")); },
            Object => { return Err(Error::new(FatalError, "Object could not converted to list")); }
        }

    }

}

/// Isolate regions management
impl Isolate {

    pub fn get_region_number(&self) -> u32 {

        let _guard = self.region_rw_lock.lock_read();

        self.regions.borrow().get_size() as u32

    }

    pub fn peek_next_region_id(&self) -> u32 {

        let _guard = self.region_rw_lock.lock_read();

        self.regions.borrow().peek_next_item_index() as u32

    }

    pub fn shrink_next_region_id(&self, from: u32, to: u32) -> u32 {

        let _guard = self.region_rw_lock.lock_write();

        self.regions.borrow_mut().shrink_next_item_index(from as usize, to as usize) as u32

    }

    /// Create a new empty region
    pub fn create_region(&self) -> Result<u32, Error> {

        let _guard = self.region_rw_lock.lock_write();

        let id = self.regions.borrow_mut().gain_item()? as u32;

        self.protected_region_ids.borrow_mut().insert(id);

        Ok(id)

    }

    pub fn list_region_ids(&self) -> Result<Vec<u32>, Error> {

        let _guard = self.region_rw_lock.lock_read();

        let mut ids = Vec::new();

        for (index, _page) in self.regions.borrow().iterate_items() {
            ids.push(index as u32);
        }

        Ok(ids)

    }

    pub fn sweep_region(&self, region_id: u32, context: &Box<dyn Context>) -> Result<(), Error> {

        let _guard = self.region_rw_lock.lock_read();

        let region = match self.regions.borrow().get(region_id as usize) {
            Some(region) => region.clone(),
            None => {
                return Err(Error::new(FatalError, "Region not found"));
            }
        };

        region.sweep_values(self.base_color.get(), context)?;

        Ok(())

    }

    pub fn is_region_empty(&self, region_id: u32) -> Result<bool, Error> {

        let _guard = self.region_rw_lock.lock_read();

        match self.regions.borrow().get(region_id as usize) {
            Some(region) => Ok(region.is_empty()),
            None => Err(Error::new(FatalError, "Region not found"))
        }

    }

    pub fn is_region_full(&self, region_id: u32) -> Result<bool, Error> {

        let _guard = self.region_rw_lock.lock_read();

        match self.regions.borrow().get(region_id as usize) {
            Some(region) => Ok(region.is_full()),
            None => Err(Error::new(FatalError, "Region not found"))
        }

    }

    pub fn need_region_refragment(&self, region_id: u32) -> Result<f32, Error> {

        let _guard = self.region_rw_lock.lock_read();

        match self.regions.borrow().get(region_id as usize) {
            Some(region) => Ok(region.need_refragment()),
            None => Err(Error::new(FatalError, "Region not found"))
        }

    }

    pub fn refragment_region(&self, region_id: u32, target_region_id: u32, context: &Box<dyn Context>) -> Result<bool, Error> {

        let _guard = self.region_rw_lock.lock_read();

        let region = match self.regions.borrow().get(region_id as usize) {
            Some(region) => region.clone(),
            None => {
                return Err(Error::new(FatalError, "Region not found"));
            }
        };
        let target_region = match self.regions.borrow().get(target_region_id as usize) {
            Some(region) => region.clone(),
            None => {
                return Err(Error::new(FatalError, "Region not found"));
            }
        };

        for value in region.list_alive_values()? {
            if target_region.is_full() {
                return Ok(false);
            }
            self.move_slot(value, target_region_id, context)?;
        }
        region.recalculate_next_empty_slot_index()?;

        Ok(true)

    }

    pub fn is_region_protected(&self, region_id: u32) -> Result<bool, Error> {

        let _guard = self.region_rw_lock.lock_read();

        match self.regions.borrow().get(region_id as usize) {
            Some(_) => Ok(self.protected_region_ids.borrow().get(&region_id).is_some()),
            None => Err(Error::new(FatalError, "Region not found"))
        }

    }

    pub fn protect_region(&self, region_id: u32) -> Result<(), Error> {

        let _guard = self.region_rw_lock.lock_write();

        match self.regions.borrow().get(region_id as usize) {
            Some(_) => {
                if self.protected_region_ids.borrow().get(&region_id).is_some() {
                    return Err(Error::new(FatalError, "Region already protected"));
                }
                self.protected_region_ids.borrow_mut().insert(region_id);
                Ok(())
            },
            None => Err(Error::new(FatalError, "Region not found"))
        }

    }

    pub fn unprotect_region(&self, region_id: u32) -> Result<(), Error> {

        let _guard = self.region_rw_lock.lock_write();

        match self.regions.borrow().get(region_id as usize) {
            Some(_) => {
                if self.protected_region_ids.borrow().get(&region_id).is_none() {
                    return Err(Error::new(FatalError, "Region not protected"));
                }
                self.protected_region_ids.borrow_mut().remove(&region_id);
                Ok(())
            },
            None => Err(Error::new(FatalError, "Region not found"))
        }
 
    }

    pub fn recycle_region(&self, region_id: u32) -> Result<(), Error> {

        let _guard = self.region_rw_lock.lock_write();

        let region = match self.regions.borrow().get(region_id as usize) {
            Some(region) => {
                if self.protected_region_ids.borrow().get(&region_id).is_some() {
                    return Err(Error::new(FatalError, "Region protected"));
                }
                region.clone()
            },
            None => {
                return Err(Error::new(FatalError, "Region not found"));
            }
        };

        if !region.is_empty() {
            return Err(Error::new(FatalError, "Region not empty"));
        }

        self.regions.borrow_mut().recycle_item(region_id as usize)

    }

}

/// Isolate garbage collection 
impl Isolate {

    /// Resolve redirections generated from refragment of slots
    pub fn resolve_real_value(&self, value: Value, layout_token: &ReentrantToken) -> Result<Value, Error> {

        if !value.is_slotted() {
            return Ok(value);
        }

        let _guard = layout_token.lock_read();

        let mut slot = value;
        loop {
            match slot.get_region_id() {
                Ok(region_id) => {
                    let region = {
                        let _guard = self.region_rw_lock.lock_read();
                        match self.regions.borrow().get(region_id as usize) {
                            Some(region) => Some(region.clone()),
                            None => None
                        }
                    };
                    slot = match region {
                        Some(region) => {
                            let new_slot = region.resolve_redirection(slot)?;
                            if new_slot == slot {
                                return Ok(slot);
                            }
                            new_slot
                        }
                        None => {
                            return Ok(slot);
                        }
                    }
                },
                Err(_) => {
                    return Ok(slot);
                }
            }
        }

    }

    // /// Schedule a collection of younger generations
    // fn schedule_collect_younger_generations(&self) {

    // }

    // /// Schedule a collection of all generations
    // fn schedule_collect_all_generations(&self) {

    // }

}

/// Isolate root management
impl Isolate {

    /// Add a value into roots
    pub fn add_root(&self, value: Value, layout_token: &ReentrantToken) -> Result<Arc<Root>, Error> {

        if !value.is_slotted() {
            return Err(Error::new(FatalError, "Only slot value could added into roots"));
        }

        let _guard = layout_token.lock_read();

        let value = self.resolve_real_value(value, layout_token)?;

        let _guard_2 = self.roots_rw_lock.lock_write();

        let mut self_roots = self.roots.borrow_mut();

        match self_roots.get(&value) {
            Some(roots) => {
                let root = roots.get_any_root();
                root.increase_reference()?;
                return Ok(root);
            },
            None => {}
        };
        
        let roots = Arc::new(Roots::new(value));

        self_roots.insert(value, roots.clone());

        let root = roots.get_any_root();
        root.increase_reference()?;

        self.move_value_out_from_nursery(value, layout_token)?;

        Ok(root)

    }

    /// Remove a value from roots
    pub fn remove_root(&self, root: &Arc<Root>) -> Result<(), Error> {

        let _guard = self.roots_rw_lock.lock_write();

        root.decrease_reference()?;

        let value = root.get_value();

        self.barrier.borrow().as_ref().map(|barrier| barrier.preremove_value_reference(value));

        let mut self_roots = self.roots.borrow_mut();

        let alone = match self_roots.get(&value) {
            None => {
                return Err(Error::new(FatalError, "Root not found"));
            },
            Some(roots) => roots.is_alone()
        };

        if alone {
            self_roots.remove(&value);
        }

        Ok(())

    }

    /// Refresh root value
    pub fn refresh_root(&self, old_value: Value, new_value: Value) -> Result<(), Error> {

        if !old_value.is_slotted() {
            return Err(Error::new(FatalError, "Only slot value could added into roots"));
        }

        if !new_value.is_slotted() {
            return Err(Error::new(FatalError, "Only slot value could added into roots"));
        }

        let _guard = self.roots_rw_lock.lock_write();

        let mut self_roots = self.roots.borrow_mut();

        let (old_roots, new_roots) = match self_roots.get(&old_value) {
            None => {
                return Ok(());
            },
            Some(old_roots) => {
                match self_roots.get(&new_value) {
                    None => (old_roots.clone(), None),
                    Some(new_roots) => (old_roots.clone(), Some(new_roots.clone()))
                }
            }
        };

        old_roots.refresh_value(old_value, new_value);

        match new_roots {
            None => { self_roots.insert(new_value, old_roots); }
            Some(new_roots) => { new_roots.merge_roots(old_roots)?; }
        }

        self_roots.remove(&old_value);

        Ok(())

    }

    pub fn list_roots(&self) -> Vec<Value> {

        let _guard = self.roots_rw_lock.lock_read();

        let mut roots = Vec::new();
        for value in self.roots.borrow().keys() {
            roots.push(*value);
        }

        roots
    }

    pub fn list_buitins(&self) -> Vec<Value> {
        vec!(
            self.boolean_prototype,
            self.integer_prototype,
            self.float_prototype,
            self.text_prototype,
            self.symbol_prototype,
            self.list_prototype,
            self.tuple_prototype,
            self.object_prototype
        )
    }

    pub fn list_values_in_nursery(&self) -> Vec<Value> {

        let _guard = self.region_rw_lock.lock_read();

        let mut values = Vec::new();

        for (_index, region) in self.regions.borrow().iterate_items() {
            for value in region.list_values_in_nursery() {
                values.push(value);
            }
        }

        values

    }

    /// Add a value into weak roots with drop listener
    pub fn add_weak_root(&self, value: Value, drop_listener: Option<Box<dyn DropListener>>, layout_token: &ReentrantToken) -> Result<Arc<WeakRoot>, Error> {

        if !value.is_slotted() {
            return Err(Error::new(FatalError, "Only slot value could added into roots"));
        }

        let _guard = layout_token.lock_read();

        let value = self.resolve_real_value(value, layout_token)?;

        let _guard_2 = self.roots_rw_lock.lock_write();

        let mut self_roots = self.weak_roots.borrow_mut();

        if self_roots.get(&value).is_none() {
            self_roots.insert(value, RefCell::new(HashSet::new()));
        }

        let weak_root = Arc::new(WeakRoot::new(&self.weak_id_generator, value, drop_listener));

        self_roots.get(&value).unwrap().borrow_mut().insert(weak_root.clone());
       
        Ok(weak_root)

    }

    /// Remove a value from weak roots
    pub fn remove_weak_root(&self, root: &Arc<WeakRoot>) -> Result<(), Error> {

        let _guard = self.roots_rw_lock.lock_write();

        let value = root.get_value();
        if value.is_none() {
            return Ok(());
        }

        let value = value.unwrap();

        let mut self_roots = self.weak_roots.borrow_mut();

        if self_roots.get(&value).is_none() {
            return Err(Error::new(FatalError, "Weak root not found"));
        }

        let drop = {
            let mut weak_roots = self_roots.get(&value).unwrap().borrow_mut();
            if !weak_roots.remove(root) {
                return Err(Error::new(FatalError, "Weak root not found"));
            }
            weak_roots.is_empty()
        };

        if drop {
            self_roots.remove(&value);
        }

        Ok(())

    }

    /// Refresh weak root value
    pub fn refresh_weak_root(&self, old_value: Value, new_value: Value) -> Result<(), Error> {

        if !old_value.is_slotted() {
            return Err(Error::new(FatalError, "Only slot value could added into roots"));
        }

        if !new_value.is_slotted() {
            return Err(Error::new(FatalError, "Only slot value could added into roots"));
        }

        let _guard = self.roots_rw_lock.lock_write();

        let mut self_roots = self.weak_roots.borrow_mut();

        match self_roots.get(&old_value) {
            None => {
                return Ok(());
            },
            Some(old_roots) => {
                match self_roots.get(&new_value) {
                    None => {
                        let mut new_roots = HashSet::new();
                        for value in old_roots.borrow().iter() {
                            value.refresh_value(old_value, new_value);
                            new_roots.insert(value.clone());
                        }
                        self_roots.insert(new_value, RefCell::new(new_roots));
                    },
                    Some(new_roots) => {
                        for value in old_roots.borrow().iter() {
                            value.refresh_value(old_value, new_value);
                            new_roots.borrow_mut().insert(value.clone());
                        }
                    }
                }
            }
        };

        self_roots.remove(&old_value);

        Ok(())

    }

}

impl Isolate {

    pub fn flip_base_color(&self) -> u8 {

        let _guard = self.region_rw_lock.lock_read();

        if self.base_color.get() == BASE_WHITE {
            self.base_color.set(BASE_BLACK);
            BASE_BLACK
        } else {
            self.base_color.set(BASE_WHITE);
            BASE_WHITE
        }

    }

    pub fn get_base_color(&self) -> u8 {

        let _guard = self.region_rw_lock.lock_read();

        self.base_color.get()

    }

    pub fn mark_as_white(&self, value: Value) -> Result<(), Error> {

        if value.is_slotted() {
            return Ok(())
        }

        let region_id = value.get_region_id()?;

        let _guard = self.region_rw_lock.lock_read();

        let base = self.base_color.get();

        match self.regions.borrow().get(region_id as usize) {
            Some(region) => region.mark_as_white(value, base),
            None => Err(Error::new(FatalError, "Region not found"))
        }

    }

    pub fn mark_as_black(&self, value: Value) -> Result<(), Error> {

        if value.is_slotted() {
            return Ok(())
        }

        let region_id = value.get_region_id()?;

        let _guard = self.region_rw_lock.lock_read();

        let base = self.base_color.get();

        match self.regions.borrow().get(region_id as usize) {
            Some(region) => region.mark_as_black(value, base),
            None => Err(Error::new(FatalError, "Region not found"))
        }
 
    }

    pub fn mark_as_gray(&self, value: Value) -> Result<bool, Error> {

        if value.is_slotted() {
            return Ok(false);
        }

        let region_id = value.get_region_id()?;

        let _guard = self.region_rw_lock.lock_read();

        let base = self.base_color.get();

        match self.regions.borrow().get(region_id as usize) {
            Some(region) => region.mark_as_gray(value, base),
            None => Err(Error::new(FatalError, "Region not found"))
        }

    }

    pub fn is_white(&self, value: Value) -> Result<bool, Error> {

        let region_id = value.get_region_id()?;

        let _guard = self.region_rw_lock.lock_read();

        let base = self.base_color.get();

        match self.regions.borrow().get(region_id as usize) {
            Some(region) => region.is_white(value, base),
            None => Err(Error::new(FatalError, "Region not found"))
        }

    }

    pub fn is_black(&self, value: Value) -> Result<bool, Error> {

        let region_id = value.get_region_id()?;

        let _guard = self.region_rw_lock.lock_read();

        let base = self.base_color.get();

        match self.regions.borrow().get(region_id as usize) {
            Some(region) => region.is_black(value, base),
            None => Err(Error::new(FatalError, "Region not found"))
        }

    }

    pub fn is_gray(&self, value: Value) -> Result<bool, Error> {

        let region_id = value.get_region_id()?;

        let _guard = self.region_rw_lock.lock_read();

        let base = self.base_color.get();

        match self.regions.borrow().get(region_id as usize) {
            Some(region) => region.is_gray(value, base),
            None => Err(Error::new(FatalError, "Region not found"))
        }

    }

    pub fn list_and_autorefresh_referenced_values(&self, value: Value, context: &Box<dyn Context>) -> Result<(Vec<Value>, Vec<Symbol>), Error> {

        let region_id = value.get_region_id()?;

        let _guard = self.region_rw_lock.lock_read();

        match self.regions.borrow().get(region_id as usize) {
            Some(region) => region.list_and_autorefresh_referenced_values(value, context),
            None => Err(Error::new(FatalError, "Region not found"))
        }

    }

}

/// Isolate references managment
impl Isolate {

    /// Add a symbol reference record to keep it from garbage collection
    pub fn add_symbol_reference(&self, symbol: Symbol) -> Result<(), Error> {

        let _guard = self.symbol_rw_lock.lock_read();

        match self.symbol_lut.borrow().get(&symbol) {
            Some(symbol_scope) => {
                symbol_scope.add_symbol_reference(symbol)
            },
            None => Err(Error::new(FatalError, "Symbol not found"))
        }

    }

    /// Remove a symbol reference record to keep it from garbage collection
    pub fn remove_symbol_reference(&self, symbol: Symbol) -> Result<(), Error> {

        let _guard = self.symbol_rw_lock.lock_read();

        match self.symbol_lut.borrow().get(&symbol) {
            Some(symbol_scope) => {
                symbol_scope.remove_symbol_reference(symbol)
            },
            None => Err(Error::new(FatalError, "Symbol not found"))
        }

    }

    /// Move a value out from the nursery
    pub fn move_value_out_from_nursery(&self, value: Value, layout_token: &ReentrantToken) -> Result<(), Error> {

        if !value.is_slotted() {
            return Ok(());
        }

        let _guard = layout_token.lock_read();

        let region_id = value.get_region_id()?;

        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };

        match region {
            Some(region) => region.move_out_from_nursery(value)?,
            None => {
                return Err(Error::new(FatalError, "Region of slot not found"));
            }
        };

        Ok(())
    }

    /// Add a reference relationship
    pub fn add_value_reference(&self, from: Value, to: Value, layout_token: &ReentrantToken) -> Result<(), Error> {

        if !from.is_slotted() {
            return Ok(());
        }

        if to.is_symbol() {
            return self.add_symbol_reference(to.extract_symbol(Symbol::new(0)));
        }

        if !to.is_slotted() {
            return Ok(());
        }

        let _guard = layout_token.lock_read();

        let to_region_id = to.get_region_id()?;
        let to_region_slot = to.get_region_slot()?;
        let from_region_id = from.get_region_id()?;
        let from_region_slot = from.get_region_slot()?;
        if (to_region_id == from_region_id) && (to_region_slot == from_region_slot) {
            return Ok(());
        }

        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(to_region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.add_reference(to, from)?,
            None => {
                return Err(Error::new(FatalError, "Region of slot not found"));
            }
        };

        Ok(())

    }

    /// Remove a reference relationship
    pub fn remove_value_reference(&self, from: Value, to: Value, layout_token: &ReentrantToken) -> Result<(), Error> {

        if !from.is_slotted() {
            return Ok(());
        }

        if to.is_symbol() {
            return self.remove_symbol_reference(to.extract_symbol(Symbol::new(0)));
        }

        if !to.is_slotted() {
            return Ok(());
        }

        let real_to = self.resolve_real_value(to, layout_token)?;

        self.barrier.borrow().as_ref().map(|barrier| barrier.preremove_value_reference(real_to));

        let _guard = layout_token.lock_read();

        let to_region_id = to.get_region_id()?;
        let to_region_slot = to.get_region_slot()?;
        let from_region_id = from.get_region_id()?;
        let from_region_slot = from.get_region_slot()?;
        if (to_region_id == from_region_id) && (to_region_slot == from_region_slot) {
            return Ok(());
        }
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(to_region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => {
                let (no_more_reference_map, to_redirection) = region.remove_reference(to, from)?;
                if no_more_reference_map {
                    region.remove_redirection_from(to, to_redirection)?;
                }
            },
            None => {
                return Err(Error::new(FatalError, "Region of slot not found"));
            }
        }

        Ok(())

    }

    /// Update moved value reference relationship
    pub fn refresh_value_reference(&self, from: Value, old_to: Value, new_to: Value, layout_token: &ReentrantToken) -> Result<(), Error> {

        if (!from.is_slotted()) || (!old_to.is_slotted()) || (!new_to.is_slotted()) {
            return Ok(());
        }

        if (from == old_to) || (from == new_to) || (old_to == new_to) {
            return Ok(());
        }

        let _guard = layout_token.lock_read();

        self.add_value_reference(from, new_to, layout_token)?;
        self.remove_value_reference(from, old_to, layout_token)?;

        Ok(())

    }

}

/// Isolate symbols management
impl Isolate {

    /// Get a symbol with specified scope and text
    pub fn get_text_symbol(&self, scope: &str, text: &str) -> Symbol {

        {
            let _guard = self.symbol_rw_lock.lock_read();
            if let Some(result) = self.symbol_scopes.borrow().get(scope) {
                let symbol = result.get_text_symbol(text);
                if self.symbol_lut.borrow().get(&symbol).is_some() {
                    return symbol;
                }
            }
        }

        {
            let _guard = self.symbol_rw_lock.lock_write();
            if let Some(result) = self.symbol_scopes.borrow().get(scope) {
                let symbol = result.get_text_symbol(text);
                self.symbol_lut.borrow_mut().insert(symbol, result.clone());
                return symbol;
            }
            let symbol_scope = Arc::new(SymbolScope::new(self.symbol_id_generator.clone(), scope));
            let symbol = symbol_scope.get_text_symbol(text);
            self.symbol_scopes.borrow_mut().insert(scope.to_owned(), symbol_scope.clone());
            self.symbol_lut.borrow_mut().insert(symbol, symbol_scope);
            symbol
        }

    }

    /// Get a symbol with specified scope and value
    pub fn get_value_symbol(&self, scope: &str, value: Value) -> Symbol {

        {
            let _guard = self.symbol_rw_lock.lock_read();
            if let Some(result) = self.symbol_scopes.borrow().get(scope) {
                let symbol = result.get_value_symbol(value);
                if self.symbol_lut.borrow().get(&symbol).is_some() {
                    return symbol;
                }
            }
        }

        {
            let _guard = self.symbol_rw_lock.lock_write();
            if let Some(result) = self.symbol_scopes.borrow().get(scope) {
                let symbol = result.get_value_symbol(value);
                self.symbol_lut.borrow_mut().insert(symbol, result.clone());
                return symbol;
            }
            let symbol_scope = Arc::new(SymbolScope::new(self.symbol_id_generator.clone(), scope));
            let symbol = symbol_scope.get_value_symbol(value);
            self.symbol_scopes.borrow_mut().insert(scope.to_owned(), symbol_scope.clone());
            self.symbol_lut.borrow_mut().insert(symbol, symbol_scope);
            symbol
        }

    }

    /// Resolve symbol info from a symbol
    pub fn resolve_symbol_info(&self, symbol: Symbol) -> Result<SymbolInfo, Error> {

        let _guard = self.symbol_rw_lock.lock_read();

        match self.symbol_lut.borrow().get(&symbol) {
            Some(symbol_scope) => {
                match symbol_scope.get_symbol_record(symbol) {
                    Some(symbol_record) => Ok(SymbolInfo::new(symbol, &symbol_scope, symbol_record)),
                    None => Err(Error::new(FatalError, "Symbol not found"))
                }
            },
            None => Err(Error::new(FatalError, "Symbol not found"))
        }

    }

    /// Recycle symbol
    pub fn recycle_symbol(&self, symbol: Symbol) -> Result<(), Error> {

        let _guard = self.symbol_rw_lock.lock_read();

        match self.symbol_lut.borrow().get(&symbol) {
            Some(symbol_scope) => {
                symbol_scope.recycle_symbol(symbol)
            },
            None => Err(Error::new(FatalError, "Symbol not found"))
        }

    }

}

/// Internal slot and traps keeper
impl Isolate {

    pub fn protect_slot_trap(&self, slot_trap: &Arc<dyn SlotTrap>) -> Result<(u64, Arc<dyn SlotTrap>), Error> {
        let protected_id = self.next_protected_id.fetch_add(1, Ordering::SeqCst);
        let _guard = self.protection_rw_lock.lock_write();
        self.protected_slot_traps.borrow_mut().insert(protected_id, slot_trap.clone());
        Ok((protected_id, slot_trap.clone()))
    }

    pub fn protect_internal_slot(&self, internal_slot: &Arc<dyn InternalSlot>) -> Result<(u64, Arc<dyn InternalSlot>), Error> {
        let protected_id = self.next_protected_id.fetch_add(1, Ordering::SeqCst);
        let _guard = self.protection_rw_lock.lock_write();
        self.protected_internal_slots.borrow_mut().insert(protected_id, internal_slot.clone());
        Ok((protected_id, internal_slot.clone()))
    }

    pub fn protect_property_trap(&self, property_trap: &Arc<dyn PropertyTrap>) -> Result<(u64, Arc<dyn PropertyTrap>), Error> {
        let protected_id = self.next_protected_id.fetch_add(1, Ordering::SeqCst);
        let _guard = self.protection_rw_lock.lock_write();
        self.protected_property_traps.borrow_mut().insert(protected_id, property_trap.clone());
        Ok((protected_id, property_trap.clone()))
    }

    pub fn unprotect_slot_trap(&self, protected_id: u64) -> Result<(), Error> {
        let _guard = self.protection_rw_lock.lock_write();
        match self.protected_slot_traps.borrow_mut().remove(&protected_id) {
            None => Err(Error::new(FatalError, "No slot trap found")),
            Some(_) => Ok(())
        }
    }

    pub fn unprotect_internal_slot(&self, protected_id: u64) -> Result<(), Error> {
        let _guard = self.protection_rw_lock.lock_write();
        match self.protected_internal_slots.borrow_mut().remove(&protected_id) {
            None => Err(Error::new(FatalError, "No internal slot found")),
            Some(_) => Ok(())
        }
    }

    pub fn unprotect_property_trap(&self, protected_id: u64) -> Result<(), Error> {
        let _guard = self.protection_rw_lock.lock_write();
        match self.protected_property_traps.borrow_mut().remove(&protected_id) {
            None => Err(Error::new(FatalError, "No property trap found")),
            Some(_) => Ok(())
        }
    }

}

/// Isolate slot managements
impl Isolate {

    /// Gain a slot with prepared prototype
    pub fn gain_slot(&self, region_id: u32, primitive_type: PrimitiveType, prototype: Value, layout_token: &ReentrantToken) -> Result<Value, Error> {

        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => {
                let id = region.gain_slot(primitive_type)?;
                let (removed_values, removed_symbols, added_values, added_symbols) = region.overwrite_own_property(id, self.prototype_symbol, prototype)?;
                for value in added_values {
                    self.add_value_reference(id, value, layout_token)?;
                }
                for symbol in added_symbols {
                    self.add_symbol_reference(symbol)?;
                }
                for value in removed_values {
                    self.remove_value_reference(id, value, layout_token)?;
                }
                for symbol in removed_symbols {
                    self.remove_symbol_reference(symbol)?;
                }
                self.mark_as_white(id)?;
                self.barrier.borrow().as_ref().map(|barrier| barrier.postgain_value(id));
                Ok(id)
            },
            None => Err(Error::new(FatalError, "Region not found"))
        }

    }

    /// Recycle a slot
    pub fn recycle_slot(&self, slot: Value, context: &Box<dyn Context>) -> Result<(), Error> {

        let region_id = slot.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };

        {
            let _guard = self.roots_rw_lock.lock_read();
            if self.roots.borrow().get(&slot).is_some() {
                return Err(Error::new(FatalError, "Root exists for slot to recycle"));
            }
        }

        match region {
            Some(region) => region.recycle_slot(slot, true, context),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// Move slot among regions
    pub fn move_slot(&self, from: Value, to_region_id: u32, context: &Box<dyn Context>) -> Result<Value, Error> {

        let _guard = context.get_slot_layout_token().lock_write();

        let from_region_id = from.get_region_id()?;
        let from_region = {
            let _guard = self.region_rw_lock.lock_read();
            let regions = self.regions.borrow();
            let region = regions.get(from_region_id as usize);
            if region.is_none() {
                return Err(Error::new(FatalError, "Region of slot not found"));
            }
            region.unwrap().clone()
        };

        let (snapshot, in_nursery, reference_map, removed_values, removed_symbols) = from_region.freeze_slot(from)?;

        let to_region = {
            let _guard = self.region_rw_lock.lock_read();
            let regions = self.regions.borrow();
            let region = regions.get(to_region_id as usize);
            if region.is_none() {
                return Err(Error::new(FatalError, "Region to move slot into not found"));
            }
            region.unwrap().clone()
        };

        let (to, added_values, added_symbols) = to_region.restore_slot(from, snapshot, in_nursery, &reference_map)?;

        for value in added_values {
            context.add_value_reference(to, value)?;
        }
        for symbol in added_symbols {
            context.add_symbol_reference(symbol)?;
        }

        let reference_map_is_none = reference_map.is_none();
        from_region.redirect_slot(from, to, reference_map)?;
        if reference_map_is_none {
            from_region.recycle_slot(from, false, context)?;
        }

        self.refresh_root(from, to)?;
        self.refresh_weak_root(from, to)?;

        for value in removed_values {
            context.remove_value_reference(from, value)?;
        }
        for symbol in removed_symbols {
            context.remove_symbol_reference(symbol)?;
        }

        Ok(to)

    }

    pub fn is_direct_value_alive(&self, value: Value, context: &Box<dyn Context>) -> Result<bool, Error> {

        let _guard = context.get_slot_layout_token().lock_read();

        let region_id = value.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            let regions = self.regions.borrow();
            let region = regions.get(region_id as usize);
            if region.is_none() {
                return Err(Error::new(FatalError, "Region of slot not found"));
            }
            region.unwrap().clone()
        };

        region.is_value_alive(value)

    }

    pub fn is_direct_value_occupied(&self, value: Value, context: &Box<dyn Context>) -> Result<bool, Error> {

        let _guard = context.get_slot_layout_token().lock_read();

        let region_id = value.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            let regions = self.regions.borrow();
            let region = regions.get(region_id as usize);
            if region.is_none() {
                return Err(Error::new(FatalError, "Region of slot not found"));
            }
            region.unwrap().clone()
        };

        region.is_value_occupied(value)

    }

    /// Notify a value is dropped from the isolate
    pub fn notify_slot_drop(&self, slot: Value) -> Result<(), Error> {

        let _guard = self.roots_rw_lock.lock_read();

        let weak_roots = self.weak_roots.borrow_mut().remove(&slot);
        match weak_roots {
            Some(weak_roots) => {
                for root in weak_roots.borrow().iter() {
                    root.notify_drop()?;
                }
            },
            None => {}
        }

        Ok(())

    }

}

/// Isolate value prototype getter and setter
impl Isolate {

    /// Get prototype of a value
    pub fn get_prototype(&self, slot: Value, context: &Box<dyn Context>) -> Result<Pinned, Error> {

        let layout_token = context.get_slot_layout_token();

        let layout_guard = layout_token.lock_read();

        let slot = self.resolve_real_value(slot, layout_token)?;

        match slot.get_primitive_type() {
            Undefined => {
                return Err(Error::new(VisitingUndefinedPrototype, "Undefined has no prototype"));
            },
            Null => {
                return Err(Error::new(VisitingNullPrototype, "Null has no prototype"));
            },
            Boolean => {
                return Pinned::new(context, self.boolean_prototype);
            },
            Integer => {
                return Pinned::new(context, self.integer_prototype);
            },
            Float => {
                return Pinned::new(context, self.float_prototype);
            },
            Symbol => {
                return Pinned::new(context, self.symbol_prototype);
            },
            Text => {
                return Pinned::new(context, self.text_prototype);
            },
            Tuple => {
                return Pinned::new(context, self.tuple_prototype);
            },
            List => {
                return Pinned::new(context, self.list_prototype);
            },
            Object => {}
        }

        let region_id = slot.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };

        match region {
            Some(region) => region.get_own_property_with_layout_guard(slot, slot, self.prototype_symbol, None, context, layout_guard, false),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// Set prototype of a value
    pub fn set_prototype(&self, slot: Value, prototype: Value, context: &Box<dyn Context>) -> Result<(), Error> {

        let layout_token = context.get_slot_layout_token();

        let layout_guard = layout_token.lock_read();

        let slot = self.resolve_real_value(slot, layout_token)?;

        match slot.get_primitive_type() {
            Undefined => Err(Error::new(MutatingUndefinedPrototype, "Undefined has no prototype")),
            Null => Err(Error::new(MutatingNullPrototype, "Null has no prototype")),
            Boolean => Err(Error::new(MutatingSealedPrototype, "Prototype of boolean is immutable")),
            Integer => Err(Error::new(MutatingSealedPrototype, "Prototype of integer is immutable")),
            Float => Err(Error::new(MutatingSealedPrototype, "Prototype of float is immutable")),
            Symbol => Err(Error::new(MutatingSealedPrototype, "Prototype of symbol is immutable")),
            Text => Err(Error::new(MutatingSealedPrototype, "Prototype of text is immutable")),
            Tuple => Err(Error::new(MutatingSealedPrototype, "Prototype of tuple is immutable")),
            List => Err(Error::new(MutatingSealedPrototype, "Prototype of list is immutable")),
            Object => {
                let region_id = slot.get_region_id()?;
                let region = {
                    let _guard = self.region_rw_lock.lock_read();
                    match self.regions.borrow().get(region_id as usize) {
                        Some(region) => Some(region.clone()),
                        None => None
                    }
                };
                match region {
                    Some(region) => region.set_prototype_with_layout_guard(slot, prototype, context, layout_guard, false),
                    None => Err(Error::new(FatalError, "Region of slot not found"))
                }
            }
        }

    }

    pub fn set_prototype_ignore_slot_trap(&self, slot: Value, prototype: Value, context: &Box<dyn Context>) -> Result<(), Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let region_id = slot.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.set_prototype_ignore_slot_trap(slot, prototype, context),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }
}

// Isolate value slot trap management
impl Isolate {

    pub fn has_slot_trap(&self, slot: Value, context: &Box<dyn Context>) -> Result<bool, Error> {
 
        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let slot = self.resolve_real_value(slot, layout_token)?;

        match slot.get_primitive_type() {
            Undefined => Err(Error::new(MutatingUndefinedPrototype, "Undefined has no slot trap supports")),
            Null => Err(Error::new(MutatingNullPrototype, "Null has no slot trap supports")),
            Boolean => Ok(false),
            Integer => Ok(false),
            Float => Ok(false),
            Symbol => Ok(false),
            Text => Ok(false),
            Tuple => Ok(false),
            List => Ok(false),
            Object => {
                let region_id = slot.get_region_id()?;
                let region = {
                    let _guard = self.region_rw_lock.lock_read();
                    match self.regions.borrow().get(region_id as usize) {
                        Some(region) => Some(region.clone()),
                        None => None
                    }
                };
                match region {
                    Some(region) => region.has_slot_trap(slot),
                    None => Err(Error::new(FatalError, "Region of slot not found"))
                }
            }
        }

    }

    /// Set slot trap of a value
    pub fn set_slot_trap(&self, slot: Value, slot_trap: Arc<dyn SlotTrap>, context: &Box<dyn Context>) -> Result<(), Error> {
 
        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let slot = self.resolve_real_value(slot, layout_token)?;

        match slot.get_primitive_type() {
            Undefined => Err(Error::new(MutatingUndefinedProperty, "Undefined has no slot trap support")),
            Null => Err(Error::new(MutatingNullProperty, "Null has no slot trap support")),
            Boolean => Err(Error::new(MutatingSealedProperty, "Slot trap of boolean is immutable")),
            Integer => Err(Error::new(MutatingSealedProperty, "Slot trap of integer is immutable")),
            Float => Err(Error::new(MutatingSealedProperty, "Slot trap of float is immutable")),
            Symbol => Err(Error::new(MutatingSealedProperty , "Slot trap of symbol is immutable")),
            Text => Err(Error::new(MutatingSealedProperty, "Slot trap of text is immutable")),
            Tuple => Err(Error::new(MutatingSealedProperty, "Slot trap of tuple is immutable")),
            List => Err(Error::new(MutatingSealedProperty, "Slot trap of list is immutable")),
            Object => {
                let region_id = slot.get_region_id()?;
                let region = {
                    let _guard = self.region_rw_lock.lock_read();
                    match self.regions.borrow().get(region_id as usize) {
                        Some(region) => Some(region.clone()),
                        None => None
                    }
                };
                match region {
                    Some(region) => region.set_slot_trap(slot, slot_trap, context),
                    None => Err(Error::new(FatalError, "Region of slot not found"))
                }
            }
        }

    }

    /// Clear slot trap of a value
    pub fn clear_slot_trap(&self, slot: Value, context: &Box<dyn Context>) -> Result<(), Error> {
 
        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let slot = self.resolve_real_value(slot, layout_token)?;

        match slot.get_primitive_type() {
            Undefined => Err(Error::new(MutatingUndefinedProperty, "Undefined has no slot trap support")),
            Null => Err(Error::new(MutatingNullProperty, "Null has no slot trap support")),
            Boolean => Err(Error::new(MutatingSealedProperty, "Slot trap of boolean is immutable")),
            Integer => Err(Error::new(MutatingSealedProperty, "Slot trap of integer is immutable")),
            Float => Err(Error::new(MutatingSealedProperty, "Slot trap of float is immutable")),
            Symbol => Err(Error::new(MutatingSealedProperty , "Slot trap of symbol is immutable")),
            Text => Err(Error::new(MutatingSealedProperty, "Slot trap of text is immutable")),
            Tuple => Err(Error::new(MutatingSealedProperty, "Slot trap of tuple is immutable")),
            List => Err(Error::new(MutatingSealedProperty, "Slot trap of list is immutable")),
            Object => {
                let region_id = slot.get_region_id()?;
                let region = {
                    let _guard = self.region_rw_lock.lock_read();
                    match self.regions.borrow().get(region_id as usize) {
                        Some(region) => Some(region.clone()),
                        None => None
                    }
                };
                match region {
                    Some(region) => region.clear_slot_trap(slot, context),
                    None => Err(Error::new(FatalError, "Region of slot not found"))
                }
            }
        }

    }

}

/// Isolate object internal slot management
impl Isolate {

    pub fn list_internal_slot_ids(&self, subject: Value, context: &Box<dyn Context>) -> Result<Vec<u64>, Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let subject = self.resolve_real_value(subject, layout_token)?;

        match subject.get_primitive_type() {
            Undefined => { return Err(Error::new(MutatingUndefinedProperty, "Undefined is immutable")); },
            Null => { return Err(Error::new(MutatingNullProperty, "Null is immutable")); },
            Boolean => { return Ok(Vec::new()); },
            Integer => { return Ok(Vec::new()); },
            Float => { return Ok(Vec::new()); },
            Symbol => { return Ok(Vec::new()); },
            Text => {},
            List => {},
            Tuple => {},
            Object => {} 
        }

        let region_id = subject.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.list_internal_slot_ids(subject),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    pub fn has_internal_slot(&self, subject: Value, index: u64, context: &Box<dyn Context>) -> Result<bool, Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let subject = self.resolve_real_value(subject, layout_token)?;

        match subject.get_primitive_type() {
            Undefined => { return Err(Error::new(MutatingUndefinedProperty, "Undefined is immutable")); },
            Null => { return Err(Error::new(MutatingNullProperty, "Null is immutable")); },
            Boolean => { return Ok(false); },
            Integer => { return Ok(false); },
            Float => { return Ok(false); },
            Symbol => { return Ok(false); },
            Text => {},
            List => {},
            Tuple => {},
            Object => {} 
        }

        let region_id = subject.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.has_internal_slot(subject, index),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// Set a specified internal slot of a value
    pub fn set_internal_slot(&self, subject: Value, index: u64, internal_slot: Arc<dyn InternalSlot>, context: &Box<dyn Context>) -> Result<(), Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let subject = self.resolve_real_value(subject, layout_token)?;

        match subject.get_primitive_type() {
            Undefined => { return Err(Error::new(MutatingUndefinedProperty, "Undefined is immutable")); },
            Null => { return Err(Error::new(MutatingNullProperty, "Null is immutable")); },
            Boolean => { return Err(Error::new(MutatingSealedProperty, "Boolean is immutable")); },
            Integer => { return Err(Error::new(MutatingSealedProperty, "Integer is immutable")); },
            Float => { return Err(Error::new(MutatingSealedProperty, "Float is immutable")); },
            Symbol => { return Err(Error::new(MutatingSealedProperty, "Symbol is immutable")); },
            Text => {},
            List => {},
            Tuple => {},
            Object => {} 
        }

        let region_id = subject.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => {
                region.set_internal_slot(subject, index, internal_slot, context)
            },
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// Clear a specified internal slot of a value
    pub fn clear_internal_slot(&self, subject: Value, index: u64, context: &Box<dyn Context>) -> Result<(), Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let subject = self.resolve_real_value(subject, layout_token)?;

        match subject.get_primitive_type() {
            Undefined => { return Err(Error::new(MutatingUndefinedProperty, "Undefined is immutable")); },
            Null => { return Err(Error::new(MutatingNullProperty, "Null is immutable")); },
            Boolean => { return Err(Error::new(MutatingSealedProperty, "Boolean is immutable")); },
            Integer => { return Err(Error::new(MutatingSealedProperty, "Integer is immutable")); },
            Float => { return Err(Error::new(MutatingSealedProperty, "Float is immutable")); },
            Symbol => { return Err(Error::new(MutatingSealedProperty, "Symbol is immutable")); },
            Text => {},
            List => {},
            Tuple => {},
            Object => {} 
        };

        let region_id = subject.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.clear_internal_slot(subject, index, context),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// Get specified internal slot from a value
    pub fn get_internal_slot<'a>(&self, subject: Value, index: u64, context: &'a Box<dyn Context>) -> Result<Option<ProtectedInternalSlot::<'a>>, Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let subject = self.resolve_real_value(subject, layout_token)?;

        match subject.get_primitive_type() {
            Undefined => { return Err(Error::new(MutatingUndefinedProperty, "Undefined is immutable")); },
            Null => { return Err(Error::new(MutatingNullProperty, "Null is immutable")); },
            Boolean => { return Err(Error::new(MutatingSealedProperty, "Boolean is immutable")); },
            Integer => { return Err(Error::new(MutatingSealedProperty, "Integer is immutable")); },
            Float => { return Err(Error::new(MutatingSealedProperty, "Float is immutable")); },
            Symbol => { return Err(Error::new(MutatingSealedProperty, "Symbol is immutable")); },
            Text => {},
            List => {},
            Tuple => {},
            Object => {} 
        };

        let region_id = subject.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.get_internal_slot(subject, index, context),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

}

impl Isolate {

    pub fn get_field_shortcuts(&self, subject: Value, context: &Box<dyn Context>) -> Result<Option<Arc<FieldShortcuts>>, Error> {

        let layout_token = context.get_slot_layout_token();

        let _layout_guard = layout_token.lock_read();

        let subject = self.resolve_real_value(subject, layout_token)?;

        match subject.get_primitive_type() {
            Undefined => { return Err(Error::new(VisitingUndefinedProperty, "Undefined has no properties")); },
            Null => { return Err(Error::new(VisitingNullProperty, "Null has no properties")); },
            Boolean => { return Ok(None); },
            Integer => { return Ok(None); },
            Float => { return Ok(None); },
            Symbol => { return Ok(None); },
            Text => { return Ok(None); },
            List => {},
            Tuple => {},
            Object => {} 
        }

        let region_id = subject.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.get_field_shortcuts(subject),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    pub fn has_field_shortcuts(&self, subject: Value, context: &Box<dyn Context>) -> Result<bool, Error> {

        let layout_token = context.get_slot_layout_token();

        let _layout_guard = layout_token.lock_read();

        let subject = self.resolve_real_value(subject, layout_token)?;

        match subject.get_primitive_type() {
            Undefined => { return Err(Error::new(VisitingUndefinedProperty, "Undefined has no properties")); },
            Null => { return Err(Error::new(VisitingNullProperty, "Null has no properties")); },
            Boolean => { return Ok(false); },
            Integer => { return Ok(false); },
            Float => { return Ok(false); },
            Symbol => { return Ok(false); },
            Text => { return Ok(false); },
            List => {},
            Tuple => {},
            Object => {} 
        }

        let region_id = subject.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.has_field_shortcuts(subject),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    pub fn update_field_shortcuts(&self, subject: Value, field_shortcuts: Arc<FieldShortcuts>, context: &Box<dyn Context>) -> Result<(), Error> {

        let layout_token = context.get_slot_layout_token();

        let _layout_guard = layout_token.lock_read();

        let subject = self.resolve_real_value(subject, layout_token)?;

        match subject.get_primitive_type() {
            Undefined => { return Err(Error::new(MutatingUndefinedProperty, "Undefined has no properties")); },
            Null => { return Err(Error::new(MutatingNullProperty, "Null has no properties")); },
            Boolean => { return Err(Error::new(MutatingSealedProperty, "Boolean value is immutable")); },
            Integer => { return Err(Error::new(MutatingSealedProperty, "Integer value is immutable")); },
            Float => { return Err(Error::new(MutatingSealedProperty, "Float value is immutable")); },
            Symbol => { return Err(Error::new(MutatingSealedProperty, "Symbol value is immutable")); },
            Text => { return Err(Error::new(MutatingSealedProperty, "Text value is immutable")); },
            List => {},
            Tuple => {},
            Object => {} 
        }

        let region_id = subject.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.update_field_shortcuts(subject, field_shortcuts),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }
    }

    pub fn clear_field_shortcuts(&self, subject: Value, context: &Box<dyn Context>) -> Result<(), Error> {

        let layout_token = context.get_slot_layout_token();

        let _layout_guard = layout_token.lock_read();

        let subject = self.resolve_real_value(subject, layout_token)?;

        match subject.get_primitive_type() {
            Undefined => { return Err(Error::new(MutatingUndefinedProperty, "Undefined has no properties")); },
            Null => { return Err(Error::new(MutatingNullProperty, "Null has no properties")); },
            Boolean => { return Err(Error::new(MutatingSealedProperty, "Boolean value is immutable")); },
            Integer => { return Err(Error::new(MutatingSealedProperty, "Integer value is immutable")); },
            Float => { return Err(Error::new(MutatingSealedProperty, "Float value is immutable")); },
            Symbol => { return Err(Error::new(MutatingSealedProperty, "Symbol value is immutable")); },
            Text => { return Err(Error::new(MutatingSealedProperty, "Text value is immutable")); },
            List => {},
            Tuple => {},
            Object => {} 
        }

        let region_id = subject.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.clear_field_shortcuts(subject),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }
    }

}

/// Isolate object own property management
impl Isolate {

    /// Get own property of a value for a symbol
    pub fn get_own_property(&self, id: Value, subject: Value, symbol: Symbol, field_token: Option<&FieldToken>, context: &Box<dyn Context>) -> Result<Pinned, Error> {
        
        let layout_token = context.get_slot_layout_token();

        let layout_guard = layout_token.lock_read();

        let id = self.resolve_real_value(id, layout_token)?;

        match id.get_primitive_type() {
            Undefined => { return Err(Error::new(VisitingUndefinedProperty, "Undefined has no properties")); },
            Null => { return Err(Error::new(VisitingNullProperty, "Null has no properties")); },
            Boolean => { return Pinned::new(context, Value::make_undefined()); },
            Integer => { return Pinned::new(context, Value::make_undefined()); },
            Float => { return Pinned::new(context, Value::make_undefined()); },
            Symbol => { return Pinned::new(context, Value::make_undefined()); },
            Text => { return Pinned::new(context, Value::make_undefined()); },
            List => {},
            Tuple => {},
            Object => {} 
        }

        let region_id = id.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.get_own_property_with_layout_guard(id, subject, symbol, field_token, context, layout_guard, false),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    pub fn get_own_property_ignore_slot_trap(&self, id: Value, subject: Value, symbol: Symbol, context: &Box<dyn Context>) -> Result<Pinned, Error> {
 
        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let id = self.resolve_real_value(id, layout_token)?;

        let region_id = id.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };

        match region {
            Some(region) => region.get_own_property_ignore_slot_trap(id, subject, symbol, context),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// Set own property of a value for a symbol
    pub fn set_own_property(&self, id: Value, subject: Value, symbol: Symbol, value: Value, context: &Box<dyn Context>) -> Result<(), Error> {

        let layout_token = context.get_slot_layout_token();

        let layout_guard = layout_token.lock_read();

        let id = self.resolve_real_value(id, layout_token)?;

        match id.get_primitive_type() {
            Undefined => { return Err(Error::new(MutatingUndefinedProperty, "Undefined is immutable")); },
            Null => { return Err(Error::new(MutatingNullProperty, "Null is immutable")); },
            Boolean => { return Err(Error::new(MutatingSealedProperty, "Boolean is immutable")); },
            Integer => { return Err(Error::new(MutatingSealedProperty, "Integer is immutable")); },
            Float => { return Err(Error::new(MutatingSealedProperty, "Float is immutable")); },
            Symbol => { return Err(Error::new(MutatingSealedProperty, "Symbol is immutable")); },
            Text => { return Err(Error::new(MutatingSealedProperty, "Text is immutable")); },
            List => {},
            Tuple => {},
            Object => {} 
        }

        let region_id = id.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.set_own_property_with_layout_guard(id, subject, symbol, value, context, layout_guard, false),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// Set own property of a value for a symbol
    pub fn set_own_property_ignore_slot_trap(&self, id: Value, subject: Value, symbol: Symbol, value: Value, context: &Box<dyn Context>) -> Result<(), Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let id = self.resolve_real_value(id, layout_token)?;

        let region_id = id.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.set_own_property_ignore_slot_trap(id, subject, symbol, value, context),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// Define own property of a value for a symbol
    pub fn define_own_property(&self, id: Value, subject: Value, symbol: Symbol, property_trap: Arc<dyn PropertyTrap>, context: &Box<dyn Context>) -> Result<(), Error> {
        
        let layout_token = context.get_slot_layout_token();

        let layout_guard = layout_token.lock_read();

        let id = self.resolve_real_value(id, layout_token)?;

        match id.get_primitive_type() {
            Undefined => { return Err(Error::new(MutatingUndefinedProperty, "Undefined is immutable")); },
            Null => { return Err(Error::new(MutatingNullProperty, "Null is immutable")); },
            Boolean => { return Err(Error::new(MutatingSealedProperty, "Boolean is immutable")); },
            Integer => { return Err(Error::new(MutatingSealedProperty, "Integer is immutable")); },
            Float => { return Err(Error::new(MutatingSealedProperty, "Float is immutable")); },
            Symbol => { return Err(Error::new(MutatingSealedProperty, "Symbol is immutable")); },
            Text => { return Err(Error::new(MutatingSealedProperty, "Text is immutable")); },
            List => {},
            Tuple => {},
            Object => {} 
        }

        let region_id = id.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.define_own_property_with_layout_guard(id, subject, symbol, property_trap, context, layout_guard, false),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// Define own property of a value for a symbol
    pub fn define_own_property_ignore_slot_trap(&self, id: Value, subject: Value, symbol: Symbol, property_trap: Arc<dyn PropertyTrap>, context: &Box<dyn Context>) -> Result<(), Error> {
        
        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let id = self.resolve_real_value(id, layout_token)?;

        let region_id = id.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.define_own_property_ignore_slot_trap(id, subject, symbol, property_trap, context),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// Delete own property from a value for a symbol
    pub fn delete_own_property(&self, id: Value, subject: Value, symbol: Symbol, context: &Box<dyn Context>) -> Result<(), Error> {
 
        let layout_token = context.get_slot_layout_token();

        let layout_guard = layout_token.lock_read();

        let id = self.resolve_real_value(id, layout_token)?;

        match id.get_primitive_type() {
            Undefined => { return Err(Error::new(MutatingUndefinedProperty, "Undefined is immutable")); },
            Null => { return Err(Error::new(MutatingNullProperty, "Null is immutable")); },
            Boolean => { return Err(Error::new(MutatingSealedProperty, "Boolean is immutable")); },
            Integer => { return Err(Error::new(MutatingSealedProperty, "Integer is immutable")); },
            Float => { return Err(Error::new(MutatingSealedProperty, "Float is immutable")); },
            Symbol => { return Err(Error::new(MutatingSealedProperty, "Symbol is immutable")); },
            Text => { return Err(Error::new(MutatingSealedProperty, "Text is immutable")); },
            List => {},
            Tuple => {},
            Object => {}
        }

        let region_id = id.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.delete_own_property_with_layout_guard(id, subject, symbol, context, layout_guard, false),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// Delete own property from a value for a symbol
    pub fn delete_own_property_ignore_slot_trap(&self, id: Value, subject: Value, symbol: Symbol, context: &Box<dyn Context>) -> Result<(), Error> {
 
        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let id = self.resolve_real_value(id, layout_token)?;

        let region_id = id.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.delete_own_property_ignore_slot_trap(id, subject, symbol, context),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// Check whether an own property of a value for a symbol exists
    pub fn has_own_property(&self, id: Value, subject: Value, symbol: Symbol, context: &Box<dyn Context>) -> Result<bool, Error> {

        let layout_token = context.get_slot_layout_token();

        let layout_guard = layout_token.lock_read();

        let id = self.resolve_real_value(id, layout_token)?;

        match id.get_primitive_type() {
            Undefined => { return Err(Error::new(VisitingUndefinedProperty, "Undefined has no properties")); },
            Null => { return Err(Error::new(VisitingNullProperty, "Null has no properties")); },
            Boolean => { return Ok(false); },
            Integer => { return Ok(false); },
            Float => { return Ok(false); },
            Text => { return Ok(false); },
            Symbol => { return Ok(false); },
            List => {},
            Tuple => {},
            Object => {}
        }

        let region_id = id.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };
        match region {
            Some(region) => region.has_own_property_with_layout_guard(id, subject, symbol, context, layout_guard),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// List own property symbols of a value
    pub fn list_own_property_symbols(&self, id: Value, subject: Value, context: &Box<dyn Context>) -> Result<HashSet<Symbol>, Error> {

        let layout_token = context.get_slot_layout_token();

        let layout_guard = layout_token.lock_read();

        let id = self.resolve_real_value(id, layout_token)?;

        match id.get_primitive_type() {
            Undefined => { return Err(Error::new(VisitingUndefinedProperty, "Undefined has no properties")); },
            Null => { return Err(Error::new(VisitingNullProperty, "Null has no properties")); },
            Boolean => { return Ok(HashSet::new()); },
            Integer => { return Ok(HashSet::new()); },
            Float => { return Ok(HashSet::new()); },
            Symbol => { return Ok(HashSet::new()); },
            Text => { return Ok(HashSet::new()); },
            List => {},
            Tuple => {},
            Object =>{} 
        }

        let region_id = id.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };

        match region {
            Some(region) => {
                let mut hash_set = HashSet::new();
                for value in region.list_own_property_symbols_with_layout_guard(id, subject, context, layout_guard, false)?.iter() {
                    hash_set.insert(*value);
                }
                Ok(hash_set)
            },
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    /// List own property symbols of a value
    pub fn list_own_property_symbols_ignore_slot_trap(&self, id: Value, subject: Value, context: &Box<dyn Context>) -> Result<HashSet<Symbol>, Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let id = self.resolve_real_value(id, layout_token)?;

        let region_id = id.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };

        match region {
            Some(region) => {
                let mut hash_set = HashSet::new();
                for value in region.list_own_property_symbols_ignore_slot_trap(id, subject, context)?.iter() {
                    hash_set.insert(*value);
                }
                Ok(hash_set)
            },
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

}

/// Isolate object property managment
impl Isolate {

    /// List property symbols of a value
    pub fn list_property_symbols(&self, subject: Value, context: &Box<dyn Context>) -> Result<HashSet<Symbol>, Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let subject = self.resolve_real_value(subject, layout_token)?;

        match subject.get_primitive_type() {
            Undefined => { return Err(Error::new(VisitingUndefinedProperty, "Undefined has no properties")); },
            Null => { return Err(Error::new(VisitingNullProperty, "Null has no properties")); },
            Boolean => {},
            Integer => {},
            Float => {},
            Symbol => {},
            Text => {},
            List => {},
            Tuple => {},
            Object => {} 
        }

        let mut hash_set = HashSet::new();

        let mut prototype = subject;
        while !prototype.is_nil() {
            for value in self.list_own_property_symbols(prototype, subject, context)?.iter() {
                hash_set.insert(*value);
            }
            prototype = self.get_prototype(prototype, context)?.get_value();
        }

        Ok(hash_set)

    }

    /// Check whether an property of a value for a symbol exists
    pub fn has_property(&self, subject: Value, symbol: Symbol, context: &Box<dyn Context>) -> Result<bool, Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let subject = self.resolve_real_value(subject, layout_token)?;

        match subject.get_primitive_type() {
            Undefined => { return Err(Error::new(VisitingUndefinedProperty, "Undefined has no properties")); },
            Null => { return Err(Error::new(VisitingNullProperty, "Null has no properties")); },
            Boolean => {},
            Integer => {},
            Float => {},
            Symbol => {},
            Text => {},
            List => {},
            Tuple => {},
            Object => {} 
        }

        let mut prototype = subject;
        while !prototype.is_nil() {
            if self.has_own_property(prototype, subject, symbol, context)? {
                return Ok(true);
            }
            prototype = self.get_prototype(prototype, context)?.get_value();
        } 

        Ok(false)

    }
    
    /// Get property of a value for a symbol
    pub fn get_property(&self, subject: Value, symbol: Symbol, field_token: Option<&FieldToken>, context: &Box<dyn Context>) -> Result<Pinned, Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let subject = self.resolve_real_value(subject, layout_token)?;

        match subject.get_primitive_type() {
            Undefined => { return Err(Error::new(VisitingUndefinedProperty, "Undefined has no properties")); },
            Null => { return Err(Error::new(VisitingNullProperty, "Null has no properties")); },
            Boolean => {},
            Integer => {},
            Float => {},
            Symbol => {},
            Text => {},
            List => {},
            Tuple => {},
            Object => {} 
        }

        let mut prototype = subject;
        while !prototype.is_nil() {
            let value = self.get_own_property(prototype, subject, symbol, field_token, context)?;
            if !value.is_undefined() {
                return Ok(value);
            }
            prototype = self.get_prototype(prototype, context)?.get_value();
        } 
        
        Pinned::new(context, Value::make_undefined())

    }

}

impl Isolate {

    pub fn is_sealed(&self, value: Value, context: &Box<dyn Context>) -> Result<bool, Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let value = self.resolve_real_value(value, layout_token)?;

        match value.get_primitive_type() {
            Undefined => { return Err(Error::new(VisitingUndefinedProperty, "Undefined has no feature for seal")); },
            Null => { return Err(Error::new(VisitingNullProperty, "Null has no feature for seal")); },
            Boolean => { return Ok(true); },
            Integer => { return Ok(true); },
            Float => { return Ok(true); },
            Symbol => { return Ok(true); },
            Text => {return Ok(true); },
            List => {},
            Tuple => {},
            Object => {} 
        }

        let region_id = value.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };

        match region {
            Some(region) => region.is_sealed(value),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

    pub fn seal_slot(&self, value: Value, context: &Box<dyn Context>) -> Result<(), Error> {

        let layout_token = context.get_slot_layout_token();

        let _guard = layout_token.lock_read();

        let value = self.resolve_real_value(value, layout_token)?;

        match value.get_primitive_type() {
            Undefined => { return Err(Error::new(VisitingUndefinedProperty, "Undefined has no feature for seal")); },
            Null => { return Err(Error::new(VisitingNullProperty, "Null has no feature for seal")); },
            Boolean => { return Ok(()); },
            Integer => { return Ok(()); },
            Float => { return Ok(()); },
            Symbol => { return Ok(()); },
            Text => {return Ok(()); },
            List => {},
            Tuple => {},
            Object => {} 
        }

        let region_id = value.get_region_id()?;
        let region = {
            let _guard = self.region_rw_lock.lock_read();
            match self.regions.borrow().get(region_id as usize) {
                Some(region) => Some(region.clone()),
                None => None
            }
        };

        match region {
            Some(region) => region.seal_slot(value),
            None => Err(Error::new(FatalError, "Region of slot not found"))
        }

    }

}

/// Isolate outlet management
impl Isolate {

    /// Set the outlet with specified ID
    pub fn add_outlet(&self, outlet: Arc<dyn Any>) -> u64 {

        let _guard = self.outlets_rw_lock.lock_write();

        let id = self.next_outlet_id.fetch_add(1, Ordering::SeqCst);

        self.outlets.borrow_mut().insert(id, outlet);

        id

    }

    /// Get the outlet with specified ID
    pub fn get_outlet(&self, id: u64) -> Option<Arc<dyn Any>> {

        let _guard = self.outlets_rw_lock.lock_read();

        match self.outlets.borrow().get(&id) {
            None => None,
            Some(outlet) => Some(outlet.clone())
        }

    }

    /// Remove the outlet with specified ID
    pub fn clear_outlet(&self, id: u64) -> Option<Arc<dyn Any>> {

        let _guard = self.outlets_rw_lock.lock_read();

        self.outlets.borrow_mut().remove(&id)

    }

}

#[cfg(test)] use super::test::TestContext2;

#[test]
fn test_isolate_creation() -> Result<(), Error> {
    Isolate::create()?;
    Ok(())
}

#[test]
fn test_isolate_text_symbol() -> Result<(), Error> {

    let isolate = Isolate::create()?;

    let test_2 = isolate.get_text_symbol("test", "test2");
    let test_2_2 = isolate.get_text_symbol("test", "test2");
    let test_2_3 = isolate.get_text_symbol("test", "test3");
    let test_3 = isolate.get_text_symbol("test2", "test3");

    assert_eq!(test_2, test_2_2);
    assert_ne!(test_2, test_2_3);
    assert_ne!(test_2, test_3);
    assert_ne!(test_2_3, test_3);

    let test_2_symbol_info = isolate.resolve_symbol_info(test_2)?;
    assert_eq!(test_2_symbol_info.get_symbol(), test_2);
    assert_eq!(test_2_symbol_info.get_symbol_scope().as_ref(), "test");
    assert!(test_2_symbol_info.is_text_symbol());
    assert!(!test_2_symbol_info.is_value_symbol());
    assert_eq!(test_2_symbol_info.get_text().unwrap().as_ref(), "test2");
    assert!(test_2_symbol_info.get_value().is_none());

    assert!(isolate.recycle_symbol(test_2).is_err());
    isolate.add_symbol_reference(test_2)?;
    assert!(isolate.recycle_symbol(test_2).is_err());
    isolate.remove_symbol_reference(test_2)?;
    assert!(isolate.recycle_symbol(test_2).is_ok());

    Ok(())
}

#[test]
fn test_isolate_value_symbol() -> Result<(), Error> {

    let isolate = Isolate::create()?;

    let test_2 = isolate.get_value_symbol("test", Value::make_null());
    let test_2_2 = isolate.get_value_symbol("test", Value::make_null());
    let test_2_3 = isolate.get_value_symbol("test", Value::make_float(4.0));
    let test_3 = isolate.get_value_symbol("test2", Value::make_float(4.0));

    assert_eq!(test_2, test_2_2);
    assert_ne!(test_2, test_2_3);
    assert_ne!(test_2, test_3);
    assert_ne!(test_2_3, test_3);

    let test_2_symbol_info = isolate.resolve_symbol_info(test_2)?;
    assert_eq!(test_2_symbol_info.get_symbol(), test_2);
    assert_eq!(test_2_symbol_info.get_symbol_scope().as_ref(), "test");
    assert!(!test_2_symbol_info.is_text_symbol());
    assert!(test_2_symbol_info.is_value_symbol());
    assert_eq!(test_2_symbol_info.get_value().unwrap(), Value::make_null());
    assert!(test_2_symbol_info.get_text().is_none());

    assert!(isolate.recycle_symbol(test_2).is_err());
    isolate.add_symbol_reference(test_2)?;
    assert!(isolate.recycle_symbol(test_2).is_err());
    isolate.remove_symbol_reference(test_2)?;
    assert!(isolate.recycle_symbol(test_2).is_ok());

    Ok(())
}

#[test]
fn test_isolate_region_management() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext2::new(isolate.clone()));

    let region_id = isolate.create_region()?;

    // region 0 is for builtin objects

    assert_eq!(region_id, 1);

    assert!(isolate.recycle_region(region_id).is_err());

    isolate.unprotect_region(region_id)?;
    isolate.recycle_region(region_id)?;

    let region_id = isolate.create_region()?;

    isolate.unprotect_region(region_id)?;

    assert_eq!(region_id, 2);

    let layout_token = isolate.create_slot_layout_token();

    let value = isolate.gain_slot(region_id, PrimitiveType::Object, Value::make_null(), &layout_token)?;

    assert!(isolate.recycle_region(region_id).is_err());

    assert!(isolate.recycle_slot(value, &context).is_err());

    isolate.add_value_reference(isolate.get_object_prototype(), value, &layout_token)?;

    assert!(isolate.recycle_slot(value, &context).is_err());

    isolate.remove_value_reference(isolate.get_object_prototype(), value, &layout_token)?;

    isolate.recycle_slot(value, &context)?;

    isolate.recycle_region(region_id)?;

    Ok(())

}

#[test]
fn test_isolate_slot_management() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext2::new(isolate.clone()));

    let layout_token = isolate.create_slot_layout_token();

    let region_id = isolate.create_region()?;

    let value = isolate.gain_slot(region_id, PrimitiveType::Object, Value::make_null(), &layout_token)?;
    let value_2 = isolate.gain_slot(region_id, PrimitiveType::Object, Value::make_null(), &layout_token)?;

    assert!(isolate.recycle_slot(value, &context).is_err());

    isolate.add_value_reference(value, value_2, &layout_token)?;

    isolate.add_value_reference(isolate.get_object_prototype(), value, &layout_token)?;

    assert!(isolate.recycle_slot(value, &context).is_err());

    isolate.remove_value_reference(isolate.get_object_prototype(), value, &layout_token)?;

    isolate.recycle_slot(value, &context)?;

    assert!(isolate.recycle_slot(value_2, &context).is_err());

    isolate.remove_value_reference(value, value_2, &layout_token)?;

    isolate.recycle_slot(value_2, &context)?;

    Ok(())

}

#[test]
fn test_isolate_slot_snapshot() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext2::new(isolate.clone()));

    let layout_token = isolate.create_slot_layout_token();

    let region_id = isolate.create_region()?;

    let value = isolate.gain_slot(region_id, PrimitiveType::Object, Value::make_null(), &layout_token)?;
    let value_slot = isolate.gain_slot(region_id, PrimitiveType::Object, Value::make_null(), &layout_token)?;
    let value_2 = isolate.gain_slot(region_id, PrimitiveType::Object, Value::make_null(), &layout_token)?;
    let value_3 = isolate.gain_slot(region_id, PrimitiveType::Object, Value::make_null(), &layout_token)?;

    isolate.move_value_out_from_nursery(value_slot, &layout_token)?;
    isolate.recycle_slot(value_slot, &context)?;

    let value_4 = isolate.move_slot(value_3, region_id, &context)?;

    assert_eq!(value_slot, value_4);

    assert!(isolate.is_direct_value_alive(value_slot, &context)?);
    assert!(!isolate.is_direct_value_alive(value_3, &context)?);
    assert!(isolate.resolve_real_value(value_3, &layout_token).is_err());

    let symbol = isolate.get_text_symbol("test", "test");

    isolate.set_own_property(value_2, value_2, symbol, value_4, &context)?;

    isolate.move_value_out_from_nursery(value, &layout_token)?;
    isolate.recycle_slot(value, &context)?;

    let value_5 = isolate.move_slot(value_4, region_id, &context)?;
    assert!(!isolate.is_direct_value_alive(value_4, &context)?);
    assert!(isolate.is_direct_value_occupied(value_4, &context)?);
    assert_eq!(isolate.resolve_real_value(value_4, &layout_token)?, value_5);
    assert_eq!(isolate.get_own_property(value_2, value_2, symbol, None, &context)?.get_value(), value_5);
    assert!(!isolate.is_direct_value_occupied(value_4, &context)?);

    Ok(())

}

#[test]
fn test_isolate_outlets() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let outlet: Arc<dyn Any> = Arc::new(Value::make_undefined());
    let outlet_2: Arc<dyn Any> = Arc::new(Value::make_null());

    let outlet_id = isolate.add_outlet(outlet.clone());
    let outlet_2_id = isolate.add_outlet(outlet_2.clone());

    assert!(Arc::ptr_eq(&isolate.get_outlet(outlet_id).unwrap(), &outlet));
    assert!(Arc::ptr_eq(&isolate.get_outlet(outlet_2_id).unwrap(), &outlet_2));

    isolate.clear_outlet(outlet_id);
    assert!(isolate.get_outlet(outlet_id).is_none());

    isolate.clear_outlet(outlet_2_id);
    assert!(isolate.get_outlet(outlet_2_id).is_none());

    Ok(())

}

#[test]
fn test_isolate_own_properties() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext2::new(isolate.clone()));

    let layout_token = isolate.create_slot_layout_token();

    let region_id = isolate.create_region()?;

    let value = isolate.gain_slot(region_id, PrimitiveType::Object, Value::make_null(), &layout_token)?;

    let symbol = isolate.get_text_symbol("test", "test");

    isolate.set_own_property(value, value, symbol, Value::make_float(3.14), &context)?;

    assert_eq!(isolate.get_own_property(value, value, symbol, None, &context)?.get_value(), Value::make_float(3.14));

    let symbols = isolate.list_own_property_symbols(value, value, &context)?;
    assert_eq!(symbols.len(), 2);
    assert!(symbols.get(&isolate.get_prototype_symbol()).is_some());
    assert!(symbols.get(&symbol).is_some());

    isolate.delete_own_property(value, value, symbol, &context)?;

    let symbols = isolate.list_own_property_symbols(value, value, &context)?;
    assert_eq!(symbols.len(), 1);
    assert!(symbols.get(&isolate.get_prototype_symbol()).is_some());

    assert_eq!(isolate.get_own_property(value, value, symbol, None, &context)?.get_value(), Value::make_undefined());

    Ok(())

}

#[test]
fn test_isolate_properties() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext2::new(isolate.clone()));

    let layout_token = isolate.create_slot_layout_token();

    let region_id = isolate.create_region()?;

    let prototype = isolate.gain_slot(region_id, PrimitiveType::Object, Value::make_null(), &layout_token)?;
    let value = isolate.gain_slot(region_id, PrimitiveType::Object, prototype, &layout_token)?;

    assert_eq!(isolate.get_prototype(value, &context)?.get_value(), prototype);

    let symbol = isolate.get_text_symbol("test", "test");

    isolate.set_own_property(prototype, prototype, symbol, Value::make_float(3.14), &context)?;

    assert_eq!(isolate.get_property(value, symbol, None, &context)?.get_value(), Value::make_float(3.14));
    assert_eq!(isolate.get_own_property(value, value, symbol, None, &context)?.get_value(), Value::make_undefined());

    let symbols = isolate.list_property_symbols(value, &context)?;
    assert_eq!(symbols.len(), 2);
    assert!(symbols.get(&isolate.get_prototype_symbol()).is_some());
    assert!(symbols.get(&symbol).is_some());

    let symbols = isolate.list_own_property_symbols(value, value, &context)?;
    assert_eq!(symbols.len(), 1);
    assert!(symbols.get(&isolate.get_prototype_symbol()).is_some());

    Ok(())

}

#[test]
fn test_isolate_seals() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext2::new(isolate.clone()));

    let layout_token = isolate.create_slot_layout_token();

    let region_id = isolate.create_region()?;

    let value = isolate.gain_slot(region_id, PrimitiveType::Object, Value::make_null(), &layout_token)?;

    assert!(!isolate.is_sealed(value, &context)?);

    isolate.seal_slot(value, &context)?;

    assert!(isolate.is_sealed(value, &context)?);

    Ok(())

}

#[test]
fn test_isolate_roots() -> Result<(), Error> {

    let isolate = Arc::new(Isolate::create()?);

    let context: Box<dyn Context> = Box::new(TestContext2::new(isolate.clone()));

    let layout_token = isolate.create_slot_layout_token();

    let region_id = isolate.create_region()?;

    let value = isolate.gain_slot(region_id, PrimitiveType::Object, Value::make_null(), &layout_token)?;

    let root = isolate.add_root(value, &layout_token)?;

    assert!(isolate.recycle_slot(value, &context).is_err());

    isolate.remove_root(&root)?;

    isolate.recycle_slot(value, &context)?;

    let value = isolate.gain_slot(region_id, PrimitiveType::Object, Value::make_null(), &layout_token)?;

    let root = isolate.add_root(value, &layout_token)?;
    let weak_root = isolate.add_weak_root(value, None, &layout_token)?;

    let value_2 = isolate.move_slot(value, region_id, &context)?;

    assert!(isolate.recycle_slot(value, &context).is_err());

    assert!(isolate.recycle_slot(value_2, &context).is_err());

    assert!(!weak_root.is_dropped());

    isolate.remove_root(&root)?;

    isolate.recycle_slot(value_2, &context)?;

    assert!(weak_root.is_dropped());

    Ok(())

}