use reaper_medium::{MediaItemTake, ReaperPointer};

use crate::{Fx, Item, ProbablyMutable   , TakeFX, WithReaperPtr};

#[derive(Debug, PartialEq)]
pub struct Take<'a, T: ProbablyMutable> {
    ptr: MediaItemTake,
    should_check: bool,
    item: &'a Item<'a, T>,
}
impl<'a, T: ProbablyMutable> WithReaperPtr for Take<'a, T> {
    fn get_pointer(&self) -> reaper_medium::ReaperPointer {
        ReaperPointer::MediaItemTake(self.ptr)
    }
    fn make_unchecked(&mut self) {
        self.should_check = false;
    }
    fn make_checked(&mut self) {
        self.should_check = true;
    }
    fn should_check(&self) -> bool {
        self.should_check
    }
}
impl<'a, T: ProbablyMutable> Take<'a, T> {
    pub fn new(ptr: MediaItemTake, item: &'a Item<'a, T>) -> Self {
        Self {
            ptr,
            should_check: true,
            item,
        }
    }
    pub fn get(&self) -> MediaItemTake {
        self.require_valid_2(self.parent_item().parent_project())
            .unwrap();
        self.ptr
    }
    pub fn parent_item(&self) -> &Item<T> {
        self.item
    }
    pub fn get_fx_by_index(&'a self, index: usize) -> Option<TakeFX<'a, T>> {
        TakeFX::from_index(self, index)
    }
}
