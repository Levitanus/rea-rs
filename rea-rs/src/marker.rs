use std::mem::MaybeUninit;

use serde_derive::{Deserialize, Serialize};

use crate::{
    ptr_wrappers::MediaTrack, utils::as_string, Color, Immutable,
    Position, ProbablyMutable, Project, Reaper, Track, WithReaperPtr,
};

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

impl MarkerRegionInfo {
    pub fn iter_rendered_tracks<'a>(
        &self,
        project: &'a Project,
    ) -> RenderedTracksIterator<'a> {
        RenderedTracksIterator {
            index: 0,
            region_index: self.user_index as i32,
            project,
            is_region: self.is_region,
        }
    }

    pub fn add_rendered_track<T: ProbablyMutable>(
        &self,
        project: &Project,
        track: &Track<T>,
        channels: impl Into<Option<u32>>,
    ) {
        if !self.is_region {
            log::warn!(
                "render matrix is available only for regions"
            );
            return;
        }
        let flag = match channels.into() {
            None => 1,
            Some(channels) => channels.checked_mul(2).unwrap_or_else(|| {
                log::warn!(
                    "channels value is too large for SetRegionRenderMatrix, using u32::MAX"
                );
                u32::MAX
            }),
        };
        let flag = i32::try_from(flag).unwrap_or_else(|_| {
            log::warn!(
                "channels value is too large for SetRegionRenderMatrix, using i32::MAX"
            );
            i32::MAX
        });
        unsafe {
            Reaper::get().low().SetRegionRenderMatrix(
                project.context().to_raw(),
                self.user_index as i32,
                track.get().as_ptr(),
                flag,
            );
        }
    }

    pub fn remove_rendered_track<T: ProbablyMutable>(
        &self,
        project: &Project,
        track: &Track<T>,
    ) {
        if !self.is_region {
            log::warn!(
                "render matrix is available only for regions"
            );
            return;
        }
        unsafe {
            Reaper::get().low().SetRegionRenderMatrix(
                project.context().to_raw(),
                self.user_index as i32,
                track.get().as_ptr(),
                -1,
            );
        }
    }
}

pub struct RenderedTracksIterator<'a> {
    index: i32,
    region_index: i32,
    project: &'a Project,
    is_region: bool,
}

impl<'a> Iterator for RenderedTracksIterator<'a> {
    type Item = Track<'a, Immutable>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.is_region {
            return None;
        }
        let low = Reaper::get().low();
        unsafe {
            let ptr = low.EnumRegionRenderMatrix(
                self.project.context().to_raw(),
                self.region_index,
                self.index,
            );
            self.index += 1;
            let ptr = MediaTrack::new(ptr)?;
            Some(Track::new(self.project, ptr))
        }
    }
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
