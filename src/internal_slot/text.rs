use std::any::Any;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::slice::Iter;
use std::str::Chars;
use std::string::FromUtf8Error;

use super::internal_slot::InternalSlot;

const AUTOSHRINK_LENGTH: usize = 64;

pub struct TextCharIterator<'a> {
    slices_iterator: Iter<'a, TextSlice>,
    char_iterator: Option<Chars<'a>>
}

impl<'a> TextCharIterator<'a> {
    fn new(slices_iterator: Iter<'a, TextSlice>) -> TextCharIterator<'a> {
        TextCharIterator {
            slices_iterator: slices_iterator,
            char_iterator: None
        }
    }
}

impl<'a> Iterator for TextCharIterator<'a> {

    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {

        if self.char_iterator.is_none() {
            match self.slices_iterator.next() {
                None => { return None; },
                Some(slice) => {
                    self.char_iterator = Some(slice.iterate_chars());
                }
            }
        }

        loop {
            match &mut self.char_iterator {
                None => {
                    panic!("Char iterator not available");        
                },
                Some(char_iterator) => {
                    match char_iterator.next() {
                        Some(result) => {
                            return Some(result);
                        },
                        None => {}
                    }
                }
            }
            match self.slices_iterator.next() {
                None => { return None; },
                Some(slice) => {
                    self.char_iterator = Some(slice.iterate_chars());
                }
            }
        }

    }

}

struct TextSlice {
    string: Arc<String>,
    utf8_from: usize,
    utf8_to: usize
}

impl TextSlice {

    fn iterate_chars(&self) -> Chars {
        self.string.get(self.utf8_from .. self.utf8_to).unwrap().chars()
    }

    fn get_utf8_length(&self) -> usize {
        self.utf8_to - self.utf8_from
    }

    // fn get_chars_count(&self) -> usize {
    //     self.iterate_chars().count()
    // }

}

impl Clone for TextSlice {
    fn clone(&self) -> Self {
        TextSlice {
            string: self.string.clone(),
            utf8_from: self.utf8_from,
            utf8_to: self.utf8_to
        }
    }
}

pub struct Text {
    slices: Vec<TextSlice>,
    cached_utf8_length: usize
}

impl Clone for Text {
    fn clone(&self) -> Self {
        Text::new_with_slices(self.slices.clone())
    }
}

impl std::fmt::Debug for Text {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_tuple("Text").field(&self.to_string()).finish()
    }
}

impl Hash for Text {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for value in self.iterate_chars() {
            value.hash(state);
        }
    }
}

impl PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        let mut self_char_iterator = self.iterate_chars();
        let mut other_char_iterator = other.iterate_chars();
        loop {
            let self_char = self_char_iterator.next();
            let other_char = other_char_iterator.next();
            if self_char.is_none() {
                if other_char.is_none() {
                    return true;
                } else {
                    return false;
                }
            } else if other_char.is_none() {
                return false;
            }
            if self_char.unwrap() != other_char.unwrap() {
                return false;
            }
        }
    }
}

impl Eq for Text {}

impl PartialOrd for Text {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Text {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut self_char_iterator = self.iterate_chars();
        let mut other_char_iterator = other.iterate_chars();
        loop {
            let self_char = self_char_iterator.next();
            let other_char = other_char_iterator.next();
            if self_char.is_none() {
                if other_char.is_none() {
                   return Ordering::Equal;
                } else {
                    return Ordering::Less;
                }
            } else if other_char.is_none() {
                return Ordering::Greater;
            }
            match self_char.unwrap().cmp(&other_char.unwrap()) {
                Ordering::Less => { return Ordering::Less; }
                Ordering::Greater => { return Ordering::Greater; }
                Ordering::Equal => {}
            }
        }
    }
}

impl ToString for Text {

    fn to_string(&self) -> String {

        let mut string = String::new();

        for slice in self.slices.iter() {
            string.push_str(slice.string.get(slice.utf8_from .. slice.utf8_to).unwrap());
        }

        string

    }

}

impl InternalSlot for Text {

    fn as_any(&self) -> &dyn Any {
        self
    }

}

// Text constructors
impl Text {

    pub fn new(string: &str) -> Text {

        if string.len() == 0 {
            return Text::new_with_slices(Vec::new());
        }

        let mut slices = Vec::new();
        slices.push(TextSlice {
            string: Arc::new(string.to_owned()),
            utf8_from: 0,
            utf8_to: string.len()
        });

        Text::new_with_slices(slices)
        
    }

    pub fn from_utf8(utf8: Vec<u8>) -> Result<Text, FromUtf8Error> {
        Ok(Text::new(&String::from_utf8(utf8)?))
    }

    pub fn from_chars(chars: Vec<char>) -> Text {

        let mut string = String::new();
        for value in chars {
            string.push(value);
        }

        Text::new(&string)
        
    }

    pub fn repeat(pattern: &Text, repeat: usize) -> Text {

        let mut slices = Vec::new();

        let mut i = 0;
        while i < repeat {
            for slice in pattern.slices.iter() {
                slices.push(slice.clone());
            }
            i += 1;
        }

        Text::new_with_slices(slices)

    } 

