use std::{
    marker::PhantomData,
    mem::MaybeUninit,
    ptr::{null_mut, NonNull},
    time::Duration,
};

use bitflags::bitflags;
use int_enum::IntEnum;
use log::debug;
use reaper_medium::{MediaItem, MediaTrack};

use crate::{
    errors::{ReaperError, ReaperResult},
    utils::{as_c_str, as_mut_i8, as_string_mut, make_c_string_buf, WithNull},
    AudioAccessor, AutomationMode, Color, Fx, GenericSend, GetLength,
    HardwareSend, HardwareSocket, Immutable, Item, KnowsProject, Mutable, Pan,
    PanLaw, PanLawMode, Position, ProbablyMutable, Project, Reaper, RecInput,
    RecMode, RecOutMode, SampleAmount, SendIntType, SoloMode, TimeMode,
    TrackFX, TrackFolderState, TrackReceive, TrackSend, VUMode, Volume,
    WithReaperPtr,
};

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
            tr.name().expect("Can not retrieve track name") == name
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

    pub fn name(&self) -> ReaperResult<String> {
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

    #[deprecated = "This function probably, doesn't work: \
        Can not found GUI for the setting."]
    /// If rec_mode records output. Otherwise — None.
    pub fn rec_out_mode(&self) -> Option<RecOutMode> {
        RecOutMode::from_raw(self.get_info_value("I_RECMODE_FLAGS") as u32)
    }
    pub fn rec_monitoring(&self) -> RecMonitoring {
        let mode = self.get_info_value("I_RECMON") as u32;
        let monitor_items = self.get_info_value("I_RECMONITEMS") != 0.0;
        RecMonitoring::new(mode, monitor_items)
    }
    #[deprecated = "This function fails in tests."]
    /// True if automatically armed when track is selected.
    pub fn auto_rec_arm(&self) -> bool {
        self.get_info_value("B_AUTO_RECARM") != 0.0
    }
    #[deprecated = "Can not find function in GUI, can not set value."]
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
                self.get_info_value("D_DUALPANL").into(),
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
    pub fn parent_send(&self) -> TrackParentSend {
        let is_enabled = self.get_info_value("B_MAINSEND") != 0.0;
        let channel_offset = self.get_info_value("C_MAINSEND_OFFS") as u16;
        let channels_amount = self.get_info_value("C_MAINSEND_NCH") as u16;
        TrackParentSend {
            is_enabled,
            channel_offset,
            channels_amount,
        }
    }

    pub fn free_item_positioning(&self) -> TrackFreeMode {
        TrackFreeMode::from_int(self.get_info_value("I_FREEMODE") as u8)
            .expect("Can not convert to TrackFreeMode")
    }

    pub fn beat_attach_mode(&self) -> TimeMode {
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
            TrackPlayOffset::Samples(SampleAmount::new(value as u32)).into()
        } else {
            TrackPlayOffset::Seconds(Duration::from_secs_f64(value)).into()
        }
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

    #[deprecated = "This function fails in tests."]
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

    #[deprecated = "This function fails in tests."]
    /// True if automatically armed when track is selected.
    ///
    /// If track is already selected and not rec armed — it will not
    /// arm track.
    pub fn set_auto_rec_arm(&mut self, value: bool) -> ReaperResult<()> {
        self.set_info_value("B_AUTO_RECARM", value as i32 as f64)
    }

    #[deprecated = "Can not find function in GUI, can not set value."]
    pub fn set_vu_mode(&mut self, value: VUMode) -> ReaperResult<()> {
        debug!("{:?}", value.to_raw());
        self.set_info_value("I_VUMODE", value.to_raw() as f64)
    }
    pub fn set_n_channels(&mut self, amount: usize) -> ReaperResult<()> {
        self.set_info_value("I_NCHAN", amount as f64)
    }
    pub fn set_selected(&mut self, selected: bool) -> ReaperResult<()> {
        self.set_info_value("I_SELECTED", selected as i32 as f64)
    }
    pub fn set_dimensions(
        &mut self,
        dimensions: TrackDimensions,
    ) -> ReaperResult<()> {
        self.set_info_value("I_TCPH", dimensions.tcp_height as f64)?;
        self.set_info_value("I_WNDH", dimensions.tcp_height_with_env as f64)?;
        self.set_info_value("I_TCPY", dimensions.tcp_pos_y as f64)?;
        self.set_info_value("I_MCPX", dimensions.mcp_pos_x as f64)?;
        self.set_info_value("I_MCPY", dimensions.mcp_pos_y as f64)?;
        self.set_info_value("I_MCPW", dimensions.mcp_width as f64)?;
        self.set_info_value("I_MCPH", dimensions.mcp_height as f64)
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
        height: Option<u32>,
    ) -> ReaperResult<()> {
        let value = match height {
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
        let pan_mode = match track_pan {
            TrackPan::BalanceLegacy(pan) => {
                self.set_info_value("D_PAN", pan.into())?;
                0
            }
            TrackPan::Balance(pan) => {
                self.set_info_value("D_PAN", pan.into())?;
                3
            }
            TrackPan::Stereo(pan, width) => {
                (
                    self.set_info_value("D_PAN", pan.into())?,
                    self.set_info_value("D_WIDTH", width.into())?,
                );
                5
            }
            TrackPan::Dual(pan_l, pan_r) => {
                (
                    self.set_info_value("D_DUALPANL", pan_l.into())?,
                    self.set_info_value("D_DUALPANL", pan_r.into())?,
                );
                6
            }
        };
        self.set_info_value("I_PANMODE", pan_mode as f64)
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
        value: TrackParentSend,
    ) -> ReaperResult<()> {
        self.set_info_value("B_MAINSEND", value.is_enabled as i32 as f64)?;
        self.set_info_value(
            "C_MAINSEND_OFFS",
            value.channel_offset as i32 as f64,
        )?;
        self.set_info_value(
            "C_MAINSEND_NCH",
            value.channels_amount as i32 as f64,
        )
    }

    pub fn set_free_item_positioning(
        &mut self,
        mode: TrackFreeMode,
    ) -> ReaperResult<()> {
        self.set_info_value("I_FREEMODE", mode.int_value() as f64)
    }

    pub fn set_beat_attach_mode(
        &mut self,
        mode: TimeMode,
    ) -> ReaperResult<()> {
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
                    TrackPlayOffset::Samples(v) => v.get() as f64,
                    TrackPlayOffset::Seconds(v) => v.as_secs_f64(),
                };
                self.set_info_value("D_PLAY_OFFSET", value)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackParentSend {
    pub is_enabled: bool,
    pub channel_offset: u16,
    /// 0 → all channels
    pub channels_amount: u16,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntEnum)]
pub enum TrackFreeMode {
    Disabled = 0,
    Enabled = 1,
    FixLanes = 2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackPlayOffset {
    Samples(SampleAmount),
    Seconds(Duration),
}
