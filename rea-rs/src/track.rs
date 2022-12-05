use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    ptr::{null_mut, NonNull},
};

use bitflags::bitflags;
use int_enum::IntEnum;
use log::debug;
use reaper_medium::{MediaItem, MediaTrack};

use crate::{
    errors::{ReaperError, ReaperResult},
    utils::{as_c_str, as_mut_i8, as_string_mut, make_c_string_buf, WithNull},
    AudioAccessor, AutomationMode, Color, Fx, GenericSend, GetLength,
    HardwareSend, HardwareSocket, Immutable, Item, KnowsProject, Mutable,
    Position, ProbablyMutable, Project, Reaper, RecordingMode,
    RecordingOutMode, SendIntType, TrackFX, TrackFolderState, TrackReceive,
    TrackSend, VUMode, WithReaperPtr,
};

pub use reaper_medium::{RecordingInput, SoloMode};

#[derive(Debug, PartialEq)]
pub struct Track<'a, T: ProbablyMutable> {
    ptr: MediaTrack,
    should_check: bool,
    project: &'a Project,
    info_buf_size: usize,
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
        let track = project.iter_tracks().find(|tr| {
            tr.get_name().expect("Can not retrieve track name") == name
        })?;
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

    pub fn get_name(&self) -> ReaperResult<String> {
        self.get_info_string("P_NAME")
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
    pub fn get_solo(&self) -> SoloMode {
        let value = self.get_info_value("I_SOLO");
        SoloMode::from_raw(value as i32)
    }
    /// when set, if anything else is soloed and this track is not muted, this
    /// track acts soloed
    pub fn is_solo_defeat(&self) -> bool {
        self.get_info_value("B_SOLO_DEFEAT") != 0.0
    }
    pub fn fx_enabled(&self) -> bool {
        self.get_info_value("I_FXEN") != 0.0
    }
    pub fn rec_armed(&self) -> bool {
        match self.get_info_value("I_RECARM") as i32 {
            0 => false,
            1 => true,
            x => panic!("Unexpected result: {}", x),
        }
    }
    pub fn rec_input(&self) -> RecordingInput {
        RecordingInput::from_raw(self.get_info_value("I_RECINPUT") as i32)
            .expect("Can not convert to RecordingInput.")
    }
    pub fn rec_mode(&self) -> RecordingMode {
        RecordingMode::from_int(self.get_info_value("I_RECMODE") as i32)
            .expect("Can not convert to RecordingMode")
    }
    /// If rec_mode records output. Otherwise — None.
    pub fn rec_out_mode(&self) -> Option<RecordingOutMode> {
        RecordingOutMode::from_raw(
            self.get_info_value("I_RECMODE_FLAGS") as u32
        )
    }
    pub fn record_monitoring(&self) -> RecordMonitoring {
        let mode = self.get_info_value("I_RECMON") as u32;
        let monitor_items = self.get_info_value("I_RECMONITEMS") != 0.0;
        RecordMonitoring::new(mode, monitor_items)
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
        debug!(
            "value: {}, channel: {}, out_idx: {}",
            value, channel, out_idx
        );
        let socket = Reaper::get().get_midi_output(out_idx as usize)?;
        debug!("socket: {:?}", socket);
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
            9 => None,
            x => Some(x),
        }
    }
    /// `None`, if [Track::height_override] is `None`.
    pub fn height_lock(&self) -> Option<bool> {
        self.height_override()?;
        Some(self.get_info_value("B_HEIGHTLOCK") != 0.0)
    }

    pub fn get_fx(&self, index: usize) -> Option<TrackFX<T>> {
        let fx = TrackFX::from_index(self, index);
        fx
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
}
impl<'a> Track<'a, Mutable> {
    fn set_info_string(
        &mut self,
        category: impl Into<String>,
        value: impl Into<String>,
    ) -> ReaperResult<()> {
        unsafe {
            let result = Reaper::get().low().GetSetMediaTrackInfo_String(
                self.get().as_ptr(),
                as_c_str(&category.into().with_null()).as_ptr(),
                as_mut_i8(value.into().as_str()),
                true,
            );
            match result {
                false => Err(ReaperError::UnsuccessfulOperation(
                    "Can not set info string.",
                )
                .into()),
                true => Ok(()),
            }
        }
    }

    pub fn set_name(&mut self, name: impl Into<String>) -> ReaperResult<()> {
        self.set_info_string("P_NAME", name)
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

    pub fn add_item(
        &mut self,
        start: Position,
        length: impl GetLength,
    ) -> Item<Mutable> {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordMonitoring {
    /// 0 → not, 1 → normal, 2 → when playing (tape)
    pub mode: u32,
    pub monitor_items: bool,
}
impl RecordMonitoring {
    pub fn new(mode: u32, monitor_items: bool) -> Self {
        Self {
            mode,
            monitor_items,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub struct TrackPerformanceFlags:u8{
        const NO_BUFFERING = 1;
        const NO_ANTICIPATIVE_FX = 2;
    }
}
