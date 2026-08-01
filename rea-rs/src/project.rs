pub use crate::utils::WithReaperPtr;
use crate::{
    db_to_linear, linear_to_db,
    ptr_wrappers::{MediaItem, MediaTrack, ReaProject},
    utils::{string_from_buf, string_from_const_i8},
    Color, CommandId, Item, MarkerRegionInfo, MarkerRegionIterator, PlayRate,
    Position, ProjectContext, ReaRsError, Reaper, ReaperResult, TimeRange,
    TimeRangeKind, TimeSignature, Track, UndoFlags,
};
use c_str_macro::c_str;
use int_enum::IntEnum;
use log::{debug, warn};
use serde_derive::{Deserialize, Serialize};
use std::{
    ffi::CString, mem::MaybeUninit, path::PathBuf, ptr::NonNull,
    time::Duration,
};

use self::project_info::{
    BoundsMode, RenderDitherFlags, RenderFadeLowPassFlags, RenderFadeShape,
    RenderNormalize, RenderSettings, RenderTail, RenderTailFlags,
};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FullRenderSettings {
    pub settings: Option<RenderSettings>,
    pub bounds: Option<(Position, Position)>,
    pub bounds_mode: Option<BoundsMode>,
    pub add_to_project: Option<bool>,
    pub dither: Option<RenderDitherFlags>,
    pub normalize: Option<RenderNormalize>,
    pub normalize_target: Option<f64>,
    pub brickwall: Option<f64>,
    pub fade_in: Option<Duration>,
    pub fade_out: Option<Duration>,
    pub fade_in_shape: Option<RenderFadeShape>,
    pub fade_out_shape: Option<RenderFadeShape>,
    pub fade_lpf: Option<RenderFadeLowPassFlags>,
    pub pad_start: Option<Duration>,
    pub pad_end: Option<Duration>,
    pub trim_start: Option<f64>,
    pub trim_end: Option<f64>,
    pub delay: Option<Duration>,
    pub channels_amount: Option<u32>,
    pub directory: Option<PathBuf>,
    pub file: Option<String>,
    pub primary_format: Option<String>,
    pub secondary_format: Option<String>,
    pub srate: Option<Option<u32>>,
    pub tail: Option<RenderTail>,
}

impl FullRenderSettings {
    pub fn from_project(project: &Project) -> ReaperResult<Self> {
        Ok(Self {
            settings: Some(project.get_render_settings()?),
            bounds: Some(project.get_render_bounds()?),
            bounds_mode: Some(project.get_render_bounds_mode()?),
            add_to_project: Some(project.get_render_add_to_project()?),
            dither: Some(project.get_render_dither()?),
            normalize: Some(project.get_render_normalize()?),
            normalize_target: Some(project.get_render_normalize_target()?),
            brickwall: Some(project.get_render_brickwall()?),
            fade_in: Some(project.get_render_fade_in()?),
            fade_out: Some(project.get_render_fade_out()?),
            fade_in_shape: Some(project.get_render_fade_in_shape()?),
            fade_out_shape: Some(project.get_render_fade_out_shape()?),
            fade_lpf: Some(project.get_render_fade_lpf()?),
            pad_start: Some(project.get_render_pad_start()?),
            pad_end: Some(project.get_render_pad_end()?),
            trim_start: Some(project.get_render_trim_start()?),
            trim_end: Some(project.get_render_trim_end()?),
            delay: Some(project.get_render_delay()?),
            channels_amount: Some(project.get_render_channels_amount()?),
            directory: Some(project.get_render_directory()?),
            file: Some(project.get_render_file()?),
            primary_format: Some(project.get_render_format(false)?),
            secondary_format: Some(project.get_render_format(true)?),
            srate: Some(project.get_render_srate()?),
            tail: Some(project.get_render_tail()?),
        })
    }

