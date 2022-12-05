use std::{marker::PhantomData, ptr::NonNull, time::Duration};

use reaper_medium::{MediaItem, MediaItemTake};

use crate::{
    Mutable, Position, ProbablyMutable, Project, Reaper, Take, WithReaperPtr,
};

#[derive(Debug, PartialEq)]
pub struct Item<'a, T: ProbablyMutable> {
    project: &'a Project,
    ptr: MediaItem,
    should_check: bool,
    phantom_mut: PhantomData<T>,
}
impl<'a, T: ProbablyMutable> WithReaperPtr<'a> for Item<'a, T> {
    type Ptr = MediaItem;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid_2(self.project).unwrap();
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
impl<'a, T: ProbablyMutable> Item<'a, T> {
    pub fn new(project: &'a Project, ptr: MediaItem) -> Self {
        Self {
            project,
            ptr,
            should_check: true,
            phantom_mut: PhantomData,
        }
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
    pub fn is_selected(&self) -> bool {
        unsafe { Reaper::get().low().IsMediaItemSelected(self.get().as_ptr()) }
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

    pub fn set_position(&mut self, position: Position) {
        unsafe {
            Reaper::get().low().SetMediaItemPosition(
                self.get().as_ptr(),
                position.into(),
                true,
            );
        }
    }
    pub fn set_length(&mut self, length: Duration) {
        unsafe {
            Reaper::get().low().SetMediaItemLength(
                self.get().as_ptr(),
                length.as_secs_f64(),
                true,
            );
        }
    }
    pub fn set_selected(&self, selected: bool) {
        unsafe {
            Reaper::get()
                .low()
                .SetMediaItemSelected(self.get().as_ptr(), selected)
        }
    }
}
