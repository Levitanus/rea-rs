use std::mem::MaybeUninit;

use bitflags::bitflags;
use float_eq::float_eq;
use log::{debug, trace};
use serde::{Deserialize, Serialize};

use crate::{
    BeatsPosition, Project, ReaRsError, Reaper, ReaperResult, Time,
    TimeSignature, WithReaperPtr,
};

/// Reapresents bpm basis for [TempoTimeSigMarker]
#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub enum TempoTimeSigMarkerBasis {
    Eight,
    EightDotted,
    Quarter,
    QuarterDotted,
    Half,
    HalfDotted,
}
impl TryFrom<f64> for TempoTimeSigMarkerBasis {
    type Error = ReaRsError;
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        match value {
            v if float_eq!(v, 0.5, abs <= 0.000_1) => Ok(Self::Eight),
            v if float_eq!(v, 0.75, abs <= 0.000_1) => Ok(Self::EightDotted),
            v if float_eq!(v, 1.0, abs <= 0.000_1) => Ok(Self::Quarter),
            v if float_eq!(v, 1.5, abs <= 0.000_1) => Ok(Self::QuarterDotted),
            v if float_eq!(v, 2.0, abs <= 0.000_1) => Ok(Self::Half),
            v if float_eq!(v, 3.0, abs <= 0.000_1) => Ok(Self::HalfDotted),
            _ => Err(ReaRsError::UnsuccessfulOperationString(format!(
                "Can not convert {value} into TempoTimeSigMarkerBasis"
            ))),
        }
    }
}
impl Into<f64> for TempoTimeSigMarkerBasis {
    fn into(self) -> f64 {
        match self {
            Self::Eight => 0.5,
            Self::EightDotted => 0.75,
            Self::Quarter => 1.0,
            Self::QuarterDotted => 1.5,
            Self::Half => 2.0,
            Self::HalfDotted => 3.0,
        }
    }
}

bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct TempoTimeSigMarkerFlags:i32{
        const SET_TIME_SIGNATURE = 1;
        const DO_NOT_SET_TEMPO = 2;
        const ALLOW_PREVIOUS_PARTIAL_MEASURE = 4;
        const SET_METRONOME_PATTERN = 8;
        const RESET_RULER_GRID = 16;
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct TempoTimeSigMarker {
    pub index: u32,
    pub time: Time,
    pub beat_position: BeatsPosition,
    pub bpm: Option<f64>,
    pub time_signature: Option<TimeSignature>,
    pub linear_tempo: bool,
    pub flags: TempoTimeSigMarkerFlags,
}
impl TempoTimeSigMarker {
    pub fn get(project: &Project, index: u32) -> ReaperResult<Option<Self>> {
        let rpr = Reaper::get();
        let (
            mut timepos_out,
            mut measurepos_out,
            mut beatpos_out,
            mut bpm_out,
            mut timesig_num_out,
            mut timesig_denom_out,
            mut lineartempo_out,
        ) = (
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
            MaybeUninit::uninit(),
        );
        let result = unsafe {
            rpr.low().GetTempoTimeSigMarker(
                project.get()?.as_ptr(),
                index as i32,
                timepos_out.as_mut_ptr(),
                measurepos_out.as_mut_ptr(),
                beatpos_out.as_mut_ptr(),
                bpm_out.as_mut_ptr(),
                timesig_num_out.as_mut_ptr(),
                timesig_denom_out.as_mut_ptr(),
                lineartempo_out.as_mut_ptr(),
            )
        };
        if !result {
            return Ok(None);
        }
        let flags = unsafe {
            rpr.low().GetSetTempoTimeSigMarkerFlag(
                project.context().to_raw(),
                index as i32,
                TempoTimeSigMarkerFlags::all().bits(),
                false,
            )
        };
        let flags = TempoTimeSigMarkerFlags::from_bits_truncate(flags);

        let time_signature = match flags
            .contains(TempoTimeSigMarkerFlags::SET_TIME_SIGNATURE)
        {
            true => Some(TimeSignature {
                numerator: unsafe { timesig_num_out.assume_init() as u32 },
                denominator: unsafe { timesig_denom_out.assume_init() as u32 },
            }),
            false => None,
        };
        let bpm =
            match flags.contains(TempoTimeSigMarkerFlags::DO_NOT_SET_TEMPO) {
                true => None,
                false => Some(unsafe { bpm_out.assume_init() }),
            };

        Ok(Some(Self {
            index: index,
            time: Time::from_seconds_f64(unsafe {
                timepos_out.assume_init()
            })?,
            beat_position: BeatsPosition::Relative {
                measure_index: unsafe { measurepos_out.assume_init() as u32 },
                beats: unsafe { beatpos_out.assume_init() },
            },
            bpm,
            time_signature,
            linear_tempo: unsafe { lineartempo_out.assume_init() },
            flags,
        }))
    }

    pub fn find(project: &Project, time: Time) -> ReaperResult<u32> {
        Ok(unsafe {
            Reaper::get().low().FindTempoTimeSigMarker(
                project.get()?.as_ptr(),
                time.as_seconds_f64(),
            )
        } as u32)
    }

    pub fn add(
        project: &Project,
        time: impl Into<Time>,
        bpm: impl Into<Option<f64>>,
        time_signature: impl Into<Option<TimeSignature>>,
        linear_tempo: bool,
        flags: TempoTimeSigMarkerFlags,
    ) -> ReaperResult<u32> {
        let time_signature = time_signature.into();
        if flags
            .contains(TempoTimeSigMarkerFlags::ALLOW_PREVIOUS_PARTIAL_MEASURE)
            && time_signature.is_some()
        {
            debug!("setting partial measure tempo marker");
            let index = Self::set_internal(
                project,
                None,
                time,
                bpm,
                None,
                linear_tempo,
                flags,
            )?;
            let mut obj =
                Self::get(project, index)?.ok_or(ReaRsError::Unexpected)?;
            obj.time_signature = time_signature;
            obj.set(project)?;
            Ok(index)
        } else {
            Self::set_internal(
                project,
                None,
                time,
                bpm,
                time_signature,
                linear_tempo,
                flags,
            )
        }
    }

    pub fn set(&self, project: &Project) -> ReaperResult<()> {
        Self::set_internal(
            project,
            self.index,
            self.time,
            self.bpm,
            self.time_signature,
            self.linear_tempo,
            self.flags,
        )?;
        Ok(())
    }

    fn set_internal(
        project: &Project,
        index: impl Into<Option<u32>>,
        time: impl Into<Time>,
        bpm: impl Into<Option<f64>>,
        time_signature: impl Into<Option<TimeSignature>>,
        linear_tempo: bool,
        mut flags: TempoTimeSigMarkerFlags,
    ) -> ReaperResult<u32> {
        let mut ptidx = match index.into() {
            Some(i) => i as i32,
            None => -1,
        };
        let bpm = match bpm.into() {
            Some(bpm) => bpm,
            None => {
                flags.insert(TempoTimeSigMarkerFlags::DO_NOT_SET_TEMPO);
                120.0
            }
        };
        let (timesig_num, timesig_denom) = match time_signature.into() {
            Some(ts) => {
                flags.insert(TempoTimeSigMarkerFlags::SET_TIME_SIGNATURE);
                (ts.numerator as i32, ts.denominator as i32)
            }
            None => (0, 0),
        };
        let time = time.into();
        trace!("setting marker with params: index: {ptidx}, bpm: {bpm}, time_signature: {timesig_num}/{timesig_denom} at time: {:?}", time);
        let rpr = Reaper::get();
        let result = unsafe {
            rpr.low().SetTempoTimeSigMarker(
                project.get()?.as_ptr(),
                ptidx,
                time.as_seconds_f64(),
                -1,
                -1.0,
                bpm,
                timesig_num,
                timesig_denom,
                linear_tempo,
            )
        };
        if ptidx == -1 {
            trace!("new marker, getting real index");
            ptidx = Self::find(project, time)? as i32;
        }
        trace!("got marker index: {ptidx}, setting flags");
        unsafe {
            rpr.low().GetSetTempoTimeSigMarkerFlag(
                project.context().to_raw(),
                ptidx,
                flags.bits(),
                true,
            );
        }
        match result {
            true => Ok(ptidx as u32),
            false => Err(ReaRsError::UnsuccessfulOperation(
                "Couldn't set TempoTimeSigMarker",
            )),
        }
    }

    pub fn get_bpm_basis(
        &self,
        project: &Project,
    ) -> ReaperResult<TempoTimeSigMarkerBasis> {
        let basis = unsafe {
            Reaper::get().low().GetSetTempoTimeSigMarkerBasis(
                project.get()?.as_ptr(),
                self.index as i32,
                0.0,
                false,
            )
        };
        TempoTimeSigMarkerBasis::try_from(basis)
    }
    pub fn set_bpm_basis(
        &self,
        project: &Project,
        basis: TempoTimeSigMarkerBasis,
    ) -> ReaperResult<()> {
        unsafe {
            Reaper::get().low().GetSetTempoTimeSigMarkerBasis(
                project.get()?.as_ptr(),
                self.index as i32,
                basis.into(),
                true,
            )
        };
        Ok(())
    }
}
