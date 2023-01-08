//! Handles all operations with Take MIDI.
//!
//! Since ReaScript API function fot retrieving MIDI data are slow, and
//! limited, all MIDI manipulations are performed on the raw midi buffer. It
//! can be retrieved by [crate::Take::get_midi]. Later all manipulation are
//! made on the [MidiEventBuilder]. It can be made in one call by
//! [crate::Take::iter_midi].
//!
//! MidiEventBuilder iterates through raw midi events as they are presented in
//! the Take. While this is a good low-level representations, it's more common
//! to filter raw events to a specific kinds.
//!
//! # Example
//!
//! ```
//! use rea_rs::{
//!     flatten_events_with_beizer_curve, flatten_midi_notes, sorted_by_ppq,
//!     to_raw_midi_events, AfterTouchMessage, AllSysMessage,
//!     ChannelPressureMessage, CCMessage, MidiEvent,
//!     MidiEventBuilder, MidiEventConsumer, MidiNoteEvent, PitchBendMessage,
//!     ProgramChangeMessage,
//! };
//!
//! // As it got from the Take
//! let buf: Vec<u8> = vec![
//!     56, 4, 0, 0, 0, 3, 0, 0, 0, 176, 1, 42, 120, 0, 0, 0, 0, 8, 0, 0,
//!     0, 255, 1, 109, 121, 116, 101, 120, 116, 1, 1, 0, 0, 0, 3, 0, 0,
//!     0, 176, 1, 45, 120, 0, 0, 0, 1, 3, 0, 0, 0, 160, 61, 88, 1, 0, 0,
//!     0, 0, 3, 0, 0, 0, 176, 1, 59, 1, 0, 0, 0, 0, 3, 0, 0, 0, 144, 61,
//!     96, 120, 0, 0, 0, 0, 3, 0, 0, 0, 176, 1, 68, 120, 0, 0, 0, 0, 3,
//!     0, 0, 0, 176, 1, 76, 120, 0, 0, 0, 0, 3, 0, 0, 0, 176, 1, 78, 1,
//!     0, 0, 0, 0, 3, 0, 0, 0, 128, 61, 0, 120, 0, 0, 0, 80, 3, 0, 0, 0,
//!     176, 1, 74, 0, 0, 0, 0, 0, 12, 0, 0, 0, 255, 15, 67, 67, 66, 90,
//!     32, 0, 205, 204, 12, 191, 10, 0, 0, 0, 48, 2, 0, 0, 0, 208, 64, 1,
//!     0, 0, 0, 0, 3, 0, 0, 0, 144, 57, 96, 0, 0, 0, 0, 0, 32, 0, 0, 0,
//!     255, 15, 78, 79, 84, 69, 32, 48, 32, 53, 55, 32, 116, 101, 120,
//!     116, 32, 34, 116, 101, 120, 116, 32, 110, 111, 116, 97, 116, 105,
//!     111, 110, 34, 120, 0, 0, 0, 80, 2, 0, 0, 0, 208, 104, 0, 0, 0, 0,
//!     0, 12, 0, 0, 0, 255, 15, 67, 67, 66, 90, 32, 0, 133, 235, 81, 63,
//!     1, 0, 0, 0, 0, 3, 0, 0, 0, 144, 64, 96, 104, 1, 0, 0, 0, 3, 0, 0,
//!     0, 176, 1, 29, 1, 0, 0, 0, 48, 2, 0, 0, 0, 208, 64, 120, 0, 0, 0,
//!     0, 3, 0, 0, 0, 176, 1, 28, 3, 0, 0, 0, 0, 3, 0, 0, 0, 128, 57, 0,
//!     120, 0, 0, 0, 0, 3, 0, 0, 0, 180, 0, 121, 0, 0, 0, 0, 0, 3, 0, 0,
//!     0, 180, 32, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 196, 95, 1, 0, 0, 0, 0,
//!     3, 0, 0, 0, 128, 64, 0, 120, 0, 0, 0, 0, 3, 0, 0, 0, 144, 59, 96,
//!     0, 0, 0, 0, 0, 23, 0, 0, 0, 255, 15, 78, 79, 84, 69, 32, 48, 32,
//!     53, 57, 32, 99, 117, 115, 116, 111, 109, 32, 116, 101, 115, 116,
//!     120, 0, 1, 0, 0, 3, 0, 0, 0, 176, 1, 29, 120, 0, 0, 0, 0, 3, 0, 0,
//!     0, 176, 1, 33, 1, 0, 0, 0, 0, 3, 0, 0, 0, 224, 65, 67, 120, 0, 0,
//!     0, 48, 3, 0, 0, 0, 176, 1, 38, 1, 0, 0, 0, 0, 3, 0, 0, 0, 224,
//!     114, 106, 120, 0, 0, 0, 0, 3, 0, 0, 0, 176, 1, 64, 1, 0, 0, 0, 0,
//!     3, 0, 0, 0, 224, 66, 112, 1, 0, 0, 0, 0, 3, 0, 0, 0, 128, 59, 0,
//!     120, 0, 0, 0, 0, 3, 0, 0, 0, 224, 65, 67, 120, 0, 0, 0, 0, 3, 0,
//!     0, 0, 224, 44, 32, 0, 0, 0, 0, 0, 34, 0, 0, 0, 255, 15, 84, 82,
//!     65, 67, 32, 100, 121, 110, 97, 109, 105, 99, 32, 99, 114, 101,
//!     115, 99, 101, 110, 100, 111, 32, 108, 101, 110, 32, 49, 46, 48,
//!     48, 48, 120, 0, 0, 0, 0, 8, 0, 0, 0, 255, 6, 109, 97, 114, 107,
//!     101, 114, 104, 1, 0, 0, 0, 3, 0, 0, 0, 144, 60, 96, 0, 0, 0, 0, 0,
//!     33, 0, 0, 0, 255, 15, 78, 79, 84, 69, 32, 48, 32, 54, 48, 32, 97,
//!     114, 116, 105, 99, 117, 108, 97, 116, 105, 111, 110, 32, 115, 116,
//!     97, 99, 99, 97, 116, 111, 120, 0, 0, 0, 48, 3, 0, 0, 0, 224, 0,
//!     64, 224, 1, 0, 0, 0, 3, 0, 0, 0, 128, 60, 0, 40, 5, 0, 0, 0, 3, 0,
//!     0, 0, 176, 123, 0,
//! ];
//! let events = MidiEventBuilder::new(buf.clone().into_iter());
//!
//! // Note-on and note-off can be used separately, if needed, but
//! // it's more common to use special iterator. (See below)
//! println!("NOTE ON EVENTS");
//! for event in events.clone().filter_note_on() {
//!     println!("{}", event);
//! }
//! println!("\n----NOTE OFF EVENTS----");
//! for event in events.clone().filter_note_off() {
//!     println!("{}", event);
//! }
//!
//! println!("\n----CC EVENTS----");
//! let cc_events: Vec<MidiEvent<CCMessage>> =
//!     events.clone().filter_cc().collect();
//! for event in cc_events.iter() {
//!     println!("{}", event);
//! }
//! println!("\n----ProgramChange EVENTS----");
//! let pr_ch_events: Vec<MidiEvent<ProgramChangeMessage>> =
//!     events.clone().filter_program_change().collect();
//! for event in pr_ch_events.iter() {
//!     println!("{}", event);
//! }
//! println!("\n----AfterTouch EVENTS----");
//! let at_events: Vec<MidiEvent<AfterTouchMessage>> =
//!     events.clone().filter_after_touch().collect();
//! for event in at_events.iter() {
//!     println!("{}", event);
//! }
//! println!("\n----PITCH EVENTS----");
//! let pitch_events: Vec<MidiEvent<PitchBendMessage>> =
//!     events.clone().filter_pitch_bend().collect();
//! for event in pitch_events.iter() {
//!     println!("{}", event);
//! }
//! println!("\n----ChannelPressure EVENTS----");
//! let ch_pr_events: Vec<MidiEvent<ChannelPressureMessage>> =
//!     events.clone().filter_channel_pressure().collect();
//! for event in ch_pr_events.iter() {
//!     println!("{}", event);
//! }
//! println!("\n----Sys EVENTS----");
//! let all_sys_events: Vec<MidiEvent<AllSysMessage>> =
//!     events.clone().filter_all_sys().collect();
//! for event in all_sys_events.iter() {
//!     println!("{}", event);
//! }
//! println!("\n\n----NOTE EVENTS----");
//! println!("======================");
//! let notes: Vec<MidiNoteEvent> =
//!     events.clone().filter_notes().collect();
//! for event in notes.iter() {
//!     println!("{:#?}", event);
//! }
//! println!("\n\n----Back to RAW EVENTS----");
//! println!("======================");
//!
//! // Now get everything back to the raw buffer.
//! // Notes are unfolded, as they represent 2 events by 1 object.
//! let raw_events = flatten_midi_notes(notes.into_iter())
//!     // Channel pressure can have beizer data inside.
//!     // So, we need to unfold them to raw events.
//!     .chain(to_raw_midi_events(flatten_events_with_beizer_curve(
//!         ch_pr_events.into_iter(),
//!     )))
//!     // The same with CC events.
//!     .chain(to_raw_midi_events(flatten_events_with_beizer_curve(
//!         cc_events.into_iter(),
//!     )))
//!     // after-touch
//!     .chain(to_raw_midi_events(at_events.into_iter()))
//!     // program change
//!     .chain(to_raw_midi_events(pr_ch_events.into_iter()))
//!     // pitch bend
//!     .chain(to_raw_midi_events(pitch_events.into_iter()))
//!     // Sys events (SysEx in this example)
//!     .chain(to_raw_midi_events(all_sys_events.into_iter()));
//!
//! // Resulted vector can be passed back to take.
//! let raw_buf: Vec<u8> =
//!     MidiEventConsumer::new(sorted_by_ppq(raw_events)).collect();
//! assert_eq!(buf.len(), raw_buf.len(), "No equal length!");
//!
//! // Note, that the original input had been tweaked a bit to avoid several
//! // different events at one position. But in the real world events could be
//! // shuffled.
//! for (idx, (left, right)) in buf.into_iter().zip(raw_buf).enumerate() {
//!     assert_eq!(left, right, "assert failed at index: {}", idx);
//! }

