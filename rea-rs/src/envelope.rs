use std::{ffi::CString, marker::PhantomData, mem::MaybeUninit};

use int_enum::IntEnum;
use reaper_medium::TrackEnvelope;

use crate::{
    errors::{ReaperError, ReaperResult},
    utils::{as_string_mut, make_c_string_buf},
    KnowsProject, Mutable, Position, ProbablyMutable, Reaper, WithReaperPtr,
    GUID,
};

#[derive(Debug, PartialEq)]
pub struct Envelope<'a, P: KnowsProject, T: ProbablyMutable> {
    ptr: TrackEnvelope,
    parent: &'a P,
    should_check: bool,
    phantom: PhantomData<T>,
}
impl<'a, P: KnowsProject, T: ProbablyMutable> WithReaperPtr<'a>
    for Envelope<'a, P, T>
{
    type Ptr = TrackEnvelope;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid_2(self.parent.project()).unwrap();
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
impl<'a, P: KnowsProject, T: ProbablyMutable> Envelope<'a, P, T> {
    pub fn new(ptr: TrackEnvelope, parent: &'a P) -> Self {
        Self {
            ptr,
            parent,
            should_check: true,
            phantom: PhantomData,
        }
    }
    pub fn guid(&self) -> GUID {
        let size = 50;
        let buf = make_c_string_buf(size);
        let ptr = buf.into_raw();
        let category = CString::new("GUID")
            .expect("Can not convert category to CString.");
        let result = unsafe {
            Reaper::get().low().GetSetEnvelopeInfo_String(
                self.get().as_ptr(),
                category.as_ptr(),
                ptr,
                false,
            )
        };
        if !result {
            panic!("Can't get GUID");
        }
        let guid_string =
            as_string_mut(ptr).expect("Can't convert ptr to String!");
        GUID::from_string(guid_string).expect("Can't convert string to GUID!")
    }

    pub fn name(&self) -> String {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().GetEnvelopeName(
                self.get().as_ptr(),
                buf,
                size as i32,
            )
        };
        match result {
            false => panic!("Can not get envelope name!"),
            true => {
                as_string_mut(buf).expect("Can not convert name to string")
            }
        }
    }

    pub fn state_chunk(&self) -> String {
        let size = i32::MAX;
        let buf = make_c_string_buf(size as usize).into_raw();
        let result = unsafe {
            Reaper::get().low().GetEnvelopeStateChunk(
                self.get().as_ptr(),
                buf,
                size as i32,
                false,
            )
        };
        match result {
            false => panic!("Can not get envelope name!"),
            true => {
                as_string_mut(buf).expect("Can not convert name to string")
            }
        }
    }
}
impl<'a, P: KnowsProject> Envelope<'a, P, Mutable> {
    pub fn insert_point(
        &mut self,
        position: Position,
        point: EnvelopePoint,
        sort: bool,
    ) -> ReaperResult<()> {
        let mut sort = MaybeUninit::new(!sort);
        let result = unsafe {
            Reaper::get().low().InsertEnvelopePoint(
                self.get().as_ptr(),
                position.into(),
                point.value,
                point.shape.int_value(),
                point.tension.into(),
                point.selected,
                sort.as_mut_ptr(),
            )
        };
        match result {
            true => Ok(()),
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not insert envelope point!",
            )
            .into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvelopePoint {
    pub value: f64,
    pub shape: EnvelopePointShape,
    // from -1.0 to 1.0
    pub tension: f64,
    pub selected: bool,
}
impl EnvelopePoint {
    pub fn new(
        value: f64,
        shape: EnvelopePointShape,
        tension: f64,
        selected: bool,
    ) -> Self {
        Self {
            value,
            shape,
            tension,
            selected,
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntEnum)]
pub enum EnvelopePointShape {
    Linear = 0,
    Square = 1,
    SlowStartEnd = 2,
    FastStart = 3,
    FastEnd = 4,
    Beizer = 5,
}