    fn new_with_slices(slices: Vec<TextSlice>) -> Text {

        (Text {
            slices: slices,
            cached_utf8_length: 0
        }).autoshrink()

    }

    pub fn concatentate(slices: Vec<Text>) -> Text {

        let mut new_slices = Vec::new();
        for slice in slices.iter() {
            for value in slice.slices.iter() {
                new_slices.push(value.clone());
            }
        }

        Text::new_with_slices(new_slices)

    }

}

// Text initializations
impl Text {

    fn autoshrink(mut self) -> Text {

        self.cached_utf8_length = self.calculate_utf8_length();

        if self.get_utf8_length() < AUTOSHRINK_LENGTH {
            self.shrink();
        }

        self
    }

    fn shrink(&mut self) {

        if self.slices.len() == 1 {
            return;
        }

        let string = self.to_string();

        let utf8_length = string.len();

        self.slices = Vec::new();

        self.slices.push(TextSlice {
            string: Arc::new(string),
            utf8_from: 0,
            utf8_to: utf8_length
        });

    }

    fn calculate_utf8_length(&self) -> usize {
        let mut utf8_length = 0;
        for slice in self.slices.iter() {
            utf8_length += slice.get_utf8_length();
        }
        utf8_length
    }

}

// Text data
impl Text {

    pub fn as_utf8(&self) -> Vec<u8> {

        let mut utf8 = Vec::new();
        for slice in &self.slices {
            for value in slice.string[slice.utf8_from .. slice.utf8_to].as_bytes() {
                utf8.push(*value);
            }
        }

        utf8

    }

    pub fn as_chars(&self) -> Vec<char> {

        let mut chars = Vec::new();
        for value in self.iterate_chars() {
            chars.push(value);
        }

        chars
    }

    pub fn iterate_chars(&self) -> TextCharIterator {
        TextCharIterator::new(self.slices.iter())
    }

    pub fn get_char_at(&self, index: usize) -> Option<char> {
        self.iterate_chars().skip(index).next()
    }

}

// Text basic properties
impl Text {

    pub fn is_empty(&self) -> bool {
        self.get_utf8_length() == 0
    }

    pub fn get_utf8_length(&self) -> usize {
        self.cached_utf8_length
    }

    pub fn get_chars_count(&self) -> usize {
        self.iterate_chars().count()
    }

}

// Text operations
impl Text {

    pub fn slice(&self, utf8_from: usize, utf8_to: usize) -> Text {

        let mut new_slices = Vec::new();

        let mut index = 0;
        let mut slices_iterator = self.slices.iter();
        loop {

            let next = slices_iterator.next();
            if next.is_none() {
                break; 
            }

            let value = next.unwrap();
            if index < utf8_to {
                if index >= utf8_from {
                    if index + value.utf8_to - value.utf8_from > utf8_to {
                        new_slices.push(TextSlice {
                            string: value.string.clone(),
                            utf8_from: value.utf8_from,
                            utf8_to: value.utf8_from + (utf8_to - index)
                        });
                    } else {
                        new_slices.push(TextSlice {
                            string: value.string.clone(),
                            utf8_from: value.utf8_from,
                            utf8_to: value.utf8_to
                        });
                    }
                } else if index + value.utf8_to - value.utf8_from > utf8_from {
                    if index + value.utf8_to - value.utf8_from > utf8_to {
                        new_slices.push(TextSlice {
                            string: value.string.clone(),
                            utf8_from: value.utf8_from + (utf8_from - index),
                            utf8_to: value.utf8_from + (utf8_to - index)
                        });
                    } else {
                        new_slices.push(TextSlice {
                            string: value.string.clone(),
                            utf8_from: value.utf8_from + (utf8_from - index),
                            utf8_to: value.utf8_to
                        });
                    }
                }
            }

            index += value.utf8_to - value.utf8_from;
            if index >= utf8_to {
                break;
            }

        }

        Text::new_with_slices(new_slices)

    }

    // TODO: rest apis

    // pub fn has_prefix(&self, text: Text) -> bool {} 
    // pub fn has_suffix(&self, text: Text) -> bool {} 
    // pub fn index_of(&self, text: Text) -> bool {} 
    // pub fn last_index_of(&self, text: Text) -> bool {} 
    // pub fn pad_start(&self, pattern: Text, len: usize) -> Text {} 
    // pub fn pad_end(&self, pattern: Text, len: usize) -> Text {} 
    // pub fn replace_once(&self, pattern: Text, replacement: Text, from: usize) -> Text {} 
    // pub fn replace_all(&self, pattern: Text, replacement: Text) -> Text {} 

    // pub fn split(&self, delimiter: Text) -> Vec<Text> {} 
    // pub fn to_lower_case(&self) -> Vec<Text> {} 
    // pub fn to_upper_case(&self) -> Vec<Text> {} 
    // pub fn to_capital_case(&self) -> Vec<Text> {} 
    // pub fn trim(&self) -> Text {} 
    // pub fn trim_start(&self) -> Text {} 
    // pub fn trim_end(&self) -> Text {} 

}

