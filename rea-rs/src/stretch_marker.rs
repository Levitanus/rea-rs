use std::ptr::null;

use serde_derive::{Deserialize, Serialize};

use crate::{
    Position, ReaRsError, Reaper, ReaperResult, SourceOffset, Take,
    WithReaperPtr,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StretchMarker {
    pub index: usize,
    pub position: Position,
    pub source_position: SourceOffset,
    pub slope: f64,
}

pub struct StretchMarkersIterator<'a> {
    index: usize,
    len: usize,
    take: &'a Take,
}
impl<'a> StretchMarkersIterator<'a> {
    pub(crate) fn new(take: &'a Take) -> ReaperResult<Self> {
        let len = take.n_stretch_markers()?;
        Ok(Self {
            index: 0,
            len,
            take,
        })
    }
}
impl<'a> Iterator for StretchMarkersIterator<'a> {
    type Item = StretchMarker;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.len {
            let current = self.index;
            self.index += 1;
            if let Some(marker) =
                self.take.stretch_marker(current).ok().flatten()
            {
                return Some(marker);
            }
        }
        None
    }
}

impl Take {
    pub fn n_stretch_markers(&self) -> ReaperResult<usize> {
        Ok(unsafe {
            Reaper::get()
                .low()
                .GetTakeNumStretchMarkers(self.get()?.as_ptr())
                as usize
        })
    }

    pub fn stretch_marker(
        &self,
        index: usize,
    ) -> ReaperResult<Option<StretchMarker>> {
        let mut position = 0.0;
        let mut source_position = 0.0;
        let idx = unsafe {
            Reaper::get().low().GetTakeStretchMarker(
                self.get()?.as_ptr(),
                index as i32,
                &mut position,
                &mut source_position,
            )
        };
        if idx < 0 {
            return Ok(None);
        }
        let slope = unsafe {
            Reaper::get()
                .low()
                .GetTakeStretchMarkerSlope(self.get()?.as_ptr(), idx)
        };
        Ok(Some(StretchMarker {
            index: idx as usize,
            position: Position::from(position),
            source_position: SourceOffset::from_secs_f64(source_position),
            slope,
        }))
    }

    pub fn iter_stretch_markers(
        &self,
    ) -> ReaperResult<StretchMarkersIterator<'_>> {
        StretchMarkersIterator::new(self)
    }
}

impl Take {
    /// Adds (if index is None) or updates (if index is Some) stretch marker.
    ///
    /// Returns resulting marker index.
    pub fn set_stretch_marker(
        &mut self,
        index: impl Into<Option<usize>>,
        position: Position,
        source_position: impl Into<Option<SourceOffset>>,
    ) -> ReaperResult<usize> {
        let source_position =
            source_position.into().map(|value| value.as_secs_f64());
        let source_position_ptr = match source_position.as_ref() {
            Some(value) => value as *const f64,
            None => null(),
        };

        let index = index.into().map(|value| value as i32).unwrap_or(-1);
        let result = unsafe {
            Reaper::get().low().SetTakeStretchMarker(
                self.get()?.as_ptr(),
                index,
                position.into(),
                source_position_ptr,
            )
        };

        if result < 0 {
            return Err(ReaRsError::UnsuccessfulOperation(
                "Can not set stretch marker",
            ));
        }
        Ok(result as usize)
    }

    pub fn set_stretch_marker_slope(
        &mut self,
        index: usize,
        slope: f64,
    ) -> ReaperResult<()> {
        let result = unsafe {
            Reaper::get().low().SetTakeStretchMarkerSlope(
                self.get()?.as_ptr(),
                index as i32,
                slope,
            )
        };
        match result {
            true => Ok(()),
            false => Err(ReaRsError::UnsuccessfulOperation(
                "Can not set stretch marker slope",
            )),
        }
    }

    /// Deletes one or more stretch markers, returns number of deleted markers.
    pub fn delete_stretch_markers(
        &mut self,
        index: usize,
        count: impl Into<Option<usize>>,
    ) -> ReaperResult<usize> {
        let count = count.into().map(|value| value as i32);
        let count_ptr = match count.as_ref() {
            Some(value) => value as *const i32,
            None => null(),
        };
        Ok(unsafe {
            Reaper::get().low().DeleteTakeStretchMarkers(
                self.get()?.as_ptr(),
                index as i32,
                count_ptr,
            ) as usize
        })
    }

    pub fn delete_stretch_marker(
        &mut self,
        index: usize,
    ) -> ReaperResult<bool> {
        Ok(self.delete_stretch_markers(index, Some(1))? > 0)
    }

    /// Delete all stretch markers from take.
    pub fn clear_stretch_markers(&mut self) -> ReaperResult<usize> {
        let count = self.n_stretch_markers()?;
        if count == 0 {
            return Ok(0);
        }
        self.delete_stretch_markers(0, Some(count))
    }
}