use serde_derive::{Deserialize, Serialize};
use std::{fmt::Display, vec::IntoIter};

/// Basic MIDI Message functionality.
pub trait MidiMessage: Display + Clone {
    /// Check if raw message is of `Self` type.
    fn from_raw(buf: Vec<u8>) -> Option<Self>
    where
        Self: Sized;
    /// Copy and return raw `Vec`
    fn get_raw(&self) -> Vec<u8>;
    /// Borrow raw `Vec`
    fn borrow_raw(&self) -> &Vec<u8>;
    /// Borrow raw `Vec` mutably
    fn borrow_raw_mut(&mut self) -> &mut Vec<u8>;
    /// Return raw representation of self.
    fn as_raw_message(&self) -> RawMidiMessage {
        RawMidiMessage::from_raw(self.get_raw()).unwrap()
    }
}

#[derive(
    Clone, PartialEq, PartialOrd, Debug, Default, Eq, Serialize, Deserialize,
)]
pub struct RawMidiMessage {
    buf: Vec<u8>,
}
impl MidiMessage for RawMidiMessage {
    /// Always Some.
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        Some(Self { buf })
    }
    fn get_raw(&self) -> Vec<u8> {
        self.buf.clone()
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.buf
    }
    fn borrow_raw_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }
}
impl RawMidiMessage {
    /// Construct a new `RawMidiMessage` from any other message type.
    ///
    /// Equivalent of from(), but generic
    pub fn from_msg<T: MidiMessage>(value: T) -> Self {
        Self::from_raw(value.get_raw()).expect("ICan not convert message.")
    }
}
impl Display for RawMidiMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RawMidiMessage<{:?}>", self.borrow_raw())
    }
}

trait ShortMessage: MidiMessage {
    fn message() -> u8;

    fn channel_private(&self) -> u8 {
        self.borrow_raw()[0] - Self::message() + 1
    }
    fn msg2(&self) -> u8 {
        self.borrow_raw()[1]
    }
    fn msg3(&self) -> u8 {
        self.borrow_raw()[2]
    }
    fn set_channel_private(&mut self, value: u8) {
        self.borrow_raw_mut()[0] = value - 1 + Self::message()
    }
    fn set_msg2(&mut self, value: u8) {
        self.borrow_raw_mut()[1] = value
    }
    fn set_msg3(&mut self, value: u8) {
        self.borrow_raw_mut()[2] = value
    }
    /// Check whether message is shorter than 4 bytes.
    fn is_short(buf: &Vec<u8>) -> Option<()> {
        match buf.len() <= 3 {
            true => Some(()),
            false => None,
        }
    }
    /// Checks if the first byte contains channel message.
    ///
    /// Ignores channel.
    fn starts_with_message(buf: &Vec<u8>) -> Option<()> {
        let msg = Self::message();
        match (msg..msg + 15).contains(&buf[0]) {
            true => Some(()),
            false => None,
        }
    }
}

/// Unifies interface for events, supports CcShapeCurve.
///
/// Currently, supports [CCMessage] and [ChannelPressureMessage].
pub trait HasBeizer: MidiMessage {
    /// basic message buffer,
    fn msg_buf(&self) -> &Vec<u8>;
    /// Raw Sys message for Beizer tension.
    fn beizer_buf(&self) -> &Vec<u8>;
    /// Set raw buffer message.
    fn set_beizer_buf(&mut self, buf: Vec<u8>);
    /// convert raw beizer data to f64
    fn beizer_tension(&self) -> Option<f64> {
        if self.beizer_buf().len() == 0 {
            return None;
        }
        let s = &self.beizer_buf().clone()[8..];
        let buf = [s[0], s[1], s[2], s[3]];
        Some(f32::from_le_bytes(buf) as f64)
    }
    /// Set beizer tension by float value,
    fn set_beizer_tension(&mut self, value: f32) {
        let data = (value as f32).to_le_bytes();
        let mut tension: Vec<u8> = vec![255, 15, 67, 67, 66, 90, 32, 0];
        tension.append(&mut data.to_vec());
        self.set_beizer_buf(tension)
    }
}

/// ControlCange Message
#[derive(
    Clone, PartialEq, PartialOrd, Debug, Default, Serialize, Deserialize,
)]
pub struct CCMessage {
    msg_buf: Vec<u8>,
    beizer_buf: Vec<u8>,
}
impl HasBeizer for CCMessage {
    fn msg_buf(&self) -> &Vec<u8> {
        &self.msg_buf
    }

    fn beizer_buf(&self) -> &Vec<u8> {
        &self.beizer_buf
    }
    fn set_beizer_buf(&mut self, buf: Vec<u8>) {
        self.beizer_buf = buf
    }
}
impl MidiMessage for CCMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        if !(0xb0..0xc0).contains(&buf[0]) {
            return None;
        }
        let beizer_buf = match buf.len() {
            3 => vec![],
            _ => Vec::from(&buf[3..]),
        };
        let cc_buf = Vec::from(&buf[..3]);
        Some(Self {
            msg_buf: cc_buf,
            beizer_buf,
        })
    }
    fn get_raw(&self) -> Vec<u8> {
        let mut buf = self.msg_buf.clone();
        let mut beizer = self.beizer_buf().clone();
        buf.append(&mut beizer);
        buf
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.msg_buf
    }
    fn borrow_raw_mut(&mut self) -> &mut Vec<u8> {
        &mut self.msg_buf
    }
}
impl ShortMessage for CCMessage {
    fn message() -> u8 {
        0xb0
    }
}
impl CCMessage {
    pub fn channel(&self) -> u8 {
        self.channel_private()
    }
    pub fn set_channel(&mut self, channel: u8) {
        self.set_channel_private(channel)
    }
    pub fn cc_num(&self) -> u8 {
        self.msg2()
    }
    pub fn cc_val(&self) -> u8 {
        self.msg3()
    }
    pub fn set_cc_num(&mut self, value: u8) {
        self.set_msg2(value)
    }
    pub fn set_cc_val(&mut self, value: u8) {
        self.set_msg3(value)
    }
}
impl Display for CCMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"ControlChangeMessage{{
        channel: {},
        cc_num: {},
        value: {},
        beizer_tension: {:?}
    }}"#,
            self.channel_private(),
            self.cc_num(),
            self.cc_val(),
            self.beizer_tension(),
        )
    }
}

