use crate::{
    errors::{ReaperError, ReaperResult},
    utils::{as_c_str, WithNull},
    AutomationMode, Envelope, Immutable, KnowsProject, Mutable, Pan, PanLaw,
    ProbablyMutable, Reaper, Track, Volume, WithReaperPtr, GUID,
};
use int_enum::IntEnum;
use reaper_medium::{MediaTrack, TrackEnvelope};
use std::ptr::null_mut;

pub trait SendIntType {
    /// <0 for receives, 0=sends, >0 for hardware outputs.
    fn as_int(&self) -> i32 {
        Self::as_int_static()
    }
    /// <0 for receives, 0=sends, >0 for hardware outputs.
    fn as_int_static() -> i32;
}

/// Main Send type, that commonly used.
///
/// Use this struct to construct new sends from one Track to another.
#[derive(Debug, PartialEq)]
pub struct TrackSend<'a, T: ProbablyMutable> {
    track: &'a Track<'a, T>,
    index: usize,
}
impl<'a, T: ProbablyMutable> TrackSend<'a, T> {}
impl<'a> TrackSend<'a, Mutable> {
    /// The only way to make TrackSend from one track to another.
    ///
    /// It should guarantee, that no track, no project itself are
    /// not borrowed as mutable (`Track<Mutable>`). But, actually, it
    /// mutates source or destination track.
    ///
    /// As TrackSend mutates only the field it responses to, and it
    /// almost can not affect other objects state — It uses a hack to
    /// mutate `<Immutable>` tracks. Keep this in mind.
    pub fn create_new(
        source: &Track<Immutable>,
        destination: &Track<Immutable>,
    ) -> Self {
        if source.project() != destination.project() {
            panic!("Tracks are from different projects")
        };
        let index = unsafe {
            Reaper::get().low().CreateTrackSend(
                source.get().as_ptr(),
                destination.get().as_ptr(),
            )
        };
        let source: &Track<'a, Mutable> =
            unsafe { std::mem::transmute(source) };
        TrackSend::new(&source, index as usize)
    }
}
impl<'a, T: ProbablyMutable> GenericSend<'a, T> for TrackSend<'a, T> {
    fn new(track: &'a Track<T>, index: usize) -> Self {
        Self { track, index }
    }
    /// Track that sends outside
    fn parent_track(&self) -> &Track<T> {
        self.track
    }

    fn index(&self) -> usize {
        self.index
    }
}
impl<'a, T: ProbablyMutable> SendIntType for TrackSend<'a, T> {
    fn as_int_static() -> i32 {
        0
    }
}
impl<'a> GenericSendMut<'a> for TrackSend<'a, Mutable> {}

/// Send, that is child of destination track.
#[derive(Debug, PartialEq)]
pub struct TrackReceive<'a, T: ProbablyMutable> {
    track: &'a Track<'a, T>,
    index: usize,
}
impl<'a, T: ProbablyMutable> TrackReceive<'a, T> {}
impl<'a, T: ProbablyMutable> GenericSend<'a, T> for TrackReceive<'a, T> {
    fn new(track: &'a Track<T>, index: usize) -> Self {
        Self { track, index }
    }
    /// Track, that receives.
    fn parent_track(&self) -> &Track<T> {
        self.track
    }

    fn index(&self) -> usize {
        self.index
    }
}
impl<'a, T: ProbablyMutable> SendIntType for TrackReceive<'a, T> {
    fn as_int_static() -> i32 {
        -1
    }
}
impl<'a> GenericSendMut<'a> for TrackReceive<'a, Mutable> {}

/// Send from Track to hardware outputs.
///
/// Supports send to rea_route: see [SendDestChannels]
#[derive(Debug, PartialEq)]
pub struct HardwareSend<'a, T: ProbablyMutable> {
    track: &'a Track<'a, T>,
    index: usize,
}
impl<'a, T: ProbablyMutable> HardwareSend<'a, T> {}
impl<'a, T: ProbablyMutable> GenericSend<'a, T> for HardwareSend<'a, T> {
    fn new(track: &'a Track<T>, index: usize) -> Self {
        Self { track, index }
    }
    fn parent_track(&self) -> &Track<T> {
        self.track
    }

    fn index(&self) -> usize {
        self.index
    }
}
impl<'a, T: ProbablyMutable> SendIntType for HardwareSend<'a, T> {
    fn as_int_static() -> i32 {
        1
    }
}
impl<'a> GenericSendMut<'a> for HardwareSend<'a, Mutable> {}

