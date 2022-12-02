use std::{marker::PhantomData, ptr::NonNull};

use reaper_medium::{MediaItem, MediaItemTake, ReaperPointer};

use crate::{Mutable, ProbablyMutable, Project, Reaper, Take, WithReaperPtr};

#[derive(Debug, PartialEq)]
pub struct Item<'a, T: ProbablyMutable> {
    project: &'a Project,
    ptr: MediaItem,
    should_check: bool,
    phantom_mut: PhantomData<T>,
}
impl<'a, T: ProbablyMutable> WithReaperPtr for Item<'a, T> {
    fn get_pointer(&self) -> reaper_medium::ReaperPointer {
        ReaperPointer::MediaItem(self.ptr)
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
impl<'a, T: ProbablyMutable> Item<'a, T> {
    pub fn new(project: &'a Project, ptr: MediaItem) -> Self {
        Self {
            project,
            ptr,
            should_check: true,
            phantom_mut: PhantomData,
        }
    }
    pub fn get(&self) -> MediaItem {
        self.require_valid_2(self.project).unwrap();
        self.ptr
    }
    pub fn get_take(&'a self, index: usize) -> Option<Take<'a, T>> {
        let ptr = self.get_take_ptr(index)?;
        Some(Take::new(ptr, self))
    }
    fn get_take_ptr(&self, index: usize) -> Option<MediaItemTake> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetTake(self.get().as_ptr(), index as i32)
        };
        let ptr = NonNull::new(ptr);
        match ptr {
            None => None,
            x => x,
        }
    }
    pub fn parent_project(&self) -> &Project {
        self.project
    }
}
impl<'a> Item<'a, Mutable> {
    pub fn get_take_mut(
        &'a mut self,
        index: usize,
    ) -> Option<Take<'a, Mutable>> {
        let ptr = self.get_take_ptr(index)?;
        Some(Take::new(ptr, self))
    }
}