#[derive(
    Clone, PartialEq, PartialOrd, Debug, Default, Serialize, Deserialize,
)]
pub struct NoteOnMessage {
    buf: Vec<u8>,
}
impl NoteOnMessage {
    pub fn new(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            buf: vec![0x90 + channel - 1, note, velocity],
        }
    }
    pub fn channel(&self) -> u8 {
        self.channel_private()
    }
    pub fn set_channel(&mut self, channel: u8) {
        self.set_channel_private(channel)
    }
    pub fn note(&self) -> u8 {
        self.msg2()
    }
    pub fn velocity(&self) -> u8 {
        self.msg3()
    }
    pub fn set_note(&mut self, value: u8) {
        self.set_msg2(value)
    }
    pub fn set_velocity(&mut self, value: u8) {
        self.set_msg3(value)
    }
}
impl ShortMessage for NoteOnMessage {
    fn message() -> u8 {
        0x90
    }
}
impl MidiMessage for NoteOnMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        Self::is_short(&buf)?;
        Self::starts_with_message(&buf)?;
        Some(Self { buf })
    }
    fn get_raw(&self) -> Vec<u8> {
        self.buf.clone()
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.buf
    }
    fn borrow_raw_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }
}
impl Display for NoteOnMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"NoteOnMessage{{
        channel: {},
        note: {},
        velocity: {},
    }}"#,
            self.channel_private(),
            self.note(),
            self.velocity(),
        )
    }
}

#[derive(
    Clone, PartialEq, PartialOrd, Debug, Default, Serialize, Deserialize,
)]
pub struct NoteOffMessage {
    buf: Vec<u8>,
}
impl NoteOffMessage {
    pub fn new(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            buf: vec![0x80 + channel - 1, note, velocity],
        }
    }
    pub fn channel(&self) -> u8 {
        self.channel_private()
    }
    pub fn set_channel(&mut self, channel: u8) {
        self.set_channel_private(channel)
    }
    pub fn note(&self) -> u8 {
        self.msg2()
    }
    pub fn velocity(&self) -> u8 {
        self.msg3()
    }
    pub fn set_note(&mut self, value: u8) {
        self.set_msg2(value)
    }
    pub fn set_velocity(&mut self, value: u8) {
        self.set_msg3(value)
    }
}
impl ShortMessage for NoteOffMessage {
    fn message() -> u8 {
        0x80
    }
}
impl MidiMessage for NoteOffMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        Self::is_short(&buf)?;
        Self::starts_with_message(&buf)?;
        Some(Self { buf })
    }
    fn get_raw(&self) -> Vec<u8> {
        self.buf.clone()
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.buf
    }
    fn borrow_raw_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }
}
impl Display for NoteOffMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"NoteOffMessage{{
        channel: {},
        note: {},
        velocity: {},
    }}"#,
            self.channel_private(),
            self.note(),
            self.velocity(),
        )
    }
}

#[derive(
    Clone, PartialEq, PartialOrd, Debug, Default, Serialize, Deserialize,
)]
pub struct PitchBendMessage {
    buf: Vec<u8>,
}
impl PitchBendMessage {
    pub fn channel(&self) -> u8 {
        self.channel_private()
    }
    pub fn set_channel(&mut self, channel: u8) {
        self.set_channel_private(channel)
    }
    /// combined MSB\LSB values
    pub fn raw_value(&self) -> u16 {
        (self.msg3() as u16) << 7 | self.msg2() as u16
    }
    /// Combined MSB\LSB as f32
    pub fn normalized_value(&self) -> f64 {
        (self.raw_value() as i32 - 8192) as f64 / 8192.0
    }
    /// Set u16 as combined MSB\LSB. 8192 is the middle.
    pub fn set_raw_value(&mut self, value: u16) {
        self.set_msg3((value >> 7) as u8);
        self.set_msg2(value as u8);
    }
    /// Set value as raw.
    pub fn set_normalized_value(&mut self, value: f64) {
        assert!((-1.0..1.0).contains(&value));
        self.set_raw_value((value * 8192.0 + 8192.0) as u16)
    }
}

#[test]
fn test_pb_values() {
    let mut pb = PitchBendMessage::from_raw(vec![224, 65, 67]).unwrap();
    assert_eq!(pb.msg2(), 65);
    assert_eq!(pb.msg3(), 67);
    assert_eq!(pb.raw_value(), 8641);
    assert_eq!(pb.normalized_value(), 0.0548095703125);
    pb.set_raw_value(8192);
    assert_eq!(pb.raw_value(), 8192);
    assert_eq!(pb.normalized_value(), 0.0);
    pb.set_raw_value(16384);
    assert_eq!(pb.raw_value(), 16384);
    assert_eq!(pb.normalized_value(), 1.0);
    pb.set_normalized_value(-0.5);
    assert_eq!(pb.raw_value(), 4096);
}

impl ShortMessage for PitchBendMessage {
    fn message() -> u8 {
        0xe0
    }
}
impl MidiMessage for PitchBendMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        Self::is_short(&buf)?;
        Self::starts_with_message(&buf)?;
        Some(Self { buf })
    }
    fn get_raw(&self) -> Vec<u8> {
        self.buf.clone()
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.buf
    }
    fn borrow_raw_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }
}
impl Display for PitchBendMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"PitchBendMessage{{
        channel: {},
        raw: {},
        normalized: {},
    }}"#,
            self.channel_private(),
            self.raw_value(),
            self.normalized_value(),
        )
    }
}

#[derive(
    Clone, PartialEq, PartialOrd, Debug, Default, Serialize, Deserialize,
)]
pub struct AfterTouchMessage {
    buf: Vec<u8>,
}
impl AfterTouchMessage {
    pub fn channel(&self) -> u8 {
        self.channel_private()
    }
    pub fn set_channel(&mut self, channel: u8) {
        self.set_channel_private(channel)
    }
    pub fn note(&self) -> u8 {
        self.msg2()
    }
    pub fn pressure(&self) -> u8 {
        self.msg3()
    }
    pub fn set_note(&mut self, value: u8) {
        self.set_msg2(value)
    }
    pub fn set_pressure(&mut self, value: u8) {
        self.set_msg3(value)
    }
}
impl ShortMessage for AfterTouchMessage {
    fn message() -> u8 {
        0xa0
    }
}
impl MidiMessage for AfterTouchMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        Self::is_short(&buf)?;
        Self::starts_with_message(&buf)?;
        Some(Self { buf })
    }
    fn get_raw(&self) -> Vec<u8> {
        self.buf.clone()
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.buf
    }
    fn borrow_raw_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }
}
impl Display for AfterTouchMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"AfterTouchMessage{{
        channel: {},
        note: {},
        pressure: {},
    }}"#,
            self.channel_private(),
            self.note(),
            self.pressure(),
        )
    }
}

#[derive(
    Clone, PartialEq, PartialOrd, Debug, Default, Serialize, Deserialize,
)]
pub struct ProgramChangeMessage {
    buf: Vec<u8>,
}
impl ProgramChangeMessage {
    pub fn channel(&self) -> u8 {
        self.channel_private()
    }
    pub fn set_channel(&mut self, channel: u8) {
        self.set_channel_private(channel)
    }
    pub fn program(&self) -> u8 {
        self.msg2()
    }
    pub fn set_program(&mut self, value: u8) {
        self.set_msg2(value)
    }
}
impl ShortMessage for ProgramChangeMessage {
    fn message() -> u8 {
        0xc0
    }
}
impl MidiMessage for ProgramChangeMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        Self::is_short(&buf)?;
        Self::starts_with_message(&buf)?;
        Some(Self { buf })
    }
    fn get_raw(&self) -> Vec<u8> {
        self.buf.clone()
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.buf
    }
    fn borrow_raw_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }
}
impl Display for ProgramChangeMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"ProgramChangeMessage{{
        channel: {},
        program: {},
    }}"#,
            self.channel_private(),
            self.program(),
        )
    }
}