pub trait GenericSend<'a, T: ProbablyMutable + 'a>: SendIntType {
    fn parent_track(&self) -> &Track<T>;
    fn index(&self) -> usize;
    fn new(track: &'a Track<T>, index: usize) -> Self;

    /// Core method to retrieve send properties.
    /// With probability of 99% you shouldn't use it.
    fn get_info_value(&self, param: impl Into<String>) -> f64 {
        let track_ptr = self.parent_track().get().as_ptr();
        unsafe {
            Reaper::get().low().GetTrackSendInfo_Value(
                track_ptr,
                self.as_int(),
                self.index() as i32,
                as_c_str(param.into().with_null()).as_ptr(),
            )
        }
    }

    fn is_mute(&self) -> bool {
        self.get_info_value("B_MUTE") as i32 != 0
    }
    /// Phase flipped if true.
    fn phase_flipped(&self) -> bool {
        self.get_info_value("B_PHASE") as i32 != 0
    }
    /// Note, that mono parameter is not equal to [SendDestChannels]
    /// and [SendSourceChannels] is_mono parameters.
    fn is_mono(&self) -> bool {
        self.get_info_value("B_MONO") as i32 != 0
    }
    fn volume(&self) -> Volume {
        Volume::from(self.get_info_value("D_VOL"))
    }
    fn pan(&self) -> Pan {
        Pan::from(self.get_info_value("D_PAN"))
    }
    fn pan_law(&self) -> PanLaw {
        PanLaw::from(self.get_info_value("D_PANLAW"))
    }
    fn send_mode(&self) -> SendMode {
        SendMode::from(self.get_info_value("I_SENDMODE"))
    }
    fn automation_mode(&self) -> AutomationMode {
        AutomationMode::from_int(self.get_info_value("I_AUTOMODE") as i32)
            .expect("Can not convert result to Automation mode.")
    }
    /// If `None` returned — the audio is off.
    fn source_channels(&self) -> Option<SendSourceChannels> {
        let value = self.get_info_value("I_SRCCHAN");
        if value < 0.0 {
            return None;
        }
        SendSourceChannels::from(value).into()
    }
    /// Returns `None` if source_channels are `None`.
    fn dest_channels(&self) -> Option<SendDestChannels> {
        self.source_channels()?;
        SendDestChannels::from(self.get_info_value("I_DSTCHAN")).into()
    }
    /// If `None` is returned — MIDI is off.
    fn midi_properties(&self) -> Option<SendMIDIProps> {
        let value = self.get_info_value("I_MIDIFLAGS");
        let flags = value as u32;
        if flags == 0b1111111100000000011111 {
            return None;
        }
        SendMIDIProps::from(value).into()
    }

    fn dest_track(&'a self) -> Option<Track<Immutable>> {
        let result = unsafe {
            Reaper::get().low().GetSetTrackSendInfo(
                self.parent_track().get().as_ptr(),
                self.as_int(),
                self.index() as i32,
                as_c_str(&String::from("P_DESTTRACK\0")).as_ptr(),
                null_mut(),
            ) as *mut reaper_low::raw::MediaTrack
        };
        let ptr = MediaTrack::new(result);
        match ptr {
            None => None,
            Some(ptr) => {
                Track::<Immutable>::new(self.parent_track().project(), ptr)
                    .into()
            }
        }
    }
    fn source_track(&'a self) -> Option<Track<Immutable>> {
        let result = unsafe {
            Reaper::get().low().GetSetTrackSendInfo(
                self.parent_track().get().as_ptr(),
                self.as_int(),
                self.index() as i32,
                as_c_str(&String::from("P_SRCTRACK\0")).as_ptr(),
                null_mut(),
            ) as *mut reaper_low::raw::MediaTrack
        };
        let ptr = MediaTrack::new(result);
        match ptr {
            None => None,
            Some(ptr) => {
                Track::<Immutable>::new(self.parent_track().project(), ptr)
                    .into()
            }
        }
    }
    /// Selector is either [EnvelopeChunk] or [GUID].
    ///
    /// In case of send it's better to use chunk.
    fn get_envelope(
        &'a self,
        selector: impl Into<EnvelopeSelector>,
    ) -> Option<Envelope<Track<T>, T>> {
        let result = unsafe {
            Reaper::get().low().GetSetTrackSendInfo(
                self.parent_track().get().as_ptr(),
                self.as_int(),
                self.index() as i32,
                as_c_str(&String::from(
                    selector.into().to_string().with_null(),
                ))
                .as_ptr(),
                null_mut(),
            ) as *mut reaper_low::raw::TrackEnvelope
        };
        let ptr = TrackEnvelope::new(result);
        match ptr {
            None => None,
            Some(ptr) => Envelope::new(ptr, self.parent_track()).into(),
        }
    }
}
pub trait GenericSendMut<'a>: SendIntType + GenericSend<'a, Mutable> {
    /// Remove send from track. This also drops the value.
    ///
    /// # Note
    ///
    /// `drop(send)` will not remove send from track.
    fn delete(self) -> ReaperResult<()>
    where
        Self: Sized,
    {
        let result = unsafe {
            match Reaper::get().low().RemoveTrackSend(
                self.parent_track().get().as_ptr(),
                self.as_int(),
                self.index() as i32,
            ) {
                true => Ok(()),
                false => Err(ReaperError::UnsuccessfulOperation(
                    "Can not delete send.",
                )
                .into()),
            }
        };
        drop(self);
        result
    }

