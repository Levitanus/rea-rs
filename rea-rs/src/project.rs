pub use crate::utils::WithReaperPtr;
use crate::{
    ptr_wrappers::{MediaItem, MediaTrack, ReaProject},
    utils::{
        as_c_str, as_c_string, as_string, as_string_mut, make_c_string_buf,
        make_string_buf, WithNull,
    },
    Color, CommandId, Immutable, Item, MarkerRegionInfo, MarkerRegionIterator,
    Mutable, PlayRate, Position, ProjectContext, ReaRsError, Reaper,
    TimeRange, TimeRangeKind, TimeSignature, Track, UndoFlags,
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
    BoundsMode, RenderSettings, RenderTail, RenderTailFlags,
};

#[derive(Debug, PartialEq)]
pub struct Project {
    context: ProjectContext,
    checked: bool,
    info_buf_size: usize,
}
impl<'a> WithReaperPtr for Project {
    type Ptr = ReaProject;
    fn get_pointer(&self) -> Self::Ptr {
        unsafe { NonNull::new_unchecked(self.context.to_raw()) }
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid().unwrap();
        self.get_pointer()
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
            let context = match context {
                ProjectContext::CurrentProject => {
                    let ptr = rpr.low().EnumProjects(
                        -1,
                        CString::from(c_str!("")).into_raw(),
                        0,
                    );
                    let ptr = NonNull::new(ptr).expect("expect project");
                    ProjectContext::Proj(ptr)
                }
                ProjectContext::Proj(ptr) => ProjectContext::Proj(ptr),
            };
            Self {
                context,
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
            let mut pr_name = project.name();
            let pr_name: String = pr_name.drain(..pr_name.len() - 4).collect();
            if name == pr_name {
                return Ok(project);
            }
        }
        Err(ReaRsError::InvalidObject("No project with the given name").into())
    }

    /// Get [reaper_medium::ProjectContext] to use with
    /// `rea-rs` crates.
    pub fn context(&self) -> ProjectContext {
        self.require_valid().unwrap();
        self.context
    }

    /// Activate project tab with the project.
    pub fn make_current_project(&self) {
        let low = Reaper::get().low();
        unsafe {
            low.SelectProjectInstance(self.context().to_raw());
        }
    }

    /// If the project tab is active.
    pub fn is_current_project(&self) -> bool {
        unsafe {
            let low = Reaper::get().low();
            let ptr =
                low.EnumProjects(-1, CString::from(c_str!("")).into_raw(), 0);
            self.context.to_raw() == ptr
        }
    }

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
        self.make_current_project();
        f()?;
        current.make_current_project();
        Ok(())
    }

    /// Whether project is dirty (i.e. needing save).
    pub fn is_dirty(&self) -> bool {
        unsafe {
            match Reaper::get().low().IsProjectDirty(self.context().to_raw()) {
                x if x <= 0 => false,
                _ => true,
            }
        }
    }

    /// Mark project dirty (i.e. needing save).
    pub fn mark_dirty(&mut self) {
        unsafe {
            Reaper::get()
                .low()
                .MarkProjectDirty(self.context().to_raw())
        }
    }

    /// Direct way to simulate pause button hit.
    pub fn pause(&mut self) {
        unsafe { Reaper::get().low().OnPauseButtonEx(self.context().to_raw()) }
    }

    pub fn is_paused(&self) -> bool {
        unsafe {
            (Reaper::get().low().GetPlayStateEx(self.context().to_raw()) & 2)
                != 0
        }
    }

    /// Direct way to simulate play button hit.
    pub fn play(&mut self) {
        unsafe { Reaper::get().low().OnPlayButtonEx(self.context().to_raw()) }
    }

    pub fn is_playing(&self) -> bool {
        unsafe {
            (Reaper::get().low().GetPlayStateEx(self.context().to_raw()) & 1)
                != 0
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
    pub fn is_recording(&self) -> bool {
        unsafe {
            (Reaper::get().low().GetPlayStateEx(self.context().to_raw()) & 4)
                != 0
        }
    }

    /// Direct way to simulate stop button hit.
    pub fn stop(&mut self) {
        unsafe { Reaper::get().low().OnStopButtonEx(self.context().to_raw()) }
    }

    pub fn is_stopped(&self) -> bool {
        unsafe {
            (Reaper::get().low().GetPlayStateEx(self.context().to_raw())
                & (1 | 2))
                == 0
        }
    }

    pub fn length(&self) -> Duration {
        unsafe {
            Duration::from_secs_f64(
                Reaper::get()
                    .low()
                    .GetProjectLength(self.context().to_raw()),
            )
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

    pub fn is_loop_enabled(&self) -> bool {
        unsafe {
            Reaper::get()
                .low()
                .GetSetRepeatEx(self.context().to_raw(), -1)
                != 0
        }
    }

    pub fn set_loop_enabled(&mut self, should_loop: bool) {
        unsafe {
            let val = match should_loop {
                true => 1,
                false => 0,
            };
            Reaper::get()
                .low()
                .GetSetRepeatEx(self.context().to_raw(), val);
        }
    }

    /// Close the project.
    pub fn close(self) {
        let rpr = Reaper::get();
        let current = rpr.current_project();
        if current.context == self.context {
            rpr.perform_action(CommandId::new(40860), 0, None);
        } else {
            self.make_current_project();
            rpr.perform_action(CommandId::new(40860), 0, None);
            current.make_current_project();
        }
    }

    /// Get time signature and tempo (BPM) at given position.
    pub fn time_signature_at_position(
        &self,
        position: Position,
    ) -> (TimeSignature, f64) {
        unsafe {
            let (mut num, mut denom, mut tempo) = (
                MaybeUninit::zeroed(),
                MaybeUninit::zeroed(),
                MaybeUninit::zeroed(),
            );
            Reaper::get().low().TimeMap_GetTimeSigAtTime(
                self.context().to_raw(),
                position.into(),
                num.as_mut_ptr(),
                denom.as_mut_ptr(),
                tempo.as_mut_ptr(),
            );
            (
                TimeSignature::new(
                    num.assume_init() as u32,
                    denom.assume_init() as u32,
                ),
                tempo.assume_init(),
            )
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
        let mut name = match name {
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
                self.context().to_raw(),
                is_region,
                start.into(),
                end.into(),
                as_c_str(&name.with_null()).as_ptr(),
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
                self.context.to_raw(),
                info.user_index as i32,
                info.is_region,
                info.position.into(),
                info.rgn_end.into(),
                as_c_str(&info.name.to_string().with_null()).as_ptr(),
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
                self.context.to_raw(),
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
    pub fn iter_markers_and_regions(&self) -> MarkerRegionIterator {
        MarkerRegionIterator::new(self)
    }

    pub fn n_tracks(&self) -> usize {
        unsafe {
            Reaper::get().low().CountTracks(self.context().to_raw()) as usize
        }
    }

    pub fn n_selected_tracks(&self) -> usize {
        unsafe {
            Reaper::get()
                .low()
                .CountSelectedTracks2(self.context().to_raw(), false)
                as usize
        }
    }

    pub fn n_items(&self) -> usize {
        unsafe {
            Reaper::get().low().CountMediaItems(self.context().to_raw())
                as usize
        }
    }

    pub fn n_selected_items(&self) -> usize {
        unsafe {
            Reaper::get()
                .low()
                .CountSelectedMediaItems(self.context().to_raw())
                as usize
        }
    }

    pub fn n_tempo_markers(&self) -> usize {
        unsafe {
            Reaper::get()
                .low()
                .CountTempoTimeSigMarkers(self.context().to_raw())
                as usize
        }
    }

    pub fn n_markers(&self) -> usize {
        self.count_markers_and_regions().0
    }
    pub fn n_regions(&self) -> usize {
        self.count_markers_and_regions().1
    }

    fn count_markers_and_regions(&self) -> (usize, usize) {
        unsafe {
            let (mut n_markers, mut n_regions) =
                (MaybeUninit::zeroed(), MaybeUninit::zeroed());
            let result = Reaper::get().low().CountProjectMarkers(
                self.context().to_raw(),
                n_markers.as_mut_ptr(),
                n_regions.as_mut_ptr(),
            );
            if result <= 0 {
                return (0, 0);
            }
            (
                n_markers.assume_init() as usize,
                n_regions.assume_init() as usize,
            )
        }
    }

    pub fn add_track(
        &mut self,
        index: impl Into<Option<usize>>,
        name: impl Into<String>,
    ) -> Track<Mutable> {
        let n_tracks = self.n_tracks();
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
        self.with_current_project(|| {
            Reaper::get().low().InsertTrackAtIndex(index as i32, true);
            Ok(())
        })
        .unwrap();
        let mut track =
            self.get_track_mut(index).expect("should have valid track");
        let name: String = name.into();
        if !name.is_empty() {
            track.set_name(name).expect("Can not set track name.")
        }
        track
    }

    pub fn get_track(&self, index: usize) -> Option<Track<Immutable>> {
        let ptr = self.get_track_ptr(index)?;
        let track = Track::new(self, ptr);
        Some(track)
    }
    pub fn get_track_mut(&mut self, index: usize) -> Option<Track<Mutable>> {
        let ptr = self.get_track_ptr(index)?;
        let track = Track::new(self, ptr);
        Some(track)
    }
    pub(crate) fn get_track_ptr(&self, index: usize) -> Option<MediaTrack> {
        unsafe {
            let ptr = Reaper::get()
                .low()
                .GetTrack(self.context.to_raw(), index as i32);
            match MediaTrack::new(ptr) {
                None => None,
                Some(ptr) => Some(ptr),
            }
        }
    }

    pub fn get_selected_track(
        &self,
        index: usize,
    ) -> Option<Track<Immutable>> {
        let ptr = self.get_selected_track_ptr(index)?;
        let track = Track::new(self, ptr);
        Some(track)
    }
    pub fn get_selected_track_mut(
        &mut self,
        index: usize,
    ) -> Option<Track<Mutable>> {
        let ptr = self.get_selected_track_ptr(index)?;
        let track = Track::new(self, ptr);
        Some(track)
    }
    fn get_selected_track_ptr(&self, index: usize) -> Option<MediaTrack> {
        unsafe {
            let ptr = MediaTrack::new(
                Reaper::get()
                    .low()
                    .GetSelectedTrack(self.context().to_raw(), index as i32),
            );
            match ptr {
                None => None,
                Some(ptr) => Some(ptr),
            }
        }
    }

    pub fn get_master_track(&self) -> Track<Immutable> {
        Track::new(self, self.get_master_track_ptr())
    }
    pub fn get_master_track_mut(&mut self) -> Track<Mutable> {
        Track::new(self, self.get_master_track_ptr())
    }
    fn get_master_track_ptr(&self) -> MediaTrack {
        unsafe {
            NonNull::new(
                Reaper::get().low().GetMasterTrack(self.context().to_raw()),
            )
            .expect("should get master track")
        }
    }

    pub fn iter_tracks(&self) -> TracksIterator {
        TracksIterator::new(self)
    }
    pub fn iter_tracks_mut(
        &mut self,
        mut f: impl FnMut(Track<Mutable>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        for track in TracksIterator::new(self) {
            let track = Track::<Mutable>::new(self, track.get());
            f(track)?
        }
        Ok(())
    }

    pub fn iter_selected_tracks(&self) -> SelectedTracksIterator {
        SelectedTracksIterator::new(self)
    }
    pub fn iter_selected_tracks_mut(
        &mut self,
        mut f: impl FnMut(Track<Mutable>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        for track in SelectedTracksIterator::new(self) {
            let track = Track::<Mutable>::new(self, track.get());
            f(track)?
        }
        Ok(())
    }

    pub fn iter_items(&'a self) -> ItemsIterator<'a> {
        ItemsIterator::new(self)
    }

    pub fn iter_selected_items(&'a self) -> SelectedItemsIterator<'a> {
        SelectedItemsIterator::new(self)
    }

    pub fn get_item(&self, index: usize) -> Option<Item<Immutable>> {
        let item = Item::new(self, self.get_item_ptr(index)?);
        Some(item)
    }
    pub fn get_item_mut(&mut self, index: usize) -> Option<Item<Mutable>> {
        let item = Item::new(self, self.get_item_ptr(index)?);
        Some(item)
    }
    fn get_item_ptr(&self, index: usize) -> Option<MediaItem> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetMediaItem(self.context().to_raw(), index as i32)
        };
        match MediaItem::new(ptr) {
            None => None,
            x => x,
        }
    }

    pub fn get_selected_item(&self, index: usize) -> Option<Item<Immutable>> {
        match self.selected_item_ptr(index) {
            Some(ptr) => Some(Item::new(self, ptr)),
            None => None,
        }
    }
    pub fn get_selected_item_mut(
        &mut self,
        index: usize,
    ) -> Option<Item<Mutable>> {
        match self.selected_item_ptr(index) {
            Some(ptr) => Some(Item::new(self, ptr)),
            None => None,
        }
    }
    fn selected_item_ptr(&self, index: usize) -> Option<MediaItem> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetSelectedMediaItem(self.context().to_raw(), index as i32)
        };
        MediaItem::new(ptr)
    }

    /// Glue items (action shortcut).
    pub fn glue_selected_items(&mut self, within_time_selection: bool) {
        let action_id = match within_time_selection {
            true => CommandId::new(41588),
            false => CommandId::new(40362),
        };
        Reaper::get().perform_action(action_id, 0, Some(self))
    }

    pub fn any_track_solo(&self) -> bool {
        unsafe { Reaper::get().low().AnyTrackSolo(self.context().to_raw()) }
    }

    /// Verbose way to make undo.
    ///
    /// # Safety
    ///
    /// [Project::end_undo_block] has to be called after.
    pub fn begin_undo_block(&mut self) {
        unsafe {
            Reaper::get()
                .low()
                .Undo_BeginBlock2(self.context().to_raw());
        }
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
    ) {
        unsafe {
            Reaper::get().low().Undo_EndBlock2(
                self.context().to_raw(),
                as_c_str(&name.into().with_null()).as_ptr(),
                flags.bits() as i32,
            )
        }
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
    pub fn undo(&mut self) -> Result<(), ReaRsError> {
        unsafe {
            match Reaper::get().low().Undo_DoUndo2(self.context().to_raw()) {
                0 => Err(ReaRsError::UnsuccessfulOperation("can not do undo")),
                _ => Ok(()),
            }
        }
    }

    /// Try to redo last undone action.
    pub fn redo(&mut self) -> Result<(), ReaRsError> {
        unsafe {
            match Reaper::get().low().Undo_DoRedo2(self.context().to_raw()) {
                0 => Err(ReaRsError::UnsuccessfulOperation("can not do redo")),
                _ => Ok(()),
            }
        }
    }

    /// Position of next audio block being processed.
    ///
    /// [Project::play_position]
    pub fn next_buffer_position(&self) -> Position {
        unsafe {
            Position::from(
                Reaper::get()
                    .low()
                    .GetPlayPosition2Ex(self.context().to_raw()),
            )
        }
    }

    /// Latency-compensated actual-what-you-hear position.
    ///
    /// [Project::next_buffer_position]
    pub fn play_position(&self) -> Position {
        unsafe {
            Position::from(
                Reaper::get()
                    .low()
                    .GetPlayPositionEx(self.context().to_raw()),
            )
        }
    }

    /// Bypass (`true`) or un-bypass (`false`) FX on all tracks.
    pub fn bypass_fx_on_all_tracks(&mut self, bypass: bool) {
        self.with_current_project(|| -> anyhow::Result<()> {
            Reaper::get().low().BypassFxAllTracks(bypass as i32);
            Ok(())
        })
        .unwrap()
    }

    /// Get the name of the next action in redo queue, if any.
    pub fn next_redo(&self) -> Option<String> {
        unsafe {
            let ptr =
                Reaper::get().low().Undo_CanRedo2(self.context().to_raw());
            match ptr.is_null() {
                true => None,
                false => {
                    Some(as_string(ptr).expect("can not convert to string"))
                }
            }
        }
    }

    /// Get the name of the next action in undo queue, if any.
    pub fn next_undo(&self) -> Option<String> {
        unsafe {
            let ptr =
                Reaper::get().low().Undo_CanUndo2(self.context().to_raw());
            match ptr.is_null() {
                true => None,
                false => {
                    Some(as_string(ptr).expect("can not convert to string"))
                }
            }
        }
    }

    /// Edit cursor position.
    pub fn get_cursor_position(&self) -> Position {
        unsafe {
            Position::from(
                Reaper::get()
                    .low()
                    .GetCursorPositionEx(self.context().to_raw()),
            )
        }
    }

    /// Set edit cursor position.
    pub fn set_cursor_position(
        &mut self,
        position: Position,
        move_view: bool,
        seek_play: bool,
    ) {
        unsafe {
            Reaper::get().low().SetEditCurPos2(
                self.context().to_raw(),
                position.into(),
                move_view,
                seek_play,
            )
        }
    }

    /// Disarm record on all tracks.
    pub fn disarm_rec_on_all_tracks(&mut self) {
        self.with_current_project(|| -> anyhow::Result<()> {
            Reaper::get().low().ClearAllRecArmed();
            Ok(())
        })
        .unwrap()
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
    ) -> anyhow::Result<String> {
        unsafe {
            let size = 1024;
            let mut param_name: String = param_name.into();
            let buf = make_string_buf(size);
            let result = Reaper::get().low().GetSetProjectInfo_String(
                self.context().to_raw(),
                as_c_str(&param_name.with_null()).as_ptr(),
                buf,
                false,
            );
            let result_string =
                as_string_mut(buf).expect("Cannot convert result to string");
            debug!("{}", result_string);
            match result {
                true => Ok(result_string),
                false => {
                    Err(ReaRsError::InvalidObject("Can not get Project info.")
                        .into())
                }
            }
        }
    }

    fn set_info_string(
        &mut self,
        param_name: impl Into<String>,
        value: impl Into<String>,
    ) -> anyhow::Result<()> {
        unsafe {
            let mut param_name: String = param_name.into();
            let value: String = value.into();
            let val = as_c_string(&value).into_raw();
            let result = Reaper::get().low().GetSetProjectInfo_String(
                self.context().to_raw(),
                as_c_str(&param_name.with_null()).as_ptr(),
                val,
                true,
            );
            match result {
                false => Err(ReaRsError::InvalidObject(
                    "can not set value to project.",
                )
                .into()),
                true => Ok(()),
            }
            // Ok(())
        }
    }

    pub fn name(&self) -> String {
        unsafe {
            let name = make_c_string_buf(self.info_buf_size);
            let name = name.into_raw();
            Reaper::get().low().GetProjectName(
                self.context().to_raw(),
                name,
                self.info_buf_size as i32,
            );
            as_string_mut(name).expect("shoudl return project name")
        }
    }

    ///  title field from Project Settings/Notes dialog
    pub fn get_title(&self) -> anyhow::Result<String> {
        self.get_info_string("PROJECT_TITLE")
    }

    ///  title field from Project Settings/Notes dialog
    pub fn set_title(
        &mut self,
        title: impl Into<String>,
    ) -> anyhow::Result<()> {
        self.set_info_string("PROJECT_TITLE", title)
    }

    ///  author field from Project Settings/Notes dialog
    pub fn get_author(&self) -> anyhow::Result<String> {
        self.get_info_string("PROJECT_AUTHOR")
    }

    ///  author field from Project Settings/Notes dialog
    pub fn set_author(
        &mut self,
        author: impl Into<String>,
    ) -> anyhow::Result<()> {
        self.set_info_string("PROJECT_AUTHOR", author)
    }

    pub fn get_marker_guid(
        &self,
        marker_index: usize,
    ) -> anyhow::Result<String> {
        let pattern = format!("MARKER_GUID:{:?}", marker_index);
        warn!("this function, probably, not working properly");
        self.get_info_string(pattern)
    }

    pub fn get_track_group_name(
        &self,
        group_index: usize,
    ) -> anyhow::Result<String> {
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
    ) -> anyhow::Result<()> {
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
    ) -> anyhow::Result<PathBuf> {
        let param_name = match secondary_path {
            false => "RECORD_PATH",
            true => "RECORD_PATH_SECONDARY",
        };
        Ok(PathBuf::from(self.get_info_string(param_name)?))
    }

    /// Project path.
    pub fn get_path(&self) -> anyhow::Result<PathBuf> {
        unsafe {
            let buf = make_c_string_buf(self.info_buf_size).into_raw();
            Reaper::get().low().GetProjectPathEx(
                self.context().to_raw(),
                buf,
                self.info_buf_size as i32,
            );
            let result = PathBuf::from(as_string_mut(buf)?);
            Ok(result)
        }
    }

    pub fn set_record_path(
        &mut self,
        secondary_path: bool,
        directory: impl Into<PathBuf>,
    ) -> anyhow::Result<()> {
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

    pub fn get_render_directory(&self) -> anyhow::Result<PathBuf> {
        Ok(PathBuf::from(self.get_info_string("RENDER_FILE")?))
    }

    pub fn set_render_directory(
        &mut self,
        directory: impl Into<PathBuf>,
    ) -> anyhow::Result<()> {
        let directory: PathBuf = directory.into();
        self.set_info_string(
            "RENDER_FILE",
            directory
                .to_str()
                .ok_or(ReaRsError::Str("Can not convert path to str"))?,
        )
    }

    ///  render file name (may contain wildcards)
    pub fn get_render_file(&self) -> anyhow::Result<String> {
        self.get_info_string("RENDER_PATTERN")
    }

    ///  render file name (may contain wildcards)
    pub fn set_render_file(
        &mut self,
        file: impl Into<String>,
    ) -> anyhow::Result<()> {
        self.set_info_string("RENDER_PATTERN", file)
    }

    /// base64-encoded sink configuration (see project files, etc).
    ///
    /// Set secondary_format to true, if you want the secondary render section
    /// format.
    pub fn get_render_format(
        &self,
        secondary_format: bool,
    ) -> anyhow::Result<String> {
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
    ) -> anyhow::Result<()> {
        let param = match secondary_format {
            false => "RENDER_FORMAT",
            true => "RENDER_FORMAT2",
        };
        self.set_info_string(param, format)
    }

    /// Filenames, that will be rendered.
    pub fn get_render_targets(&self) -> anyhow::Result<Vec<String>> {
        Ok(self
            .get_info_string("RENDER_TARGETS")?
            .split(",")
            .map(|i| String::from(i))
            .collect::<Vec<String>>())
    }

    /// Will return `PlayRate::from(1.0)` in normal conditions.
    pub fn get_play_rate(&self, position: impl Into<Position>) -> PlayRate {
        unsafe {
            PlayRate::from(Reaper::get().low().Master_GetPlayRateAtTime(
                position.into().into(),
                self.context().to_raw(),
            ))
        }
    }

    pub fn save(&mut self, force_save_as: bool) {
        unsafe {
            Reaper::get()
                .low()
                .Main_SaveProject(self.context().to_raw(), force_save_as)
        }
    }

    pub fn select_all_items(&mut self, should_select: bool) {
        unsafe {
            Reaper::get()
                .low()
                .SelectAllMediaItems(self.context().to_raw(), should_select)
        }
    }

    pub fn select_all_tracks(&mut self, should_select: bool) {
        self.with_current_project(|| {
            let id = match should_select {
                true => CommandId::new(40297),
                false => CommandId::new(40296),
            };
            Reaper::get().perform_action(id, 0, Some(self));
            Ok(())
        })
        .unwrap()
    }

    pub fn solo_all_tracks(&mut self, solo: bool) {
        self.with_current_project(|| {
            Reaper::get().low().SoloAllTracks(match solo {
                true => 1,
                false => 0,
            });
            Ok(())
        })
        .expect("should not be error in inner closure.")
    }
    pub fn mute_all_tracks(&mut self, mute: bool) {
        self.with_current_project(|| {
            Reaper::get().low().MuteAllTracks(mute);
            Ok(())
        })
        .expect("should not be error in inner closure.")
    }
    pub fn clear_all_rec_armed_tracks(&mut self) {
        self.with_current_project(|| {
            Reaper::get().low().ClearAllRecArmed();
            Ok(())
        })
        .expect("should not be error in inner closure.")
    }

    fn get_info_value(&self, param_name: impl Into<String>) -> f64 {
        unsafe {
            Reaper::get().low().GetSetProjectInfo(
                self.context().to_raw(),
                as_c_str(&param_name.into().with_null()).as_ptr(),
                0.0,
                false,
            )
        }
    }
    fn set_info_value(&mut self, param_name: impl Into<String>, value: f64) {
        unsafe {
            Reaper::get().low().GetSetProjectInfo(
                self.context().to_raw(),
                as_c_str(&param_name.into().with_null()).as_ptr(),
                value,
                true,
            );
        }
    }

    pub fn get_render_bounds_mode(&self) -> BoundsMode {
        let val = self.get_info_value("RENDER_BOUNDSFLAG");
        BoundsMode::from_int(val as u32)
            .expect("should convert to bounds mode.")
    }
    pub fn set_render_bounds_mode(&mut self, mode: BoundsMode) {
        let mode = mode.int_value();
        self.set_info_value("RENDER_BOUNDSFLAG", mode as f64)
    }

    pub fn get_render_settings(&self) -> RenderSettings {
        let mut settings =
            RenderSettings::from_raw(self.get_info_value("RENDER_SETTINGS"));
        settings.add_to_project =
            self.get_info_value("RENDER_ADDTOPROJ") as u32 == 1;
        settings
    }
    pub fn set_render_settings(&mut self, settings: RenderSettings) {
        self.set_info_value("RENDER_SETTINGS", settings.to_raw());
        let add_to_proj = match settings.add_to_project {
            true => 1,
            false => 0,
        };
        self.set_info_value("RENDER_ADDTOPROJ", add_to_proj as f64);
    }

    pub fn get_render_channels_amount(&self) -> u32 {
        self.get_info_value("RENDER_CHANNELS") as u32
    }
    pub fn set_render_channels_amount(&mut self, channels_amount: u32) {
        self.set_info_value("RENDER_CHANNELS", channels_amount as f64)
    }

    /// If None — then sample rate from Reaper settings used.
    pub fn get_srate(&self) -> Option<u32> {
        match self.get_info_value("PROJECT_SRATE") as u32 {
            0 => None,
            val => Some(val),
        }
    }
    /// If None — then sample rate from Reaper settings used.
    pub fn set_srate(&mut self, srate: impl Into<Option<u32>>) {
        let srate = srate.into();
        self.set_info_value("PROJECT_SRATE", srate.unwrap_or(0) as f64);
        match srate {
            None => self.set_info_value("PROJECT_SRATE_USE", 1.0),
            Some(_) => self.set_info_value("PROJECT_SRATE_USE", 0.0),
        };
    }

    /// If None — then project sample rate used.
    pub fn get_render_srate(&self) -> Option<u32> {
        match self.get_info_value("RENDER_SRATE") as u32 {
            0 => None,
            val => Some(val),
        }
    }
    /// If None — then project sample rate used.
    pub fn set_render_srate(&mut self, srate: impl Into<Option<u32>>) {
        let srate = srate.into();
        self.set_info_value("RENDER_SRATE", srate.unwrap_or(0) as f64);
    }

    /// Get in tuple (start, end)
    ///
    /// Valid only when [Project::get_render_bounds_mode] is
    /// [BoundsMode::Custom]
    pub fn get_render_bounds(&self) -> (Position, Position) {
        let start = self.get_info_value("RENDER_STARTPOS");
        let end = self.get_info_value("RENDER_ENDPOS");
        (Position::from(start), Position::from(end))
    }
    /// Valid only when [Project::get_render_bounds_mode] is
    /// [BoundsMode::Custom]
    pub fn set_render_bounds(
        &mut self,
        start: impl Into<Position>,
        end: impl Into<Position>,
    ) {
        self.set_info_value("RENDER_STARTPOS", start.into().into());
        self.set_info_value("RENDER_ENDPOS", end.into().into());
    }

    pub fn get_render_tail(&self) -> RenderTail {
        let tail =
            Duration::from_millis(self.get_info_value("RENDER_TAILMS") as u64);
        let flags_raw = self.get_info_value("RENDER_TAILFLAG");
        let flags = RenderTailFlags::from_bits(flags_raw as u32)
            .expect("Can not get tail flags");
        RenderTail { tail, flags }
    }
    pub fn set_render_tail(&mut self, render_tail: RenderTail) {
        let tail = render_tail.tail.as_millis() as f64;
        let flags = render_tail.flags.bits();
        self.set_info_value("RENDER_TAILMS", tail);
        self.set_info_value("RENDER_TAILFLAG", flags as f64);
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
        /// Add rendered files to project.
        pub add_to_project: bool,
    }
    impl RenderSettings {
        pub fn new(
            mode: RenderMode,
            use_mono: bool,
            add_to_project: bool,
        ) -> Self {
            Self {
                mode,
                use_mono,
                add_to_project,
            }
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
                add_to_project: false,
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
    type Item = Track<'a, Immutable>;
    fn next(&mut self) -> Option<Self::Item> {
        let track = self.project.get_track(self.index);
        self.index += 1;
        track
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
    type Item = Track<'a, Immutable>;
    fn next(&mut self) -> Option<Self::Item> {
        let track = self.project.get_selected_track(self.index);
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
    type Item = Item<'a, Immutable>;
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.project.get_item(self.index);
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
    type Item = Item<'a, Immutable>;
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.project.get_selected_item(self.index);
        self.index += 1;
        item
    }
}