/// Represents all types of Sys messages, starting from `0xf0`
///
/// Later it can be differentiated to Notation, Lyrics, Text etc.
#[derive(
    Clone, PartialEq, PartialOrd, Debug, Default, Serialize, Deserialize,
)]
pub struct AllSysMessage {
    buf: Vec<u8>,
}
impl MidiMessage for AllSysMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        if buf[0] < 0xf0 {
            return None;
        }
        Some(Self { buf })
    }
    fn get_raw(&self) -> Vec<u8> {
        self.buf.clone()
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.buf
    }
    fn borrow_raw_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }
}
impl Display for AllSysMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AllSysMessage{{raw: {:?}}}", self.borrow_raw(),)
    }
}

/// Represents Text messages `0xf0, 0x01`
#[derive(
    Clone, PartialEq, PartialOrd, Debug, Default, Serialize, Deserialize,
)]
pub struct TextMessage {
    buf: Vec<u8>,
}
impl TextMessage {
    pub fn text(&self) -> String {
        String::from_utf8(self.get_raw()[2..].to_vec())
            .expect("Cannot decode text message to utf-8")
    }
    pub fn set_text(&mut self, text: impl Into<String>) {
        let mut text: String = text.into();
        let mut buf = vec![0xf0, 0x01];
        buf.append(unsafe { text.as_mut_vec() });
        self.buf = buf;
    }
}
impl MidiMessage for TextMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        if buf[0] < 0xf0 {
            return None;
        }
        if buf[1] == 0x01 {
            Some(Self { buf })
        } else {
            None
        }
    }
    fn get_raw(&self) -> Vec<u8> {
        self.buf.clone()
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.buf
    }
    fn borrow_raw_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }
}
impl Display for TextMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TextMessage{{text: {:?}}}", self.text(),)
    }
}

/// Represents Notation messages `0xf0, 0x0f`
#[derive(Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub struct NotationMessage {
    buf: Vec<u8>,
    // notation: Notation,
}

/// Partially parsed notation.
#[derive(Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub enum Notation {
    /// Note notation: channel(1-based), note, tokens
    Note {
        channel: u8,
        note: u8,
        tokens: Vec<String>,
    },
    /// Track notation: tokens
    Track(Vec<String>),
    /// Unknown notation: tokens, including the type.
    Unknown(Vec<String>),
}
impl Notation {
    fn as_tokens_string(self) -> String {
        match self {
            Notation::Note {
                channel,
                note,
                mut tokens,
            } => {
                let mut v = vec![
                    String::from("NOTE"),
                    format!("{}", channel - 1),
                    format!("{}", note),
                ];
                v.append(&mut tokens);
                v.join(" ")
            }
            Notation::Track(mut tk) => {
                let mut v = vec![String::from("TRAC")];
                v.append(&mut tk);
                v.join(" ")
            }
            Notation::Unknown(tk) => tk.join(" "),
        }
    }
}
impl From<Notation> for NotationMessage {
    fn from(value: Notation) -> Self {
        Self {
            buf: NotationMessage::text_to_buf(value.as_tokens_string()),
        }
    }
}
impl NotationMessage {
    pub fn notation(&self) -> Notation {
        let text = self.text();
        let tokens: Vec<&str> = text.split(" ").collect();
        match tokens[0] {
            "NOTE" => Notation::Note {
                channel: tokens[1]
                    .parse::<u8>()
                    .expect("Should be channel number")
                    + 1,
                note: tokens[2].parse::<u8>().expect("Should be note number"),
                tokens: tokens[3..]
                    .into_iter()
                    .map(|s| String::from(*s))
                    .collect(),
            },
            "TRAC" => Notation::Track(
                tokens[1..].into_iter().map(|s| String::from(*s)).collect(),
            ),
            _ => Notation::Unknown(
                tokens.into_iter().map(|s| String::from(s)).collect(),
            ),
        }
    }
    pub fn set_notation(&mut self, notation: Notation) {
        let tokens = notation.as_tokens_string();
        self.set_text(tokens)
    }
    fn text_to_buf(text: impl Into<String>) -> Vec<u8> {
        let mut text: String = text.into();
        let mut buf = vec![0xff, 0x0f];
        buf.append(unsafe { text.as_mut_vec() });
        buf
    }
    fn text(&self) -> String {
        String::from_utf8(self.get_raw()[2..].to_vec())
            .expect("Cannot decode text message to utf-8")
    }
    fn set_text(&mut self, text: impl Into<String>) {
        self.buf = Self::text_to_buf(text);
    }
}
impl MidiMessage for NotationMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        if buf[0] < 0xff {
            return None;
        }
        if buf[1] == 0x0f {
            Some(Self { buf })
        } else {
            None
        }
    }
    fn get_raw(&self) -> Vec<u8> {
        self.buf.clone()
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.buf
    }
    fn borrow_raw_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }
}
impl Display for NotationMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NotationMessage{{notation: {}}}", self.notation(),)
    }
}
impl Display for Notation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Note {
                channel,
                note,
                tokens,
            } => write!(
                f,
                "Notation: Note(channel:{}, note: {}, tokens: {:?})",
                channel, note, tokens
            ),
            Self::Track(tk) => {
                write!(f, "Notation: Track(tokens: {:?})", tk)
            }
            Self::Unknown(tk) => {
                write!(f, "Unknown: Track(tokens: {:?})", tk)
            }
        }
    }
}

#[derive(
    Clone, PartialEq, PartialOrd, Debug, Default, Serialize, Deserialize,
)]
pub struct ChannelPressureMessage {
    msg: Vec<u8>,
    beizer_buf: Vec<u8>,
}
impl HasBeizer for ChannelPressureMessage {
    fn msg_buf(&self) -> &Vec<u8> {
        &self.msg
    }

    fn beizer_buf(&self) -> &Vec<u8> {
        &self.beizer_buf
    }
    fn set_beizer_buf(&mut self, buf: Vec<u8>) {
        self.beizer_buf = buf
    }
}
impl MidiMessage for ChannelPressureMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        Self::starts_with_message(&buf)?;
        let beizer_buf = match buf.len() {
            2 => vec![],
            _ => Vec::from(&buf[2..]),
        };
        let cc_buf = Vec::from(&buf[..2]);
        Some(Self {
            msg: cc_buf,
            beizer_buf,
        })
    }
    fn get_raw(&self) -> Vec<u8> {
        let mut buf = self.msg.clone();
        let mut beizer = self.beizer_buf.clone();
        buf.append(&mut beizer);
        buf
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.msg
    }
    fn borrow_raw_mut(&mut self) -> &mut Vec<u8> {
        &mut self.msg
    }
}
impl ShortMessage for ChannelPressureMessage {
    fn message() -> u8 {
        0xd0
    }
}
impl ChannelPressureMessage {
    pub fn channel(&self) -> u8 {
        self.channel_private()
    }
    pub fn set_channel(&mut self, channel: u8) {
        self.set_channel_private(channel)
    }
    pub fn pressure(&self) -> u8 {
        self.msg2()
    }
    pub fn set_pressure(&mut self, value: u8) {
        self.set_msg2(value)
    }
}
impl Display for ChannelPressureMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"ChannelPressureMessage{{
        channel: {},
        pressure: {},
        beizer_tension: {:?},
    }}"#,
            self.channel_private(),
            self.pressure(),
            self.beizer_tension(),
        )
    }
}

