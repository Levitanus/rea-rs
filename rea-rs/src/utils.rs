use crate::{reaper_pointer::ReaperPointer, Project, ReaRsError, Reaper};
use std::{
    ffi::CStr,
    panic::{self, AssertUnwindSafe},
};

/// Convert a valid, NUL-terminated C string pointer to a Rust `String`.
///
/// # Safety
///
/// `ptr` must be non-null, point to readable memory, and refer to a
/// NUL-terminated string for the duration of this call.
pub unsafe fn string_from_const_i8(
    ptr: *const i8,
) -> Result<String, ReaRsError> {
    if ptr.is_null() {
        return Err(ReaRsError::NullPtr("C string"));
    }
    let value: &CStr = CStr::from_ptr(ptr);
    let value = value.to_str()?;
    let value = String::from(value);
    Ok(value)
}

/// Convert a NUL-terminated C output buffer to a Rust UTF-8 string.
///
/// The buffer must be non-empty and contain a NUL terminator. If it is full
/// and has no terminator, the native response is treated as truncated.
pub fn string_from_buf(buf: &[i8]) -> Result<String, ReaRsError> {
    if buf.is_empty() {
        return Err(ReaRsError::InvalidObject("buffer must not be empty"));
    }

    let nul_pos = buf.iter().position(|ch| *ch == 0).ok_or(
        ReaRsError::InvalidObject("Can not get value string terminator"),
    )?;
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
            return Ok(self.get_pointer());
        }
        let ptr = self.get_pointer();
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
            return Ok(self.get_pointer());
        }
        let ptr = self.get_pointer();
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
        let result = panic::catch_unwind(AssertUnwindSafe(|| f(self)));
        self.make_checked();
        match result {
            Ok(result) => result,
            Err(payload) => panic::resume_unwind(payload),
        }
    }
}
