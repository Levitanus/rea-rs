use std::{ffi::c_char, mem::MaybeUninit};

use crate::{
    ptr_wrappers::{
        self, MediaItem, MediaItemTake, PcmSource, ReaProject, TrackEnvelope,
    },
    utils::{as_c_str, as_c_string, as_string, string_from_buf, WithNull},
    AudioAccessor, Color, Envelope, FXParent, Item, KnowsProject,
    MidiEventBuilder, Pan, PanLaw, Pitch, PlayRate, Project, ProjectContext,
    ReaRsError, Reaper, ReaperResult, Source, SourceOffset, TakeFX, Volume,
    WithReaperPtr, FX, GUID,
};
use int_enum::IntEnum;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone)]
pub struct Take {
    ptr: MediaItemTake,
    should_check: bool,
    item_ptr: Option<MediaItem>,
    project_ptr: Option<ReaProject>,
}
impl FXParent<TakeFX> for Take {
    fn n_fx(&self) -> ReaperResult<usize> {
        Ok(unsafe {
            Reaper::get().low().TakeFX_GetCount(self.get()?.as_ptr()) as usize
        })
    }
    fn get_fx(&self, index: usize) -> ReaperResult<Option<TakeFX>> {
        TakeFX::from_index(self, index)
    }
}
impl KnowsProject for Take {
    fn project(&self) -> Project {
        match self.project_ptr {
            Some(ptr) => Project::new(ProjectContext::Proj(ptr)),
            None => Project::new(ProjectContext::CurrentProject),
        }
    }
}
impl WithReaperPtr for Take {
    type Ptr = MediaItemTake;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Result<Self::Ptr, ReaRsError> {
        self.require_valid_2(&self.project())
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
impl Take {
    pub fn new(ptr: MediaItemTake, item: &Item) -> ReaperResult<Self> {
        Ok(Self {
            ptr,
            should_check: true,
            item_ptr: Some(item.get()?),
            project_ptr: Some(item.project().get()?),
        })
    }

    pub fn item(&self) -> ReaperResult<Item> {
        let item = match self.item_ptr {
            Some(ptr) => Item::from_raw_project_ptr(self.project_ptr, ptr),
            None => {
                let ptr = unsafe {
                    Reaper::get()
                        .low()
                        .GetMediaItemTake_Item(self.get()?.as_ptr())
                };
                Item::from_raw_project_ptr(
                    self.project_ptr,
                    MediaItem::new(ptr)
                        .ok_or(ReaRsError::NullPtr("MediaItem"))?,
                )
            }
        };
        Ok(item)
    }

    pub fn get_visible_fx(&self) -> ReaperResult<Option<TakeFX>> {
        let result = unsafe {
            Reaper::get()
                .low()
                .TakeFX_GetChainVisible(self.get()?.as_ptr())
        };
        if result < 0 {
            Ok(None)
        } else {
            TakeFX::from_index(self, result as usize)
        }
    }

    pub fn is_active(&self) -> ReaperResult<bool> {
        Ok(self.item()?.active_take()?.get()? == self.get()?)
    }

    pub fn is_midi(&self) -> ReaperResult<bool> {
        Ok(unsafe { Reaper::get().low().TakeIsMIDI(self.get()?.as_ptr()) })
    }

    pub fn n_envelopes(&self) -> ReaperResult<usize> {
        Ok(unsafe {
            Reaper::get().low().CountTakeEnvelopes(self.get()?.as_ptr())
                as usize
        })
    }

    pub fn n_midi_events(&self) -> ReaperResult<usize> {
        let mut notes = MaybeUninit::zeroed();
        let mut cc = MaybeUninit::zeroed();
        let mut sysex = MaybeUninit::zeroed();
        Ok(unsafe {
            Reaper::get().low().MIDI_CountEvts(
                self.get()?.as_ptr(),
                notes.as_mut_ptr(),
                cc.as_mut_ptr(),
                sysex.as_mut_ptr(),
            ) as usize
        })
    }

    pub fn name(&self) -> ReaperResult<String> {
        let result =
            unsafe { Reaper::get().low().GetTakeName(self.get()?.as_ptr()) };
        Ok(as_string(result).expect("Can not convert name to string"))
    }

    pub fn source(&self) -> ReaperResult<Option<Source>> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetMediaItemTake_Source(self.get()?.as_ptr())
        };
        match PcmSource::new(ptr) {
            None => Ok(None),
            Some(ptr) => Ok(Some(Source::new(self, ptr)?)),
        }
    }

