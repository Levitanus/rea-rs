use std::{fmt::Display, vec::IntoIter};

pub trait MidiMessage: Display {
    fn from_raw(buf: Vec<u8>) -> Option<Self>
    where
        Self: Sized;
    fn get_raw(&self) -> Vec<u8>;
    fn borrow_raw(&self) -> &Vec<u8>;
    fn as_raw_message(&self) -> RawMidiMessage {
        RawMidiMessage::from_raw(self.get_raw()).unwrap()
    }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct RawMidiMessage {
    buf: Vec<u8>,
}
impl MidiMessage for RawMidiMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        Some(Self { buf })
    }
    fn get_raw(&self) -> Vec<u8> {
        self.buf.clone()
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.buf
    }
}
impl Display for RawMidiMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RawMidiMessage<{:?}>", self.borrow_raw())
    }
}

trait ShortMessage: MidiMessage {
    fn message() -> u8;

    fn channel(&self) -> u8 {
        self.borrow_raw()[0] - Self::message()
    }
    fn msg2(&self) -> u8 {
        self.borrow_raw()[1]
    }
    fn msg3(&self) -> u8 {
        self.borrow_raw()[2]
    }
    fn is_short(buf: &Vec<u8>) -> Option<()> {
        match buf.len() <= 3 {
            true => Some(()),
            false => None,
        }
    }
    fn starts_with_message(buf: &Vec<u8>) -> Option<()> {
        let msg = Self::message();
        match (msg..msg + 15).contains(&buf[0]) {
            true => Some(()),
            false => None,
        }
    }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct ControlChangeMessage {
    cc_buf: Vec<u8>,
    beizer_buf: Option<Vec<u8>>,
}
impl MidiMessage for ControlChangeMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        if !(0xb0..0xc0).contains(&buf[0]) {
            return None;
        }
        let beizer_buf = match buf.len() {
            3 => None,
            _ => Some(Vec::from(&buf[3..])),
        };
        let cc_buf = Vec::from(&buf[..3]);
        Some(Self { cc_buf, beizer_buf })
    }
    fn get_raw(&self) -> Vec<u8> {
        let mut buf = self.cc_buf.clone();
        let mut beizer = self.beizer_buf.clone().unwrap_or(Vec::new());
        buf.append(&mut beizer);
        buf
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.cc_buf
    }
}
impl ShortMessage for ControlChangeMessage {
    fn message() -> u8 {
        0xb0
    }
}
impl ControlChangeMessage {
    pub fn cc_num(&self) -> u8 {
        self.msg2()
    }
    pub fn cc_val(&self) -> u8 {
        self.msg3()
    }
    pub fn beizer_tension(&self) -> Option<f64> {
        let s = &self.beizer_buf.as_deref().clone()?[8..];
        let buf = [s[0], s[1], s[2], s[3]];
        Some(f32::from_le_bytes(buf) as f64)
    }
}
impl Display for ControlChangeMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"ControlChangeMessage{{
        channel: {},
        cc_num: {},
        value: {},
        beizer_tension: {:?}
    }}"#,
            self.channel(),
            self.cc_num(),
            self.cc_val(),
            self.beizer_tension(),
        )
    }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct NoteOnMessage {
    buf: Vec<u8>,
}
impl NoteOnMessage {
    pub fn new(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            buf: vec![0x90 + channel, note, velocity],
        }
    }
    pub fn note(&self) -> u8 {
        self.msg2()
    }
    pub fn velocity(&self) -> u8 {
        self.msg3()
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
            self.channel(),
            self.note(),
            self.velocity(),
        )
    }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct NoteOffMessage {
    buf: Vec<u8>,
}
impl NoteOffMessage {
    pub fn new(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            buf: vec![0x80 + channel, note, velocity],
        }
    }
    pub fn note(&self) -> u8 {
        self.msg2()
    }
    pub fn velocity(&self) -> u8 {
        self.msg3()
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
            self.channel(),
            self.note(),
            self.velocity(),
        )
    }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct PitchBendMessage {
    buf: Vec<u8>,
}
impl PitchBendMessage {
    pub fn raw_value(&self) -> u16 {
        (self.msg3() as u16) << 7 | self.msg2() as u16
    }
    pub fn normalized_value(&self) -> f64 {
        (self.raw_value() as i32 - 8192) as f64 / 8192.0
    }
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
            self.channel(),
            self.raw_value(),
            self.normalized_value(),
        )
    }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct AfterTouchMessage {
    buf: Vec<u8>,
}
impl AfterTouchMessage {
    pub fn note(&self) -> u8 {
        self.msg2()
    }
    pub fn pressure(&self) -> u8 {
        self.msg3()
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
            self.channel(),
            self.note(),
            self.pressure(),
        )
    }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct ProgramChangeMessage {
    buf: Vec<u8>,
}
impl ProgramChangeMessage {
    pub fn program(&self) -> u8 {
        self.msg2()
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
}
impl Display for ProgramChangeMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"ProgramChangeMessage{{
        channel: {},
        program: {},
    }}"#,
            self.channel(),
            self.program(),
        )
    }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct ChannelPressureMessage {
    cc_buf: Vec<u8>,
    beizer_buf: Option<Vec<u8>>,
}
impl MidiMessage for ChannelPressureMessage {
    fn from_raw(buf: Vec<u8>) -> Option<Self> {
        Self::starts_with_message(&buf)?;
        let beizer_buf = match buf.len() {
            2 => None,
            _ => Some(Vec::from(&buf[2..])),
        };
        let cc_buf = Vec::from(&buf[..2]);
        Some(Self { cc_buf, beizer_buf })
    }
    fn get_raw(&self) -> Vec<u8> {
        let mut buf = self.cc_buf.clone();
        let mut beizer = self.beizer_buf.clone().unwrap_or(Vec::new());
        buf.append(&mut beizer);
        buf
    }
    fn borrow_raw(&self) -> &Vec<u8> {
        &self.cc_buf
    }
}
impl ShortMessage for ChannelPressureMessage {
    fn message() -> u8 {
        0xd0
    }
}
impl ChannelPressureMessage {
    pub fn pressure(&self) -> u8 {
        self.msg2()
    }
    pub fn beizer_tension(&self) -> Option<f64> {
        let s = &self.beizer_buf.as_deref().clone()?[8..];
        let buf = [s[0], s[1], s[2], s[3]];
        Some(f32::from_le_bytes(buf) as f64)
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
            self.channel(),
            self.pressure(),
            self.beizer_tension(),
        )
    }
}

