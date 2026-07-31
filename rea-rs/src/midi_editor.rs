use crate::{
    ptr_wrappers::{Hwnd, MediaItemTake},
    ReaRsError, Reaper, ReaperResult, Take, WithReaperPtr,
};

#[derive(Debug, PartialEq)]
pub struct MIDIEditor {
    hwnd: Hwnd,
    checked: bool,
}
impl WithReaperPtr for MIDIEditor {
    type Ptr = Hwnd;

    fn get_pointer(&self) -> Self::Ptr {
        self.hwnd
    }

    fn get(&self) -> Result<Self::Ptr, ReaRsError> {
        self.require_valid()
    }

    fn make_unchecked(&mut self) {
        self.checked = false
    }

    fn make_checked(&mut self) {
        self.checked = true
    }

    fn should_check(&self) -> bool {
        self.checked
    }
}
impl MIDIEditor {
    pub fn new(hwnd: Hwnd) -> Self {
        Self {
            hwnd,
            checked: true,
        }
    }
    pub fn get_active_take(&self) -> ReaperResult<Take> {
        let rpr = Reaper::get().low();
        let ptr = unsafe { rpr.MIDIEditor_GetTake(self.get()?.as_ptr()) };
        match MediaItemTake::new(ptr) {
            None => Err(ReaRsError::NullPtr("MIDI editor take")),
            Some(ptr) => Take::new(ptr, None),
        }
    }

    pub fn enum_takes(
        &self,
        editable_only: bool,
    ) -> impl Iterator<Item = Take> {
        let low = Reaper::get().low();
        let ptr = self.hwnd;
        let mut idx = 0;
        std::iter::from_fn(move || {
            let result = unsafe {
                low.MIDIEditor_EnumTakes(ptr.as_ptr(), idx, editable_only)
            };
            let take_ptr = MediaItemTake::new(result)?;
            idx += 1;
            Some(Take::new(take_ptr, None).expect("should be valid Take"))
        })
    }
}