    /// Core method to set send properties.
    /// With probability of 99% you shouldn't use it.
    fn set_info_value(
        &mut self,
        param: impl Into<String>,
        value: f64,
    ) -> ReaperResult<()> {
        let mut param = param.into();
        let result = unsafe {
            Reaper::get().low().SetTrackSendInfo_Value(
                self.parent_track().get().as_ptr(),
                self.as_int(),
                self.index() as i32,
                as_c_str(&param.with_null()).as_ptr(),
                value,
            )
        };
        match result {
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set value.")
                    .into())
            }
            true => Ok(()),
        }
    }

    fn set_mute(&mut self, mute: bool) -> ReaperResult<()> {
        let mute = match mute {
            true => 1.0,
            false => 0.0,
        };
        self.set_info_value("B_MUTE", mute)
    }
    /// Phase flipped if true.
    fn set_phase(&mut self, phase: bool) -> ReaperResult<()> {
        let phase = match phase {
            true => 1.0,
            false => 0.0,
        };
        self.set_info_value("B_PHASE", phase)
    }
    fn set_mono(&mut self, mono: bool) -> ReaperResult<()> {
        let mono = match mono {
            true => 1.0,
            false => 0.0,
        };
        self.set_info_value("B_MONO", mono)
    }
    fn set_volume(&mut self, volume: impl Into<Volume>) -> ReaperResult<()> {
        self.set_info_value("D_VOL", volume.into().into())
    }
    fn set_pan(&mut self, pan: impl Into<Pan>) -> ReaperResult<()> {
        self.set_info_value("D_PAN", pan.into().into())
    }
    fn set_pan_law(&mut self, pan_law: PanLaw) -> ReaperResult<()> {
        self.set_info_value("D_PANLAW", pan_law.into())
    }
    fn set_send_mode(&mut self, send_mode: SendMode) -> ReaperResult<()> {
        self.set_info_value("I_SENDMODE", send_mode.into())
    }

    /// Can not set Bypass
    /// Use None for default track mode.
    fn set_automation_mode(
        &mut self,
        automation_mode: AutomationMode,
    ) -> ReaperResult<()> {
        self.set_info_value("I_AUTOMODE", automation_mode.int_value().into())
    }
    /// Pass `None` if want to turn audio off.
    fn set_source_channels(
        &mut self,
        channels: Option<SendSourceChannels>,
    ) -> ReaperResult<()> {
        let value = match channels {
            None => -1.0,
            Some(value) => value.into(),
        };
        self.set_info_value("I_SRCCHAN", value)
    }
    /// If source channels are off it will return
    /// [ReaperError::InvalidObject]
    fn set_dest_channels(
        &mut self,
        channels: SendDestChannels,
    ) -> ReaperResult<()> {
        {
            self.source_channels().ok_or(ReaperError::InvalidObject(
                "source channels are None. set them at first.",
            ))?;
        }
        self.set_info_value("I_DSTCHAN", channels.into())
    }

    /// Pass `None` if want to turn MIDI off.
    fn set_midi_properties(
        &mut self,
        properties: impl Into<Option<SendMIDIProps>>,
    ) -> ReaperResult<()> {
        let param = "I_MIDIFLAGS";
        match properties.into() {
            None => {
                self.set_info_value(param, 0b1111111100000000011111 as f64)
            }
            Some(properties) => self.set_info_value(param, properties.into()),
        }
    }
}

pub enum EnvelopeChunk {
    Volume,
    Pan,
    Mute,
    VolumePreFx,
    PanPreFx,
    WidthPreFx,
    With,
    TrimVolume,
}
impl ToString for EnvelopeChunk {
    fn to_string(&self) -> String {
        match self {
            Self::VolumePreFx => "<VOLENV".into(),
            Self::PanPreFx => "<PANENV".into(),
            Self::Mute => "<MUTEENV".into(),
            Self::Volume => "<VOLENV2".into(),
            Self::Pan => "<PANENV2".into(),
            Self::WidthPreFx => "<WIDTHENV".into(),
            Self::With => "<WIDTHENV2".into(),
            Self::TrimVolume => "<VOLENV3".into(),
        }
    }
}

