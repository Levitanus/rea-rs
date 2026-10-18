use serde::{Deserialize, Serialize};

use crate::{
    KnowsProject, Measure, Project, ReaRsError, Reaper, ReaperResult, Take,
    Time,
};

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum QuantizeStart {
    ProjectStart,
    MeasureStart,
    Time(Time),
    Ppq(u32),
    Quarters(f64),
}
impl From<Time> for QuantizeStart {
    fn from(value: Time) -> Self {
        Self::Time(value)
    }
}
impl From<u32> for QuantizeStart {
    fn from(value: u32) -> Self {
        Self::Ppq(value)
    }
}
impl From<f64> for QuantizeStart {
    fn from(value: f64) -> Self {
        Self::Quarters(value)
    }
}

/// Quantize given position.
///
/// Returns error if:
/// * `start > position+interval/2`
/// * `start` is [QuantizeStart::Ppq]
///
/// if project is None, current project will be used for quartes conversion and
/// measure position search
pub fn by_time<'a>(
    start: impl Into<QuantizeStart>,
    interval: impl Into<Time>,
    position: impl Into<Time>,
    project: impl Into<Option<&'a Project>>,
) -> ReaperResult<Time> {
    let interval = interval.into();
    let position = position.into();
    let project = project.into();

    let start = match start.into() {
        QuantizeStart::ProjectStart => Time::default(),
        QuantizeStart::Time(t) => t,
        QuantizeStart::Ppq(p) => {
            return Err(ReaRsError::InvalidArgument(
                "start",
                format!("{:?}", QuantizeStart::Ppq(p)),
                "Can not convert PPQ to Time",
            ))
        }
        QuantizeStart::Quarters(q) => match project {
            None => Time::from_quarters(q, &Reaper::get().current_project())?,
            Some(p) => Time::from_quarters(q, p)?,
        },
        QuantizeStart::MeasureStart => {
            let measure = match project {
                None => Measure::from_time(
                    position,
                    &Reaper::get().current_project(),
                )?,
                Some(p) => Measure::from_time(position, p)?,
            };
            measure.start
        }
    };
    if position < start {
        if start - position > interval / 2 {
            return Err(ReaRsError::InvalidArgument(
                "position, interval or start",
                format!(
                    "position: {:?}, start: {:?}, interval: {:?}",
                    position, start, interval
                ),
                "position is too far before the start.",
            ));
        } else {
            return Ok(start);
        }
    }
    let length = position - start;
    let remainder = length % interval;
    let lower = position - remainder;
    if remainder >= interval / 2 {
        Ok(lower + interval)
    } else {
        Ok(lower)
    }
}

/// Quantize given position.
///
/// Returns error if:
/// * `start > position+interval`
/// * `start` is [QuantizeStart::Ppq]
///
/// if project is None, current project will be used for quartes conversion and
/// measure position search
pub fn by_quarters<'a>(
    start: impl Into<QuantizeStart>,
    interval_quarters: impl Into<f64>,
    position_quarters: impl Into<f64>,
    project: impl Into<Option<&'a Project>>,
) -> ReaperResult<f64> {
    let interval = interval_quarters.into();
    let position = position_quarters.into();
    let project = project.into();

    let start = match start.into() {
        QuantizeStart::ProjectStart => 0.0,
        QuantizeStart::Time(t) => match project {
            None => t.as_quarters(&Reaper::get().current_project())?,
            Some(p) => t.as_quarters(p)?,
        },
        QuantizeStart::Ppq(p) => {
            return Err(ReaRsError::InvalidArgument(
                "start",
                format!("{:?}", QuantizeStart::Ppq(p)),
                "Can not convert PPQ to Time",
            ))
        }
        QuantizeStart::Quarters(q) => q,
        QuantizeStart::MeasureStart => {
            let measure = match project {
                None => Measure::from_quarters(
                    position,
                    &Reaper::get().current_project(),
                )?,
                Some(p) => Measure::from_quarters(position, p)?,
            };
            measure.qn_start
        }
    };
    if position < start {
        if start - position > interval / 2.0 {
            return Err(ReaRsError::InvalidArgument(
                "position, interval or start",
                format!(
                    "position: {:?}, start: {:?}, interval: {:?}",
                    position, start, interval
                ),
                "position is too far before the start.",
            ));
        } else {
            return Ok(start);
        }
    }
    let length = position - start;
    let remainder = length % interval;
    let lower = position - remainder;
    if remainder >= interval / 2.0 {
        Ok(lower + interval)
    } else {
        Ok(lower)
    }
}