    pub fn apply_to_project(&self, project: &mut Project) -> ReaperResult<()> {
        if let Some(settings) = self.settings {
            project.set_render_settings(settings)?;
        }
        if let Some((start, end)) = self.bounds {
            project.set_render_bounds(start, end)?;
        }
        if let Some(bounds_mode) = self.bounds_mode {
            project.set_render_bounds_mode(bounds_mode)?;
        }
        if let Some(add_to_project) = self.add_to_project {
            project.set_render_add_to_project(add_to_project)?;
        }
        if let Some(dither) = self.dither {
            project.set_render_dither(dither)?;
        }
        if let Some(normalize) = self.normalize {
            project.set_render_normalize(normalize)?;
        }
        if let Some(normalize_target) = self.normalize_target {
            project.set_render_normalize_target(normalize_target)?;
        }
        if let Some(brickwall) = self.brickwall {
            project.set_render_brickwall(brickwall)?;
        }
        if let Some(fade_in) = self.fade_in {
            project.set_render_fade_in(fade_in)?;
        }
        if let Some(fade_out) = self.fade_out {
            project.set_render_fade_out(fade_out)?;
        }
        if let Some(fade_in_shape) = self.fade_in_shape {
            project.set_render_fade_in_shape(fade_in_shape)?;
        }
        if let Some(fade_out_shape) = self.fade_out_shape {
            project.set_render_fade_out_shape(fade_out_shape)?;
        }
        if let Some(fade_lpf) = self.fade_lpf {
            project.set_render_fade_lpf(fade_lpf)?;
        }
        if let Some(pad_start) = self.pad_start {
            project.set_render_pad_start(pad_start)?;
        }
        if let Some(pad_end) = self.pad_end {
            project.set_render_pad_end(pad_end)?;
        }
        if let Some(trim_start) = self.trim_start {
            project.set_render_trim_start(trim_start)?;
        }
        if let Some(trim_end) = self.trim_end {
            project.set_render_trim_end(trim_end)?;
        }
        if let Some(delay) = self.delay {
            project.set_render_delay(delay)?;
        }
        if let Some(channels_amount) = self.channels_amount {
            project.set_render_channels_amount(channels_amount)?;
        }
        if let Some(directory) = &self.directory {
            project.set_render_directory(directory.clone())?;
        }
        if let Some(file) = &self.file {
            project.set_render_file(file.clone())?;
        }
        if let Some(primary_format) = &self.primary_format {
            project.set_render_format(primary_format.clone(), false)?;
        }
        if let Some(secondary_format) = &self.secondary_format {
            project.set_render_format(secondary_format.clone(), true)?;
        }
        if let Some(srate) = self.srate {
            project.set_render_srate(srate)?;
        }
        if let Some(tail) = self.tail {
            project.set_render_tail(tail)?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct Project {
    // context: ProjectContext,
    pointer: ReaProject,
    checked: bool,
    info_buf_size: usize,
}

impl<'a> WithReaperPtr for Project {
    type Ptr = ReaProject;
    fn get_pointer(&self) -> Self::Ptr {
        unsafe { NonNull::new_unchecked(self.pointer.as_ptr()) }
    }
    fn get(&self) -> Result<ReaProject, ReaRsError> {
        self.require_valid()
    }
    fn make_unchecked(&mut self) {
        self.checked = false;
    }
    fn make_checked(&mut self) {
        self.checked = true;
    }
    fn should_check(&self) -> bool {
        self.checked
    }
}
impl<'a> Project {
    /// New object from the project context.
    ///
    /// It will never hold pseudo-context of `CURRENT_PROJECT`,
    /// but hold real pointer. So, for example, project, returned from
    /// [Reaper::current_project] will not always remain the project
    /// in the active project tab, if the tab changes.
    ///
    /// # Note
    ///
    /// It is better to get all opened projects in once
    /// by [Reaper::iter_projects].
    pub fn new(context: ProjectContext) -> Self {
        let rpr = Reaper::get();
        unsafe {
            let pointer = match context {
                ProjectContext::CurrentProject => {
                    let ptr = rpr.low().EnumProjects(
                        -1,
                        CString::from(c_str!("")).into_raw(),
                        0,
                    );
                    NonNull::new(ptr).expect("expect project")
                }
                ProjectContext::Proj(ptr) => ptr,
            };
            Self {
                pointer,
                checked: true,
                info_buf_size: 1024 * 10,
            }
        }
    }

    /// Get opened project with a given name, if any.
    ///
    /// # Note
    ///
    /// This operation, probably, of O(n³) complexity in the worst case,
    /// do not use a lot.
    pub fn from_name(name: impl Into<String>) -> anyhow::Result<Self> {
        let name: String = name.into();
        for project in Reaper::get().iter_projects() {
            let mut pr_name = project.name()?;
            let pr_name: String = pr_name.drain(..pr_name.len() - 4).collect();
            if name == pr_name {
                return Ok(project);
            }
        }
        Err(ReaRsError::InvalidObject("No project with the given name").into())
    }

    /// Get the underlying project context for compatibility with older APIs.
    pub fn context(&self) -> ProjectContext {
        ProjectContext::Proj(self.pointer)
    }

    /// Activate project tab with the project.
    pub fn make_current_project(&self) -> ReaperResult<()> {
        let low = Reaper::get().low();
        unsafe {
            low.SelectProjectInstance(self.get()?.as_ptr());
        }
        Ok(())
    }

    /// If the project tab is active.
    pub fn is_current_project(&self) -> bool {
        let low = Reaper::get().low();
        let ptr = unsafe {
            low.EnumProjects(-1, CString::from(c_str!("")).into_raw(), 0)
        };
        self.pointer.as_ptr() == ptr
    }

    /// Focus project for performing the closure
    pub fn with_current_project(
        &self,
        mut f: impl FnMut() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let rpr = Reaper::get();
        let current = rpr.current_project();
        let ret = self == &current;
        if ret {
            return f();
        }
        let _ = self.make_current_project();
        f()?;
        let _ = current.make_current_project();
        Ok(())
    }

    /// Whether project is dirty (i.e. needing save).
    pub fn is_dirty(&self) -> Result<bool, ReaRsError> {
        unsafe {
            match Reaper::get().low().IsProjectDirty(self.get()?.as_ptr()) {
                x if x <= 0 => Ok(false),
                _ => Ok(true),
            }
        }
    }

    /// Mark project dirty (i.e. needing save).
    pub fn mark_dirty(&mut self) -> ReaperResult<()> {
        unsafe {
            Reaper::get().low().MarkProjectDirty(self.get()?.as_ptr());
        }
        Ok(())
    }

    pub fn get_last_touched_track(&self) -> ReaperResult<Option<Track>> {
        let ptr = Reaper::get().low().GetLastTouchedTrack();
        match MediaTrack::new(ptr) {
            None => Ok(None),
            Some(ptr) => Ok(Some(Track::new(self.get()?, ptr))),
        }
    }

    pub fn get_last_touched_track_mut(
        &mut self,
    ) -> ReaperResult<Option<Track>> {
        let ptr = Reaper::get().low().GetLastTouchedTrack();
        match MediaTrack::new(ptr) {
            None => Ok(None),
            Some(ptr) => Ok(Some(Track::new(self.get()?, ptr))),
        }
    }

    /// Direct way to simulate pause button hit.
    pub fn pause(&mut self) -> ReaperResult<()> {
        unsafe { Reaper::get().low().OnPauseButtonEx(self.get()?.as_ptr()) }
        Ok(())
    }

    pub fn is_paused(&self) -> Result<bool, ReaRsError> {
        unsafe {
            Ok(
                (Reaper::get().low().GetPlayStateEx(self.get()?.as_ptr()) & 2)
                    != 0,
            )
        }
    }

    /// Direct way to simulate play button hit.
    pub fn play(&mut self) -> ReaperResult<()> {
        unsafe { Reaper::get().low().OnPlayButtonEx(self.get()?.as_ptr()) }
        Ok(())
    }

    pub fn is_playing(&self) -> Result<bool, ReaRsError> {
        unsafe {
            Ok(
                (Reaper::get().low().GetPlayStateEx(self.get()?.as_ptr()) & 1)
                    != 0,
            )
        }
    }

    /// Hit record button.
    ///
    /// # Note
    ///
    /// This is sugar on top of the cation invocation.
    pub fn record(&mut self) {
        self.with_current_project(|| -> anyhow::Result<()> {
            Reaper::get().perform_action(CommandId::new(1013), 0, Some(self));
            Ok(())
        })
        .unwrap()
    }
    pub fn is_recording(&self) -> Result<bool, ReaRsError> {
        unsafe {
            Ok(
                (Reaper::get().low().GetPlayStateEx(self.get()?.as_ptr()) & 4)
                    != 0,
            )
        }
    }

    /// Direct way to simulate stop button hit.
    pub fn stop(&mut self) -> ReaperResult<()> {
        unsafe { Reaper::get().low().OnStopButtonEx(self.get()?.as_ptr()) }
        Ok(())
    }

    pub fn is_stopped(&self) -> Result<bool, ReaRsError> {
        unsafe {
            Ok((Reaper::get().low().GetPlayStateEx(self.get()?.as_ptr())
                & (1 | 2))
                == 0)
        }
    }

    pub fn length(&self) -> Result<Duration, ReaRsError> {
        unsafe {
            Ok(Duration::from_secs_f64(
                Reaper::get().low().GetProjectLength(self.get()?.as_ptr()),
            ))
        }
    }

    pub fn get_time_range(&'a self, kind: TimeRangeKind) -> TimeRange<'a> {
        TimeRange::new(self, kind)
    }

    pub fn get_loop_selection(&'a self) -> TimeRange<'a> {
        TimeRange::new(self, TimeRangeKind::LoopSelection)
    }
    pub fn get_time_selection(&'a self) -> TimeRange<'a> {
        TimeRange::new(self, TimeRangeKind::TimeSelection)
    }

    pub fn is_loop_enabled(&self) -> Result<bool, ReaRsError> {
        unsafe {
            Ok(Reaper::get().low().GetSetRepeatEx(self.get()?.as_ptr(), -1)
                != 0)
        }
    }

    pub fn set_loop_enabled(&mut self, should_loop: bool) -> ReaperResult<()> {
        unsafe {
            let val = match should_loop {
                true => 1,
                false => 0,
            };
            Reaper::get()
                .low()
                .GetSetRepeatEx(self.get()?.as_ptr(), val);
        }
        Ok(())
    }

    /// Close the project.
    pub fn close(self) -> ReaperResult<()> {
        let rpr = Reaper::get();
        let current = rpr.current_project();
        if current.get()? == self.get()? {
            rpr.perform_action(CommandId::new(40860), 0, None);
            Ok(())
        } else {
            let _ = self.make_current_project();
            rpr.perform_action(CommandId::new(40860), 0, None);
            let _ = current.make_current_project();
            Ok(())
        }
    }

    /// Get time signature and tempo (BPM) at given position.
    pub fn time_signature_at_position(
        &self,
        position: Position,
    ) -> Result<(TimeSignature, f64), ReaRsError> {
        unsafe {
            let (mut num, mut denom, mut tempo) = (
                MaybeUninit::zeroed(),
                MaybeUninit::zeroed(),
                MaybeUninit::zeroed(),
            );
            Reaper::get().low().TimeMap_GetTimeSigAtTime(
                self.get()?.as_ptr(),
                position.into(),
                num.as_mut_ptr(),
                denom.as_mut_ptr(),
                tempo.as_mut_ptr(),
            );
            Ok((
                TimeSignature::new(
                    num.assume_init() as u32,
                    denom.assume_init() as u32,
                ),
                tempo.assume_init(),
            ))
        }
    }

    /// Create new marker and return its index.
    ///
    /// Return [ReaRsError::Unexpected] if reaper can't add marker.
    ///
    /// If it is possible, the index will be the same as desired,
    /// but if it is busy, new index will be returned.
    ///
    /// If a marker with the same position and name exists,
    /// no new marker will be created, and existing index will be returned.
    ///
    /// index is not an enum index, but user-index.
    pub fn add_marker(
        &mut self,
        position: Position,
        name: Option<impl Into<String>>,
        color: impl Into<Option<Color>>,
        desired_index: impl Into<Option<usize>>,
    ) -> anyhow::Result<usize> {
        self.add_marker_or_region(
            false,
            name,
            color,
            desired_index,
            position,
            Position::from(0.0),
        )
    }

    /// Create new region and return its index.
    ///
    /// Return [ReaRsError::Unexpected] if reaper can't add marker.
    ///
    /// If it is possible, the index will be the same as desired,
    /// but if it is busy, new index will be returned.
    ///
    /// If a marker with the same position and name exists,
    /// no new marker will be created, and existing index will be returned.
    ///
    /// index is not an enum index, but user-index.
    pub fn add_region(
        &mut self,
        start: Position,
        end: Position,
        name: Option<impl Into<String>>,
        color: impl Into<Option<Color>>,
        desired_index: impl Into<Option<usize>>,
    ) -> anyhow::Result<usize> {
        self.add_marker_or_region(true, name, color, desired_index, start, end)
    }

    fn add_marker_or_region(
        &mut self,
        is_region: bool,
        name: Option<impl Into<String>>,
        color: impl Into<Option<Color>>,
        desired_index: impl Into<Option<usize>>,
        start: Position,
        end: Position,
    ) -> anyhow::Result<usize> {
        let rpr = Reaper::get();
        let name = match name {
            None => String::from(""),
            Some(s) => s.into(),
        };
        let color: Option<Color> = color.into();
        let color = match color {
            None => 0,
            Some(clr) => clr.to_native() | 0x1000000,
        };
        let desired_index = desired_index.into();
        let desired_index: i32 = match desired_index {
            None => -1,
            Some(idx) => idx as i32,
        };
        unsafe {
            let result = rpr.low().AddProjectMarker2(
                self.get()?.as_ptr(),
                is_region,
                start.into(),
                end.into(),
                CString::new(name)?.as_ptr(),
                desired_index,
                color,
            );
            match result {
                -1 => Err(ReaRsError::Unexpected.into()),
                _ => Ok(result as usize),
            }
        }
    }

    /// Set marker or region from info.
    pub fn set_marker_or_region(
        &mut self,
        info: MarkerRegionInfo,
    ) -> anyhow::Result<()> {
        unsafe {
            match Reaper::get().low().SetProjectMarker3(
                self.get()?.as_ptr(),
                info.user_index as i32,
                info.is_region,
                info.position.into(),
                info.rgn_end.into(),
                CString::new(info.name.to_string())?.as_ptr(),
                info.color.to_native(),
            ) {
                true => Ok(()),
                false => Err(ReaRsError::Unexpected.into()),
            }
        }
    }

    pub fn delete_marker(&mut self, user_index: usize) -> anyhow::Result<()> {
        self.delete_marker_or_region(user_index, false)
    }

    pub fn delete_region(&mut self, user_index: usize) -> anyhow::Result<()> {
        self.delete_marker_or_region(user_index, true)
    }

    fn delete_marker_or_region(
        &mut self,
        user_index: usize,
        region: bool,
    ) -> anyhow::Result<()> {
        unsafe {
            match Reaper::get().low().DeleteProjectMarker(
                self.get()?.as_ptr(),
                user_index as i32,
                region,
            ) {
                true => Ok(()),
                false => Err(ReaRsError::Unexpected.into()),
            }
        }
    }

    /// Get iterator through all project markers and regions.
    ///
    /// Since markers and regions are messed up in indexes and API,
    /// it's better to work with them through iteration.
    ///
    /// # Example
    /// ```no_run
    /// # use rea_rs::{Project, ProjectContext};
    /// let project = Project::new(ProjectContext::CurrentProject);
    /// assert_eq!(
    ///     project
    ///     .iter_markers_and_regions()
    ///     .find(|info| !info.is_region && info.user_index == 2)
    ///     .unwrap()
    ///     .position
    ///     .as_duration()
    ///     .as_secs_f64(),
    /// 4.0
    /// );
    /// ```
    pub fn iter_markers_and_regions(&self) -> MarkerRegionIterator<'_> {
        MarkerRegionIterator::new(self)
    }

    pub fn n_tracks(&self) -> Result<usize, ReaRsError> {
        unsafe {
            Ok(Reaper::get().low().CountTracks(self.get()?.as_ptr()) as usize)
        }
    }

    pub fn n_selected_tracks(&self) -> Result<usize, ReaRsError> {
        unsafe {
            Ok(Reaper::get()
                .low()
                .CountSelectedTracks2(self.get()?.as_ptr(), false)
                as usize)
        }
    }

    pub fn n_items(&self) -> Result<usize, ReaRsError> {
        unsafe {
            Ok(Reaper::get().low().CountMediaItems(self.get()?.as_ptr())
                as usize)
        }
    }

    pub fn n_selected_items(&self) -> Result<usize, ReaRsError> {
        unsafe {
            Ok(Reaper::get()
                .low()
                .CountSelectedMediaItems(self.get()?.as_ptr())
                as usize)
        }
    }

    pub fn n_tempo_markers(&self) -> Result<usize, ReaRsError> {
        unsafe {
            Ok(Reaper::get()
                .low()
                .CountTempoTimeSigMarkers(self.get()?.as_ptr())
                as usize)
        }
    }

    pub fn n_markers(&self) -> Result<usize, ReaRsError> {
        Ok(self.count_markers_and_regions()?.0)
    }
    pub fn n_regions(&self) -> Result<usize, ReaRsError> {
        Ok(self.count_markers_and_regions()?.1)
    }

    fn count_markers_and_regions(&self) -> Result<(usize, usize), ReaRsError> {
        unsafe {
            let (mut n_markers, mut n_regions) =
                (MaybeUninit::zeroed(), MaybeUninit::zeroed());
            let result = Reaper::get().low().CountProjectMarkers(
                self.get()?.as_ptr(),
                n_markers.as_mut_ptr(),
                n_regions.as_mut_ptr(),
            );
            if result <= 0 {
                return Ok((0, 0));
            }
            Ok((
                n_markers.assume_init() as usize,
                n_regions.assume_init() as usize,
            ))
        }
    }

    pub fn add_track(
        &mut self,
        index: impl Into<Option<usize>>,
        name: impl Into<String>,
    ) -> Result<Track, ReaRsError> {
        let n_tracks = self.n_tracks()?;
        let index = match index.into() {
            None => n_tracks,
            Some(idx) => {
                if idx <= n_tracks {
                    idx
                } else {
                    n_tracks
                }
            }
        };
        let _ = self.with_current_project(|| {
            Reaper::get().low().InsertTrackAtIndex(index as i32, true);
            Ok(())
        });
        let mut track = self
            .get_track(index)?
            .ok_or(ReaRsError::Str("Can not add track at given index"))?;
        let name: String = name.into();
        if !name.is_empty() {
            track.set_name(name)?
        }
        Ok(track)
    }

    pub fn get_track(&self, index: usize) -> ReaperResult<Option<Track>> {
        unsafe {
            let ptr = MediaTrack::new(
                Reaper::get()
                    .low()
                    .GetTrack(self.get()?.as_ptr(), index as i32),
            );
            match ptr {
                None => Ok(None),
                Some(ptr) => Ok(Some(Track::new(self.get_pointer(), ptr))),
            }
        }
    }

    pub fn get_selected_track(
        &self,
        index: usize,
    ) -> ReaperResult<Option<Track>> {
        unsafe {
            let ptr = MediaTrack::new(
                Reaper::get()
                    .low()
                    .GetSelectedTrack(self.get()?.as_ptr(), index as i32),
            );
            match ptr {
                None => Ok(None),
                Some(ptr) => Ok(Some(Track::new(self.get_pointer(), ptr))),
            }
        }
    }

    pub fn get_master_track(&self) -> ReaperResult<Track> {
        let ptr = unsafe {
            NonNull::new(
                Reaper::get().low().GetMasterTrack(self.get()?.as_ptr()),
            )
            .ok_or(ReaRsError::NullPtr("Null Master Track"))?
        };
        Ok(Track::new(self.get_pointer(), ptr))
    }

    pub fn iter_tracks(&self) -> TracksIterator<'_> {
        TracksIterator::new(self)
    }

    pub fn iter_selected_tracks(&self) -> SelectedTracksIterator<'_> {
        SelectedTracksIterator::new(self)
    }

    pub fn iter_items(&'a self) -> ItemsIterator<'a> {
        ItemsIterator::new(self)
    }

    pub fn iter_selected_items(&'a self) -> SelectedItemsIterator<'a> {
        SelectedItemsIterator::new(self)
    }

    pub fn get_item(&self, index: usize) -> ReaperResult<Option<Item>> {
        let project_ptr = self.get()?;
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetMediaItem(project_ptr.as_ptr(), index as i32)
        };
        match MediaItem::new(ptr) {
            None => Ok(None),
            Some(x) => {
                Ok(Some(Item::from_raw_project_ptr(Some(project_ptr), x)))
            }
        }
    }

    pub fn get_selected_item(
        &self,
        index: usize,
    ) -> ReaperResult<Option<Item>> {
        let project_ptr = self.get()?;
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetSelectedMediaItem(project_ptr.as_ptr(), index as i32)
        };
        match MediaItem::new(ptr) {
            None => Ok(None),
            Some(x) => {
                Ok(Some(Item::from_raw_project_ptr(Some(project_ptr), x)))
            }
        }
    }

    /// Glue items (action shortcut).
    pub fn glue_selected_items(&mut self, within_time_selection: bool) {
        let action_id = match within_time_selection {
            true => CommandId::new(41588),
            false => CommandId::new(40362),
        };
        Reaper::get().perform_action(action_id, 0, Some(self))
    }

    pub fn any_track_solo(&self) -> Result<bool, ReaRsError> {
        unsafe { Ok(Reaper::get().low().AnyTrackSolo(self.get()?.as_ptr())) }
    }

    /// Verbose way to make undo.
    ///
    /// # Safety
    ///
    /// [Project::end_undo_block] has to be called after.
    pub fn begin_undo_block(&mut self) -> ReaperResult<()> {
        debug!("begin_undo_block");
        unsafe {
            Reaper::get().low().Undo_BeginBlock2(self.get()?.as_ptr());
        }
        Ok(())
    }

    /// Verbose way to make undo: name is the name shown in undo list.
    ///
    /// # Safety
    ///
    /// [Project::begin_undo_block] has to be called before.
    pub fn end_undo_block(
        &mut self,
        name: impl Into<String>,
        flags: UndoFlags,
    ) -> ReaperResult<()> {
        let name = name.into();
        debug!("end undo block: {}", name);
        unsafe {
            Reaper::get().low().Undo_EndBlock2(
                self.get()?.as_ptr(),
                CString::new(name)?.as_ptr(),
                flags.bits() as i32,
            )
        }
        Ok(())
    }

    /// Call function in undo block with given name.
    ///
    /// # Note
    ///
    /// Probably, it's better to use `UndoFlags.all()`
    /// by default.
    pub fn with_undo_block(
        &mut self,
        undo_name: impl Into<String>,
        flags: UndoFlags,
        f: impl FnMut() -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        Reaper::get().with_undo_block(undo_name, flags, Some(self), f)
    }

    /// Try to undo last action.
    pub fn undo(&mut self) -> ReaperResult<()> {
        unsafe {
            match Reaper::get().low().Undo_DoUndo2(self.get()?.as_ptr()) {
                0 => Err(ReaRsError::UnsuccessfulOperation("can not do undo")),
                _ => Ok(()),
            }
        }
    }

    /// Try to redo last undone action.
    pub fn redo(&mut self) -> ReaperResult<()> {
        unsafe {
            match Reaper::get().low().Undo_DoRedo2(self.get()?.as_ptr()) {
                0 => Err(ReaRsError::UnsuccessfulOperation("can not do redo")),
                _ => Ok(()),
            }
        }
    }

    /// Position of next audio block being processed.
    ///
    /// [Project::play_position]
    pub fn next_buffer_position(&self) -> Result<Position, ReaRsError> {
        unsafe {
            Ok(Position::from(
                Reaper::get().low().GetPlayPosition2Ex(self.get()?.as_ptr()),
            ))
        }
    }

    /// Latency-compensated actual-what-you-hear position.
    ///
    /// [Project::next_buffer_position]
    pub fn play_position(&self) -> Result<Position, ReaRsError> {
        unsafe {
            Ok(Position::from(
                Reaper::get().low().GetPlayPositionEx(self.get()?.as_ptr()),
            ))
        }
    }

    /// Bypass (`true`) or un-bypass (`false`) FX on all tracks.
    pub fn bypass_fx_on_all_tracks(
        &mut self,
        bypass: bool,
    ) -> ReaperResult<()> {
        self.with_current_project(|| {
            Reaper::get().low().BypassFxAllTracks(bypass as i32);
            Ok(())
        })
        .map_err(|_| ReaRsError::InvalidObject("Project"))
    }

    /// Get the name of the next action in redo queue, if any.
    pub fn next_redo(&self) -> Result<Option<String>, ReaRsError> {
        unsafe {
            let ptr = Reaper::get().low().Undo_CanRedo2(self.get()?.as_ptr());
            match ptr.is_null() {
                true => Ok(None),
                false => Ok(Some(string_from_const_i8(ptr)?)),
            }
        }
    }

    /// Get the name of the next action in undo queue, if any.
    pub fn next_undo(&self) -> Result<Option<String>, ReaRsError> {
        unsafe {
            let ptr = Reaper::get().low().Undo_CanUndo2(self.get()?.as_ptr());
            match ptr.is_null() {
                true => Ok(None),
                false => Ok(Some(string_from_const_i8(ptr)?)),
            }
        }
    }

    /// Edit cursor position.
    pub fn get_cursor_position(&self) -> Result<Position, ReaRsError> {
        let project = self.get()?;
        unsafe {
            Ok(Position::from(
                Reaper::get().low().GetCursorPositionEx(project.as_ptr()),
            ))
        }
    }

    /// Set edit cursor position.
    pub fn set_cursor_position(
        &mut self,
        position: Position,
        move_view: bool,
        seek_play: bool,
    ) -> ReaperResult<()> {
        let project = self.get()?;
        unsafe {
            Reaper::get().low().SetEditCurPos2(
                project.as_ptr(),
                position.into(),
                move_view,
                seek_play,
            )
        }
        Ok(())
    }

    /// Disarm record on all tracks.
    pub fn disarm_rec_on_all_tracks(&mut self) -> ReaperResult<()> {
        self.with_current_project(|| -> anyhow::Result<()> {
            Reaper::get().low().ClearAllRecArmed();
            Ok(())
        })
        .map_err(|_| ReaRsError::InvalidObject("Project"))
    }

    /// Check if there is any FX window in focus.
    ///
    /// Returns enough data for getting Fx by yourself,
    /// as it will be easier, than conquer borrow checker or
    /// force you to pass closure inside.
    ///
    /// [FocusedFxResult]
    pub fn focused_fx(&self) -> Option<FocusedFxResult> {
        if !self.is_current_project() {
            return None;
        }
        unsafe {
            let (mut track, mut item, mut fx) = (
                MaybeUninit::zeroed(),
                MaybeUninit::zeroed(),
                MaybeUninit::zeroed(),
            );
            match Reaper::get().low().GetFocusedFX2(
                track.as_mut_ptr(),
                item.as_mut_ptr(),
                fx.as_mut_ptr(),
            ) {
                1 => Some(FocusedFxResult {
                    track_index: track.assume_init() - 1,
                    item_index: None,
                    take_index: None,
                    fx_index: fx.assume_init() as usize,
                }),
                2 => {
                    let fx = fx.assume_init() as usize;
                    Some(FocusedFxResult {
                        track_index: track.assume_init() - 1,
                        item_index: Some(item.assume_init() as usize),
                        take_index: Some(fx / 2_usize.pow(16)),
                        fx_index: fx & 2_usize.pow(16),
                    })
                }
                _ => None,
            }
        }
    }

    /// Overwrite default size of string buffer,
    /// used to set and get string values:
    ///
    /// `Project::get_info_string`
    /// `Project::set_info_string`
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rea_rs::{Reaper, Project};
    /// let mut pr = Reaper::get().current_project();
    /// let directory = match pr.get_render_directory(){
    ///     Err(_) => {
    ///                 pr.set_string_param_size(2048);
    ///                 pr.get_render_directory()
    ///                     .expect("another reason of error")
    ///             },
    ///     Ok(dir) => dir,
    /// };
    /// ```
    pub fn set_string_param_size(&mut self, size: usize) {
        self.info_buf_size = size;
    }

    fn get_info_string(
        &self,
        param_name: impl Into<String>,
    ) -> ReaperResult<String> {
        unsafe {
            if self.info_buf_size < 2 {
                return Err(ReaRsError::InvalidObject(
                    "Project string buffer size must be at least 2.",
                )
                .into());
            }
            let mut buf = vec![0_i8; self.info_buf_size];
            let project = self.get()?;
            let result = Reaper::get().low().GetSetProjectInfo_String(
                project.as_ptr(),
                CString::new(param_name.into())?.as_ptr(),
                buf.as_mut_ptr(),
                false,
            );
            if !result {
                return Err(ReaRsError::InvalidObject(
                    "Can not get Project info.",
                )
                .into());
            }

            let result_string = string_from_buf(&buf).map_err(|err| match err {
                ReaRsError::UnsuccessfulOperation("Buffer is too small for value") => {
                    ReaRsError::InvalidObject(
                        "Project info string is too long for buffer. Increase it with set_string_param_size.",
                    )
                }
                _ => err,
            })?;
            debug!("{}", result_string);
            Ok(result_string)
        }
    }

    fn set_info_string(
        &mut self,
        param_name: impl Into<String>,
        value: impl Into<String>,
    ) -> ReaperResult<()> {
        let value: String = value.into();
        let val = CString::new(value)?.into_raw();
        let project = self.get()?;
        let result = unsafe {
            Reaper::get().low().GetSetProjectInfo_String(
                project.as_ptr(),
                CString::new(param_name.into())?.as_ptr(),
                val,
                true,
            )
        };
        match result {
            false => {
                Err(ReaRsError::InvalidObject("can not set value to project.")
                    .into())
            }
            true => Ok(()),
        }
    }

    pub fn name(&self) -> Result<String, ReaRsError> {
        let project = self.get()?;
        let mut name = vec![0_i8; self.info_buf_size];
        unsafe {
            Reaper::get().low().GetProjectName(
                project.as_ptr(),
                name.as_mut_ptr(),
                self.info_buf_size as i32,
            );
        }
        string_from_buf(&name)
    }

    ///  title field from Project Settings/Notes dialog
    pub fn get_title(&self) -> ReaperResult<String> {
        self.get_info_string("PROJECT_TITLE")
    }

    ///  title field from Project Settings/Notes dialog
    pub fn set_title(&mut self, title: impl Into<String>) -> ReaperResult<()> {
        self.set_info_string("PROJECT_TITLE", title)
    }

    ///  author field from Project Settings/Notes dialog
    pub fn get_author(&self) -> ReaperResult<String> {
        self.get_info_string("PROJECT_AUTHOR")
    }

    ///  author field from Project Settings/Notes dialog
    pub fn set_author(
        &mut self,
        author: impl Into<String>,
    ) -> ReaperResult<()> {
        self.set_info_string("PROJECT_AUTHOR", author)
    }

    pub fn get_marker_guid(
        &self,
        marker_index: usize,
    ) -> ReaperResult<String> {
        let pattern = format!("MARKER_GUID:{:?}", marker_index);
        warn!("this function, probably, not working properly");
        self.get_info_string(pattern)
    }

    pub fn get_track_group_name(
        &self,
        group_index: usize,
    ) -> ReaperResult<String> {
        let group_index = match group_index {
            0..=63 => group_index + 1,
            _ => {
                return Err(ReaRsError::InvalidObject(
                    "group_index must be in range 0..64",
                )
                .into())
            }
        };
        let pattern = format!("TRACK_GROUP_NAME:{:?}", group_index);
        warn!("this function, probably, not working properly");
        self.get_info_string(pattern)
    }

    pub fn set_track_group_name(
        &mut self,
        group_index: usize,
        track_group_name: impl Into<String>,
    ) -> ReaperResult<()> {
        let group_index = match group_index {
            0..=63 => group_index + 1,
            _ => {
                return Err(ReaRsError::InvalidObject(
                    "group_index must be in range 0..64",
                )
                .into())
            }
        };
        let pattern = format!("TRACK_GROUP_NAME:{:?}", group_index);
        warn!("this function, probably, not working properly");
        self.set_info_string(pattern, track_group_name)
    }

    pub fn get_record_path(
        &self,
        secondary_path: bool,
    ) -> ReaperResult<PathBuf> {
        let param_name = match secondary_path {
            false => "RECORD_PATH",
            true => "RECORD_PATH_SECONDARY",
        };
        Ok(PathBuf::from(self.get_info_string(param_name)?))
    }

    /// Project path.
    pub fn get_path(&self) -> ReaperResult<PathBuf> {
        let project = self.get()?;
        unsafe {
            let mut buf = vec![0_i8; self.info_buf_size];
            Reaper::get().low().GetProjectPathEx(
                project.as_ptr(),
                buf.as_mut_ptr(),
                self.info_buf_size as i32,
            );
            let result = PathBuf::from(string_from_buf(&buf)?);
            Ok(result)
        }
    }

    pub fn set_record_path(
        &mut self,
        secondary_path: bool,
        directory: impl Into<PathBuf>,
    ) -> ReaperResult<()> {
        let param_name = match secondary_path {
            false => "RECORD_PATH",
            true => "RECORD_PATH_SECONDARY",
        };
        let directory: PathBuf = directory.into();
        self.set_info_string(
            param_name,
            directory
                .to_str()
                .ok_or(ReaRsError::Str("Can not convert path to str"))?,
        )
    }

    pub fn get_render_directory(&self) -> ReaperResult<PathBuf> {
        Ok(PathBuf::from(self.get_info_string("RENDER_FILE")?))
    }

    pub fn set_render_directory(
        &mut self,
        directory: impl Into<PathBuf>,
    ) -> ReaperResult<()> {
        let directory: PathBuf = directory.into();
        self.set_info_string(
            "RENDER_FILE",
            directory
                .to_str()
                .ok_or(ReaRsError::Str("Can not convert path to str"))?,
        )
    }

    ///  render file name (may contain wildcards)
    pub fn get_render_file(&self) -> ReaperResult<String> {
        self.get_info_string("RENDER_PATTERN")
    }

    ///  render file name (may contain wildcards)
    pub fn set_render_file(
        &mut self,
        file: impl Into<String>,
    ) -> ReaperResult<()> {
        self.set_info_string("RENDER_PATTERN", file)
    }

    /// base64-encoded sink configuration (see project files, etc).
    ///
    /// Set secondary_format to true, if you want the secondary render section
    /// format.
    pub fn get_render_format(
        &self,
        secondary_format: bool,
    ) -> ReaperResult<String> {
        let param = match secondary_format {
            false => "RENDER_FORMAT",
            true => "RENDER_FORMAT2",
        };
        self.get_info_string(param)
    }

    /// base64-encoded secondary sink configuration.
    ///
    /// Set secondary_format to true, if you want the secondary render section
    /// format.
    ///
    /// Callers can also pass a simple 4-byte string (non-base64-encoded),
    /// e.g. "evaw" or "l3pm", to use default settings for that sink type.
    ///
    /// # Typical formats
    ///
    /// "wave" "aiff" "caff" "iso " "ddp " "flac" "mp3l" "oggv" "OggS"
    pub fn set_render_format(
        &mut self,
        format: impl Into<String>,
        secondary_format: bool,
    ) -> ReaperResult<()> {
        let param = match secondary_format {
            false => "RENDER_FORMAT",
            true => "RENDER_FORMAT2",
        };
        self.set_info_string(param, format)
    }

    /// Filenames, that will be rendered.
    pub fn get_render_targets(&self) -> ReaperResult<Vec<String>> {
        Ok(self
            .get_info_string("RENDER_TARGETS")?
            .split(";")
            .map(|i| String::from(i))
            .collect::<Vec<String>>())
    }

    /// Will return `PlayRate::from(1.0)` in normal conditions.
    pub fn get_play_rate(
        &self,
        position: impl Into<Position>,
    ) -> ReaperResult<PlayRate> {
        let project = self.get()?;
        Ok(unsafe {
            PlayRate::from(Reaper::get().low().Master_GetPlayRateAtTime(
                position.into().into(),
                project.as_ptr(),
            ))
        })
    }

    pub fn save(&mut self, force_save_as: bool) -> ReaperResult<()> {
        let project = self.get()?;
        unsafe {
            Reaper::get()
                .low()
                .Main_SaveProject(project.as_ptr(), force_save_as)
        }
        Ok(())
    }

    pub fn select_all_items(
        &mut self,
        should_select: bool,
    ) -> ReaperResult<()> {
        let project = self.get()?;
        unsafe {
            Reaper::get()
                .low()
                .SelectAllMediaItems(project.as_ptr(), should_select)
        }
        Ok(())
    }

    pub fn select_all_tracks(
        &mut self,
        should_select: bool,
    ) -> ReaperResult<()> {
        self.with_current_project(|| {
            let id = match should_select {
                true => CommandId::new(40297),
                false => CommandId::new(40296),
            };
            Reaper::get().perform_action(id, 0, Some(self));
            Ok(())
        })
        .map_err(|_| ReaRsError::InvalidObject("Project"))
    }

    pub fn solo_all_tracks(&mut self, solo: bool) -> ReaperResult<()> {
        self.with_current_project(|| {
            Reaper::get().low().SoloAllTracks(match solo {
                true => 1,
                false => 0,
            });
            Ok(())
        })
        .map_err(|_| ReaRsError::InvalidObject("Project"))
    }
    pub fn mute_all_tracks(&mut self, mute: bool) -> ReaperResult<()> {
        self.with_current_project(|| {
            Reaper::get().low().MuteAllTracks(mute);
            Ok(())
        })
        .map_err(|_| ReaRsError::InvalidObject("Project"))
    }

    pub fn clear_all_rec_armed_tracks(&mut self) -> ReaperResult<()> {
        self.with_current_project(|| {
            Reaper::get().low().ClearAllRecArmed();
            Ok(())
        })
        .map_err(|_| ReaRsError::InvalidObject("Project"))
    }

    fn get_info_value(
        &self,
        param_name: impl Into<String>,
    ) -> ReaperResult<f64> {
        let project = self.get()?;
        Ok(unsafe {
            Reaper::get().low().GetSetProjectInfo(
                project.as_ptr(),
                CString::new(param_name.into())?.as_ptr(),
                0.0,
                false,
            )
        })
    }

    fn set_info_value(
        &mut self,
        param_name: impl Into<String>,
        value: f64,
    ) -> ReaperResult<()> {
        let project = self.get()?;
        unsafe {
            Reaper::get().low().GetSetProjectInfo(
                project.as_ptr(),
                CString::new(param_name.into())?.as_ptr(),
                value,
                true,
            );
        }
        Ok(())
    }

    pub fn get_render_bounds_mode(&self) -> ReaperResult<BoundsMode> {
        let val = self.get_info_value("RENDER_BOUNDSFLAG")?;
        BoundsMode::from_int(val as u32)
            .map_err(|e| ReaRsError::IntEnum(e.to_string()))
    }

    pub fn set_render_bounds_mode(
        &mut self,
        mode: BoundsMode,
    ) -> ReaperResult<()> {
        let mode = mode.int_value();
        self.set_info_value("RENDER_BOUNDSFLAG", mode as f64)
    }

    /// Collect all possible project render settings.
    pub fn get_full_render_settings(
        &self,
    ) -> ReaperResult<FullRenderSettings> {
        FullRenderSettings::from_project(self)
    }

    /// Apply render settings to project.
    pub fn apply_full_render_settings(
        &mut self,
        settings: &FullRenderSettings,
    ) -> ReaperResult<()> {
        settings.apply_to_project(self)
    }

    pub fn get_render_settings(&self) -> ReaperResult<RenderSettings> {
        let settings =
            RenderSettings::from_raw(self.get_info_value("RENDER_SETTINGS")?);
        Ok(settings)
    }
    pub fn set_render_settings(
        &mut self,
        settings: RenderSettings,
    ) -> ReaperResult<()> {
        self.set_info_value("RENDER_SETTINGS", settings.to_raw())
    }

    pub fn get_render_add_to_project_flags(
        &self,
    ) -> ReaperResult<self::project_info::RenderAddToProjectFlags> {
        let raw = self.get_info_value("RENDER_ADDTOPROJ")? as u32;
        self::project_info::RenderAddToProjectFlags::from_bits(raw).ok_or(
            ReaRsError::InvalidObject(
                "Can not get render add-to-project flags",
            ),
        )
    }
    pub fn set_render_add_to_project_flags(
        &mut self,
        flags: self::project_info::RenderAddToProjectFlags,
    ) -> ReaperResult<()> {
        self.set_info_value("RENDER_ADDTOPROJ", flags.bits() as f64)
    }

    pub fn get_render_add_to_project(&self) -> ReaperResult<bool> {
        Ok(self.get_render_add_to_project_flags()?.contains(
            self::project_info::RenderAddToProjectFlags::ADD_TO_PROJECT,
        ))
    }
    pub fn set_render_add_to_project(
        &mut self,
        add_to_project: bool,
    ) -> ReaperResult<()> {
        let mut flags = self.get_render_add_to_project_flags()?;
        if add_to_project {
            flags.insert(
                self::project_info::RenderAddToProjectFlags::ADD_TO_PROJECT,
            );
        } else {
            flags.remove(
                self::project_info::RenderAddToProjectFlags::ADD_TO_PROJECT,
            );
        }
        self.set_render_add_to_project_flags(flags)
    }

    pub fn get_render_dither(&self) -> ReaperResult<RenderDitherFlags> {
        let raw = self.get_info_value("RENDER_DITHER")? as u32;
        RenderDitherFlags::from_bits(raw).ok_or(ReaRsError::InvalidObject(
            "Can not get render dither flags",
        ))
    }
    pub fn set_render_dither(
        &mut self,
        flags: RenderDitherFlags,
    ) -> ReaperResult<()> {
        self.set_info_value("RENDER_DITHER", flags.bits() as f64)
    }

    pub fn get_render_normalize(&self) -> ReaperResult<RenderNormalize> {
        let raw = self.get_info_value("RENDER_NORMALIZE")?;
        Ok(RenderNormalize::from_raw(raw))
    }
    pub fn set_render_normalize(
        &mut self,
        settings: RenderNormalize,
    ) -> ReaperResult<()> {
        self.set_info_value("RENDER_NORMALIZE", settings.to_raw())
    }

    /// Render normalization target level in decibels.
    ///
    /// REAPER stores this as a linear amplitude ratio internally, so the
    /// value is converted to and from dB at the API boundary.
    pub fn get_render_normalize_target(&self) -> ReaperResult<f64> {
        let raw = self.get_info_value("RENDER_NORMALIZE_TARGET")?;
        Ok(linear_to_db(raw))
    }
    pub fn set_render_normalize_target(
        &mut self,
        value: f64,
    ) -> ReaperResult<()> {
        self.set_info_value("RENDER_NORMALIZE_TARGET", db_to_linear(value))
    }

    /// Render brickwall limit level in decibels.
    ///
    /// REAPER stores this as a linear amplitude ratio internally, so the
    /// value is converted to and from dB at the API boundary.
    pub fn get_render_brickwall(&self) -> ReaperResult<f64> {
        let raw = self.get_info_value("RENDER_BRICKWALL")?;
        Ok(linear_to_db(raw))
    }
    pub fn set_render_brickwall(&mut self, value: f64) -> ReaperResult<()> {
        self.set_info_value("RENDER_BRICKWALL", db_to_linear(value))
    }

    /// Render fade-in duration.
    ///
    /// REAPER stores this as seconds internally, so the value is converted to
    /// and from [Duration] at the API boundary.
    pub fn get_render_fade_in(&self) -> ReaperResult<Duration> {
        let raw = self.get_info_value("RENDER_FADEIN")?;
        Ok(Duration::from_secs_f64(raw))
    }
    pub fn set_render_fade_in(&mut self, value: Duration) -> ReaperResult<()> {
        self.set_info_value("RENDER_FADEIN", value.as_secs_f64())
    }

    /// Render fade-out duration.
    ///
    /// REAPER stores this as seconds internally, so the value is converted to
    /// and from [Duration] at the API boundary.
    pub fn get_render_fade_out(&self) -> ReaperResult<Duration> {
        let raw = self.get_info_value("RENDER_FADEOUT")?;
        Ok(Duration::from_secs_f64(raw))
    }
    pub fn set_render_fade_out(
        &mut self,
        value: Duration,
    ) -> ReaperResult<()> {
        self.set_info_value("RENDER_FADEOUT", value.as_secs_f64())
    }

    pub fn get_render_fade_in_shape(&self) -> ReaperResult<RenderFadeShape> {
        let raw = self.get_info_value("RENDER_FADEINSHAPE")? as u32;
        RenderFadeShape::from_int(raw)
            .map_err(|e| ReaRsError::IntEnum(e.to_string()))
    }
    pub fn set_render_fade_in_shape(
        &mut self,
        shape: RenderFadeShape,
    ) -> ReaperResult<()> {
        self.set_info_value("RENDER_FADEINSHAPE", shape.int_value() as f64)
    }

    pub fn get_render_fade_out_shape(&self) -> ReaperResult<RenderFadeShape> {
        let raw = self.get_info_value("RENDER_FADEOUTSHAPE")? as u32;
        RenderFadeShape::from_int(raw)
            .map_err(|e| ReaRsError::IntEnum(e.to_string()))
    }
    pub fn set_render_fade_out_shape(
        &mut self,
        shape: RenderFadeShape,
    ) -> ReaperResult<()> {
        self.set_info_value("RENDER_FADEOUTSHAPE", shape.int_value() as f64)
    }

    pub fn get_render_fade_lpf(&self) -> ReaperResult<RenderFadeLowPassFlags> {
        let raw = self.get_info_value("RENDER_FADELPF")? as u32;
        RenderFadeLowPassFlags::from_bits(raw).ok_or(
            ReaRsError::InvalidObject("Can not get render fade LPF flags"),
        )
    }
    pub fn set_render_fade_lpf(
        &mut self,
        flags: RenderFadeLowPassFlags,
    ) -> ReaperResult<()> {
        self.set_info_value("RENDER_FADELPF", flags.bits() as f64)
    }

    /// Render padding before the start in duration.
    ///
    /// REAPER stores this as seconds internally, so the value is converted to
    /// and from [Duration] at the API boundary.
    pub fn get_render_pad_start(&self) -> ReaperResult<Duration> {
        let raw = self.get_info_value("RENDER_PADSTART")?;
        Ok(Duration::from_secs_f64(raw))
    }
    pub fn set_render_pad_start(
        &mut self,
        value: Duration,
    ) -> ReaperResult<()> {
        self.set_info_value("RENDER_PADSTART", value.as_secs_f64())
    }

    /// Render padding after the end in duration.
    ///
    /// REAPER stores this as seconds internally, so the value is converted to
    /// and from [Duration] at the API boundary.
    pub fn get_render_pad_end(&self) -> ReaperResult<Duration> {
        let raw = self.get_info_value("RENDER_PADEND")?;
        Ok(Duration::from_secs_f64(raw))
    }
    pub fn set_render_pad_end(&mut self, value: Duration) -> ReaperResult<()> {
        self.set_info_value("RENDER_PADEND", value.as_secs_f64())
    }

    /// Trim threshold at the render start in decibels.
    ///
    /// REAPER stores this as a linear amplitude ratio internally, so the
    /// value is converted to and from dB at the API boundary.
    pub fn get_render_trim_start(&self) -> ReaperResult<f64> {
        let raw = self.get_info_value("RENDER_TRIMSTART")?;
        Ok(linear_to_db(raw))
    }
    pub fn set_render_trim_start(&mut self, value: f64) -> ReaperResult<()> {
        self.set_info_value("RENDER_TRIMSTART", db_to_linear(value))
    }

    /// Trim threshold at the render end in decibels.
    ///
    /// REAPER stores this as a linear amplitude ratio internally, so the
    /// value is converted to and from dB at the API boundary.
    pub fn get_render_trim_end(&self) -> ReaperResult<f64> {
        let raw = self.get_info_value("RENDER_TRIMEND")?;
        Ok(linear_to_db(raw))
    }
    pub fn set_render_trim_end(&mut self, value: f64) -> ReaperResult<()> {
        self.set_info_value("RENDER_TRIMEND", db_to_linear(value))
    }

    /// Delay before the render starts.
    ///
    /// REAPER stores this as seconds internally, so the value is converted to
    /// and from [Duration] at the API boundary.
    pub fn get_render_delay(&self) -> ReaperResult<Duration> {
        let raw = self.get_info_value("RENDER_DELAY")?;
        Ok(Duration::from_secs_f64(raw))
    }
    pub fn set_render_delay(&mut self, value: Duration) -> ReaperResult<()> {
        self.set_info_value("RENDER_DELAY", value.as_secs_f64())
    }

    pub fn get_render_channels_amount(&self) -> ReaperResult<u32> {
        Ok(self.get_info_value("RENDER_CHANNELS")? as u32)
    }
    pub fn set_render_channels_amount(
        &mut self,
        channels_amount: u32,
    ) -> ReaperResult<()> {
        self.set_info_value("RENDER_CHANNELS", channels_amount as f64)
    }

    /// If None — then sample rate from Reaper settings used.
    pub fn get_srate(&self) -> ReaperResult<Option<u32>> {
        match self.get_info_value("PROJECT_SRATE")? as u32 {
            0 => Ok(None),
            val => Ok(Some(val)),
        }
    }
    /// If None — then sample rate from Reaper settings used.
    pub fn set_srate(
        &mut self,
        srate: impl Into<Option<u32>>,
    ) -> ReaperResult<()> {
        let srate = srate.into();
        self.set_info_value("PROJECT_SRATE", srate.unwrap_or(0) as f64)?;
        match srate {
            None => self.set_info_value("PROJECT_SRATE_USE", 1.0),
            Some(_) => self.set_info_value("PROJECT_SRATE_USE", 0.0),
        }
    }

    /// If None — then project sample rate used.
    pub fn get_render_srate(&self) -> ReaperResult<Option<u32>> {
        match self.get_info_value("RENDER_SRATE")? as u32 {
            0 => Ok(None),
            val => Ok(Some(val)),
        }
    }
    /// If None — then project sample rate used.
    pub fn set_render_srate(
        &mut self,
        srate: impl Into<Option<u32>>,
    ) -> ReaperResult<()> {
        let srate = srate.into();
        self.set_info_value("RENDER_SRATE", srate.unwrap_or(0) as f64)
    }

    /// Get in tuple (start, end)
    ///
    /// Valid only when [Project::get_render_bounds_mode] is
    /// [BoundsMode::Custom]
    pub fn get_render_bounds(&self) -> ReaperResult<(Position, Position)> {
        let start = self.get_info_value("RENDER_STARTPOS")?;
        let end = self.get_info_value("RENDER_ENDPOS")?;
        Ok((Position::from(start), Position::from(end)))
    }
    /// Valid only when [Project::get_render_bounds_mode] is
    /// [BoundsMode::Custom]
    pub fn set_render_bounds(
        &mut self,
        start: impl Into<Position>,
        end: impl Into<Position>,
    ) -> ReaperResult<()> {
        self.set_info_value("RENDER_STARTPOS", start.into().into())?;
        self.set_info_value("RENDER_ENDPOS", end.into().into())
    }

    pub fn get_render_tail(&self) -> ReaperResult<RenderTail> {
        let tail = Duration::from_millis(
            self.get_info_value("RENDER_TAILMS")? as u64,
        );
        let flags_raw = self.get_info_value("RENDER_TAILFLAG")?;
        let flags = RenderTailFlags::from_bits(flags_raw as u32)
            .ok_or(ReaRsError::InvalidObject("Can not get tail flags"))?;
        Ok(RenderTail { tail, flags })
    }
    pub fn set_render_tail(
        &mut self,
        render_tail: RenderTail,
    ) -> ReaperResult<()> {
        let tail = render_tail.tail.as_millis() as f64;
        let flags = render_tail.flags.bits();
        self.set_info_value("RENDER_TAILMS", tail)?;
        self.set_info_value("RENDER_TAILFLAG", flags as f64)
    }
}

pub mod project_info {
    use std::time::Duration;

