use std::{
    ffi::c_char,
    mem::{transmute, MaybeUninit},
    time::Duration,
};

use crate::{
    errors::{ReaperError, ReaperStaticResult},
    ptr_wrappers::{self, MediaItemTake, PcmSource},
    utils::{
        as_c_str, as_c_string, as_string, as_string_mut, make_c_string_buf,
        WithNull,
    },
    AudioAccessor, Color, FXParent, Immutable, Item, KnowsProject,
    MidiEventBuilder, Mutable, Pan, PanLaw, Pitch, PlayRate, ProbablyMutable,
    Project, Reaper, Source, TakeFX, Volume, WithReaperPtr, FX, GUID,
};
use int_enum::IntEnum;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, PartialEq)]
pub struct Take<'a, T: ProbablyMutable> {
    ptr: MediaItemTake,
    should_check: bool,
    item: &'a Item<'a, T>,
}
impl<'a, T: ProbablyMutable> FXParent<'a, TakeFX<'a, Immutable>>
    for Take<'a, T>
{
    fn n_fx(&self) -> usize {
        unsafe {
            Reaper::get().low().TakeFX_GetCount(self.get().as_ptr()) as usize
        }
    }
    fn get_fx(&self, index: usize) -> Option<TakeFX<Immutable>> {
        let fx = TakeFX::from_index(unsafe { transmute(self) }, index);
        fx
    }
}
impl<'a, T: ProbablyMutable> KnowsProject for Take<'a, T> {
    fn project(&self) -> &Project {
        self.item.project()
    }
}
impl<'a, T: ProbablyMutable> WithReaperPtr for Take<'a, T> {
    type Ptr = MediaItemTake;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid_2(self.item().project()).unwrap();
        self.ptr
    }
    fn make_unchecked(&mut self) {
        self.should_check = false;
    }
    fn make_checked(&mut self) {
        self.should_check = true;
    }
    fn should_check(&self) -> bool {
        self.should_check
    }
}
impl<'a, T: ProbablyMutable> Take<'a, T> {
    pub fn new(ptr: MediaItemTake, item: &'a Item<'a, T>) -> Self {
        Self {
            ptr,
            should_check: true,
            item,
        }
    }
    pub fn item(&self) -> &Item<T> {
        self.item
    }
    pub fn get_visible_fx(&'a self) -> Option<TakeFX<'a, Immutable>> {
        let result = unsafe {
            Reaper::get()
                .low()
                .TakeFX_GetChainVisible(self.get().as_ptr())
        };
        if result < 0 {
            None
        } else {
            TakeFX::from_index(unsafe { transmute(self) }, result as usize)
        }
    }
    pub fn is_active(&self) -> bool {
        self.get() == self.item().active_take().get()
    }
    pub fn is_midi(&self) -> bool {
        unsafe { Reaper::get().low().TakeIsMIDI(self.get().as_ptr()) }
    }
    pub fn n_envelopes(&self) -> usize {
        unsafe {
            Reaper::get().low().CountTakeEnvelopes(self.get().as_ptr())
                as usize
        }
    }
    pub fn n_fx(&self) -> usize {
        unsafe {
            Reaper::get().low().TakeFX_GetCount(self.get().as_ptr()) as usize
        }
    }
    pub fn n_midi_events(&self) -> usize {
        let mut notes = MaybeUninit::zeroed();
        let mut cc = MaybeUninit::zeroed();
        let mut sysex = MaybeUninit::zeroed();
        unsafe {
            Reaper::get().low().MIDI_CountEvts(
                self.get().as_ptr(),
                notes.as_mut_ptr(),
                cc.as_mut_ptr(),
                sysex.as_mut_ptr(),
            ) as usize
        }
    }
    pub fn name(&self) -> String {
        let result =
            unsafe { Reaper::get().low().GetTakeName(self.get().as_ptr()) };
        as_string(result).expect("Can not convert name to string")
    }

    pub fn source(&self) -> Option<Source<Immutable>> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetMediaItemTake_Source(self.get().as_ptr())
        };
        match PcmSource::new(ptr) {
            None => None,
            Some(ptr) => Some(Source::new(unsafe { transmute(self) }, ptr)),
        }
    }

    /// Get iterator on human-readable MIDI events.
    ///
    /// See [crate::midi]
    pub fn iter_midi(
        &self,
        buf_size_override: impl Into<Option<i32>>,
    ) -> ReaperStaticResult<MidiEventBuilder> {
        let buf = self.get_midi(buf_size_override)?;
        Ok(MidiEventBuilder::new(buf.into_iter()))
    }

    /// Get take raw midi data.
    ///
    /// It is quite useless as it is, but, it can be used several times with
    /// [MidiEventBuilder] for iterating through various event types.
    ///
    /// if buffer_size is not overrided — max size will be used.
    ///
    /// # In the case one desire to iter through raw binary data
    ///
    /// MIDI buffer is returned as a list of `{ int offset, char flag, int
    /// msglen, unsigned char msg[] }`.
    /// - offset: MIDI ticks from previous event
    /// - flag: &1=selected &2=muted
    /// - flag high 4 bits for CC shape: &16=linear, &32=slow start/end,
    ///   &16|32=fast start, &64=fast end, &64|16=bezier
    /// - msg: the MIDI message.
    /// - A meta-event of type 0xF followed by 'CCBZ ' and 5 more bytes
    ///   represents bezier curve data for the previous MIDI event: 1 byte for
    ///   the bezier type (usually 0) and 4 bytes for the bezier tension as a
    ///   float.
    /// - For tick intervals longer than a 32 bit word can represent,
    ///   zero-length meta events may be placed between valid events.
    pub fn get_midi(
        &self,
        buf_size_override: impl Into<Option<i32>>,
    ) -> ReaperStaticResult<Vec<u8>> {
        let size = buf_size_override.into().unwrap_or(i32::MAX - 100);
        let mut buf = vec![0_u8; size as usize];
        let raw = buf.as_mut_ptr() as *mut c_char;
        let mut size = MaybeUninit::new(size);
        let result = unsafe {
            Reaper::get().low().MIDI_GetAllEvts(
                self.get().as_ptr(),
                raw,
                size.as_mut_ptr(),
            )
        };
        let size = unsafe { size.assume_init() };
        buf.truncate(size as usize);
        match result {
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not get midi"))
            }
            true => Ok(buf),
        }
    }

    fn get_info_string(
        &self,
        category: impl Into<String>,
        size: usize,
    ) -> ReaperStaticResult<String> {
        let mut category = category.into();
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().GetSetMediaItemTakeInfo_String(
                self.get().as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
                buf,
                false,
            )
        };
        match result {
            true => {
                Ok(as_string_mut(buf)
                    .expect("Can not convert value to string."))
            }
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not get value"))
            }
        }
    }

    pub fn guid(&self) -> GUID {
        let guid_str = self
            .get_info_string("GUID", 50)
            .expect("Can not get guid string");
        GUID::from_string(guid_str).expect("can not convert string to GUID")
    }

    fn get_info_value(&self, category: impl Into<String>) -> f64 {
        let mut category = category.into();
        unsafe {
            Reaper::get().low().GetMediaItemTakeInfo_Value(
                self.get().as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
            )
        }
    }

    pub fn start_offset(&self) -> Duration {
        Duration::from_secs_f64(self.get_info_value("D_STARTOFFS"))
    }

    pub fn volume(&self) -> Volume {
        Volume::from(self.get_info_value("D_VOL"))
    }

    pub fn pan(&self) -> Pan {
        Pan::from(self.get_info_value("D_PAN"))
    }

    pub fn pan_law(&self) -> PanLaw {
        PanLaw::from(self.get_info_value("D_PANLAW"))
    }

    pub fn play_rate(&self) -> PlayRate {
        PlayRate::from(self.get_info_value("D_PLAYRATE"))
    }

    /// take pitch adjustment in semitones, -12=one octave down, 0=normal,
    /// +12=one octave up, etc
    pub fn pitch(&self) -> Pitch {
        Pitch::from(self.get_info_value("D_PITCH"))
    }

    /// preserve pitch when changing playback rate
    pub fn preserve_pitch(&self) -> bool {
        self.get_info_value("B_PPITCH") != 0.0
    }

    /// Y-position (relative to top of track) in pixels (read-only)
    pub fn y_pos(&self) -> usize {
        self.get_info_value("I_LASTY") as usize
    }

    /// height in pixels (read-only)
    pub fn height(&self) -> usize {
        self.get_info_value("I_LASTH") as usize
    }

    pub fn channel_mode(&self) -> TakeChannelMode {
        TakeChannelMode::from_int(self.get_info_value("I_CHANMODE") as i32)
            .expect("can not convert value to channel mode")
    }

    pub fn pitch_mode(&self) -> Option<TakePitchMode> {
        let result = self.get_info_value("I_PITCHMODE") as i32;
        match result {
            x if x < 0 => None,
            y => Some(TakePitchMode::from_raw(y)),
        }
    }

    /// if None → default.
    pub fn color(&self) -> Option<Color> {
        let raw = self.get_info_value("I_CUSTOMCOLOR") as i32;
        if raw == 0 {
            return None;
        }
        Some(Color::from_native(raw & 0xffffff))
    }
}

