use super::super::util::PageItemFactory;

pub struct TestPageItemFactory {}

impl TestPageItemFactory {

    pub fn new() -> TestPageItemFactory {
        TestPageItemFactory {}
    }

}

impl PageItemFactory<u32> for TestPageItemFactory {

    fn create_item(&self, id: usize) -> Box<u32> {
        Box::new(id as u32)
    }

}
