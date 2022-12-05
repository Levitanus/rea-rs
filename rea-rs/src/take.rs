use reaper_medium::MediaItemTake;

use crate::{
    Fx, Item, KnowsProject, ProbablyMutable, Project, TakeFX, WithReaperPtr,
};

#[derive(Debug, PartialEq)]
pub struct Take<'a, T: ProbablyMutable> {
    ptr: MediaItemTake,
    should_check: bool,
    item: &'a Item<'a, T>,
}
impl<'a, T: ProbablyMutable> KnowsProject for Take<'a, T> {
    fn project(&self) -> &Project {
        self.item.parent_project()
    }
}
impl<'a, T: ProbablyMutable> WithReaperPtr<'a> for Take<'a, T> {
    type Ptr = MediaItemTake;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid_2(self.parent_item().parent_project())
            .unwrap();
        self.ptr
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
    pub fn parent_item(&self) -> &Item<T> {
        self.item
    }
    pub fn get_fx(&'a self, index: usize) -> Option<TakeFX<'a, T>> {
        TakeFX::from_index(self, index)
    }
}
