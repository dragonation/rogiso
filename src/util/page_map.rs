use std::collections::HashSet;

use std::mem::MaybeUninit;

use std::ops::Index;

use super::super::base::Error;
use super::super::base::ErrorType::*;


const MAX_PAGE_SHIFT: usize = 10;
const MAX_PAGE_ITEMS: usize = 1 << MAX_PAGE_SHIFT;
const MAX_TABLE_ITEMS: usize = MAX_PAGE_ITEMS - 8;

const MAX_ITEMS: usize = (((MAX_TABLE_ITEMS << MAX_PAGE_SHIFT) + MAX_PAGE_ITEMS) << MAX_PAGE_SHIFT) + MAX_PAGE_ITEMS;

pub trait PageItemFactory<T> {
    fn create_item(&self, id: usize) -> Box<T>;
}


pub struct PageIterator<'a, T, F: PageItemFactory<T>> {
    index_iterator: std::collections::hash_set::Iter<'a, usize>,
    page_map: &'a PageMap<T, F>
}

impl<'a, T, F: PageItemFactory<T>> Iterator for PageIterator<'a, T, F> {

    type Item = (usize, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        match self.index_iterator.next() {
            None => None,
            Some(index) => Some((*index, &self.page_map[*index]))
        }
    }

}


struct Page<T> where {
    items: [Option<Box<T>>; MAX_PAGE_ITEMS]
}

impl<T> Page<T> {}


struct PageTable<T> {
    pages: [Option<Box<Page<T>>>; MAX_PAGE_ITEMS]
}


pub struct PageMap<T, F: PageItemFactory<T>> {
    page_item_factory: F,
    next_index: u32,
    size: u32,
    tables: [Option<Box<PageTable<T>>>; MAX_TABLE_ITEMS],
    occupieds: HashSet<usize>
}

impl<T, F: PageItemFactory<T>> PageMap<T, F> {

    pub fn new(page_item_factory: F) -> PageMap<T, F> {
        PageMap {
            page_item_factory: page_item_factory,
            next_index: 0,
            size: 0,
            tables: {
                let mut array: [MaybeUninit<Option<Box<PageTable<T>>>>; MAX_TABLE_ITEMS] = unsafe { 
                    MaybeUninit::uninit().assume_init() 
                };
                for (_, slot) in array.iter_mut().enumerate() {
                    let table: Option<Box<PageTable<T>>> = None;
                    *slot = MaybeUninit::new(table);
                }
                unsafe { 
                    std::mem::transmute::<_, [Option<Box<PageTable<T>>>; MAX_TABLE_ITEMS]>(array) 
                }
            },
            occupieds: HashSet::new()
        }
    }

    pub fn gain_item<'a>(&'a mut self) -> Result<usize, Error> {

        let index = loop {
            let index = self.next_index as usize;
            if index >= MAX_ITEMS {
                return Err(Error::new(OutOfSpace, "No more space is available"));
            }
            self.next_index += 1;
            if self.get(index).is_none() {
                break index;
            }
        };

        let table_index = (index >> (MAX_PAGE_SHIFT << 1)) & (MAX_PAGE_ITEMS - 1);
        let table_is_none = self.tables[table_index].is_none();
        if table_is_none {
            self.tables[table_index] = Some(Box::new(PageTable {
                pages: {
                    let mut array: [MaybeUninit<Option<Box<Page<T>>>>; MAX_PAGE_ITEMS] = unsafe { 
                        MaybeUninit::uninit().assume_init() 
                    };
                    for (_, slot) in array.iter_mut().enumerate() {
                        let page: Option<Box<Page<T>>> = None;
                        *slot = MaybeUninit::new(page);
                    }
                    unsafe { 
                        std::mem::transmute::<_, [Option<Box<Page<T>>>; MAX_PAGE_ITEMS]>(array) 
                    }
                }
            }));
        }
        let mut table: Option<&'a mut Box<PageTable<T>>> = Option::from(&mut self.tables[table_index]);

        let page_index = (index >> MAX_PAGE_SHIFT) & (MAX_PAGE_ITEMS - 1);
        let page_is_none = table.as_ref().unwrap().pages[page_index].is_none();
        if page_is_none {
            table.as_mut().unwrap().pages[page_index] = Some(Box::new(Page {
                items: {
                    let mut array: [MaybeUninit<Option<Box<T>>>; MAX_PAGE_ITEMS] = unsafe { 
                        MaybeUninit::uninit().assume_init() 
                    };
                    for (_, slot) in array.iter_mut().enumerate() {
                        let page: Option<Box<T>> = None;
                        *slot = MaybeUninit::new(page);
                    }
                    unsafe { 
                        std::mem::transmute::<_, [Option<Box<T>>; MAX_PAGE_ITEMS]>(array) 
                    }
                }
            }));
        }
        let mut page: Option<&'a mut Box<Page<T>>> = Option::from(&mut table.unwrap().pages[page_index]);

        let item_index = index & (MAX_PAGE_ITEMS - 1);
        let item_is_none = page.as_ref().unwrap().items[item_index].is_none();
        if item_is_none {
            page.as_mut().unwrap().items[item_index] = Some(self.page_item_factory.create_item(index));
        }

        self.size += 1;

        self.occupieds.insert(index);

        Ok(index)

    }

    pub fn recycle_item<'a>(&'a mut self, index: usize) -> Result<(), Error> {

        let table_index = (index >> (MAX_PAGE_SHIFT << 1)) & (MAX_PAGE_ITEMS - 1);
        let table: Option<&'a mut Box<PageTable<T>>> = Option::from(&mut self.tables[table_index]);
        if table.is_none() {
            return Err(Error::new(FatalError, "Item not found"));
        }

        let page_index = (index >> MAX_PAGE_SHIFT) & (MAX_PAGE_ITEMS - 1);
        let mut page: Option<&'a mut Box<Page<T>>> = Option::from(&mut table.unwrap().pages[page_index]);
        if page.is_none() {
            return Err(Error::new(FatalError, "Item not found"));
        }

        let item_index = index & (MAX_PAGE_ITEMS - 1);
        if page.as_ref().unwrap().items[item_index].is_none() {
            return Err(Error::new(FatalError, "Item not found"));
        }

        page.as_mut().unwrap().items[item_index] = None;
        self.size -= 1;

        self.occupieds.remove(&index);

        Ok(())

    }

    pub fn get_size(&self) -> usize {

        self.size as usize

    }

    pub fn peek_next_item_index(&self) -> usize {

        self.next_index as usize

    }

    pub fn shrink_next_item_index(&mut self, from: usize, to: usize) -> usize {

        if (self.next_index == from as u32) && (to < from) {
            self.next_index = to as u32;
        }

        self.next_index as usize

    }

    pub fn iterate_items<'a>(&'a self) -> PageIterator<'a, T, F> {

        PageIterator {
            index_iterator: self.occupieds.iter(),
            page_map: &self
        }

    }

    pub fn get<'a>(&'a self, index: usize) -> Option<&'a T> {

        let index = index as usize;

        let table_index = (index >> (MAX_PAGE_SHIFT << 1)) & (MAX_PAGE_ITEMS - 1);
        let table = &self.tables[table_index];
        if table.is_none() {
            return None;
            // return false;
        }

        let page_index = (index >> MAX_PAGE_SHIFT) & (MAX_PAGE_ITEMS - 1);
        let page = &table.as_ref().unwrap().pages[page_index];
        if page.is_none() {
            return None;
            // return false;
        }

        let item_index = index & (MAX_PAGE_ITEMS - 1);
        let item = &page.as_ref().unwrap().items[item_index];
        if item.is_none() {
            return None;
            // return 
        }

        Some(item.as_ref().unwrap())

    }

}