/// Generic Midi event, that easily converted to the binary format.
#[derive(
    Clone, PartialEq, PartialOrd, Debug, Default, Serialize, Deserialize,
)]
pub struct MidiEvent<T: MidiMessage> {
    position_in_ppq: u32,
    is_selected: bool,
    is_muted: bool,
    cc_shape_kind: CcShapeKind,
    /// Message can be as ordinary 3-bytes midi-message,
    /// as well as SysEx and custom messages, including lyrics and text.
    message: T,
}
impl<T: MidiMessage> MidiEvent<T> {
    /// Position in ppq depends on Take. You can convert by the
    /// [crate::Position::as_ppq].
    pub fn new(
        position_in_ppq: u32,
        is_selected: bool,
        is_muted: bool,
        cc_shape_kind: CcShapeKind,
        message: T,
    ) -> Self {
        Self {
            position_in_ppq,
            is_selected,
            is_muted,
            cc_shape_kind,
            message,
        }
    }
    /// Morph Event to be of other type, replacing original message. Other
    /// contents are no moved and copied.
    pub fn with_new_message<S: MidiMessage>(
        event: MidiEvent<S>,
        message: T,
    ) -> Self {
        Self {
            position_in_ppq: event.ppq_position(),
            is_selected: event.selected(),
            is_muted: event.muted(),
            cc_shape_kind: event.cc_shape_kind(),
            message,
        }
    }
    pub fn ppq_position(&self) -> u32 {
        self.position_in_ppq
    }
    pub fn set_ppq_position(&mut self, position: u32) {
        self.position_in_ppq = position;
    }
    pub fn selected(&self) -> bool {
        self.is_selected
    }
    pub fn set_selected(&mut self, selected: bool) {
        self.is_selected = selected;
    }
    pub fn muted(&self) -> bool {
        self.is_muted
    }
    pub fn set_muted(&mut self, muted: bool) {
        self.is_muted = muted;
    }
    pub fn cc_shape_kind(&self) -> CcShapeKind {
        self.cc_shape_kind
    }
    pub fn set_cc_shape_kind(&mut self, cc_shape_kind: CcShapeKind) {
        self.cc_shape_kind = cc_shape_kind;
    }
    pub fn message(&self) -> &T {
        &self.message
    }
    pub fn message_mut(&mut self) -> &mut T {
        &mut self.message
    }
    pub fn set_message(&mut self, message: T) {
        self.message = message;
    }
}

impl<T: MidiMessage> Display for MidiEvent<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"MidiEvent{{
    ppq_position: {},
    selected: {},
    muted: {},
    cc_shape_kind: {:?},
    message: {},
}}"#,
            self.ppq_position(),
            self.selected(),
            self.muted(),
            self.cc_shape_kind(),
            self.message()
        )
    }
}

/// Special Event type, holds Midi Notes, representing 2 raw Events
#[derive(
    Clone, PartialEq, PartialOrd, Debug, Default, Serialize, Deserialize,
)]
pub struct MidiNoteEvent {
    pub start_in_ppq: u32,
    pub end_in_ppq: u32,
    pub is_selected: bool,
    pub is_muted: bool,
    /// 1-based
    pub channel: u8,
    pub note: u8,
    pub on_velocity: u8,
    pub off_velocity: u8,
}
impl MidiNoteEvent {
    pub fn new(
        start_in_ppq: u32,
        end_in_ppq: u32,
        is_selected: bool,
        is_muted: bool,
        channel: u8,
        note: u8,
        on_velocity: u8,
        off_velocity: u8,
    ) -> Self {
        Self {
            start_in_ppq,
            end_in_ppq,
            is_selected,
            is_muted,
            channel,
            note,
            on_velocity,
            off_velocity,
        }
    }
    pub fn from_raw_parts(
        note_on: MidiEvent<NoteOnMessage>,
        note_off: MidiEvent<NoteOffMessage>,
    ) -> Self {
        assert_eq!(
            note_on.message().channel_private(),
            note_off.message().channel_private()
        );
        assert_eq!(note_on.message().note(), note_off.message().note());
        Self::new(
            note_on.ppq_position(),
            note_off.ppq_position(),
            note_on.selected(),
            note_on.muted(),
            note_on.message().channel_private(),
            note_on.message().note(),
            note_on.message().velocity(),
            note_off.message().velocity(),
        )
    }
    pub fn to_raw_parts(
        &self,
    ) -> (MidiEvent<NoteOnMessage>, MidiEvent<NoteOffMessage>) {
        let on = MidiEvent {
            position_in_ppq: self.start_in_ppq,
            is_selected: self.is_selected,
            is_muted: self.is_muted,
            cc_shape_kind: CcShapeKind::Square,
            message: NoteOnMessage::new(
                self.channel,
                self.note,
                self.on_velocity,
            ),
        };
        let off = MidiEvent {
            position_in_ppq: self.end_in_ppq,
            is_selected: self.is_selected,
            is_muted: self.is_muted,
            cc_shape_kind: CcShapeKind::Square,
            message: NoteOffMessage::new(
                self.channel,
                self.note,
                self.off_velocity,
            ),
        };
        (on, off)
    }
}

impl Into<MidiEventBuilder> for IntoIter<u8> {
    fn into(self) -> MidiEventBuilder {
        MidiEventBuilder {
            buf: self,
            current_ppq: 0,
        }
    }
}

/// Iterates over raw take midi data and builds [MidiEvent] objects.
///
/// See example in the [module doc](crate::midi)
#[derive(Debug, Clone)]
pub struct MidiEventBuilder {
    buf: IntoIter<u8>,
    current_ppq: u32,
}
impl MidiEventBuilder {
    /// Accepts only raw midi data, as described (and got from) in the
    /// [Take::get_midi] doc.
    pub fn new(buf: IntoIter<u8>) -> Self {
        Self {
            buf: buf,
            current_ppq: 0,
        }
    }
    /// Iter only through `MidiEvent<CCMessage>`
    ///
    /// # Note
    ///
    /// With reverse iteration, additional step of
    /// [crate::midi::flatten_events_with_beizer_curve] is required:
    /// ```
    /// use rea_rs::midi::*;
    /// let events = vec![MidiEvent::new(
    ///     0,
    ///     true,
    ///     false,
    ///     CcShapeKind::Square,
    ///     CCMessage::from_raw(vec![176, 74, 2]).unwrap(),
    /// )]; // etc...
    /// let raw = MidiEventConsumer::new(
    ///     to_raw_midi_events(
    ///         flatten_events_with_beizer_curve(events.into_iter())
    ///     )
    /// );
    /// ```
    pub fn filter_cc(self) -> FilterCC {
        FilterCC { midi_events: self }
    }
    /// Iter only through `MidiEvent<NoteOnMessage>`
    pub fn filter_note_on(self) -> FilterNoteOn {
        FilterNoteOn { midi_events: self }
    }
    /// Iter only through `MidiEvent<NoteOffMessage>`
    pub fn filter_note_off(self) -> FilterNoteOff {
        FilterNoteOff { midi_events: self }
    }
    /// Iter only through `MidiEvent<PitchBendMessage>`
    pub fn filter_pitch_bend(self) -> FilterPitchBend {
        FilterPitchBend { midi_events: self }
    }
    /// Iter only through `MidiEvent<AfterTouchMessage>`
    pub fn filter_after_touch(self) -> FilterAfterTouch {
        FilterAfterTouch { midi_events: self }
    }
    /// Iter only through `MidiEvent<ChannelPressureMessage>`
    ///
    /// # Note
    ///
    /// With reverse iteration, additional step of
    /// [crate::midi::flatten_events_with_beizer_curve] is required:
    /// ```
    /// use rea_rs::midi::*;
    /// let events = vec![MidiEvent::new(
    ///     0,
    ///     true,
    ///     false,
    ///     CcShapeKind::Square,
    ///     ChannelPressureMessage::from_raw(vec![0xd2, 74]).unwrap(),
    /// )]; // etc...
    /// let raw = MidiEventConsumer::new(
    ///     to_raw_midi_events(
    ///         flatten_events_with_beizer_curve(events.into_iter())
    ///     )
    /// );
    /// ```
    pub fn filter_channel_pressure(self) -> FilterChannelPressure {
        FilterChannelPressure { midi_events: self }
    }
    /// Iter only through `MidiEvent<ProgramChangeMessage>`
    pub fn filter_program_change(self) -> FilterProgramChange {
        FilterProgramChange { midi_events: self }
    }
    /// Iter only through `MidiEvent<AllSysMessage>`
    pub fn filter_all_sys(self) -> FilterAllSys {
        FilterAllSys { midi_events: self }
    }
    /// Iter only through [MidiNoteEvent]
    ///
    /// # Note
    ///
    /// With reverse iteration, additional step of
    /// [crate::midi::flatten_midi_notes] is required:
    /// ```
    /// use rea_rs::midi::*;
    /// let events = vec![MidiNoteEvent::new(0, 20, true, false, 2, 60, 65, 97)]; // etc...
    /// let raw = MidiEventConsumer::new(
    ///     to_raw_midi_events(
    ///         flatten_midi_notes(events.into_iter())
    ///     )
    /// );
    /// ```
    pub fn filter_notes(self) -> FilterNotes<Self> {
        FilterNotes::from(self)
    }

