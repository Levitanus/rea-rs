use core::panic;
use std::{
    ffi::CString,
    marker::PhantomData,
    mem::{transmute, MaybeUninit},
    path::PathBuf,
    ptr::{null_mut, NonNull},
};

use bitflags::bitflags;
use int_enum::IntEnum;
use reaper_medium::{MediaItem, MediaTrack, TrackEnvelope};
use serde_derive::{Deserialize, Serialize};

use crate::{
    errors::{ReaperError, ReaperResult, ReaperStaticResult},
    utils::{
        as_c_str, as_c_string, as_string, as_string_mut, make_c_string_buf,
        WithNull,
    },
    AudioAccessor, AutomationMode, Color, Envelope, EnvelopeSelector,
    FXParent, GenericSend, GetLength, HardwareSend, HardwareSocket, Immutable,
    Item, KnowsProject, Mutable, Pan, PanLaw, PanLawMode, Position,
    PositionPixel, ProbablyMutable, Project, Reaper, RecInput, RecMode,
    RecOutMode, RectPixel, SendIntType, SoloMode, TimeMode, TrackFX,
    TrackFolderState, TrackReceive, TrackSend, VUMode, Volume, WithReaperPtr,
    FX, GUID,
};

#[derive(Debug, PartialEq)]
pub struct Track<'a, T: ProbablyMutable> {
    ptr: MediaTrack,
    should_check: bool,
    project: &'a Project,
    /// Used to get string info about the track.
    /// default is 512
    pub info_buf_size: usize,
    phantom_mut: PhantomData<T>,
}
impl<'a, T: ProbablyMutable> WithReaperPtr<'a> for Track<'a, T> {
    type Ptr = MediaTrack;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid_2(&self.project).unwrap();
        self.ptr
    }
    fn make_unchecked(&mut self) {
        self.should_check = false
    }
    fn make_checked(&mut self) {
        self.should_check = true
    }
    fn should_check(&self) -> bool {
        self.should_check
    }
}
impl<'a, T: ProbablyMutable> FXParent<'a, TrackFX<'a, Immutable>>
    for Track<'a, T>
{
    fn n_fx(&self) -> usize {
        unsafe {
            Reaper::get().low().TrackFX_GetCount(self.get().as_ptr()) as usize
        }
    }
    fn get_fx(&'a self, index: usize) -> Option<TrackFX<Immutable>> {
        let fx = TrackFX::from_index(unsafe { transmute(self) }, index);
        fx
    }
}
impl<'a, T: ProbablyMutable> KnowsProject for Track<'a, T> {
    fn project(&self) -> &Project {
        self.project
    }
}
impl<'a, T: ProbablyMutable> Track<'a, T> {
    pub fn new(project: &'a Project, pointer: MediaTrack) -> Self {
        Self {
            ptr: pointer,
            project,
            should_check: true,
            info_buf_size: 512,
            phantom_mut: PhantomData,
        }
    }
    pub fn from_index(project: &'a Project, index: usize) -> Option<Self> {
        let ptr = project.get_track_ptr(index)?;
        Some(Self::new(project, ptr))
    }
    pub fn from_name(
        project: &'a Project,
        name: impl Into<String>,
    ) -> Option<Self> {
        let name = name.into();
        let track = project.iter_tracks().find(|tr| tr.name() == name)?;
        let index = track.index();
        Self::from_index(project, index)
    }
    pub fn from_point(
        project: &'a Project,
        point: PositionPixel,
    ) -> Option<Self> {
        let mut info_out = MaybeUninit::zeroed();
        let ptr = unsafe {
            Reaper::get().low().GetTrackFromPoint(
                point.x as i32,
                point.y as i32,
                info_out.as_mut_ptr(),
            )
        };
        match MediaTrack::new(ptr) {
            None => None,
            Some(ptr) => Some(Self::new(project, ptr)),
        }
    }
    pub fn from_guid(project: &'a Project, guid: GUID) -> Option<Self> {
        let track = project.iter_tracks().find(|tr| tr.guid() == guid)?;
        let index = track.index();
        Self::from_index(project, index)
    }

    fn get_info_string(
        &self,
        category: impl Into<String>,
    ) -> ReaperResult<String> {
        unsafe {
            let buf = make_c_string_buf(self.info_buf_size).into_raw();
            let result = Reaper::get().low().GetSetMediaTrackInfo_String(
                self.get().as_ptr(),
                as_c_str(&category.into().with_null()).as_ptr(),
                buf,
                false,
            );
            match result {
                false => Err(ReaperError::UnsuccessfulOperation(
                    "Can not get info string",
                )
                .into()),
                true => Ok(as_string_mut(buf)?),
            }
        }
    }

    pub fn name(&self) -> String {
        self.get_info_string("P_NAME")
            .expect("Can not get track name")
    }
    pub fn icon(&self) -> Option<PathBuf> {
        let string = self
            .get_info_string("P_ICON")
            .expect("Can not get track icon");
        match string.is_empty() {
            true => None,
            false => Some(PathBuf::from(string)),
        }
    }
    pub fn mcp_layout(&self) -> Option<String> {
        let string = self
            .get_info_string("P_MCP_LAYOUT")
            .expect("Can not get layout");
        match string.is_empty() {
            true => None,
            false => Some(string),
        }
    }
    pub fn tcp_layout(&self) -> Option<String> {
        let string = self
            .get_info_string("P_TCP_LAYOUT")
            .expect("Can not get layout");
        match string.is_empty() {
            true => None,
            false => Some(string),
        }
    }
    /// allows querying screen position + size of track WALTER elements
    /// (tcp.size queries screen position and size of entire TCP, etc).
    pub fn ui_element_rect(
        &self,
        element: impl Into<String>,
    ) -> ReaperResult<RectPixel> {
        let mut category = String::from("P_UI_RECT:");
        category += &element.into();
        let result = self.get_info_string(category)?;
        let mut tokens = result.split(" ");
        let x: u32 = tokens.next().unwrap().parse().unwrap();
        let y: u32 = tokens.next().unwrap().parse().unwrap();
        let width: u32 = tokens.next().unwrap().parse().unwrap();
        let height: u32 = tokens.next().unwrap().parse().unwrap();
        Ok(RectPixel::new(x, y, width, height))
    }

