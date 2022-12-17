use std::mem::MaybeUninit;

use serde_derive::{Deserialize, Serialize};

use crate::{utils::as_string, Color, Position, Project, Reaper};

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct MarkerRegionInfo {
    pub is_region: bool,
    pub user_index: usize,
    pub enum_index: usize,
    pub position: Position,
    pub rgn_end: Position,
    pub name: String,
    pub color: Color,
}

pub struct MarkerRegionIterator<'a> {
    index: i32,
    project: &'a Project,
}
impl<'a> MarkerRegionIterator<'a> {
    pub(crate) fn new(project: &'a Project) -> Self {
        Self { index: 0, project }
    }
}
impl<'a> Iterator for MarkerRegionIterator<'a> {
    type Item = MarkerRegionInfo;
    fn next(&mut self) -> Option<Self::Item> {
        let low = Reaper::get().low();
        unsafe {
            let mut is_region = MaybeUninit::zeroed();
            let mut pos = MaybeUninit::zeroed();
            let mut end = MaybeUninit::zeroed();
            let mut name_buf = MaybeUninit::zeroed();
            let mut user_index = MaybeUninit::zeroed();
            let mut native_color = MaybeUninit::zeroed();
            let result = low.EnumProjectMarkers3(
                self.project.context().to_raw(),
                self.index,
                is_region.as_mut_ptr(),
                pos.as_mut_ptr(),
                end.as_mut_ptr(),
                name_buf.as_mut_ptr(),
                user_index.as_mut_ptr(),
                native_color.as_mut_ptr(),
            );
            self.index += 1;
            match result {
                x if x <= 0 => None,
                _ => Some(MarkerRegionInfo {
                    enum_index: (self.index - 1) as usize,
                    user_index: user_index.assume_init() as usize,
                    is_region: is_region.assume_init(),
                    position: Position::from(pos.assume_init()),
                    rgn_end: Position::from(end.assume_init()),
                    name: as_string(name_buf.assume_init())
                        .expect("should return string"),
                    color: Color::from_native(native_color.assume_init()),
                }),
            }
        }
    }
}
