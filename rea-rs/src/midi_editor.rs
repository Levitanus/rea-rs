use crate::{
    ptr_wrappers::{Hwnd, MediaItem, MediaItemTake},
    Immutable, Item, Mutable, Project, Reaper, WithReaperPtr,
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

    fn get(&self) -> Self::Ptr {
        self.require_valid().expect("NullHWND");
        self.get_pointer()
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
    pub fn item<'a>(&'a self, project: &'a Project) -> Item<Immutable> {
        Item::new(project, self.item_ptr())
    }
    pub fn item_mut<'a>(&'a mut self, project: &'a Project) -> Item<Mutable> {
        Item::new(project, self.item_ptr())
    }
    fn item_ptr(&self) -> MediaItem {
        let rpr = Reaper::get().low();
        let ptr = unsafe { rpr.MIDIEditor_GetTake(self.get().as_ptr()) };
        match MediaItemTake::new(ptr) {
            None => panic!("Null ptr! Probably, midi editor no londer active"),
            Some(ptr) => {
                let item_ptr =
                    unsafe { rpr.GetMediaItemTake_Item(ptr.as_ptr()) };
                MediaItem::new(item_ptr).expect("NullPtr. Strange, that valid Take don't have valid parent Item")
            }
        }
    }
}