    /// Get iterator on human-readable MIDI events.
    ///
    /// See [crate::midi]
    pub fn iter_midi(
        &self,
        buf_size_override: impl Into<Option<i32>>,
    ) -> ReaperResult<MidiEventBuilder> {
        let buf = self.get_midi(buf_size_override)?;
        Ok(MidiEventBuilder::new(buf.into_iter()))
    }

    /// Get take raw midi data.
    ///
    /// It is quite useless as it is, but, it can be used several times with
    /// [MidiEventBuilder] for iterating through various event types.
    ///
    /// if buffer_size is not overrided - max size will be used.
    pub fn get_midi(
        &self,
        buf_size_override: impl Into<Option<i32>>,
    ) -> ReaperResult<Vec<u8>> {
        let size = buf_size_override.into().unwrap_or(i32::MAX - 100);
        let mut buf = vec![0_u8; size as usize];
        let raw = buf.as_mut_ptr() as *mut c_char;
        let mut size = MaybeUninit::new(size);
        let result = unsafe {
            Reaper::get().low().MIDI_GetAllEvts(
                self.get()?.as_ptr(),
                raw,
                size.as_mut_ptr(),
            )
        };
        let size = unsafe { size.assume_init() };
        buf.truncate(size as usize);
        match result {
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not get midi"))
            }
            true => Ok(buf),
        }
    }

    fn get_info_string(
        &self,
        category: impl Into<String>,
        size: usize,
    ) -> ReaperResult<String> {
        if size < 2 {
            return Err(ReaRsError::InvalidObject(
                "buffer size must be at least 2",
            ));
        }
        let mut category = category.into();
        let mut buf = vec![0_i8; size];
        let result = unsafe {
            Reaper::get().low().GetSetMediaItemTakeInfo_String(
                self.get()?.as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
                buf.as_mut_ptr(),
                false,
            )
        };
        if !result {
            return Err(ReaRsError::UnsuccessfulOperation(
                "Can not get value",
            ));
        }

        string_from_buf(&buf)
    }

    pub fn guid(&self) -> ReaperResult<GUID> {
        let guid_str = self.get_info_string("GUID", 50)?;
        Ok(GUID::from_string(guid_str)
            .expect("can not convert string to GUID"))
    }

    fn get_info_value(
        &self,
        category: impl Into<String>,
    ) -> ReaperResult<f64> {
        let mut category = category.into();
        Ok(unsafe {
            Reaper::get().low().GetMediaItemTakeInfo_Value(
                self.get()?.as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
            )
        })
    }

    pub fn get_envelope(
        &self,
        index: usize,
    ) -> ReaperResult<Option<Envelope<'_, Self>>> {
        let rpr = Reaper::get();
        let ptr = unsafe {
            rpr.low()
                .GetTakeEnvelope(self.get()?.as_ptr(), index as i32)
        };
        if let Some(env) = TrackEnvelope::new(ptr) {
            Ok(Some(Envelope::new(env, self)))
        } else {
            Ok(None)
        }
    }

    pub fn start_offset(&self) -> ReaperResult<SourceOffset> {
        Ok(SourceOffset::from_secs_f64(
            self.get_info_value("D_STARTOFFS")?,
        ))
    }

    pub fn volume(&self) -> ReaperResult<Volume> {
        Ok(Volume::from(self.get_info_value("D_VOL")?))
    }

    pub fn pan(&self) -> ReaperResult<Pan> {
        Ok(Pan::from(self.get_info_value("D_PAN")?))
    }

    pub fn pan_law(&self) -> ReaperResult<PanLaw> {
        Ok(PanLaw::from(self.get_info_value("D_PANLAW")?))
    }

    pub fn play_rate(&self) -> ReaperResult<PlayRate> {
        Ok(PlayRate::from(self.get_info_value("D_PLAYRATE")?))
    }

    /// take pitch adjustment in semitones, -12=one octave down, 0=normal,
    /// +12=one octave up, etc
    pub fn pitch(&self) -> ReaperResult<Pitch> {
        Ok(Pitch::from(self.get_info_value("D_PITCH")?))
    }

    /// preserve pitch when changing playback rate
    pub fn preserve_pitch(&self) -> ReaperResult<bool> {
        Ok(self.get_info_value("B_PPITCH")? != 0.0)
    }

    /// Y-position (relative to top of track) in pixels (read-only)
    pub fn y_pos(&self) -> ReaperResult<usize> {
        Ok(self.get_info_value("I_LASTY")? as usize)
    }

    /// height in pixels (read-only)
    pub fn height(&self) -> ReaperResult<usize> {
        Ok(self.get_info_value("I_LASTH")? as usize)
    }

    pub fn channel_mode(&self) -> ReaperResult<TakeChannelMode> {
        Ok(
            TakeChannelMode::from_int(
                self.get_info_value("I_CHANMODE")? as i32
            )
            .expect("can not convert value to channel mode"),
        )
    }

    pub fn pitch_mode(&self) -> ReaperResult<Option<TakePitchMode>> {
        let result = self.get_info_value("I_PITCHMODE")? as i32;
        match result {
            x if x < 0 => Ok(None),
            y => Ok(Some(TakePitchMode::from_raw(y))),
        }
    }

    /// if None -> default.
    pub fn color(&self) -> ReaperResult<Option<Color>> {
        let raw = self.get_info_value("I_CUSTOMCOLOR")? as i32;
        if raw == 0 {
            return Ok(None);
        }
        Ok(Some(Color::from_native(raw & 0xffffff)))
    }

    pub fn peaks(
        &self,
        peakrate: f64,
        starttime: f64,
        numchannels: usize,
        numsamplesperchannel: usize,
        spectral_peaks: bool,
    ) -> ReaperResult<TakePeaksResult> {
        let block_size = match spectral_peaks {
            true => 3,
            false => 2,
        };
        let capacity = numsamplesperchannel * numchannels * block_size;
        let mut buf: Vec<f64> = Vec::with_capacity(100);
        for _ in 0..capacity {
            buf.push(0.0);
        }
        let want_extra_type = match spectral_peaks {
            true => 115,
            false => 0,
        };
        let result = unsafe {
            Reaper::get().low().GetMediaItemTake_Peaks(
                self.get()?.as_ptr(),
                peakrate,
                starttime,
                numchannels as i32,
                numsamplesperchannel as i32,
                want_extra_type,
                buf.as_mut_ptr(),
            )
        };
        if result <= 0 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "Can not get peaks",
            ));
        }
        Ok(TakePeaksResult::new(result, buf, capacity / block_size))
    }

    pub fn add_audio_accessor(
        &mut self,
    ) -> anyhow::Result<AudioAccessor<'_, Self>> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .CreateTakeAudioAccessor(self.get()?.as_ptr())
        };
        match ptr_wrappers::AudioAccessor::new(ptr) {
            None => {
                Err(ReaRsError::InvalidObject("Can not create audio accessor")
                    .into())
            }
            Some(ptr) => Ok(AudioAccessor::new(self, ptr)),
        }
    }

    pub fn set_active(&mut self) -> ReaperResult<()> {
        unsafe { Reaper::get().low().SetActiveTake(self.get()?.as_ptr()) }
        Ok(())
    }

    /// Add FX at given position, or return existing one.
    ///
    /// If `even_if_exists` is `false`, plugin will be added only
    /// if no plugin exists on track.
    ///
    /// Otherwise, if position is None -> the last slot will be used.
    /// The resulting FX will have real index, that may differ from the
    /// desired.
    pub fn add_fx(
        &mut self,
        name: impl Into<String>,
        position: impl Into<Option<u8>>,
        even_if_exists: bool,
    ) -> ReaperResult<Option<TakeFX>> {
        let insatantinate = match even_if_exists {
            false => 1_i32,
            true => match position.into() {
                None => -1_i32,
                Some(pos) => -1000 - pos as i32,
            },
        };
        let index = unsafe {
            Reaper::get().low().TakeFX_AddByName(
                self.get()?.as_ptr(),
                as_c_str(name.into().with_null()).as_ptr(),
                insatantinate,
            )
        };
        TakeFX::from_index(self, index as usize)
    }

    pub fn get_fx_mut(
        &mut self,
        index: usize,
    ) -> ReaperResult<Option<TakeFX>> {
        TakeFX::from_index(self, index)
    }

    pub fn get_visible_fx_mut(&mut self) -> ReaperResult<Option<TakeFX>> {
        let result = unsafe {
            Reaper::get()
                .low()
                .TakeFX_GetChainVisible(self.get()?.as_ptr())
        };
        if result < 0 {
            Ok(None)
        } else {
            TakeFX::from_index(self, result as usize)
        }
    }

    pub fn get_envelope_mut(
        &mut self,
        index: usize,
    ) -> ReaperResult<Option<Envelope<'_, Self>>> {
        let rpr = Reaper::get();
        let ptr = unsafe {
            rpr.low()
                .GetTakeEnvelope(self.get()?.as_ptr(), index as i32)
        };
        if let Some(env) = TrackEnvelope::new(ptr) {
            Ok(Some(Envelope::new(env, self)))
        } else {
            Ok(None)
        }
    }

    pub fn select_all_midi_events(
        &mut self,
        select: bool,
    ) -> ReaperResult<()> {
        assert!(self.is_midi()?);
        unsafe {
            Reaper::get()
                .low()
                .MIDI_SelectAll(self.get()?.as_ptr(), select)
        }
        Ok(())
    }

    pub fn sort_midi(&mut self) -> ReaperResult<()> {
        assert!(self.is_midi()?);
        unsafe { Reaper::get().low().MIDI_Sort(self.get()?.as_ptr()) }
        Ok(())
    }

    pub fn source_mut(&mut self) -> ReaperResult<Option<Source>> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetMediaItemTake_Source(self.get()?.as_ptr())
        };
        match PcmSource::new(ptr) {
            None => Ok(None),
            Some(ptr) => Ok(Some(Source::new(self, ptr)?)),
        }
    }

    pub fn set_source(&mut self, source: Source) -> ReaperResult<()> {
        let result = unsafe {
            Reaper::get().low().SetMediaItemTake_Source(
                self.get()?.as_ptr(),
                source.get()?.as_ptr(),
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("can not set source"))
            }
        }
    }

    /// Set raw MIDI to take.
    ///
    /// Probably, it's bad idea to construct it DIY, so, see:
    /// - [crate::midi]
    /// - [Take::get_midi]
    /// - [Take::iter_midi]
    pub fn set_midi(&mut self, mut midi: Vec<u8>) -> ReaperResult<()> {
        let raw = midi.as_mut_ptr() as *mut c_char;
        let result = unsafe {
            Reaper::get().low().MIDI_SetAllEvts(
                self.get()?.as_ptr(),
                raw,
                midi.len() as i32,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not set midi"))
            }
        }
    }

    fn set_info_string(
        &mut self,
        category: impl Into<String>,
        string: impl Into<String>,
    ) -> ReaperResult<()> {
        let mut category = category.into();
        let string = string.into();
        let buf = as_c_string(&string).into_raw();
        let result = unsafe {
            Reaper::get().low().GetSetMediaItemTakeInfo_String(
                self.get()?.as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
                buf,
                true,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not get value"))
            }
        }
    }

    pub fn set_name(&mut self, name: impl Into<String>) -> ReaperResult<()> {
        self.set_info_string("P_NAME", name)
    }
    pub fn set_guid(&mut self, guid: GUID) -> ReaperResult<()> {
        self.set_info_string("GUID", guid.to_string())
    }
    fn set_info_value(
        &mut self,
        category: impl Into<String>,
        value: f64,
    ) -> ReaperResult<()> {
        let category = category.into();
        let result = unsafe {
            Reaper::get().low().SetMediaItemTakeInfo_Value(
                self.get()?.as_ptr(),
                as_c_string(&category).as_ptr(),
                value,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaRsError::UnsuccessfulOperation("Can not set value"))
            }
        }
    }

    pub fn set_start_offset(
        &mut self,
        offset: SourceOffset,
    ) -> ReaperResult<()> {
        self.set_info_value("D_STARTOFFS", offset.as_secs_f64())
    }

    pub fn set_volume(&mut self, volume: Volume) -> ReaperResult<()> {
        self.set_info_value("D_VOL", volume.into())
    }

    pub fn set_pan(&mut self, pan: Pan) -> ReaperResult<()> {
        self.set_info_value("D_PAN", pan.into())
    }

    pub fn set_pan_law(&mut self, pan_law: PanLaw) -> ReaperResult<()> {
        self.set_info_value("D_PANLAW", pan_law.into())
    }

    pub fn set_play_rate(&mut self, play_rate: PlayRate) -> ReaperResult<()> {
        self.set_info_value("D_PLAYRATE", play_rate.into())
    }

    /// take pitch adjustment in semitones, -12=one octave down, 0=normal,
    /// +12=one octave up, etc
    pub fn set_pitch(&mut self, pitch: Pitch) -> ReaperResult<()> {
        self.set_info_value("D_PITCH", pitch.get())
    }

    /// preserve pitch when changing playback rate
    pub fn set_preserve_pitch(&mut self, preserve: bool) -> ReaperResult<()> {
        self.set_info_value("B_PPITCH", preserve as i32 as f64)
    }

    pub fn set_channel_mode(
        &mut self,
        mode: TakeChannelMode,
    ) -> ReaperResult<()> {
        self.set_info_value("I_CHANMODE", mode.int_value() as f64)
    }

    pub fn set_pitch_mode(
        &mut self,
        mode: Option<TakePitchMode>,
    ) -> ReaperResult<()> {
        let value = match mode {
            None => -1,
            Some(mode) => mode.as_raw(),
        };
        self.set_info_value("I_PITCHMODE", value as f64)
    }

    /// if None -> default.
    pub fn set_color(&mut self, color: Option<Color>) -> ReaperResult<()> {
        let color = match color {
            None => 0,
            Some(color) => color.to_native() | 0x1000000,
        };
        self.set_info_value("I_CUSTOMCOLOR", color as f64)
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

/// Return struct for Take::peaks()
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TakePeaksResult {
    result: i32,
    peaks: Vec<f64>,
    blocksize: usize,
}
impl TakePeaksResult {
    fn new(result: i32, peaks: Vec<f64>, blocksize: usize) -> Self {
        Self {
            result,
            peaks,
            blocksize,
        }
    }
    pub fn return_val_raw(&self) -> i32 {
        self.result
    }
    pub fn peaks_raw(&self) -> &Vec<f64> {
        &self.peaks
    }
    pub fn peaks_raw_mut(&mut self) -> &mut Vec<f64> {
        &mut self.peaks
    }
    pub fn output_mode(&self) -> usize {
        (self.result / 0xf00000) as usize
    }
    pub fn is_spectral_available(&self) -> bool {
        (self.result / 0x1000000) != 0
    }
    pub fn num_samples_available(&self) -> usize {
        (self.result % 0xf0000) as usize
    }
    pub fn peaks_max(&self) -> Vec<f64> {
        let mut ret = Vec::new();
        for i in 0..self.blocksize {
            ret.push(self.peaks[i])
        }
        ret
    }
    pub fn peaks_min(&self) -> Vec<f64> {
        let mut ret = Vec::new();
        for i in self.blocksize..self.blocksize * 2 {
            ret.push(self.peaks[i])
        }
        ret
    }
    pub fn peaks_extra(&self) -> Option<Vec<f64>> {
        if !self.is_spectral_available() {
            return None;
        }
        let mut ret = Vec::new();
        for i in self.blocksize * 2..self.blocksize * 3 {
            ret.push(self.peaks[i])
        }
        Some(ret)
    }
    pub fn max(&self) -> f64 {
        self.peaks[0..self.blocksize]
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max)
    }
    pub fn min(&self) -> f64 {
        self.peaks[self.blocksize..self.blocksize * 2]
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min)
    }
}
