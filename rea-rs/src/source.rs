use reaper_medium::PcmSource;

use crate::{KnowsProject, ProbablyMutable, Take, WithReaperPtr};

#[derive(Debug, PartialEq)]
pub struct Source<'a, T: ProbablyMutable> {
    take: &'a Take<'a, T>,
    ptr: PcmSource,
    should_check: bool,
}
impl<'a, T: ProbablyMutable> WithReaperPtr<'a> for Source<'a, T> {
    type Ptr = PcmSource;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid_2(self.take.project()).unwrap();
        self.get_pointer()
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
impl<'a, T: ProbablyMutable> Source<'a, T> {
    pub fn new(take: &'a Take<'a, T>, ptr: PcmSource) -> Self {
        Self {
            take,
            ptr,
            should_check: true,
        }
    }
}