pub enum EnvelopeSelector {
    Chunk(EnvelopeChunk),
    Guid(GUID),
}
impl ToString for EnvelopeSelector {
    fn to_string(&self) -> String {
        let start = String::from("P_ENV:");
        let append = match self {
            Self::Chunk(ch) => ch.to_string(),
            Self::Guid(guid) => guid.to_string(),
        };
        start + &append
    }
}
impl Into<EnvelopeSelector> for EnvelopeChunk {
    fn into(self) -> EnvelopeSelector {
        EnvelopeSelector::Chunk(self)
    }
}
impl Into<EnvelopeSelector> for GUID {
    fn into(self) -> EnvelopeSelector {
        EnvelopeSelector::Guid(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendMode {
    PostFader,
    PreFx,
    PostFx,
}
impl From<f64> for SendMode {
    fn from(value: f64) -> Self {
        let value = value as i32;
        match value {
            0 => Self::PostFader,
            1 => Self::PreFx,
            2 | 3 => Self::PostFx,
            _ => panic!("Can not infer SendMode"),
        }
    }
}
impl Into<f64> for SendMode {
    fn into(self) -> f64 {
        let value = match self {
            Self::PostFader => 0,
            Self::PreFx => 1,
            Self::PostFx => 3,
        };
        value as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SendSourceChannels {
    pub channel: u32,
    pub is_mono: bool,
}
impl SendSourceChannels {
    pub fn new(channel: u32, is_mono: bool) -> Self {
        Self { channel, is_mono }
    }
}
impl From<f64> for SendSourceChannels {
    fn from(value: f64) -> Self {
        let value = value as u32;
        let channel = value & !1024;
        let is_mono = value & 1024 != 0;
        Self { channel, is_mono }
    }
}
impl Into<f64> for SendSourceChannels {
    fn into(self) -> f64 {
        let is_mono = match self.is_mono {
            true => 1024,
            false => 0,
        };
        let value = self.channel | is_mono;
        value as f64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SendDestChannels {
    pub channel: u32,
    pub is_mono: bool,
    pub to_rea_route: bool,
}
impl SendDestChannels {
    pub fn new(channel: u32, is_mono: bool, to_rea_route: bool) -> Self {
        Self {
            channel,
            is_mono,
            to_rea_route,
        }
    }
}
impl From<f64> for SendDestChannels {
    fn from(value: f64) -> Self {
        let value = value as u32;
        let channel = value & !1024 & !512;
        let is_mono = value & 1024 != 0;
        let to_rea_route = value & 512 != 0;
        Self {
            channel,
            is_mono,
            to_rea_route,
        }
    }
}
impl Into<f64> for SendDestChannels {
    fn into(self) -> f64 {
        let is_mono = match self.is_mono {
            true => 1024,
            false => 0,
        };
        let to_rea_route = match self.to_rea_route {
            true => 512,
            false => 0,
        };
        let value = self.channel | is_mono | to_rea_route;
        value as f64
    }
}
impl From<SendSourceChannels> for SendDestChannels {
    fn from(value: SendSourceChannels) -> Self {
        Self {
            channel: value.channel,
            is_mono: value.is_mono,
            to_rea_route: false,
        }
    }
}
impl Into<SendSourceChannels> for SendDestChannels {
    fn into(self) -> SendSourceChannels {
        SendSourceChannels {
            channel: self.channel,
            is_mono: self.is_mono,
        }
    }
}

/// How send manage MIDI flow.
///
/// buses and channels are 1-based. e.g. bus1 == 1, bus16 == 16.
/// if bus or channel set to 0 — this means all busses or all channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SendMIDIProps {
    pub source_bus: u8,
    pub source_channel: u8,
    pub dest_bus: u8,
    pub dest_channel: u8,
}
impl SendMIDIProps {
    pub fn new(
        source_bus: u8,
        source_channel: u8,
        dest_bus: u8,
        dest_channel: u8,
    ) -> Self {
        Self {
            source_bus,
            source_channel,
            dest_bus,
            dest_channel,
        }
    }
}
impl From<f64> for SendMIDIProps {
    fn from(value: f64) -> Self {
        let flags = value as u32;

        let ch_flags = flags % 0b10000000000;
        let bus_flags = flags >> 14;
        // bus
        let src_bus = bus_flags % 0b100000;
        let dst_bus = bus_flags >> 8;
        // channel
        let src_ch = ch_flags % 0b100000;
        let dst_ch = ch_flags >> 5;
        Self::new(src_bus as u8, src_ch as u8, dst_bus as u8, dst_ch as u8)
    }
}
impl Into<f64> for SendMIDIProps {
    fn into(self) -> f64 {
        let (src_bus, src_ch, dst_bus, dst_ch) = (
            self.source_bus,
            self.source_channel,
            self.dest_bus,
            self.dest_channel,
        );
        let dst_ch = (dst_ch as u32) << 5;
        let src_bus = (src_bus as u32) << 14;
        let dst_bus = (dst_bus as u32) << 22;
        let flags = src_bus | (src_ch as u32) | dst_bus | dst_ch;
        flags as f64
    }
}