    use bitflags::bitflags;
    use int_enum::IntEnum;
    use serde_derive::{Deserialize, Serialize};

    #[repr(u32)]
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, IntEnum, Serialize, Deserialize,
    )]
    pub enum BoundsMode {
        Custom = 0,
        EntireProject = 1,
        TimeSelection = 2,
        AllRegions = 3,
        SelectedItems = 4,
        SelectedRegions = 5,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct RenderSettings {
        pub mode: RenderMode,
        /// Render tracks with mono media to mono files.
        pub use_mono: bool,
    }
    impl RenderSettings {
        pub fn new(
            mode: RenderMode,
            use_mono: bool,
            add_to_project: bool,
        ) -> Self {
            Self { mode, use_mono }
        }
        pub(crate) fn to_raw(&self) -> f64 {
            let val = self.mode.int_value()
                | match self.use_mono {
                    true => 16,
                    false => 0,
                };
            val as f64
        }
        pub(crate) fn from_raw(value: f64) -> Self {
            let int_mode = value as u32 & !16;
            let use_mono = value as u32 & 16 != 0;
            Self {
                mode: RenderMode::from_int(int_mode)
                    .expect("can not convert to render mode"),
                use_mono,
            }
        }
    }

    #[repr(u32)]
    #[derive(
        Debug, Clone, Copy, IntEnum, PartialEq, Eq, Serialize, Deserialize,
    )]
    pub enum RenderMode {
        MasterMix = 0,
        MasterAndStems = 1,
        Stems = 2,
        RenderMatrix = 8,
        SelectedItems = 32,
        SelectedItemsViaMaster = 64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct RenderTail {
        pub tail: Duration,
        pub flags: RenderTailFlags,
    }
    impl RenderTail {
        pub fn new(tail: Duration, flags: RenderTailFlags) -> Self {
            Self { tail, flags }
        }
    }

    bitflags! {
        #[derive(Serialize, Deserialize)]
        pub struct RenderTailFlags:u32{
            const IN_CUSTOM_BOUNDS=1;
            const IN_ENTIRE_PROJECT=2;
            const IN_TIME_SELECTION=4;
            const IN_ALL_REGIONS=8;
            const IN_SELECTED_ITEMS=16;
            const IN_SELECTED_REGIONS=32;

        }
    }

    bitflags! {
        #[derive(Serialize, Deserialize)]
        pub struct RenderDitherFlags: u32 {
            const DITHER = 1;
            const NOISE_SHAPING = 2;
            const DITHER_STEMS = 4;
            const NOISE_SHAPING_STEMS = 8;
            const DISABLE_ALL = 16;
        }
    }

    bitflags! {
        #[derive(Serialize, Deserialize)]
        pub struct RenderAddToProjectFlags: u32 {
            const ADD_TO_PROJECT = 1;
            const SKIP_LIKELY_SILENT_FILES = 2;
        }
    }

    #[repr(u32)]
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, IntEnum, Serialize, Deserialize,
    )]
    pub enum RenderNormalizeMode {
        LufsI = 0,
        Rms = 2,
        Peak = 4,
        TruePeak = 6,
        LufsMMax = 8,
        LufsSMax = 10,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum RenderMonoAdjustment {
        None,
        Minus3Db,
        Plus3Db,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum RenderNormalizeTargetMode {
        None,
        AsIfFilesPlayTogether,
        ToLoudestFile,
        AsIfFilesPlayTogetherCommonGain,
        ToMasterMix,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum RenderLimitMode {
        None,
        AsIfFilesPlayTogether,
        ToMasterMix,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct RenderNormalize {
        pub enabled: bool,
        pub mode: RenderNormalizeMode,
        pub mono_adjustment: RenderMonoAdjustment,
        pub target_mode: RenderNormalizeTargetMode,
        pub brickwall_limit: bool,
        pub brickwall_limit_true_peak: bool,
        pub only_normalize_too_loud: bool,
        pub only_normalize_too_quiet: bool,
        pub apply_fade_in: bool,
        pub apply_fade_out: bool,
        pub trim_start_silence: bool,
        pub trim_end_silence: bool,
        pub pad_start_silence: bool,
        pub pad_end_silence: bool,
        pub disable_all_postprocessing: bool,
        pub limit_mode: RenderLimitMode,
    }

    impl Default for RenderNormalize {
        fn default() -> Self {
            Self {
                enabled: false,
                mode: RenderNormalizeMode::LufsI,
                mono_adjustment: RenderMonoAdjustment::None,
                target_mode: RenderNormalizeTargetMode::None,
                brickwall_limit: false,
                brickwall_limit_true_peak: false,
                only_normalize_too_loud: false,
                only_normalize_too_quiet: false,
                apply_fade_in: false,
                apply_fade_out: false,
                trim_start_silence: false,
                trim_end_silence: false,
                pad_start_silence: false,
                pad_end_silence: false,
                disable_all_postprocessing: false,
                limit_mode: RenderLimitMode::None,
            }
        }
    }

    impl RenderNormalize {
        pub fn to_raw(&self) -> f64 {
            let mut raw = 0_u32;
            if self.enabled {
                raw |= 1;
            }
            raw |= self.mode.int_value();

            match self.mono_adjustment {
                RenderMonoAdjustment::Minus3Db => raw |= 16,
                RenderMonoAdjustment::Plus3Db => raw |= 16 | (8 << 16),
                RenderMonoAdjustment::None => {}
            }

            match self.target_mode {
                RenderNormalizeTargetMode::AsIfFilesPlayTogether => raw |= 32,
                RenderNormalizeTargetMode::ToLoudestFile => raw |= 4096,
                RenderNormalizeTargetMode::AsIfFilesPlayTogetherCommonGain => {
                    raw |= 32 | 4096;
                }
                RenderNormalizeTargetMode::ToMasterMix => raw |= 16 << 16,
                RenderNormalizeTargetMode::None => {}
            }

            if self.brickwall_limit {
                raw |= 64;
            }
            if self.brickwall_limit_true_peak {
                raw |= 128;
            }
            if self.only_normalize_too_loud {
                raw |= 256;
            }
            if self.only_normalize_too_quiet {
                raw |= 2048;
            }
            if self.apply_fade_in {
                raw |= 512;
            }
            if self.apply_fade_out {
                raw |= 1024;
            }
            if self.trim_start_silence {
                raw |= 16_384;
            }
            if self.trim_end_silence {
                raw |= 32_768;
            }
            if self.pad_start_silence {
                raw |= 1 << 16;
            }
            if self.pad_end_silence {
                raw |= 2 << 16;
            }
            if self.disable_all_postprocessing {
                raw |= 4 << 16;
            }
            match self.limit_mode {
                RenderLimitMode::AsIfFilesPlayTogether => raw |= 32 << 16,
                RenderLimitMode::ToMasterMix => raw |= 64 << 16,
                RenderLimitMode::None => {}
            }
            raw as f64
        }

        pub fn from_raw(value: f64) -> Self {
            let raw = value as u32;
            let mode = RenderNormalizeMode::from_int(raw & 0x0e)
                .unwrap_or(RenderNormalizeMode::LufsI);
            let mono_adjustment = if raw & (8 << 16) != 0 {
                RenderMonoAdjustment::Plus3Db
            } else if raw & 16 != 0 {
                RenderMonoAdjustment::Minus3Db
            } else {
                RenderMonoAdjustment::None
            };
            let target_mode = match (
                raw & 32 != 0,
                raw & 4096 != 0,
                raw & (16 << 16) != 0,
            ) {
                (true, false, false) => {
                    RenderNormalizeTargetMode::AsIfFilesPlayTogether
                }
                (false, true, false) => {
                    RenderNormalizeTargetMode::ToLoudestFile
                }
                (true, true, false) => {
                    RenderNormalizeTargetMode::AsIfFilesPlayTogetherCommonGain
                }
                (false, false, true) => RenderNormalizeTargetMode::ToMasterMix,
                _ => RenderNormalizeTargetMode::None,
            };
            let limit_mode =
                match (raw & (32 << 16) != 0, raw & (64 << 16) != 0) {
                    (true, false) => RenderLimitMode::AsIfFilesPlayTogether,
                    (false, true) => RenderLimitMode::ToMasterMix,
                    _ => RenderLimitMode::None,
                };
            Self {
                enabled: raw & 1 != 0,
                mode,
                mono_adjustment,
                target_mode,
                brickwall_limit: raw & 64 != 0,
                brickwall_limit_true_peak: raw & 128 != 0,
                only_normalize_too_loud: raw & 256 != 0,
                only_normalize_too_quiet: raw & 2048 != 0,
                apply_fade_in: raw & 512 != 0,
                apply_fade_out: raw & 1024 != 0,
                trim_start_silence: raw & 16_384 != 0,
                trim_end_silence: raw & 32_768 != 0,
                pad_start_silence: raw & (1 << 16) != 0,
                pad_end_silence: raw & (2 << 16) != 0,
                disable_all_postprocessing: raw & (4 << 16) != 0,
                limit_mode,
            }
        }
    }

    #[repr(u32)]
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, IntEnum, Serialize, Deserialize,
    )]
    pub enum RenderFadeShape {
        Linear = 0,
        EqualPower = 1,
        EqualGain = 2,
        SShape = 3,
    }

    bitflags! {
        #[derive(Serialize, Deserialize)]
        pub struct RenderFadeLowPassFlags: u32 {
            const FADE_IN = 1;
            const FADE_OUT = 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::project_info::{
        BoundsMode, RenderDitherFlags, RenderLimitMode, RenderMonoAdjustment,
        RenderNormalize, RenderNormalizeMode, RenderNormalizeTargetMode,
        RenderSettings,
    };
    use super::{FullRenderSettings, Position};
    use std::path::PathBuf;

    #[test]
    fn render_dither_flags_round_trip() {
        let flags = RenderDitherFlags::DITHER
            | RenderDitherFlags::NOISE_SHAPING
            | RenderDitherFlags::DISABLE_ALL;
        assert_eq!(flags.bits(), 19);
        assert_eq!(RenderDitherFlags::from_bits(19).unwrap(), flags);
    }

    #[test]
    fn render_normalize_round_trip() {
        let settings = RenderNormalize {
            enabled: true,
            mode: RenderNormalizeMode::Peak,
            mono_adjustment: RenderMonoAdjustment::Plus3Db,
            target_mode: RenderNormalizeTargetMode::ToMasterMix,
            brickwall_limit: true,
            brickwall_limit_true_peak: true,
            only_normalize_too_loud: true,
            apply_fade_in: true,
            trim_start_silence: true,
            pad_end_silence: true,
            disable_all_postprocessing: true,
            limit_mode: RenderLimitMode::AsIfFilesPlayTogether,
            ..Default::default()
        };
        let raw = settings.to_raw();
        assert_eq!(RenderNormalize::from_raw(raw), settings);
    }

    #[test]
    fn db_conversion_round_trip() {
        let db = -6.020599913279624;
        assert!(
            (super::linear_to_db(super::db_to_linear(db)) - db).abs() < 1e-12
        );
    }

    #[test]
    fn duration_conversion_round_trip() {
        let duration = std::time::Duration::from_millis(12);
        assert_eq!(
            std::time::Duration::from_secs_f64(duration.as_secs_f64()),
            duration
        );
    }

    #[test]
    fn full_render_settings_serialization_round_trip() {
        let settings = FullRenderSettings {
            settings: Some(RenderSettings::new(
                super::project_info::RenderMode::MasterMix,
                true,
                true,
            )),
            bounds: Some((Position::from(1.0), Position::from(2.0))),
            bounds_mode: Some(BoundsMode::TimeSelection),
            channels_amount: Some(2),
            directory: Some(PathBuf::from("/tmp/render")),
            file: Some("render.wav".to_string()),
            primary_format: Some("wav".to_string()),
            secondary_format: Some("wav".to_string()),
            srate: Some(Some(48000)),
            ..Default::default()
        };

        let serialized = serde_json::to_string(&settings).unwrap();
        let deserialized: FullRenderSettings =
            serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.settings, settings.settings);
        assert_eq!(deserialized.bounds, settings.bounds);
        assert_eq!(deserialized.bounds_mode, settings.bounds_mode);
        assert_eq!(deserialized.channels_amount, settings.channels_amount);
        assert_eq!(deserialized.directory, settings.directory);
        assert_eq!(deserialized.file, settings.file);
        assert_eq!(deserialized.primary_format, settings.primary_format);
        assert_eq!(deserialized.secondary_format, settings.secondary_format);
        assert_eq!(deserialized.srate, settings.srate);
    }
}

/// Returned by [Project::focused_fx]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FocusedFxResult {
    pub track_index: i32,
    pub item_index: Option<usize>,
    pub take_index: Option<usize>,
    pub fx_index: usize,
}

pub struct TracksIterator<'a> {
    project: &'a Project,
    index: usize,
}
impl<'a> TracksIterator<'a> {
    pub fn new(project: &'a Project) -> Self {
        Self { project, index: 0 }
    }
}
impl<'a> Iterator for TracksIterator<'a> {
    type Item = Track;
    fn next(&mut self) -> Option<Self::Item> {
        match self.project.get_track(self.index) {
            Ok(track) => {
                self.index += 1;
                track
            }
            Err(_) => return None,
        }
    }
}
impl<'a> DoubleEndedIterator for TracksIterator<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let count = self.project.n_tracks().unwrap_or(0);
        if self.index == 0 {
            self.index = count;
        }
        match self.project.get_track(self.index - 1) {
            Ok(track) => {
                self.index -= 1;
                if self.index == 0 {
                    self.index = count + 2;
                }
                track
            }
            Err(_) => return None,
        }
    }
}