#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
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
    pub fn with_new_message(
        event: MidiEvent<RawMidiMessage>,
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

#[derive(Clone, PartialEq, PartialOrd, Debug, Default)]
pub struct MidiNoteEvent {
    pub start_in_ppq: u32,
    pub end_in_ppq: u32,
    pub is_selected: bool,
    pub is_muted: bool,
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
        assert_eq!(note_on.message().channel(), note_off.message().channel());
        assert_eq!(note_on.message().note(), note_off.message().note());
        Self::new(
            note_on.ppq_position(),
            note_off.ppq_position(),
            note_on.selected(),
            note_on.muted(),
            note_on.message().channel(),
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

/// Iterates over raw take midi data and builds SourceMediaEvent objects.
#[derive(Debug, Clone)]
pub struct MidiEventBuilder {
    buf: IntoIter<u8>,
    current_ppq: u32,
}
impl MidiEventBuilder {
    pub(crate) fn new(buf: IntoIter<u8>) -> Self {
        Self {
            buf: buf,
            current_ppq: 0,
        }
    }
    pub fn filter_cc(self) -> FilterCC {
        FilterCC { midi_events: self }
    }
    pub fn filter_note_on(self) -> FilterNoteOn {
        FilterNoteOn { midi_events: self }
    }
    pub fn filter_note_off(self) -> FilterNoteOff {
        FilterNoteOff { midi_events: self }
    }
    pub fn filter_pitch_bend(self) -> FilterPitchBend {
        FilterPitchBend { midi_events: self }
    }
    pub fn filter_after_touch(self) -> FilterAfterTouch {
        FilterAfterTouch { midi_events: self }
    }
    pub fn filter_channel_pressure(self) -> FilterChannelPressure {
        FilterChannelPressure { midi_events: self }
    }
    pub fn filter_program_change(self) -> FilterProgramChange {
        FilterProgramChange { midi_events: self }
    }
    pub fn filter_notes(self) -> FilterNotes {
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

/// Iterates through SourceMediaEvent objects and builds raw midi data
/// to be passed to take.
#[derive(Debug)]
pub struct MidiEventConsumer {
    events: IntoIter<MidiEvent<RawMidiMessage>>,
    last_ppq: u32,
    buf: Option<IntoIter<u8>>,
}
impl MidiEventConsumer {
    /// Build iterator.
    ///
    /// If sort is true — vector would be sorted by ppq_position.
    /// Be careful, this costs additional O(log n) operation in the worst case.
    pub fn new(
        mut events: Vec<MidiEvent<RawMidiMessage>>,
        sort: bool,
    ) -> Self {
        if sort == true {
            events.sort_by_key(|ev| ev.ppq_position());
        }
        Self {
            events: events.into_iter(),
            last_ppq: 0,
            buf: None,
        }
    }

    /// Checks if some events are left and builds new buf for iteration.
    fn next_buf(&mut self) -> Option<i8> {
        match self.events.next() {
            None => None,
            Some(mut event) => {
                let size = event.message().get_raw().len() + 9;
                let pos = event.ppq_position();
                let mut offset = (pos - self.last_ppq).to_le_bytes().to_vec();
                self.last_ppq = pos;
                let flag = (event.selected() as u8)
                    | ((event.muted() as u8) << 1)
                    | event.cc_shape_kind().to_raw();
                let mut length =
                    event.message().get_raw().len().to_le_bytes().to_vec();
                //
                let mut buf = Vec::with_capacity(size);
                buf.append(&mut offset);
                buf.push(flag);
                buf.append(&mut length);
                buf.append(&mut event.message_mut().get_raw());
                //
                self.buf = Some(buf.into_iter());
                // Some(i8)
                Some(self.buf.as_mut().unwrap().next().unwrap() as i8)
            }
        }
    }
}

impl Iterator for MidiEventConsumer {
    type Item = i8;
    fn next(&mut self) -> Option<Self::Item> {
        match self.buf.as_mut() {
            Some(buf) => match buf.next() {
                Some(next) => Some(next as i8),
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
    Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default,
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

pub struct FilterCC {
    midi_events: MidiEventBuilder,
}
impl From<MidiEventBuilder> for FilterCC {
    fn from(value: MidiEventBuilder) -> Self {
        Self { midi_events: value }
    }
}
impl Iterator for FilterCC {
    type Item = MidiEvent<ControlChangeMessage>;

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
        let message = match ControlChangeMessage::from_raw(buf) {
            None => return self.next(),
            Some(m) => m,
        };
        Some(MidiEvent::with_new_message(item, message))
    }
}

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
                println!("beizer");
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

pub struct FilterNotes {
    midi_events: MidiEventBuilder,
    note_ons: Vec<MidiEvent<NoteOnMessage>>,
}
impl From<MidiEventBuilder> for FilterNotes {
    fn from(value: MidiEventBuilder) -> Self {
        Self {
            midi_events: value,
            note_ons: Vec::new(),
        }
    }
}
impl Iterator for FilterNotes {
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
                let off = MidiEvent::with_new_message(item, msg);
                let on = self
                    .note_ons
                    .iter()
                    .position(|i| {
                        i.message().note() == off.message().note()
                            && i.message().channel() == off.message().channel()
                    })
                    .expect(
                        format!(
                            "There is no note on for note_off: {:#?}",
                            off.message()
                        )
                        .as_str(),
                    );
                let on = self.note_ons.swap_remove(on);
                Some(MidiNoteEvent::from_raw_parts(on, off))
            }
        }
    }
}

#[test]
fn test() {
    let buf: Vec<u8> = vec![
        56, 4, 0, 0, 0, 3, 0, 0, 0, 176, 1, 42, 120, 0, 0, 0, 0, 8, 0, 0, 0,
        255, 1, 109, 121, 116, 101, 120, 116, 0, 0, 0, 0, 0, 3, 0, 0, 0, 176,
        1, 45, 120, 0, 0, 0, 1, 3, 0, 0, 0, 160, 61, 88, 0, 0, 0, 0, 0, 3, 0,
        0, 0, 176, 1, 59, 0, 0, 0, 0, 0, 3, 0, 0, 0, 144, 61, 96, 120, 0, 0,
        0, 0, 3, 0, 0, 0, 176, 1, 68, 120, 0, 0, 0, 0, 3, 0, 0, 0, 176, 1, 76,
        120, 0, 0, 0, 0, 3, 0, 0, 0, 176, 1, 78, 0, 0, 0, 0, 0, 3, 0, 0, 0,
        128, 61, 0, 120, 0, 0, 0, 80, 3, 0, 0, 0, 176, 1, 74, 0, 0, 0, 0, 0,
        12, 0, 0, 0, 255, 15, 67, 67, 66, 90, 32, 0, 205, 204, 12, 191, 0, 0,
        0, 0, 48, 2, 0, 0, 0, 208, 64, 0, 0, 0, 0, 0, 3, 0, 0, 0, 144, 57, 96,
        0, 0, 0, 0, 0, 32, 0, 0, 0, 255, 15, 78, 79, 84, 69, 32, 48, 32, 53,
        55, 32, 116, 101, 120, 116, 32, 34, 116, 101, 120, 116, 32, 110, 111,
        116, 97, 116, 105, 111, 110, 34, 120, 0, 0, 0, 80, 2, 0, 0, 0, 208,
        104, 0, 0, 0, 0, 0, 12, 0, 0, 0, 255, 15, 67, 67, 66, 90, 32, 0, 133,
        235, 81, 63, 0, 0, 0, 0, 0, 3, 0, 0, 0, 144, 64, 96, 104, 1, 0, 0, 0,
        3, 0, 0, 0, 176, 1, 29, 0, 0, 0, 0, 48, 2, 0, 0, 0, 208, 64, 120, 0,
        0, 0, 0, 3, 0, 0, 0, 176, 1, 28, 0, 0, 0, 0, 0, 3, 0, 0, 0, 128, 57,
        0, 120, 0, 0, 0, 0, 3, 0, 0, 0, 180, 0, 121, 0, 0, 0, 0, 0, 3, 0, 0,
        0, 180, 32, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 196, 95, 0, 0, 0, 0, 0, 3,
        0, 0, 0, 128, 64, 0, 120, 0, 0, 0, 0, 3, 0, 0, 0, 144, 59, 96, 0, 0,
        0, 0, 0, 23, 0, 0, 0, 255, 15, 78, 79, 84, 69, 32, 48, 32, 53, 57, 32,
        99, 117, 115, 116, 111, 109, 32, 116, 101, 115, 116, 120, 0, 0, 0, 0,
        3, 0, 0, 0, 176, 1, 29, 120, 0, 0, 0, 0, 3, 0, 0, 0, 176, 1, 33, 0, 0,
        0, 0, 0, 3, 0, 0, 0, 224, 65, 67, 120, 0, 0, 0, 48, 3, 0, 0, 0, 176,
        1, 38, 0, 0, 0, 0, 0, 3, 0, 0, 0, 224, 114, 106, 120, 0, 0, 0, 0, 3,
        0, 0, 0, 176, 1, 64, 0, 0, 0, 0, 0, 3, 0, 0, 0, 224, 66, 112, 0, 0, 0,
        0, 0, 3, 0, 0, 0, 128, 59, 0, 120, 0, 0, 0, 0, 3, 0, 0, 0, 224, 65,
        67, 120, 0, 0, 0, 0, 3, 0, 0, 0, 224, 44, 32, 0, 0, 0, 0, 0, 34, 0, 0,
        0, 255, 15, 84, 82, 65, 67, 32, 100, 121, 110, 97, 109, 105, 99, 32,
        99, 114, 101, 115, 99, 101, 110, 100, 111, 32, 108, 101, 110, 32, 49,
        46, 48, 48, 48, 120, 0, 0, 0, 0, 8, 0, 0, 0, 255, 6, 109, 97, 114,
        107, 101, 114, 104, 1, 0, 0, 0, 3, 0, 0, 0, 144, 60, 96, 0, 0, 0, 0,
        0, 33, 0, 0, 0, 255, 15, 78, 79, 84, 69, 32, 48, 32, 54, 48, 32, 97,
        114, 116, 105, 99, 117, 108, 97, 116, 105, 111, 110, 32, 115, 116, 97,
        99, 99, 97, 116, 111, 120, 0, 0, 0, 48, 3, 0, 0, 0, 224, 0, 64, 224,
        1, 0, 0, 0, 3, 0, 0, 0, 128, 60, 0, 40, 5, 0, 0, 0, 3, 0, 0, 0, 176,
        123, 0,
    ];
    let events = MidiEventBuilder::new(buf.into_iter());

    println!("NOTE ON EVENTS");
    for event in events.clone().filter_note_on() {
        println!("{}", event);
    }
    println!("\n----NOTE OFF EVENTS----");
    for event in events.clone().filter_note_off() {
        println!("{}", event);
    }
    println!("\n----CC EVENTS----");
    for event in events.clone().filter_cc() {
        println!("{}", event);
    }
    println!("\n----AfterTouch EVENTS----");
    for event in events.clone().filter_after_touch() {
        println!("{}", event);
    }
    println!("\n----PITCH EVENTS----");
    for event in events.clone().filter_pitch_bend() {
        println!("{}", event);
    }
    println!("\n----ChannelPressure EVENTS----");
    for event in events.clone().filter_channel_pressure() {
        println!("{}", event);
    }
    println!("\n\n----NOTE EVENTS----");
    println!("======================");
    for event in events.clone().filter_notes() {
        println!("{:#?}", event);
    }
}