    fn next_4(&mut self) -> Option<[u8; 4]> {
        match (
            self.buf.next(),
            self.buf.next(),
            self.buf.next(),
            self.buf.next(),
        ) {
            (Some(a), Some(b), Some(c), Some(d)) => Some([a, b, c, d]),
            _ => None,
        }
    }
}
impl Iterator for MidiEventBuilder {
    type Item = MidiEvent<RawMidiMessage>;

    fn next(&mut self) -> Option<Self::Item> {
        let result = match self.next_4() {
            Some(value) => value,
            None => return None,
        };
        let offset = u32::from_le_bytes(result);
        let flag = self
            .buf
            .next()
            .expect("unexpectetly ended. Should be flag.");
        // let flags = MidiEventFlags::from_bits_truncate(flag);
        let length =
            u32::from_le_bytes(self.next_4().expect("should take length"));
        if length == 0 {
            return None;
        }
        self.current_ppq += offset;
        let buf = self.buf.by_ref().take(length as usize);
        let result = Some(MidiEvent {
            position_in_ppq: self.current_ppq,
            cc_shape_kind: CcShapeKind::from_raw(flag & 0b11110000)
                .expect("Can not infer CcShapeKind, received from take."),
            is_selected: (flag & 1) != 0,
            is_muted: (flag & 2) != 0,
            message: RawMidiMessage {
                buf: Vec::from_iter(buf),
            },
        });
        result
    }
}

/// Convert different kinds of messages into raw events.
///
/// See [flatten_events_with_beizer_curve], [flatten_midi_notes],
/// [sorted_by_ppq]
pub fn to_raw_midi_events<T: MidiMessage>(
    iter: impl Iterator<Item = MidiEvent<T>>,
) -> impl Iterator<Item = MidiEvent<RawMidiMessage>> {
    iter.map(|i| {
        let msg = i.message().as_raw_message();
        MidiEvent::with_new_message(i, msg)
    })
}

/// Make sure, that events will be iterated back in right order.
///
/// This is needed if buffer was split by event types and later consumed back.
pub fn sorted_by_ppq(
    events: impl Iterator<Item = MidiEvent<RawMidiMessage>>,
) -> IntoIter<MidiEvent<RawMidiMessage>> {
    let mut events: Vec<_> = events.collect();
    events.sort_by(|a, b| {
        let ppq = a.ppq_position().partial_cmp(&b.ppq_position()).unwrap();
        match ppq {
            std::cmp::Ordering::Equal => std::cmp::Ordering::Greater,
            x => x,
        }
    });
    events.into_iter()
}

/// Iterates through [MidiEvent] objects and builds raw midi data
/// to be passed to take.
#[derive(Debug)]
pub struct MidiEventConsumer<T: Iterator<Item = MidiEvent<RawMidiMessage>>> {
    events: T,
    last_ppq: u32,
    buf: Option<IntoIter<u8>>,
}
impl<T: Iterator<Item = MidiEvent<RawMidiMessage>>> MidiEventConsumer<T> {
    /// Build iterator.
    ///
    /// If events are not sorted py ppq position, then [new_sorted] has to be
    /// used.
    pub fn new(events: T) -> Self {
        Self {
            events: events,
            last_ppq: 0,
            buf: None,
        }
    }

    /// Checks if some events are left and builds new buf for iteration.
    fn next_buf(&mut self) -> Option<u8> {
        match self.events.next() {
            None => None,
            Some(mut event) => {
                let size = event.message().get_raw().len() + 9;
                let pos = event.ppq_position();
                let mut offset = (pos - self.last_ppq).to_le_bytes().to_vec();
                self.last_ppq = pos;
                let muted = match event.muted() {
                    true => 2,
                    false => 0,
                };
                let flag = (event.selected() as u8)
                    | muted
                    | event.cc_shape_kind().to_raw();
                let mut length = (event.message().get_raw().len() as i32)
                    .to_le_bytes()
                    .to_vec();
                //
                let mut buf = Vec::with_capacity(size);
                buf.append(&mut offset);
                buf.push(flag);
                buf.append(&mut length);
                buf.append(&mut event.message_mut().get_raw());
                //
                self.buf = Some(buf.into_iter());
                // Some(i8)
                Some(self.buf.as_mut().unwrap().next().unwrap())
            }
        }
    }
}

impl<T: Iterator<Item = MidiEvent<RawMidiMessage>>> Iterator
    for MidiEventConsumer<T>
{
    type Item = u8;
    fn next(&mut self) -> Option<Self::Item> {
        match self.buf.as_mut() {
            Some(buf) => match buf.next() {
                Some(next) => Some(next),
                None => self.next_buf(),
            },
            None => self.next_buf(),
        }
    }
}

/// Represents MediaItemTake midi CC shape kind.
///
/// # Note
///
/// If CcShapeKind::Beizer is given to CC event, additional midi event
/// should be put at the same position:
/// 0xF followed by 'CCBZ ' and 5 more bytes represents
/// bezier curve data for the previous MIDI event:
/// - 1 byte for the bezier type (usually 0)
/// - 4 bytes for the bezier tension as a float.
#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Debug,
    Default,
    Serialize,
    Deserialize,
)]
pub enum CcShapeKind {
    #[default]
    Square,
    Linear,
    SlowStartEnd,
    FastStart,
    FastEnd,
    Beizer,
}
impl CcShapeKind {
    /// CcShapeKind from u8.
    ///
    /// Returns Err if can not find proper variant.
    pub fn from_raw(value: u8) -> Result<Self, String> {
        match value {
            v if v == 0 => Ok(Self::Square),
            v if v == 16 => Ok(Self::Linear),
            v if v == 32 => Ok(Self::SlowStartEnd),
            v if v == 16 | 32 => Ok(Self::FastStart),
            v if v == 64 => Ok(Self::FastEnd),
            v if v == 16 | 64 => Ok(Self::Beizer),
            _ => Err(format!("not a cc shape: {:?}", value)),
        }
    }

    /// u8 representation of CcShapeKind
    pub fn to_raw(&self) -> u8 {
        match self {
            Self::Square => 0,
            Self::Linear => 16,
            Self::SlowStartEnd => 32,
            Self::FastStart => 16 | 32,
            Self::FastEnd => 64,
            Self::Beizer => 16 | 64,
        }
    }
}

/// Iterates through CC events. Better not to use outside the module.
pub struct FilterCC {
    midi_events: MidiEventBuilder,
}
impl From<MidiEventBuilder> for FilterCC {
    fn from(value: MidiEventBuilder) -> Self {
        Self { midi_events: value }
    }
}
impl Iterator for FilterCC {
    type Item = MidiEvent<CCMessage>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.midi_events.next()?;
        let buf = match item.cc_shape_kind() {
            CcShapeKind::Beizer => {
                let mut buf = item.message().get_raw();
                let beizer = self
                    .midi_events
                    .next()
                    .expect("should be beizer tension value");
                buf.append(&mut beizer.message().get_raw());
                buf
            }
            _ => item.message().get_raw(),
        };
        let message = match CCMessage::from_raw(buf) {
            None => return self.next(),
            Some(m) => m,
        };
        Some(MidiEvent::with_new_message(item, message))
    }
}

/// Iterates through Note On events. Better not to use outside the module.
pub struct FilterNoteOn {
    midi_events: MidiEventBuilder,
}
impl From<MidiEventBuilder> for FilterNoteOn {
    fn from(value: MidiEventBuilder) -> Self {
        Self { midi_events: value }
    }
}
impl Iterator for FilterNoteOn {
    type Item = MidiEvent<NoteOnMessage>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.midi_events.next()?;
        let message = match NoteOnMessage::from_raw(item.message().get_raw()) {
            None => return self.next(),
            Some(m) => m,
        };
        Some(MidiEvent::with_new_message(item, message))
    }
}