    /// Get Vec of RazorEdit areas.
    ///
    /// Can be empty.
    ///
    /// For every envelope selected will be returned dedicated
    /// [RazorEdit] with envelope GUID. RazorEdit without GUID
    /// corresponding to [Track]
    pub fn razor_edits(&self) -> Vec<RazorEdit> {
        let result = self
            .get_info_string("P_RAZOREDITS_EXT")
            .expect("Can not get razor edits");
        result
            .split(",")
            .filter(|v| !v.is_empty())
            .map(|item| RazorEdit::from_str(item))
            .collect()
    }

    pub fn guid(&self) -> GUID {
        GUID::from_string(
            self.get_info_string("GUID")
                .expect("Can not retrieve guid string"),
        )
        .expect("Can not get GUID from guid string")
    }

    fn get_info_value(&self, category: impl Into<String>) -> f64 {
        unsafe {
            Reaper::get().low().GetMediaTrackInfo_Value(
                self.get().as_ptr(),
                as_c_str(&category.into().with_null()).as_ptr(),
            )
        }
    }

    pub fn index(&self) -> usize {
        match self.get_info_value("IP_TRACKNUMBER") as i32 {
            0 => panic!("Track is not found!"),
            idx => idx as usize - 1,
        }
    }
    pub fn muted(&self) -> bool {
        self.get_info_value("B_MUTE") != 0.0
    }
    pub fn phase_flipped(&self) -> bool {
        self.get_info_value("B_PHASE") != 0.0
    }
    /// True, while playback and rec_armed and monitored.
    pub fn is_currently_monitored(&self) -> bool {
        self.get_info_value("B_RECMON_IN_EFFECT") != 0.0
    }
    pub fn solo(&self) -> SoloMode {
        let value = self.get_info_value("I_SOLO");
        SoloMode::from_int(value as i32).expect("Can not convert to SoloMode.")
    }
    /// when set, if anything else is soloed and this track is not muted, this
    /// track acts soloed
    pub fn solo_defeat(&self) -> bool {
        self.get_info_value("B_SOLO_DEFEAT") != 0.0
    }
    pub fn fx_bypassed(&self) -> bool {
        self.get_info_value("I_FXEN") == 0.0
    }
    pub fn rec_armed(&self) -> bool {
        match self.get_info_value("I_RECARM") as i32 {
            0 => false,
            1 => true,
            x => panic!("Unexpected result: {}", x),
        }
    }
    pub fn rec_input(&self) -> RecInput {
        RecInput::from_raw(self.get_info_value("I_RECINPUT"))
            .expect("Can not convert to RecordInput.")
    }

    pub fn rec_mode(&self) -> RecMode {
        RecMode::from_int(self.get_info_value("I_RECMODE") as i32)
            .expect("Can not convert to RecordingMode")
    }

    /// If rec_mode records output. Otherwise — None.
    pub fn rec_out_mode(&self) -> Option<RecOutMode> {
        RecOutMode::from_raw(self.get_info_value("I_RECMODE_FLAGS") as u32)
    }
    pub fn rec_monitoring(&self) -> RecMonitoring {
        let mode = self.get_info_value("I_RECMON") as u32;
        let monitor_items = self.get_info_value("I_RECMONITEMS") != 0.0;
        RecMonitoring::new(mode, monitor_items)
    }
    /// True if automatically armed when track is selected.
    pub fn auto_rec_arm(&self) -> bool {
        self.get_info_value("B_AUTO_RECARM") != 0.0
    }
    pub fn vu_mode(&self) -> VUMode {
        VUMode::from_raw(self.get_info_value("I_VUMODE") as u32)
    }
    pub fn n_channels(&self) -> usize {
        self.get_info_value("I_NCHAN") as usize
    }
    pub fn n_items(&self) -> usize {
        unsafe {
            Reaper::get()
                .low()
                .GetTrackNumMediaItems(self.get().as_ptr())
                as usize
        }
    }
    pub fn n_envelopes(&self) -> usize {
        unsafe {
            Reaper::get().low().CountTrackEnvelopes(self.get().as_ptr())
                as usize
        }
    }
    pub fn selected(&self) -> bool {
        self.get_info_value("I_SELECTED") != 0.0
    }
    pub fn dimensions(&self) -> TrackDimensions {
        let tcp_height = self.get_info_value("I_TCPH") as u32;
        let tcp_height_with_env = self.get_info_value("I_WNDH") as u32;
        let tcp_pos_y = self.get_info_value("I_TCPY") as u32;
        let mcp_pos_x = self.get_info_value("I_MCPX") as u32;
        let mcp_pos_y = self.get_info_value("I_MCPY") as u32;
        let mcp_width = self.get_info_value("I_MCPW") as u32;
        let mcp_height = self.get_info_value("I_MCPH") as u32;
        TrackDimensions {
            tcp_height,
            tcp_height_with_env,
            tcp_pos_y,
            mcp_pos_x,
            mcp_pos_y,
            mcp_width,
            mcp_height,
        }
    }
    pub fn folder_state(&self) -> TrackFolderState {
        let depth = self.get_info_value("I_FOLDERDEPTH") as i32;
        let compact = self.get_info_value("I_FOLDERCOMPACT") as u32;
        TrackFolderState::from_raw(depth, compact)
    }

    /// Get channel and hardware midi out socket, if any.
    ///
    /// ch=0 → all channels
    pub fn midi_hardware_out(&self) -> Option<(u8, HardwareSocket)> {
        let value = self.get_info_value("I_MIDIHWOUT") as i32;
        if value < 0 {
            return None;
        }
        let channel = value & 0b11111;
        let out_idx = value >> 5;
        let socket = Reaper::get().get_midi_output(out_idx as usize)?;
        (channel as u8, socket).into()
    }

    pub fn performance_flags(&self) -> TrackPerformanceFlags {
        TrackPerformanceFlags::from_bits_truncate(
            self.get_info_value("I_PERFFLAGS") as u8,
        )
    }

    /// If track height was overrided by script.
    pub fn height_override(&self) -> Option<u32> {
        match self.get_info_value("I_HEIGHTOVERRIDE") as u32 {
            0 => None,
            x => Some(x),
        }
    }
    /// `None`, if [Track::height_override] is `None`.
    pub fn height_lock(&self) -> Option<bool> {
        self.height_override()?;
        Some(self.get_info_value("B_HEIGHTLOCK") != 0.0)
    }

