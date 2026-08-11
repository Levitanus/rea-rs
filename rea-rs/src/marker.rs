use std::{ffi::CString, mem::MaybeUninit, ptr::null};

use serde_derive::{Deserialize, Serialize};

use crate::{
    ptr_wrappers::MediaTrack,
    utils::{string_from_const_i8},
    Color, Position, Project, ReaRsError, Reaper, ReaperResult, Track,
    WithReaperPtr,
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
    fn get_info_value(
        &self,
        project: &Project,
        parameter_name: impl Into<String>,
    ) -> ReaperResult<f64> {
        let parameter_name = parameter_name.into();
        let low = Reaper::get().low();
        unsafe {
            let marker = low.GetRegionOrMarker(
                project.context().to_raw(),
                self.enum_index as i32,
                null(),
            );
            if marker.is_null() {
                log::warn!(
                    "failed to get ProjectMarker for enum_index={} in project",
                    self.enum_index
                );
                return Err(crate::ReaRsError::InvalidObject("Marker"));
            }
            let parameter_name_cstring = CString::new(parameter_name)?;
            Ok(low.GetRegionOrMarkerInfo_Value(
                project.context().to_raw(),
                marker,
                parameter_name_cstring.as_ptr(),
            ))
        }
    }

    fn set_info_value(
        &self,
        project: &Project,
        parameter_name: impl Into<String>,
        value: f64,
    ) -> ReaperResult<()> {
        let parameter_name = parameter_name.into();
        let low = Reaper::get().low();

        let marker = unsafe {
            low.GetRegionOrMarker(
                project.context().to_raw(),
                self.enum_index as i32,
                null(),
            )
        };
        if marker.is_null() {
            log::warn!(
                "failed to get ProjectMarker for enum_index={} in project",
                self.enum_index
            );
            return Err(ReaRsError::InvalidObject("Marker"));
        }
        let parameter_name_cstring = CString::new(parameter_name)?;
        unsafe {
            low.SetRegionOrMarkerInfo_Value(
                project.context().to_raw(),
                marker,
                parameter_name_cstring.as_ptr(),
                value,
            );
        }
        Ok(())
    }

    pub fn is_selected(&self, project: &Project) -> ReaperResult<bool> {
        Ok(self.get_info_value(project, "B_UISEL")? > 0.0)
    }

    pub fn set_selected(
        &self,
        project: &Project,
        selected: bool,
    ) -> ReaperResult<()> {
        let selected = if selected { 1.0 } else { 0.0 };
        self.set_info_value(project, "B_UISEL", selected)
    }

    pub fn is_hidden(&self, project: &Project) -> ReaperResult<bool> {
        Ok(self.get_info_value(project, "B_HIDDEN")? > 0.0)
    }

    pub fn set_hidden(
        &self,
        project: &Project,
        hidden: bool,
    ) -> ReaperResult<()> {
        let hidden = if hidden { 1.0 } else { 0.0 };
        self.set_info_value(project, "B_HIDDEN", hidden)
    }

    pub fn is_lane_visible(&self, project: &Project) -> ReaperResult<bool> {
        Ok(self.get_info_value(project, "B_VISIBLE")? > 0.0)
    }

    pub fn set_lane_visible(
        &self,
        project: &Project,
        visible: bool,
    ) -> ReaperResult<()> {
        let visible = if visible { 1.0 } else { 0.0 };
        self.set_info_value(project, "B_VISIBLE", visible)
    }

    pub fn get_lane(&self, project: &Project) -> ReaperResult<Option<u32>> {
        let lane = self.get_info_value(project, "I_LANENUMBER")? as i32;
        if lane < 0 {
            Ok(None)
        } else {
            Ok(Some(lane as u32))
        }
    }

    pub fn set_lane(
        &self,
        project: &Project,
        lane: Option<u32>,
    ) -> ReaperResult<()> {
        let lane = lane.map(|v| v as f64).unwrap_or(-1.0);
        self.set_info_value(project, "I_LANENUMBER", lane)
    }

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

    pub fn add_rendered_track(
        &self,
        project: &Project,
        track: &Track,
        channels: impl Into<Option<u32>>,
    ) -> ReaperResult<()> {
        if !self.is_region {
            log::warn!("render matrix is available only for regions");
            return Ok(());
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
                track.get()?.as_ptr(),
                flag,
            );
        }
        Ok(())
    }

    pub fn remove_rendered_track(
        &self,
        project: &Project,
        track: &Track,
    ) -> ReaperResult<()> {
        if !self.is_region {
            log::warn!("render matrix is available only for regions");
            return Ok(());
        }
        unsafe {
            Reaper::get().low().SetRegionRenderMatrix(
                project.context().to_raw(),
                self.user_index as i32,
                track.get()?.as_ptr(),
                -1,
            );
        }
        Ok(())
    }
}

pub struct RenderedTracksIterator<'a> {
    index: i32,
    region_index: i32,
    project: &'a Project,
    is_region: bool,
}

impl<'a> Iterator for RenderedTracksIterator<'a> {
    type Item = Track;

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
            Some(Track::new(self.project.get().ok()?, ptr))
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
                    name: string_from_const_i8(name_buf.assume_init())
                        .expect("should return string"),
                    color: Color::from_native(native_color.assume_init()),
                }),
            }
        }
    }
}