/// Iterates through Note Off. Better not to use outside the module.
pub struct FilterNoteOff {
    midi_events: MidiEventBuilder,
}
impl From<MidiEventBuilder> for FilterNoteOff {
    fn from(value: MidiEventBuilder) -> Self {
        Self { midi_events: value }
    }
}
impl Iterator for FilterNoteOff {
    type Item = MidiEvent<NoteOffMessage>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.midi_events.next()?;
        let message = match NoteOffMessage::from_raw(item.message().get_raw())
        {
            None => return self.next(),
            Some(m) => m,
        };
        Some(MidiEvent::with_new_message(item, message))
    }
}

/// Iterates through Pitch events. Better not to use outside the module.
pub struct FilterPitchBend {
    midi_events: MidiEventBuilder,
}
impl From<MidiEventBuilder> for FilterPitchBend {
    fn from(value: MidiEventBuilder) -> Self {
        Self { midi_events: value }
    }
}
impl Iterator for FilterPitchBend {
    type Item = MidiEvent<PitchBendMessage>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.midi_events.next()?;
        let message =
            match PitchBendMessage::from_raw(item.message().get_raw()) {
                None => return self.next(),
                Some(m) => m,
            };
        Some(MidiEvent::with_new_message(item, message))
    }
}

/// Iterates through AfterTouch. Better not to use outside the module.
pub struct FilterAfterTouch {
    midi_events: MidiEventBuilder,
}
impl From<MidiEventBuilder> for FilterAfterTouch {
    fn from(value: MidiEventBuilder) -> Self {
        Self { midi_events: value }
    }
}
impl Iterator for FilterAfterTouch {
    type Item = MidiEvent<AfterTouchMessage>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.midi_events.next()?;
        let message =
            match AfterTouchMessage::from_raw(item.message().get_raw()) {
                None => return self.next(),
                Some(m) => m,
            };
        Some(MidiEvent::with_new_message(item, message))
    }
}

/// Iterates through Ch Pressure events. Better not to use outside the module.
pub struct FilterChannelPressure {
    midi_events: MidiEventBuilder,
}
impl From<MidiEventBuilder> for FilterChannelPressure {
    fn from(value: MidiEventBuilder) -> Self {
        Self { midi_events: value }
    }
}
impl Iterator for FilterChannelPressure {
    type Item = MidiEvent<ChannelPressureMessage>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.midi_events.next()?;
        let buf = match item.cc_shape_kind() {
            CcShapeKind::Beizer => {
                let mut buf = item.message().get_raw();
                let beizer = self
                    .midi_events
                    .next()
                    .expect("should be beizer tension value");
                buf.append(&mut beizer.message().get_raw());
                buf
            }
            _ => item.message().get_raw(),
        };
        let message = match ChannelPressureMessage::from_raw(buf) {
            None => return self.next(),
            Some(m) => m,
        };
        Some(MidiEvent::with_new_message(item, message))
    }
}

/// Iterates through Pr Change events. Better not to use outside the module.
pub struct FilterProgramChange {
    midi_events: MidiEventBuilder,
}
impl From<MidiEventBuilder> for FilterProgramChange {
    fn from(value: MidiEventBuilder) -> Self {
        Self { midi_events: value }
    }
}
impl Iterator for FilterProgramChange {
    type Item = MidiEvent<ProgramChangeMessage>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.midi_events.next()?;
        let message =
            match ProgramChangeMessage::from_raw(item.message().get_raw()) {
                None => return self.next(),
                Some(m) => m,
            };
        Some(MidiEvent::with_new_message(item, message))
    }
}

/// Iterates through Sys events. Better not to use outside the module.
pub struct FilterAllSys {
    midi_events: MidiEventBuilder,
}
impl From<MidiEventBuilder> for FilterAllSys {
    fn from(value: MidiEventBuilder) -> Self {
        Self { midi_events: value }
    }
}
impl Iterator for FilterAllSys {
    type Item = MidiEvent<AllSysMessage>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.midi_events.next()?;
        let message = match AllSysMessage::from_raw(item.message().get_raw()) {
            None => return self.next(),
            Some(m) => {
                // Filter Beizer Curve messages
                let s = &m.borrow_raw()[..6];
                if s == [255, 15, 67, 67, 66, 90] {
                    return self.next();
                }
                m
            }
        };
        Some(MidiEvent::with_new_message(item, message))
    }
}

/// Iterates through Note events. Better not to use outside the module.
pub struct FilterNotes<T: Iterator<Item = MidiEvent<RawMidiMessage>>> {
    midi_events: T,
    note_ons: Vec<MidiEvent<NoteOnMessage>>,
}
impl<T: Iterator<Item = MidiEvent<RawMidiMessage>>> FilterNotes<T> {
    pub fn new(events: T) -> Self {
        Self {
            midi_events: events,
            note_ons: Vec::new(),
        }
    }
}
impl From<MidiEventBuilder> for FilterNotes<MidiEventBuilder> {
    fn from(value: MidiEventBuilder) -> Self {
        Self::new(value)
    }
}
impl<T: Iterator<Item = MidiEvent<RawMidiMessage>>> Iterator
    for FilterNotes<T>
{
    type Item = MidiNoteEvent;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.midi_events.next()?;
        let raw_msg = item.message().get_raw();
        match NoteOnMessage::from_raw(raw_msg.clone()) {
            None => (),
            Some(msg) => {
                self.note_ons.push(MidiEvent::with_new_message(item, msg));
                return self.next();
            }
        }
        match NoteOffMessage::from_raw(raw_msg) {
            None => return self.next(),
            Some(msg) => {
                let off =
                    MidiEvent::with_new_message(item.clone(), msg.clone());
                let on = match self.note_ons.iter().position(|i| {
                    i.message().note() == off.message().note()
                        && i.message().channel_private()
                            == off.message().channel_private()
                }) {
                    Some(on) => self.note_ons.swap_remove(on),
                    None => {
                        eprintln!("No Note On for note-off: {:?}", off);
                        let mut on = MidiEvent::with_new_message(
                            item,
                            NoteOnMessage::new(
                                msg.channel_private(),
                                msg.note(),
                                msg.velocity(),
                            ),
                        );
                        on.set_ppq_position(0);
                        on
                    }
                };
                Some(MidiNoteEvent::from_raw_parts(on, off))
            }
        }
    }
}

/// Convert MidiNote events to Raw events.
pub fn flatten_midi_notes(
    iter: impl Iterator<Item = MidiNoteEvent>,
) -> impl Iterator<Item = MidiEvent<RawMidiMessage>> {
    iter.flat_map(|i| {
        let out: [MidiEvent<RawMidiMessage>; 2];
        let on_msg = NoteOnMessage::new(i.channel, i.note, i.on_velocity);
        let off_msg = NoteOffMessage::new(i.channel, i.note, i.off_velocity);
        let on = MidiEvent::new(
            i.start_in_ppq,
            i.is_selected,
            i.is_muted,
            CcShapeKind::Square,
            on_msg.as_raw_message(),
        );
        let off = MidiEvent::new(
            i.end_in_ppq,
            i.is_selected,
            i.is_muted,
            CcShapeKind::Square,
            off_msg.as_raw_message(),
        );
        out = [on, off];
        out.into_iter()
    })
}