    pub fn volume(&self) -> Volume {
        Volume::from(self.get_info_value("D_VOL"))
    }
    pub fn pan(&self) -> TrackPan {
        let pan_mode = self.get_info_value("I_PANMODE") as u32;
        match pan_mode {
            0 => TrackPan::BalanceLegacy(self.get_info_value("D_PAN").into()),
            3 => TrackPan::Balance(self.get_info_value("D_PAN").into()),
            5 => TrackPan::Stereo(
                self.get_info_value("D_PAN").into(),
                self.get_info_value("D_WIDTH").into(),
            ),
            6 => TrackPan::Dual(
                self.get_info_value("D_DUALPANL").into(),
                self.get_info_value("D_DUALPANR").into(),
            ),
            _ => panic!("Can not infer pan mode!"),
        }
    }

    pub fn pan_law(&self) -> PanLaw {
        self.get_info_value("D_PANLAW").into()
    }
    pub fn pan_law_mode(&self) -> PanLawMode {
        (self.get_info_value("I_PANLAW_FLAGS") as i32).into()
    }

    pub fn visible_in_mcp(&self) -> bool {
        self.get_info_value("B_SHOWINMIXER") != 0.0
    }
    pub fn visible_in_tcp(&self) -> bool {
        self.get_info_value("B_SHOWINTCP") != 0.0
    }
    /// Get channel offset of parent track. None, if no parent send.
    pub fn parent_send(&self) -> Option<u32> {
        let is_enabled = self.get_info_value("B_MAINSEND") != 0.0;
        if !is_enabled {
            return None;
        }
        Some(self.get_info_value("C_MAINSEND_OFFS") as u32)
    }

    pub fn free_item_positioning(&self) -> bool {
        self.get_info_value("B_FREEMODE") != 0.0
    }

    pub fn time_base(&self) -> TimeMode {
        let value = self.get_info_value("C_BEATATTACHMODE") as i32;
        TimeMode::from_int(value).expect("Can not convert to TimeMode.")
    }

    /// scale of fx+send area in MCP (0=minimum allowed, 1=maximum allowed)
    pub fn mcp_fx_send_scale(&self) -> f64 {
        self.get_info_value("F_MCP_FXSEND_SCALE")
    }
    /// scale of fx parameter area in MCP (0=minimum allowed, 1=maximum
    /// allowed)
    pub fn mcp_fx_param_scale(&self) -> f64 {
        self.get_info_value("F_MCP_FXPARM_SCALE")
    }
    /// scale of send area as proportion of the fx+send total area (0=minimum
    /// allowed, 1=maximum allowed)
    pub fn mcp_fx_send_region_scale(&self) -> f64 {
        self.get_info_value("F_MCP_SENDRGN_SCALE")
    }
    /// scale of TCP parameter area when TCP FX are embedded (0=min allowed,
    /// default, 1=max allowed)
    pub fn tcp_fx_param_scale(&self) -> f64 {
        self.get_info_value("F_TCP_FXPARM_SCALE")
    }

    pub fn play_offset(&self) -> Option<TrackPlayOffset> {
        let flag = self.get_info_value("I_PLAY_OFFSET_FLAG") as u8;
        let value = self.get_info_value("D_PLAY_OFFSET");
        if flag & 1 == 1 {
            None
        } else if flag & 2 == 2 {
            TrackPlayOffset::Samples(value as i32).into()
        } else {
            TrackPlayOffset::Seconds(value).into()
        }
    }

    /// On Master Track is_rec_fx represents monitoring chain.
    pub fn get_fx_ny_name(
        &self,
        name: impl Into<String>,
        is_rec_fx: bool,
    ) -> Option<TrackFX<Immutable>> {
        let fx =
            TrackFX::from_name(unsafe { transmute(self) }, name, is_rec_fx);
        fx
    }

    /// Get first instrument FX on track, if any.
    pub fn get_fx_instrument(&self) -> Option<TrackFX<T>> {
        let index = unsafe {
            Reaper::get()
                .low()
                .TrackFX_GetInstrument(self.get().as_ptr())
        };
        TrackFX::from_index(self, index as usize)
    }

    pub fn n_sends(&self) -> usize {
        unsafe {
            Reaper::get().low().GetTrackNumSends(
                self.get().as_ptr(),
                TrackSend::<Immutable>::as_int_static(),
            ) as usize
        }
    }
    pub fn n_receives(&self) -> usize {
        unsafe {
            Reaper::get().low().GetTrackNumSends(
                self.get().as_ptr(),
                TrackReceive::<Immutable>::as_int_static(),
            ) as usize
        }
    }
    pub fn n_hardware_sends(&self) -> usize {
        unsafe {
            Reaper::get().low().GetTrackNumSends(
                self.get().as_ptr(),
                HardwareSend::<Immutable>::as_int_static(),
            ) as usize
        }
    }

    pub fn get_automation_mode(&self) -> AutomationMode {
        let value = unsafe {
            Reaper::get()
                .low()
                .GetTrackAutomationMode(self.get().as_ptr())
        };
        AutomationMode::from_int(value)
            .expect("Can not convert to automation mode.")
    }

    pub fn get_color(&self) -> Color {
        unsafe {
            Color::from_native(
                Reaper::get().low().GetTrackColor(self.get().as_ptr()),
            )
        }
    }

    /// Get status of all track groups for specified parameter as bits.
    ///
    /// Returns 2 u32 values, each representing 32 track groups.
    ///
    /// See [Track::set_group_membership] for example.
    pub fn group_membership(&self, group: TrackGroupParam) -> (u32, u32) {
        let rpr_low = Reaper::get().low();
        let ptr = self.get().as_ptr();
        let group_name = CString::new(group.as_str())
            .expect("Can not convert group to CString");
        let low = unsafe {
            rpr_low.GetSetTrackGroupMembership(ptr, group_name.as_ptr(), 0, 0)
        };
        let high = unsafe {
            rpr_low.GetSetTrackGroupMembershipHigh(
                ptr,
                group_name.as_ptr(),
                0,
                0,
            )
        };
        (low, high)
    }

