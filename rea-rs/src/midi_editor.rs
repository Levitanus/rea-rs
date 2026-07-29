use crate::{
    ptr_wrappers::{Hwnd, MediaItem, MediaItemTake},
    Item, Project, ReaRsError, Reaper, ReaperResult, WithReaperPtr,
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
    pub fn item(&self, project: &Project) -> ReaperResult<Item> {
        Item::new(project, self.item_ptr()?)
    }
    pub fn item_mut(&mut self, project: &Project) -> ReaperResult<Item> {
        Item::new(project, self.item_ptr()?)
    }
    fn item_ptr(&self) -> ReaperResult<MediaItem> {
        let rpr = Reaper::get().low();
        let ptr = unsafe { rpr.MIDIEditor_GetTake(self.get()?.as_ptr()) };
        match MediaItemTake::new(ptr) {
            None => Err(ReaRsError::NullPtr("MIDI editor take")),
            Some(ptr) => {
                let item_ptr =
                    unsafe { rpr.GetMediaItemTake_Item(ptr.as_ptr()) };
                MediaItem::new(item_ptr)
                    .ok_or(ReaRsError::NullPtr("MIDI editor item"))
            }
        }
    }
}