/// Unfold CC events with Beizer data to separate raw events
pub fn flatten_events_with_beizer_curve(
    iter: impl Iterator<Item = MidiEvent<impl HasBeizer>>,
) -> impl Iterator<Item = MidiEvent<RawMidiMessage>> {
    iter.flat_map(|i| {
        let mut out: Vec<MidiEvent<RawMidiMessage>> = Vec::new();
        let msg = i.message();
        out.push(MidiEvent::with_new_message(
            i.clone(),
            RawMidiMessage::from_raw(msg.msg_buf().clone()).unwrap(),
        ));
        match msg.beizer_buf().len() != 0 {
            false => (),
            true => {
                let buf = msg.beizer_buf().clone();
                let mut evt = MidiEvent::with_new_message(
                    i,
                    RawMidiMessage::from_raw(buf).unwrap(),
                );
                evt.set_cc_shape_kind(CcShapeKind::Square);
                out.push(evt);
            }
        }
        out.into_iter()
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        flatten_events_with_beizer_curve, flatten_midi_notes, sorted_by_ppq,
        to_raw_midi_events, CCMessage, ChannelPressureMessage, MidiEvent,
        MidiEventBuilder, MidiEventConsumer, MidiNoteEvent,
    };

    #[test]
    fn test_flatten_notes() {
        let notes_buf = [
            30, 0, 0, 0, 0, 3, 0, 0, 0, 144, 61, 96, //
            30, 0, 0, 0, 0, 3, 0, 0, 0, 144, 57, 96, //
            80, 0, 0, 0, 0, 3, 0, 0, 0, 128, 61, 0, //
            30, 0, 0, 0, 0, 3, 0, 0, 0, 144, 64, 96, //
            57, 0, 0, 0, 0, 3, 0, 0, 0, 128, 57, 0, //
            0, 0, 0, 0, 0, 3, 0, 0, 0, 128, 64, 0, //
            120, 0, 0, 0, 0, 3, 0, 0, 0, 144, 59, 96, //
            0, 0, 0, 0, 0, 3, 0, 0, 0, 128, 59, 0,
        ];
        let events =
            MidiEventBuilder::new(notes_buf.clone().to_vec().into_iter());
        print!(
            "not_filtered events: {:?}",
            events.clone().collect::<Vec<_>>()
        );
        println!("\n----NOTE EVENTS----");
        let note_events: Vec<MidiNoteEvent> =
            events.clone().filter_notes().collect();
        for event in note_events.iter() {
            println!("{:?}", event);
        }
        let raw_events =
            to_raw_midi_events(flatten_midi_notes(note_events.into_iter()));
        let raw_buf: Vec<u8> =
            MidiEventConsumer::new(sorted_by_ppq(raw_events)).collect();
        println!("{:?}", raw_buf);
        assert_eq!(notes_buf.len(), raw_buf.len(), "No equal length!");
        for (idx, (left, right)) in
            notes_buf.into_iter().zip(raw_buf).enumerate()
        {
            println!("[{}], left: {}, right: {}", idx, left, right);
            assert_eq!(left, right, "assert failed at index: {}", idx);
        }
    }

    #[test]
    fn test_flatten_beizer_at_cc() {
        let cc_buf = [
            56, 4, 0, 0, 0, 3, 0, 0, 0, 176, 1, 42, //
            120, 0, 0, 0, 0, 3, 0, 0, 0, 176, 1, 76, //
            120, 0, 0, 0, 80, 3, 0, 0, 0, 176, 1, 78, //
            0, 0, 0, 0, 0, 12, 0, 0, 0, 255, 15, 67, 67, 66, 90, 32, 0, 205,
            204, 12, 191, 120, //
            0, 0, 0, 0, 3, 0, 0, 0, 176, 2, 74, //
        ];
        let events =
            MidiEventBuilder::new(cc_buf.clone().to_vec().into_iter());
        println!("\n----CC EVENTS----");
        let cc_events: Vec<MidiEvent<CCMessage>> =
            events.clone().filter_cc().collect();
        for event in cc_events.iter() {
            println!("{}", event);
        }
        let raw_events = to_raw_midi_events(flatten_events_with_beizer_curve(
            cc_events.into_iter(),
        ));
        let raw_buf: Vec<u8> =
            MidiEventConsumer::new(sorted_by_ppq(raw_events)).collect();
        println!("{:?}", raw_buf);
        assert_eq!(cc_buf.len(), raw_buf.len(), "No equal length!");
        for (idx, (left, right)) in cc_buf.into_iter().zip(raw_buf).enumerate()
        {
            println!("[{}], left: {}, right: {}", idx, left, right);
            assert_eq!(left, right, "assert failed at index: {}", idx);
        }
    }
    #[test]
    fn test_flatten_beizer_at_ch_pr() {
        let ch_pr_buf = [
            0, 0, 0, 0, 48, 2, 0, 0, 0, 208, 64, //
            120, 0, 0, 0, 80, 2, 0, 0, 0, 208, 104, //
            0, 0, 0, 0, 0, 12, 0, 0, 0, 255, 15, 67, 67, 66, 90, 32, 0, 133,
            235, 81, 63, //
            3, 0, 0, 0, 48, 2, 0, 0, 0, 208, 64,
        ];
        let events =
            MidiEventBuilder::new(ch_pr_buf.clone().to_vec().into_iter());
        print!(
            "not_filtered events: {:?}",
            events.clone().collect::<Vec<_>>()
        );
        println!("\n----ChannelPressure EVENTS----");
        let cc_events: Vec<MidiEvent<ChannelPressureMessage>> =
            events.clone().filter_channel_pressure().collect();
        for event in cc_events.iter() {
            println!("{}", event);
        }
        let raw_events = to_raw_midi_events(flatten_events_with_beizer_curve(
            cc_events.into_iter(),
        ));
        let raw_buf: Vec<u8> =
            MidiEventConsumer::new(sorted_by_ppq(raw_events)).collect();
        println!("{:?}", raw_buf);
        assert_eq!(ch_pr_buf.len(), raw_buf.len(), "No equal length!");
        for (idx, (left, right)) in
            ch_pr_buf.into_iter().zip(raw_buf).enumerate()
        {
            println!("[{}], left: {}, right: {}", idx, left, right);
            assert_eq!(left, right, "assert failed at index: {}", idx);
        }
    }

    #[test]
    fn test_flatten_beizer_both() {
        let ch_pr_buf = [
            56, 4, 0, 0, 0, 3, 0, 0, 0, 176, 1, 42, //
            120, 0, 0, 0, 0, 3, 0, 0, 0, 176, 1, 76, //
            1, 0, 0, 0, 48, 2, 0, 0, 0, 208, 64, //
            120, 0, 0, 0, 80, 2, 0, 0, 0, 208, 104, //
            0, 0, 0, 0, 0, 12, 0, 0, 0, 255, 15, 67, 67, 66, 90, 32, 0, 133,
            235, 81, 63, //
            120, 0, 0, 0, 80, 3, 0, 0, 0, 176, 1, 78, //
            0, 0, 0, 0, 0, 12, 0, 0, 0, 255, 15, 67, 67, 66, 90, 32, 0, 205,
            204, 12, 191, 120, //
            0, 0, 0, 0, 3, 0, 0, 0, 176, 2, 74, //
            3, 0, 0, 0, 48, 2, 0, 0, 0, 208, //
            64, 120, 0, 0, 0, 80, 3, 0, 0, 0, 176, 1, 74, //
            0, 0, 0, 0, 0, 12, 0, 0, 0, 255, 15, 67, 67, 66, 90, 32, 0, 205,
            204, 12, 191, //
            10, 0, 0, 0, 48, 2, 0, 0, 0, 208, 64,
        ];
        let events =
            MidiEventBuilder::new(ch_pr_buf.clone().to_vec().into_iter());
        print!(
            "not_filtered events: {:?}",
            events.clone().collect::<Vec<_>>()
        );
        println!("\n----ChannelPressure EVENTS----");
        let ch_pr_events: Vec<MidiEvent<ChannelPressureMessage>> =
            events.clone().filter_channel_pressure().collect();
        for event in ch_pr_events.iter() {
            println!("{}", event);
        }
        println!("\n----CC EVENTS----");
        let cc_events: Vec<MidiEvent<CCMessage>> =
            events.clone().filter_cc().collect();
        for event in cc_events.iter() {
            println!("{}", event);
        }
        let raw_events = to_raw_midi_events(
            flatten_events_with_beizer_curve(ch_pr_events.into_iter()).chain(
                flatten_events_with_beizer_curve(cc_events.into_iter()),
            ),
        );
        let raw_buf: Vec<u8> =
            MidiEventConsumer::new(sorted_by_ppq(raw_events)).collect();
        println!("{:?}", raw_buf);
        assert_eq!(ch_pr_buf.len(), raw_buf.len(), "No equal length!");
        for (idx, (left, right)) in
            ch_pr_buf.into_iter().zip(raw_buf).enumerate()
        {
            println!("[{}], left: {}, right: {}", idx, left, right);
            assert_eq!(left, right, "assert failed at index: {}", idx);
        }
    }
}