    fn get_item_parametrized(&self, index: usize) -> Option<Item<T>> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetTrackMediaItem(self.get().as_ptr(), index as i32)
        };
        match MediaItem::new(ptr) {
            None => None,
            Some(ptr) => Item::new(self.project(), ptr).into(),
        }
    }

    /// pitch 128 for CC0, 129 for CC1 etc.
    pub fn note_name(&self, channel: u8, pitch: u16) -> Option<String> {
        let raw = unsafe {
            Reaper::get().low().GetTrackMIDINoteNameEx(
                self.project().context().to_raw(),
                self.get().as_ptr(),
                pitch as i32,
                channel as i32,
            )
        };
        match raw.is_null() {
            true => None,
            false => {
                Some(as_string(raw).expect("Can not receive note name string"))
            }
        }
    }

    pub fn chunk(&self) -> ReaperStaticResult<String> {
        let size = i32::MAX;
        let buf = make_c_string_buf(size as usize).into_raw();
        let result = unsafe {
            Reaper::get().low().GetTrackStateChunk(
                self.get().as_ptr(),
                buf,
                size as i32,
                false,
            )
        };
        match result {
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not get chunk"))
            }
            true => {
                Ok(as_string_mut(buf)
                    .expect("Can not convert chunk to string"))
            }
        }
    }

    /// Get string, that will differ only if midi changed.
    pub fn midi_hash(&self, notes_only: bool) -> Option<String> {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().MIDI_GetTrackHash(
                self.get().as_ptr(),
                notes_only,
                buf,
                size as i32,
            )
        };
        match result {
            false => None,
            true => Some(
                as_string_mut(buf).expect("Can not convert hash to string"),
            ),
        }
    }

    /// Get peak value for the channel.
    ///
    /// If Channel == 1024 || 1025 -> loudness will be returned.
    /// Only if master track, or track in vu mode.
    pub fn peak(&self, channel: u32) -> Volume {
        let result = unsafe {
            Reaper::get()
                .low()
                .Track_GetPeakInfo(self.get().as_ptr(), channel as i32)
        };
        Volume::from(result)
    }

    fn get_envelope_parametrized(
        &self,
        index: usize,
    ) -> Option<Envelope<Self, T>> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetTrackEnvelope(self.get().as_ptr(), index as i32)
        };
        match TrackEnvelope::new(ptr) {
            None => None,
            Some(ptr) => Some(Envelope::new(ptr, self)),
        }
    }

    fn get_envelope_by_chunk_parametrized(
        &self,
        selector: EnvelopeSelector,
    ) -> Option<Envelope<Self, T>> {
        let mut chunk = match selector {
            EnvelopeSelector::Chunk(chunk) => chunk.to_string(),
            EnvelopeSelector::Guid(guid) => guid.to_string(),
        };
        let ptr = unsafe {
            Reaper::get().low().GetTrackEnvelopeByChunkName(
                self.get().as_ptr(),
                as_c_str(chunk.with_null()).as_ptr(),
            )
        };
        match TrackEnvelope::new(ptr) {
            None => None,
            Some(ptr) => Some(Envelope::new(ptr, self)),
        }
    }

    fn get_envelope_by_name_parametrized(
        &self,
        name: impl Into<String>,
    ) -> Option<Envelope<Self, T>> {
        let mut name = name.into();
        let ptr = unsafe {
            Reaper::get().low().GetTrackEnvelopeByName(
                self.get().as_ptr(),
                as_c_str(name.with_null()).as_ptr(),
            )
        };
        match TrackEnvelope::new(ptr) {
            None => None,
            Some(ptr) => Some(Envelope::new(ptr, self)),
        }
    }
}
impl<'a> Track<'a, Immutable> {
    pub fn get_parent_track(&self) -> Option<Track<Immutable>> {
        let ptr =
            unsafe { Reaper::get().low().GetParentTrack(self.get().as_ptr()) };
        match MediaTrack::new(ptr) {
            None => None,
            Some(ptr) => Track::new(self.project(), ptr).into(),
        }
    }

    pub fn get_item(&self, index: usize) -> Option<Item<Immutable>> {
        self.get_item_parametrized(index)
    }

    pub fn get_envelope(
        &self,
        index: usize,
    ) -> Option<Envelope<Self, Immutable>> {
        self.get_envelope_parametrized(index)
    }

