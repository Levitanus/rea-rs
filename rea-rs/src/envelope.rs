use std::marker::PhantomData;

use reaper_medium::TrackEnvelope;

use crate::{KnowsProject, ProbablyMutable, WithReaperPtr};

#[derive(Debug, PartialEq)]
pub struct Envelope<'a, P: KnowsProject, T: ProbablyMutable> {
    ptr: TrackEnvelope,
    parent: &'a P,
    should_check: bool,
    phantom: PhantomData<T>,
}
impl<'a, P: KnowsProject, T: ProbablyMutable> WithReaperPtr<'a>
    for Envelope<'a, P, T>
{
    type Ptr = TrackEnvelope;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid_2(self.parent.project()).unwrap();
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
impl<'a, P: KnowsProject, T: ProbablyMutable> Envelope<'a, P, T> {
    pub fn new(ptr: TrackEnvelope, parent: &'a P) -> Self {
        Self {
            ptr,
            parent,
            should_check: true,
            phantom: PhantomData,
        }
    }
}