impl<T, F: PageItemFactory<T>> Index<usize> for PageMap<T, F> {

    type Output = T;

    #[inline]
    fn index<'a>(&'a self, index: usize) -> &'a Self::Output {

        let table_index = (index >> (MAX_PAGE_SHIFT << 1)) & (MAX_PAGE_ITEMS - 1);
        let table = &self.tables[table_index];

        let page_index = (index >> MAX_PAGE_SHIFT) & (MAX_PAGE_ITEMS - 1);
        let page = &table.as_ref().unwrap().pages[page_index];

        let item_index = index & (MAX_PAGE_ITEMS - 1);
        let item = &page.as_ref().unwrap().items[item_index];

        item.as_ref().unwrap()

    }

}


#[cfg(test)] use super::super::test::TestPageItemFactory;

#[test]
fn test_page_size() {
    assert_eq!(std::mem::size_of::<Page<u32>>(), 8192);
}

#[test]
fn test_page_table_size() {
    assert_eq!(std::mem::size_of::<PageTable<u32>>(), 8192);
}

#[test]
fn test_page_map_size() {
    assert_eq!(std::mem::size_of::<PageMap<u32, TestPageItemFactory>>(), 8192);
}

#[test]
fn test_page_map_items() -> Result<(), Error> {

    let mut page_map = PageMap::new(TestPageItemFactory::new());

    assert_eq!(page_map.get_size(), 0);
    let index = page_map.gain_item()?;

    assert_eq!(page_map[index], index as u32);

    assert_eq!(index, 0);
    assert_eq!(page_map.get_size(), 1);

    assert!(page_map.recycle_item(index).is_ok());

    let index = page_map.gain_item()?;
    assert_eq!(page_map[index], index as u32);
    assert_eq!(index, 1);
    assert_eq!(page_map.get_size(), 1);
    assert!(page_map.recycle_item(index).is_ok());

    assert_eq!(page_map.get_size(), 0);

    Ok(())

}

#[test]
#[should_panic]
fn test_page_map_items_inavailable() {

    let page_map = PageMap::new(TestPageItemFactory::new());

    let _item = page_map[0];

}

#[test]
#[should_panic]
fn test_page_map_items_recycled() {

    let mut page_map = PageMap::new(TestPageItemFactory::new());

    match page_map.gain_item() {
        Ok(index) => {
            match page_map.recycle_item(index) {
                Ok(_) => {},
                Err(_) => {
                    return;
                }
            }
        },
        Err(_) => { return; }
    }

    let _item = page_map[0];

}

#[test]
fn test_page_map_iterator() -> Result<(), Error> {

    let mut page_map = PageMap::new(TestPageItemFactory::new());

    let index = page_map.gain_item()?;
    let index_2 = page_map.gain_item()?;
    let index_3 = page_map.gain_item()?;

    page_map.recycle_item(index_2)?;

    let mut indices = HashSet::new();
    for (_index, value) in page_map.iterate_items() {
        indices.insert(value);
    }

    assert_eq!(indices.len(), 2);
    assert!(indices.get(&(index as u32)).is_some());
    assert!(indices.get(&(index_3 as u32)).is_some());

    Ok(())

}