/// Quantize given position.
///
/// Returns error if:
/// * `start > position+interval`
///
/// if start is not [QuantizeStart::MeasureStart] or [QuantizeStart::Ppq],
/// [by_quarters] will be used for conversion.
pub fn by_ppq<'a>(
    start: impl Into<QuantizeStart>,
    interval_ppq: impl Into<u32>,
    position_ppq: impl Into<u32>,
    take: &Take,
) -> ReaperResult<u32> {
    let interval = interval_ppq.into();
    let position = position_ppq.into();

    let start = match start.into() {
        QuantizeStart::ProjectStart => {
            let position_quarters = take.get_quarters_from_ppq(position)?;
            let interval_quarters = take
                .get_quarters_from_ppq(position + interval)?
                - position_quarters;
            let quarters = by_quarters(
                QuantizeStart::ProjectStart,
                interval_quarters,
                position_quarters,
                &take.project(),
            )?;
            return take.get_ppq_from_quarters(quarters);
        }
        QuantizeStart::Time(t) => {
            let position_quarters = take.get_quarters_from_ppq(position)?;
            let interval_quarters = take
                .get_quarters_from_ppq(position + interval)?
                - position_quarters;
            let quarters = by_quarters(
                QuantizeStart::Quarters(t.as_quarters(&take.project())?),
                interval_quarters,
                position_quarters,
                &take.project(),
            )?;
            return take.get_ppq_from_quarters(quarters);
        }
        QuantizeStart::Ppq(p) => p,
        QuantizeStart::Quarters(q) => take.get_ppq_from_quarters(q)?,
        QuantizeStart::MeasureStart => {
            take.get_ppq_start_of_measure(position)?
        }
    };
    if position < start {
        if (start - position) * 2 > interval {
            return Err(ReaRsError::InvalidArgument(
                "position, interval or start",
                format!(
                    "position: {:?}, start: {:?}, interval: {:?}",
                    position, start, interval
                ),
                "position is too far before the start.",
            ));
        } else {
            return Ok(start);
        }
    }
    let length = position - start;
    let remainder = length % interval;
    let lower = position - remainder;
    if remainder * 2 >= interval {
        Ok(lower + interval)
    } else {
        Ok(lower)
    }
}

/// Quantize given position in generic u32 units
///
/// Could be sample amount or PPQ or whatever.
pub fn generic_u32(
    start: u32,
    interval: u32,
    position: u32,
) -> ReaperResult<u32> {
    if position < start {
        if (start - position) * 2 > interval {
            return Err(ReaRsError::InvalidArgument(
                "position, interval or start",
                format!(
                    "position: {:?}, start: {:?}, interval: {:?}",
                    position, start, interval
                ),
                "position is too far before the start.",
            ));
        } else {
            return Ok(start);
        }
    }
    let length = position - start;
    let remainder = length % interval;
    let lower = position - remainder;
    if remainder * 2 >= interval {
        Ok(lower + interval)
    } else {
        Ok(lower)
    }
}

/// Quantize given position in generic u32 units
///
/// Could be sample amount or PPQ or whatever.
pub fn generic_f64(
    start: f64,
    interval: f64,
    position: f64,
) -> ReaperResult<f64> {
    if position < start {
        if start - position > interval / 2.0 {
            return Err(ReaRsError::InvalidArgument(
                "position, interval or start",
                format!(
                    "position: {:?}, start: {:?}, interval: {:?}",
                    position, start, interval
                ),
                "position is too far before the start.",
            ));
        } else {
            return Ok(start);
        }
    }
    let length = position - start;
    let remainder = length % interval;
    let lower = position - remainder;
    if remainder >= interval / 2.0 {
        Ok(lower + interval)
    } else {
        Ok(lower)
    }
}