impl<'a> Take<'a, Mutable> {
    pub fn add_audio_accessor(
        &mut self,
    ) -> AudioAccessor<'a, Take<Mutable>, Mutable> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .CreateTakeAudioAccessor(self.get().as_ptr())
        };
        match ptr_wrappers::AudioAccessor::new(ptr) {
            None => panic!("Can not create audio accessor"),
            Some(ptr) => AudioAccessor::new(self, ptr),
        }
    }

    pub fn set_active(&mut self) {
        unsafe { Reaper::get().low().SetActiveTake(self.get().as_ptr()) }
    }

    /// Add FX at given position, or return existing one.
    ///
    /// If `even_if_exists` is `false`, plugin will be added only
    /// if no plugin exists on track.
    ///
    /// Otherwise, if position is None → the last slot will be used.
    /// The resulting FX will have real index, that may differ from the
    /// desired.
    pub fn add_fx(
        &mut self,
        name: impl Into<String>,
        position: impl Into<Option<u8>>,
        even_if_exists: bool,
    ) -> Option<TakeFX<Mutable>> {
        let insatantinate = match even_if_exists {
            false => 1 as i32,
            true => match position.into() {
                None => -1 as i32,
                Some(pos) => -1000 - pos as i32,
            },
        };
        let index = unsafe {
            Reaper::get().low().TakeFX_AddByName(
                self.get().as_ptr(),
                as_c_str(name.into().with_null()).as_ptr(),
                insatantinate,
            )
        };
        TakeFX::<Mutable>::from_index(self, index as usize)
    }

    pub fn get_fx_mut(
        &'a mut self,
        index: usize,
    ) -> Option<TakeFX<'a, Mutable>> {
        TakeFX::from_index(self, index)
    }
    pub fn get_visible_fx_mut(&'a mut self) -> Option<TakeFX<'a, Mutable>> {
        let result = unsafe {
            Reaper::get()
                .low()
                .TakeFX_GetChainVisible(self.get().as_ptr())
        };
        if result < 0 {
            None
        } else {
            TakeFX::from_index(self, result as usize)
        }
    }

    pub fn select_all_midi_events(&mut self, select: bool) {
        assert!(self.is_midi());
        unsafe {
            Reaper::get()
                .low()
                .MIDI_SelectAll(self.get().as_ptr(), select)
        }
    }

    pub fn sort_midi(&mut self) {
        assert!(self.is_midi());
        unsafe { Reaper::get().low().MIDI_Sort(self.get().as_ptr()) }
    }

    pub fn source_mut(&mut self) -> Option<Source<Mutable>> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetMediaItemTake_Source(self.get().as_ptr())
        };
        match PcmSource::new(ptr) {
            None => None,
            Some(ptr) => Some(Source::new(self, ptr)),
        }
    }

    pub fn set_source<T: ProbablyMutable>(
        &mut self,
        source: Source<T>,
    ) -> ReaperStaticResult<()> {
        let result = unsafe {
            Reaper::get().low().SetMediaItemTake_Source(
                self.get().as_ptr(),
                source.get().as_ptr(),
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("can not set source"))
            }
        }
    }

    /// Set raw MIDI to take.
    ///
    /// Probably, it's bad idea to construct it DIY, so, see:
    /// - [crate::midi]
    /// - [Take::get_midi]
    /// - [Take::iter_midi]
    pub fn set_midi(&mut self, mut midi: Vec<u8>) -> ReaperStaticResult<()> {
        let raw = midi.as_mut_ptr() as *mut c_char;
        let result = unsafe {
            Reaper::get().low().MIDI_SetAllEvts(
                self.get().as_ptr(),
                raw,
                midi.len() as i32,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set midi"))
            }
        }
    }

    fn set_info_string(
        &mut self,
        category: impl Into<String>,
        string: impl Into<String>,
    ) -> ReaperStaticResult<()> {
        let mut category = category.into();
        let string = string.into();
        let buf = as_c_string(&string).into_raw();
        let result = unsafe {
            Reaper::get().low().GetSetMediaItemTakeInfo_String(
                self.get().as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
                buf,
                true,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not get value"))
            }
        }
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.set_info_string("P_NAME", name)
            .expect("Can not set name")
    }
    pub fn set_guid(&mut self, guid: GUID) {
        self.set_info_string("GUID", guid.to_string())
            .expect("Can not set guid")
    }
    fn set_info_value(
        &mut self,
        category: impl Into<String>,
        value: f64,
    ) -> ReaperStaticResult<()> {
        let category = category.into();
        let result = unsafe {
            Reaper::get().low().SetMediaItemTakeInfo_Value(
                self.get().as_ptr(),
                as_c_string(&category).as_ptr(),
                value,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set value"))
            }
        }
    }

    pub fn set_start_offset(
        &mut self,
        offset: Duration,
    ) -> ReaperStaticResult<()> {
        self.set_info_value("D_STARTOFFS", offset.as_secs_f64())
    }

    pub fn set_volume(&mut self, volume: Volume) {
        self.set_info_value("D_VOL", volume.into())
            .expect("Can not set volume")
    }

    pub fn set_pan(&mut self, pan: Pan) {
        self.set_info_value("D_PAN", pan.into())
            .expect("Can not set pan")
    }

    pub fn set_pan_law(&mut self, pan_law: PanLaw) {
        self.set_info_value("D_PANLAW", pan_law.into())
            .expect("can't set pan law")
    }

    pub fn set_play_rate(
        &mut self,
        play_rate: PlayRate,
    ) -> ReaperStaticResult<()> {
        self.set_info_value("D_PLAYRATE", play_rate.into())
    }

    /// take pitch adjustment in semitones, -12=one octave down, 0=normal,
    /// +12=one octave up, etc
    pub fn set_pitch(&mut self, pitch: Pitch) -> ReaperStaticResult<()> {
        self.set_info_value("D_PITCH", pitch.get())
    }

    /// preserve pitch when changing playback rate
    pub fn set_preserve_pitch(&mut self, preserve: bool) {
        self.set_info_value("B_PPITCH", preserve as i32 as f64)
            .expect("can not set preserve pitch")
    }

    pub fn set_channel_mode(&mut self, mode: TakeChannelMode) {
        self.set_info_value("I_CHANMODE", mode.int_value() as f64)
            .expect("can not set channel mode")
    }

    pub fn set_pitch_mode(&mut self, mode: Option<TakePitchMode>) {
        let value = match mode {
            None => -1,
            Some(mode) => mode.as_raw(),
        };
        self.set_info_value("I_PITCHMODE", value as f64)
            .expect("can not set pitch mode")
    }

    /// if None → default.
    pub fn set_color(&mut self, color: Option<Color>) {
        let color = match color {
            None => 0,
            Some(color) => color.to_native() | 0x1000000,
        };
        self.set_info_value("I_CUSTOMCOLOR", color as f64).unwrap()
    }
}

#[repr(i32)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, IntEnum, Serialize, Deserialize,
)]
pub enum TakeChannelMode {
    Normal = 0,
    ReserveStereo = 1,
    DownMix = 2,
    Left = 3,
    Right = 4,
}

/// Represents pitch shifter and setting.
///
/// Currently, holds only raw values, but later, probably, will hold additional
/// representation of them human-readably.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TakePitchMode {
    shifter: u32,
    parameter: u32,
}
impl TakePitchMode {
    pub fn new(shifter: u16, parameter: u16) -> Self {
        Self {
            shifter: shifter as u32,
            parameter: parameter as u32,
        }
    }
    pub fn shifter(&self) -> u16 {
        self.shifter as u16
    }
    pub fn parameter(&self) -> u16 {
        self.parameter as u16
    }
    pub fn from_raw(raw: i32) -> Self {
        let raw = raw as u32;
        let shifter = (raw >> 0xf) & 0xffff;
        let parameter = raw & 0xffff;
        Self { shifter, parameter }
    }
    pub fn as_raw(&self) -> i32 {
        (self.shifter << 0xf | self.parameter) as i32
    }
}