#[cfg(test)] use std::collections::HashSet;

#[test]
fn test_simple_text() {

    let text = Text::new("test");

    assert_eq!(&text.to_string(), "test");
    assert_eq!(text.get_utf8_length(), 4);
    assert_eq!(text.get_chars_count(), 4);

}

#[test]
fn test_slices() {

    let text = Text::new("test");

    assert_eq!(&text.slice(1, 3).to_string(), "es");
    assert_eq!(text.slice(1, 3).get_chars_count(), 2);

}

#[test]
fn test_concatentate() {

    let foo = Text::new("foo");
    let space = Text::new(" ");
    let bar = Text::new("bar");

    let foo_bar = Text::concatentate([foo, space, bar].to_vec());

    assert_eq!(&foo_bar.to_string(), "foo bar");

    assert_eq!(&foo_bar.slice(2, 5).to_string(), "o b");
    assert_eq!(&foo_bar.slice(1, 2).to_string(), "o");
    assert_eq!(&foo_bar.slice(0, 3).to_string(), "foo");

}

#[test]
fn test_repeat() {
    assert_eq!(&Text::repeat(&Text::new("a "), 4).to_string(), "a a a a ");
}

#[test]
fn test_char_iterator() {

    let foo = Text::new("foo");
    let space = Text::new(" ");
    let bar = Text::new("bar");

    let foo_bar = Text::concatentate([foo, space, bar].to_vec());

    let mut chars = foo_bar.iterate_chars();

    assert_eq!(chars.next().unwrap(), 'f');
    assert_eq!(chars.next().unwrap(), 'o');
    assert_eq!(chars.next().unwrap(), 'o');
    assert_eq!(chars.next().unwrap(), ' ');
    assert_eq!(chars.next().unwrap(), 'b');
    assert_eq!(chars.next().unwrap(), 'a');
    assert_eq!(chars.next().unwrap(), 'r');

    assert!(chars.next().is_none());

}

#[test]
fn test_get_char_at() {

    let foo = Text::new("foo");
    let space = Text::new(" ");
    let bar = Text::new("bar");

    let foo_bar = Text::concatentate([foo, space, bar].to_vec());

    assert_eq!(foo_bar.get_char_at(0).unwrap(), 'f');
    assert_eq!(foo_bar.get_char_at(3).unwrap(), ' ');
    assert_eq!(foo_bar.get_char_at(4).unwrap(), 'b');
    assert_eq!(foo_bar.get_char_at(5).unwrap(), 'a');

    assert!(foo_bar.get_char_at(10).is_none());

}

#[test]
fn test_equal() {

    let foo = Text::new("foo");
    let space = Text::new(" ");
    let bar = Text::new("bar");

    let foo_bar = Text::concatentate([foo.clone(), space.clone(), bar.clone()].to_vec());
    let foo_bar_2 = Text::concatentate([foo, space, bar].to_vec());
    let foo_bar_3 = Text::new("foo bar");
    let foo_bar_4 = Text::new("foo bar");

    assert_eq!(foo_bar, foo_bar_2);
    assert_eq!(foo_bar, foo_bar_3);
    assert_eq!(foo_bar_3, foo_bar_4);
    assert_eq!(foo_bar_3, foo_bar_2);

}

#[test]
fn test_hash() {

    let foo = Text::new("foo");
    let space = Text::new(" ");
    let bar = Text::new("bar");

    let foo_bar = Text::concatentate([foo, space, bar].to_vec());
    let foo_bar_2 = Text::new("foo bar");

    let mut hash_set = HashSet::new();

    hash_set.insert(foo_bar);

    assert!(hash_set.get(&foo_bar_2).is_some());

}

#[test]
fn test_ordering() {

    assert!(Text::new("a") < Text::new("b"));
    assert!(Text::new("a") < Text::new("ab"));
    assert!(Text::new("a") <= Text::new("a"));
    assert!(Text::new("a") <= Text::new("ab"));
    assert!(Text::new("b") > Text::new("a"));
    assert!(Text::new("ab") > Text::new("a"));
    assert!(Text::new("a") >= Text::new("a"));
    assert!(Text::new("ab") >= Text::new("a"));

}

#[test]
fn test_from_chars() {

    assert_eq!(Text::new("abc"), Text::from_chars(['a', 'b', 'c'].to_vec()));

}

#[test]
fn test_from_utf8() {

    let utf8 = Text::concatentate([Text::new("a"), Text::new("bc")].to_vec()).as_utf8();

    assert_eq!(Text::from_utf8(utf8).unwrap(), Text::new("abc"));

}

#[test]
fn test_shrink() {

    let abc = Text::concatentate([Text::new("a"), Text::new("bc")].to_vec());

    assert_eq!(&abc.to_string(), "abc");
    assert_eq!(abc.slices.len(), 1);

}