pub struct SelectedTracksIterator<'a> {
    project: &'a Project,
    index: usize,
}
impl<'a> SelectedTracksIterator<'a> {
    pub fn new(project: &'a Project) -> Self {
        Self { project, index: 0 }
    }
}
impl<'a> Iterator for SelectedTracksIterator<'a> {
    type Item = Track;
    fn next(&mut self) -> Option<Self::Item> {
        let track = self.project.get_selected_track(self.index).ok()?;
        self.index += 1;
        track
    }
}

pub struct ItemsIterator<'a> {
    project: &'a Project,
    index: usize,
}
impl<'a> ItemsIterator<'a> {
    pub fn new(project: &'a Project) -> Self {
        Self { project, index: 0 }
    }
}
impl<'a> Iterator for ItemsIterator<'a> {
    type Item = Item;
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.project.get_item(self.index).ok()?;
        self.index += 1;
        item
    }
}

pub struct SelectedItemsIterator<'a> {
    project: &'a Project,
    index: usize,
}
impl<'a> SelectedItemsIterator<'a> {
    pub fn new(project: &'a Project) -> Self {
        Self { project, index: 0 }
    }
}
impl<'a> Iterator for SelectedItemsIterator<'a> {
    type Item = Item;
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.project.get_selected_item(self.index).ok()?;
        self.index += 1;
        item
    }
}
