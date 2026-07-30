use crate::{reaper_pointer::ReaperPointer, Project, ReaRsError, Reaper};
use std::ffi::CStr;

/// Returns self as a null-terminated String. Implemented only for [String].
pub trait WithNull: Clone {
    /// If not `\0` at the end, it will be added.
    fn with_null(self) -> String;
}
impl WithNull for String {
    fn with_null(mut self) -> String {
        if !self.ends_with("\0") {
            self.push('\0');
        }
        self
    }
}

/// Convert pointer to CStr to String.
pub fn string_from_const_i8(ptr: *const i8) -> Result<String, ReaRsError> {
    let value: &CStr = unsafe { CStr::from_ptr(ptr) };
    let value = value.to_str()?;
    let value = String::from(value);
    Ok(value)
}

/// Convert an in-place C output buffer to Rust String with basic
/// truncation/encoding checks.
pub fn string_from_buf(buf: &[i8]) -> Result<String, ReaRsError> {
    if buf.len() < 2 {
        return Err(ReaRsError::InvalidObject(
            "buffer size must be at least 2",
        ));
    }

    let nul_pos = buf.iter().position(|ch| *ch == 0).ok_or(
        ReaRsError::InvalidObject("Can not get value string terminator"),
    )?;
    if nul_pos == buf.len() - 1 {
        return Err(ReaRsError::UnsuccessfulOperation(
            "Buffer is too small for value",
        ));
    }

    String::from_utf8(buf[..nul_pos].iter().map(|ch| *ch as u8).collect())
        .map_err(|_| {
            ReaRsError::InvalidObject("Can not decode value as UTF-8")
        })
}

/// Guarantees that REAPER object has valid pointer.
///
/// Gives the API user as much control, as he wishes.
///
/// By default, implementation has to check validity
/// with every access to the pointer e.g. with every
/// method call. But the amount of checks can be reduced
/// by the [WithReaperPtr::with_valid_ptr()] method,
/// or by manually turning validation checks off and on
/// by [WithReaperPtr::make_unchecked] and
/// [WithReaperPtr::make_checked] respectively.
///
/// # Implementation
///
/// - `get_pointer` should return raw NonNull unchecked
/// ReaperPointer.
/// - After invocation of `make_unchecked`, method `should_check`
/// has to return `false`.
/// - After invocation of `make_checked`, method `should_check`
/// has to return `true`.
/// - `get()` call should invoke either `require_valid`
/// or `require_valid_2`.
pub trait WithReaperPtr {
    type Ptr: Into<ReaperPointer> + Clone;
    /// Get underlying ReaperPointer.
    fn get_pointer(&self) -> Self::Ptr;
    /// Get underlying ReaperPointer with validity check.
    fn get(&self) -> Result<Self::Ptr, ReaRsError> {
        self.require_valid()
    }
    /// Turn validity checks off.
    fn make_unchecked(&mut self);
    /// Turn validity checks on.
    fn make_checked(&mut self);
    /// State of validity checks.
    fn should_check(&self) -> bool;

    /// Return [ReaRsError::NullPtr] if check failed.
    ///
    /// # Note
    ///
    /// Will not check if turned off by
    /// [`WithReaperPtr::make_unchecked`].
    fn require_valid(&self) -> Result<Self::Ptr, ReaRsError> {
        if !self.should_check() {
            return Ok(self.get()?);
        }
        let ptr = self.get()?;
        match Reaper::get().validate_ptr(ptr.clone()) {
            true => Ok(ptr),
            false => Err(ReaRsError::NullPtr("reaper object").into()),
        }
    }

    /// Return [ReaRsError::NullPtr] if check failed.
    ///
    /// # Note
    ///
    /// Will not check if turned off by
    /// [`WithReaperPtr::make_unchecked`].
    fn require_valid_2(
        &self,
        project: &Project,
    ) -> Result<Self::Ptr, ReaRsError> {
        if !self.should_check() {
            return Ok(self.get()?);
        }
        let ptr = self.get()?;
        match Reaper::get().validate_ptr_2(project, ptr.clone()) {
            true => Ok(ptr),
            false => Err(ReaRsError::NullPtr("reaper object").into()),
        }
    }

    /// Perform function with only one validity check.
    ///
    /// Returns [ReaRsError::NullPtr] if the first check
    /// failed. Also propagates any error returned from
    /// function.
    fn with_valid_ptr(
        &mut self,
        mut f: impl FnMut(&mut Self) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.require_valid()?;
        self.make_unchecked();
        (f)(self)?;
        self.make_checked();
        Ok(())
    }
}