    pub fn get_envelope_by_chunk(
        &self,
        selector: EnvelopeSelector,
    ) -> Option<Envelope<Self, Immutable>> {
        self.get_envelope_by_chunk_parametrized(selector)
    }
    pub fn get_envelope_by_name(
        &self,
        name: impl Into<String>,
    ) -> Option<Envelope<Self, Immutable>> {
        self.get_envelope_by_name_parametrized(name)
    }
}
impl<'a> Track<'a, Mutable> {
    pub fn get_parent_track(mut self) -> Option<Track<'a, Mutable>> {
        let ptr =
            unsafe { Reaper::get().low().GetParentTrack(self.get().as_ptr()) };
        match MediaTrack::new(ptr) {
            None => None,
            Some(ptr) => {
                self.ptr = ptr;
                Some(self)
            }
        }
    }

    pub fn make_only_selected_track(&self) {
        self.project()
            .with_current_project(|| {
                unsafe {
                    Reaper::get()
                        .low()
                        .SetOnlyTrackSelected(self.get().as_ptr())
                };
                Ok(())
            })
            .unwrap();
    }

    pub fn get_item(&mut self, index: usize) -> Option<Item<Mutable>> {
        self.get_item_parametrized(index)
    }

    pub fn get_envelope(
        &mut self,
        index: usize,
    ) -> Option<Envelope<Self, Mutable>> {
        self.get_envelope_parametrized(index)
    }

    pub fn get_envelope_by_chunk(
        &mut self,
        selector: EnvelopeSelector,
    ) -> Option<Envelope<Self, Mutable>> {
        self.get_envelope_by_chunk_parametrized(selector)
    }
    pub fn get_envelope_by_name(
        &mut self,
        name: impl Into<String>,
    ) -> Option<Envelope<Self, Mutable>> {
        self.get_envelope_by_name_parametrized(name)
    }

    pub fn get_fx_mut(&mut self, index: usize) -> Option<TrackFX<Mutable>> {
        let fx = TrackFX::from_index(self, index);
        fx
    }

    /// On Master Track is_rec_fx represents monitoring chain.
    pub fn get_fx_ny_name_mut(
        &mut self,
        name: impl Into<String>,
        is_rec_fx: bool,
    ) -> Option<TrackFX<Mutable>> {
        let fx = TrackFX::from_name(self, name, is_rec_fx);
        fx
    }

    pub fn set_chunk(
        &mut self,
        chunk: impl Into<String>,
        need_undo: bool,
    ) -> ReaperStaticResult<()> {
        let mut chunk = chunk.into();
        let result = unsafe {
            Reaper::get().low().SetTrackStateChunk(
                self.get().as_ptr(),
                as_c_str(chunk.with_null()).as_ptr(),
                need_undo,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set chunk!"))
            }
        }
    }

    /// pitch 128-255 for CC. 128=CC0, 129=CC1 etc.
    pub fn set_note_name(
        &mut self,
        channel: u8,
        pitch: u16,
        note_name: impl Into<String>,
    ) -> ReaperStaticResult<()> {
        let mut note_name = note_name.into();
        let result = unsafe {
            Reaper::get().low().SetTrackMIDINoteNameEx(
                self.project().context().to_raw(),
                self.get().as_ptr(),
                pitch as i32,
                channel as i32,
                as_c_str(note_name.with_null()).as_ptr(),
            )
        };
        match result {
            true => Ok(()),
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not set note name.",
            )),
        }
    }

    fn set_info_string(
        &mut self,
        category: impl Into<String>,
        value: impl Into<String>,
    ) -> ReaperStaticResult<()> {
        let mut category = category.into();
        let value = value.into();
        let result = unsafe {
            Reaper::get().low().GetSetMediaTrackInfo_String(
                self.get().as_ptr(),
                as_c_str(&category.with_null()).as_ptr(),
                as_c_string(&value).into_raw(),
                true,
            )
        };
        match result {
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not set info string.",
            )),
            true => Ok(()),
        }
    }

    /// Set Vec of RazorEdit areas.
    ///
    /// Can be empty.
    ///
    /// For every envelope selected should be provided dedicated
    /// [RazorEdit] with envelope GUID. RazorEdit without GUID
    /// corresponding to [Track]
    pub fn set_razor_edits(
        &mut self,
        edits: Vec<RazorEdit>,
    ) -> ReaperStaticResult<()> {
        let edits: Vec<String> =
            edits.into_iter().map(|e| e.to_string()).collect();
        let edits = edits.join(",");
        self.set_info_string("P_RAZOREDITS_EXT", edits)?;
        Ok(())
    }

    pub fn set_guid(&mut self, guid: GUID) {
        self.set_info_string("GUID", guid.to_string())
            .expect("Can not set GUID");
    }

    pub fn set_name(
        &mut self,
        name: impl Into<String>,
    ) -> ReaperStaticResult<()> {
        self.set_info_string("P_NAME", name)
    }

    pub fn set_icon(&mut self, path: PathBuf) -> ReaperStaticResult<()> {
        self.set_info_string(
            "P_ICON",
            path.to_str().ok_or(ReaperError::InvalidObject(
                "can not convert path to string",
            ))?,
        )
    }

    pub fn set_mcp_layout(
        &mut self,
        layout: impl Into<String>,
    ) -> ReaperStaticResult<()> {
        self.set_info_string("P_MCP_LAYOUT", layout)
    }
    pub fn set_tcp_layout(
        &mut self,
        layout: impl Into<String>,
    ) -> ReaperStaticResult<()> {
        self.set_info_string("P_TCP_LAYOUT", layout)
    }

    pub fn add_audio_accessor(
        &mut self,
    ) -> ReaperResult<AudioAccessor<Self, Mutable>> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .CreateTrackAudioAccessor(self.get().as_ptr())
        };
        let ptr = NonNull::new(ptr).ok_or(ReaperError::NullPtr)?;
        Ok(AudioAccessor::new(self, ptr))
    }

    /// Add FX at given position, or return existing one.
    ///
    /// If `even_if_exists` is `false`, plugin will be added only
    /// if no plugin exists on track.
    ///
    /// Otherwise, if position is None → the last slot will be used.
    /// The resulting FX will have real index, that may differ from the
    /// desired.
    ///
    /// If `input_fx` is `true` → input fx chain on regular Track will be used,
    /// of monitoring FX chain in case of master track.
    pub fn add_fx(
        &mut self,
        name: impl Into<String>,
        position: impl Into<Option<u8>>,
        input_fx: bool,
        even_if_exists: bool,
    ) -> Option<TrackFX<Mutable>> {
        let insatantinate = match even_if_exists {
            false => 1 as i32,
            true => match position.into() {
                None => -1 as i32,
                Some(pos) => -1000 - pos as i32,
            },
        };
        let index = unsafe {
            Reaper::get().low().TrackFX_AddByName(
                self.get().as_ptr(),
                as_c_str(name.into().with_null()).as_ptr(),
                input_fx,
                insatantinate,
            )
        };
        TrackFX::<Mutable>::from_index(self, index as usize)
    }

    /// Add an empty [Item] to Track.
    ///
    /// Item will not have any takes.
    pub fn add_item(
        &mut self,
        start: impl Into<Position>,
        length: impl GetLength,
    ) -> Item<Mutable> {
        let start = start.into();
        let ptr = unsafe {
            Reaper::get().low().AddMediaItemToTrack(self.get().as_ptr())
        };
        let ptr = MediaItem::new(ptr).expect("Can not add track.");
        let mut item = Item::<Mutable>::new(self.project(), ptr);
        item.set_position(start);
        item.set_length(length.get_length(start));
        item
    }

    pub fn add_midi_item(
        &mut self,
        start: impl Into<Position>,
        length: impl GetLength,
    ) -> Item<Mutable> {
        let qn = MaybeUninit::new(false);
        let start = start.into();
        let end = Position::from(length.get_length(start)) + start;
        let ptr = unsafe {
            Reaper::get().low().CreateNewMIDIItemInProj(
                self.get().as_ptr(),
                start.into(),
                end.into(),
                qn.as_ptr(),
            )
        };
        let ptr = MediaItem::new(ptr).expect("Can not make item.");
        Item::<Mutable>::new(self.project, ptr)
    }

    /// Add HardwareSend, that sends audio or midi to hardware outs.
    ///
    /// All future tweaks done on the HardwareSend.
    ///
    /// Try to keep send object as little as possible. It is accessed
    /// by indexing, so everything falls, as sends are changed.
    ///
    /// # Note
    ///
    /// To add regular track send use [crate::send::TrackSend::create_new]
    pub fn add_hardware_send(&mut self) -> HardwareSend<Mutable> {
        let index = unsafe {
            Reaper::get()
                .low()
                .CreateTrackSend(self.get().as_ptr(), null_mut())
        };
        HardwareSend::<Mutable>::new(self, index as usize)
    }

    pub fn delete(self) {
        unsafe { Reaper::get().low().DeleteTrack(self.get().as_ptr()) };
    }

    pub fn set_automation_mode(&mut self, mode: AutomationMode) {
        unsafe {
            Reaper::get()
                .low()
                .SetTrackAutomationMode(self.get().as_ptr(), mode.int_value())
        }
    }

    pub fn set_color(&mut self, color: impl Into<Color>) {
        unsafe {
            Reaper::get()
                .low()
                .SetTrackColor(self.get().as_ptr(), color.into().to_native())
        }
    }

    fn set_info_value(
        &mut self,
        param: impl Into<String>,
        value: f64,
    ) -> ReaperResult<()> {
        let mut param_name: String = param.into();
        let result = unsafe {
            Reaper::get().low().SetMediaTrackInfo_Value(
                self.get().as_ptr(),
                as_c_str(&param_name.with_null()).as_ptr(),
                value,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set value.")
                    .into())
            }
        }
    }

    /// If socket is None — delete hardware out.
    ///
    /// ch=0 → all channels
    pub fn set_midi_hardware_out(
        &mut self,
        channel: u8,
        socket: Option<HardwareSocket>,
    ) -> ReaperResult<()> {
        let mut value = match socket {
            None => -1,
            Some(socket) => socket.index() as i32,
        };
        if value != -1 {
            value <<= 5;
            value |= channel as i32;
        }
        self.set_info_value("I_MIDIHWOUT", value as f64)
    }

    pub fn set_muted(&mut self, state: bool) -> ReaperResult<()> {
        let value = match state {
            true => 1.0,
            false => 0.0,
        };
        self.set_info_value("B_MUTE", value)
    }
    pub fn set_phase_flipped(&mut self, state: bool) -> ReaperResult<()> {
        let value = match state {
            true => 1.0,
            false => 0.0,
        };
        self.set_info_value("B_PHASE", value)
    }
    pub fn set_solo(&mut self, mode: SoloMode) -> ReaperResult<()> {
        self.set_info_value("I_SOLO", mode.int_value() as f64)
    }
    /// when set, if anything else is soloed and this track is not muted, this
    /// track acts soloed
    pub fn set_solo_defeat(&mut self, state: bool) -> ReaperResult<()> {
        self.set_info_value("B_SOLO_DEFEAT", state as i32 as f64)
    }
    pub fn set_fx_bypassed(&mut self, state: bool) -> ReaperResult<()> {
        self.set_info_value("I_FXEN", !state as i32 as f64)
    }
    pub fn set_rec_armed(&mut self, state: bool) -> ReaperResult<()> {
        let value = match state {
            true => 1.0,
            false => 0.0,
        };
        self.set_info_value("I_RECARM", value)
    }
    pub fn set_rec_input(&mut self, rec_input: RecInput) -> ReaperResult<()> {
        self.set_info_value("I_RECINPUT", rec_input.to_raw() as f64)
    }
    pub fn set_rec_mode(&mut self, rec_mode: RecMode) -> ReaperResult<()> {
        self.set_info_value("I_RECMODE", rec_mode.int_value() as f64)
    }

    /// If rec_mode records output. Otherwise — None.
    pub fn set_rec_out_mode(&mut self, flags: RecOutMode) -> ReaperResult<()> {
        self.set_info_value("I_RECMODE_FLAGS", flags.to_raw() as f64)
    }
    pub fn set_rec_monitoring(
        &mut self,
        value: RecMonitoring,
    ) -> ReaperResult<()> {
        self.set_info_value("I_RECMON", value.mode as f64)?;
        self.set_info_value("I_RECMONITEMS", value.monitor_items as i32 as f64)
    }

    /// True if automatically armed when track is selected.
    ///
    /// If track is already selected and not rec armed — it will not
    /// arm track.
    pub fn set_auto_rec_arm(&mut self, value: bool) -> ReaperResult<()> {
        self.set_info_value("B_AUTO_RECARM", value as i32 as f64)
    }

    pub fn set_vu_mode(&mut self, value: VUMode) -> ReaperResult<()> {
        self.set_info_value("I_VUMODE", value.to_raw() as f64)
    }
    pub fn set_n_channels(&mut self, amount: usize) -> ReaperResult<()> {
        self.set_info_value("I_NCHAN", amount as f64)
    }
    pub fn set_selected(&mut self, selected: bool) -> ReaperResult<()> {
        self.set_info_value("I_SELECTED", selected as i32 as f64)
    }
    pub fn set_folder_state(
        &mut self,
        state: TrackFolderState,
    ) -> ReaperResult<()> {
        let (depth, compact) = state.to_raw();
        self.set_info_value("I_FOLDERDEPTH", depth as f64)?;
        match compact {
            None => Ok(()),
            Some(compact) => {
                self.set_info_value("I_FOLDERCOMPACT", compact as f64)
            }
        }
    }

    pub fn set_performance_flags(
        &mut self,
        flags: TrackPerformanceFlags,
    ) -> ReaperResult<()> {
        self.set_info_value("I_PERFFLAGS", flags.bits() as f64)
    }

    /// If track height was overrided by script.
    pub fn set_height_override(
        &mut self,
        height: impl Into<Option<u32>>,
    ) -> ReaperResult<()> {
        let value = match height.into() {
            Some(x) => x,
            None => 0,
        };
        self.set_info_value("I_HEIGHTOVERRIDE", value as f64)
    }
    pub fn set_height_lock(&mut self, value: bool) -> ReaperResult<()> {
        self.set_info_value("B_HEIGHTLOCK", value as i32 as f64)
    }

    pub fn set_volume(&mut self, volume: Volume) -> ReaperResult<()> {
        self.set_info_value("D_VOL", volume.into())
    }
    pub fn set_pan(&mut self, track_pan: TrackPan) -> ReaperResult<()> {
        match track_pan {
            TrackPan::BalanceLegacy(pan) => {
                self.set_info_value("I_PANMODE", 0 as f64)?;
                self.set_info_value("D_PAN", pan.into())?;
            }
            TrackPan::Balance(pan) => {
                self.set_info_value("I_PANMODE", 3 as f64)?;
                self.set_info_value("D_PAN", pan.into())?;
            }
            TrackPan::Stereo(pan, width) => {
                self.set_info_value("I_PANMODE", 5 as f64)?;
                self.set_info_value("D_PAN", pan.into())?;
                self.set_info_value("D_WIDTH", width.into())?;
            }
            TrackPan::Dual(pan_l, pan_r) => {
                self.set_info_value("I_PANMODE", 6 as f64)?;
                self.set_info_value("D_DUALPANL", pan_l.into())?;
                self.set_info_value("D_DUALPANR", pan_r.into())?;
            }
        };
        Ok(())
    }

    pub fn set_pan_law(&mut self, law: PanLaw) -> ReaperResult<()> {
        self.set_info_value("D_PANLAW", law.into())
    }
    pub fn set_pan_law_mode(
        &mut self,
        law_mode: PanLawMode,
    ) -> ReaperResult<()> {
        self.set_info_value("I_PANLAW_FLAGS", law_mode.int_value() as f64)
    }

    pub fn set_visible_in_mcp(&mut self, value: bool) -> ReaperResult<()> {
        self.set_info_value("B_SHOWINMIXER", value as i32 as f64)
    }
    pub fn set_visible_in_tcp(&mut self, value: bool) -> ReaperResult<()> {
        self.set_info_value("B_SHOWINTCP", value as i32 as f64)
    }
    pub fn set_parent_send(
        &mut self,
        parent_ch_offset: impl Into<Option<u32>>,
    ) -> ReaperResult<()> {
        let value: Option<u32> = parent_ch_offset.into();
        match value {
            None => {
                self.set_info_value("B_MAINSEND", 0.0)?;
                Ok(())
            }
            Some(value) => {
                self.set_info_value("B_MAINSEND", 1.0)?;
                self.set_info_value("C_MAINSEND_OFFS", value as f64)?;
                Ok(())
            }
        }
    }

    pub fn set_free_item_positioning(
        &mut self,
        free: bool,
        update_timeline: bool,
    ) -> ReaperResult<()> {
        self.set_info_value("B_FREEMODE", free as i32 as f64)?;
        if update_timeline {
            Reaper::get().update_timeline()
        };
        Ok(())
    }

    pub fn set_time_base(&mut self, mode: TimeMode) -> ReaperResult<()> {
        self.set_info_value("C_BEATATTACHMODE", mode.int_value() as f64)
    }

    /// scale of fx+send area in MCP (0=minimum allowed, 1=maximum allowed)
    pub fn set_mcp_fx_send_scale(&mut self, value: f64) -> ReaperResult<()> {
        self.set_info_value("F_MCP_FXSEND_SCALE", value)
    }
    /// scale of fx parameter area in MCP (0=minimum allowed, 1=maximum
    /// allowed)
    pub fn set_mcp_fx_param_scale(&mut self, value: f64) -> ReaperResult<()> {
        self.set_info_value("F_MCP_FXPARM_SCALE", value)
    }
    /// scale of send area as proportion of the fx+send total area (0=minimum
    /// allowed, 1=maximum allowed)
    pub fn set_mcp_fx_send_region_scale(
        &mut self,
        value: f64,
    ) -> ReaperResult<()> {
        self.set_info_value("F_MCP_SENDRGN_SCALE", value)
    }
    /// scale of TCP parameter area when TCP FX are embedded (0=min allowed,
    /// default, 1=max allowed)
    pub fn set_tcp_fx_param_scale(&mut self, value: f64) -> ReaperResult<()> {
        self.set_info_value("F_TCP_FXPARM_SCALE", value)
    }

    pub fn set_play_offset(
        &mut self,
        play_offset: Option<TrackPlayOffset>,
    ) -> ReaperResult<()> {
        match play_offset {
            None => self.set_info_value("I_PLAY_OFFSET_FLAG", 1.0),
            Some(mode) => {
                let value = match mode {
                    TrackPlayOffset::Samples(v) => {
                        self.set_info_value("I_PLAY_OFFSET_FLAG", 2.0)?;
                        v as f64
                    }
                    TrackPlayOffset::Seconds(v) => {
                        self.set_info_value("I_PLAY_OFFSET_FLAG", 0.0)?;
                        v
                    }
                };
                self.set_info_value("D_PLAY_OFFSET", value)
            }
        }
    }

    /// Set track membership for specified parameter in track groups.
    ///
    /// If masks are `None` — corresponding true bits of groups will be used.
    /// For complete rewrite of values use [u32::MAX].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rea_rs::{TrackGroupParam, Reaper};
    /// use bitvec::prelude::*;
    ///
    /// let mut pr = Reaper::get().current_project();
    /// let mut tr = pr.get_track_mut(0).unwrap();
    /// assert_eq!(tr.index(), 0);
    ///
    /// let (mut low_u32, mut high_u32) =
    ///     tr.group_membership(TrackGroupParam::MuteLead);
    /// let (low, high) = (
    ///     low_u32.view_bits_mut::<Lsb0>(),
    ///     high_u32.view_bits_mut::<Lsb0>(),
    /// );
    /// low.set(3, true);
    /// low.set(5, true);
    /// high.set(6, true);
    /// tr.set_group_membership(
    ///     TrackGroupParam::MuteLead,
    ///     low.load(),
    ///     high.load(),
    ///     None,
    ///     None
    /// );
    /// let (low_u32, high_u32) =
    ///     tr.group_membership(TrackGroupParam::MuteLead);
    /// assert!(low_u32 & 0b1000 > 0);
    /// assert!(low_u32 & 0b100000 > 0);
    /// assert!(low_u32 & 0b1000000 == 0);
    /// assert!(high_u32 & 0b1000000 > 0);
    /// ```
    pub fn set_group_membership(
        &mut self,
        group: TrackGroupParam,
        low_groups: u32,
        high_groups: u32,
        low_groups_set_mask: impl Into<Option<u32>>,
        high_groups_set_mask: impl Into<Option<u32>>,
    ) {
        let rpr_low = Reaper::get().low();
        let ptr = self.get().as_ptr();
        let group_name = CString::new(group.as_str())
            .expect("Can not convert group to CString");
        unsafe {
            rpr_low.GetSetTrackGroupMembership(
                ptr,
                group_name.as_ptr(),
                low_groups_set_mask.into().unwrap_or(low_groups),
                low_groups,
            );
            rpr_low.GetSetTrackGroupMembershipHigh(
                ptr,
                group_name.as_ptr(),
                high_groups_set_mask.into().unwrap_or(high_groups),
                high_groups,
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecMonitoring {
    /// 0 → not, 1 → normal, 2 → when playing (tape)
    pub mode: u32,
    pub monitor_items: bool,
}
impl RecMonitoring {
    pub fn new(mode: u32, monitor_items: bool) -> Self {
        Self {
            mode,
            monitor_items,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackDimensions {
    pub tcp_height: u32,
    pub tcp_height_with_env: u32,
    pub tcp_pos_y: u32,
    pub mcp_pos_x: u32,
    pub mcp_pos_y: u32,
    pub mcp_width: u32,
    pub mcp_height: u32,
}

bitflags! {
    /// Flags for optimization.
    #[derive(Serialize, Deserialize)]
    pub struct TrackPerformanceFlags:u8{
        const NO_BUFFERING = 1;
        const NO_ANTICIPATIVE_FX = 2;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TrackPan {
    ///  Reaper 1.0 balance pan.
    BalanceLegacy(Pan),
    /// Default balance pan v3+
    Balance(Pan),
    /// Stereo pan: first is pan, second is width.
    Stereo(Pan, Pan),
    /// Dual pal mode: first is L, second is R
    Dual(Pan, Pan),
}

/// Represent latency of track sound.
///
/// Can be negative.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TrackPlayOffset {
    Samples(i32),
    /// 1.0 == 1 second
    Seconds(f64),
}

/// Keeps parameters of track groups.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TrackGroupParam {
    VolumeLead,
    VolumeFollow,
    VolumeVcaLead,
    VolumeVcaFollow,
    PanLead,
    PanFollow,
    WidthLead,
    WidthFollow,
    MuteLead,
    MuteFollow,
    SoloLead,
    SoloFollow,
    RecarmLead,
    RecarmFollow,
    PolarityLead,
    PolarityFollow,
    AutomodeLead,
    AutomodeFollow,
    VolumeReverse,
    PanReverse,
    WidthReverse,
    NoLeadWhenFollow,
    VolumeVcaFollowIsprefx,
}

impl TrackGroupParam {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VolumeLead => "VOLUME_LEAD",
            Self::VolumeFollow => "VOLUME_FOLLOW",
            Self::VolumeVcaLead => "VOLUME_VCA_LEAD",
            Self::VolumeVcaFollow => "VOLUME_VCA_FOLLOW",
            Self::PanLead => "PAN_LEAD",
            Self::PanFollow => "PAN_FOLLOW",
            Self::WidthLead => "WIDTH_LEAD",
            Self::WidthFollow => "WIDTH_FOLLOW",
            Self::MuteLead => "MUTE_LEAD",
            Self::MuteFollow => "MUTE_FOLLOW",
            Self::SoloLead => "SOLO_LEAD",
            Self::SoloFollow => "SOLO_FOLLOW",
            Self::RecarmLead => "RECARM_LEAD",
            Self::RecarmFollow => "RECARM_FOLLOW",
            Self::PolarityLead => "POLARITY_LEAD",
            Self::PolarityFollow => "POLARITY_FOLLOW",
            Self::AutomodeLead => "AUTOMODE_LEAD",
            Self::AutomodeFollow => "AUTOMODE_FOLLOW",
            Self::VolumeReverse => "VOLUME_REVERSE",
            Self::PanReverse => "PAN_REVERSE",
            Self::WidthReverse => "WIDTH_REVERSE",
            Self::NoLeadWhenFollow => "NO_LEAD_WHEN_FOLLOW",
            Self::VolumeVcaFollowIsprefx => "VOLUME_VCA_FOLLOW_ISPREFX",
        }
    }
}

/// Represents RazorEdit area of [Track]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RazorEdit {
    /// start time
    pub start: Position,
    /// end time
    pub end: Position,
    /// If `None` → it's track selection,
    /// if Some([GUID]) → it's envelope selection.
    pub envelope_guid: Option<GUID>,
    /// probably, can be considered as visible envelope index.
    pub top_y_pos: f64,
    /// probably, can be considered as visible envelope index.
    pub bot_y_pos: f64,
}
impl RazorEdit {
    pub(crate) fn from_str(data: &str) -> Self {
        let mut tokens = data.split(" ");
        let start: f64 = tokens.next().unwrap().parse().unwrap();
        let end: f64 = tokens.next().unwrap().parse().unwrap();
        let guid = tokens
            .next()
            .unwrap()
            .strip_prefix("\"")
            .unwrap()
            .strip_suffix("\"")
            .unwrap();
        let guid = match guid.is_empty() {
            true => None,
            false => Some(String::from(guid)),
        };
        let top_y_pos: f64 = tokens.next().unwrap().parse().unwrap();
        let bot_y_pos: f64 = tokens.next().unwrap().parse().unwrap();
        let start = Position::from(start);
        let end = Position::from(end);
        let guid = match guid {
            Some(v) => Some(GUID::from_string(v).unwrap()),
            None => None,
        };
        Self {
            start,
            end,
            envelope_guid: guid,
            top_y_pos,
            bot_y_pos,
        }
    }
    pub(crate) fn to_string(&self) -> String {
        let guid = match self.envelope_guid {
            None => String::from(""),
            Some(guid) => guid.to_string(),
        };
        let start: f64 = self.start.into();
        let end: f64 = self.end.into();
        let line = format!(
            "{:?} {:?} \"{}\" {:?} {:?}",
            start, end, guid, self.top_y_pos, self.bot_y_pos
        );
        line
    }
}
