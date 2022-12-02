use std::{
    ffi::{c_char, CStr, CString},
    str::Utf8Error,
};

use reaper_medium::ReaperPointer;

use crate::{
    errors::{ReaperError, ReaperResult},
    Project, Reaper,
};

pub(crate) trait WithNull: Clone {
    fn with_null(&mut self) -> &String;
}
impl WithNull for String {
    fn with_null(&mut self) -> &String {
        if !self.ends_with("\0") {
            self.push('\0');
        }
        self
    }
}

/// Convert string to CString pointer for using with low-level.
pub fn as_mut_i8<'a>(value: impl Into<&'a str>) -> *mut i8 {
    unsafe {
        let value: &str = value.into();
        let vec: Vec<u8> = value.chars().map(|val| val as u8).collect();
        let string: CString = CString::from_vec_unchecked(vec);
        string.into_raw()
    }
}

/// Convert string to CStr pointer for using with low-level.
pub fn as_c_char<'a>(value: impl Into<&'a str>) -> *const c_char {
    let value = String::from(value.into());
    let value = value + "\0";
    let value = value.as_str();
    let value = CStr::from_bytes_with_nul(value.as_bytes()).unwrap();
    value.as_ptr()
}

pub fn as_c_string<'a>(value: &'a String) -> CString {
    let value = CString::new(value.as_bytes()).unwrap();
    value
}

pub fn as_c_str<'a>(value: &'a String) -> &'a CStr {
    let value = CStr::from_bytes_with_nul(value.as_bytes()).unwrap();
    value
}

/// Convert pointer to CStr to String.
pub fn as_string(ptr: *const i8) -> Result<String, Utf8Error> {
    unsafe {
        let value: &CStr = CStr::from_ptr(ptr);
        let value = value.to_str()?;
        let value = String::from(value);
        Ok(value)
    }
}

/// Convert pointer to CString to String.
pub fn as_string_mut(ptr: *mut i8) -> Result<String, Utf8Error> {
    unsafe { Ok(String::from(CString::from_raw(ptr).to_str()?)) }
}

/// Make empty CString pointer of the given size.
pub fn make_string_buf(size: usize) -> *mut i8 {
    unsafe {
        let buf: Vec<u8> = vec![0; size];
        CString::from_vec_unchecked(buf).into_raw()
    }
}

/// Make empty CString of the given size.
pub fn make_c_string_buf(size: usize) -> CString {
    unsafe {
        let buf: Vec<u8> = vec![0; size];
        CString::from_vec_unchecked(buf)
    }
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
/// - Every method call should invoke either `require_valid`
/// or `require_valid_2`.
pub trait WithReaperPtr {
    /// Get underlying ReaperPointer.
    fn get_pointer(&self) -> ReaperPointer;
    /// Turn validity checks off.
    fn make_unchecked(&mut self);
    /// Turn validity checks on.
    fn make_checked(&mut self);
    /// State of validity checks.
    fn should_check(&self) -> bool;

    /// Return [ReaperError::NullPtr] if check failed.
    ///
    /// # Note
    ///
    /// Will not check if turned off by
    /// [`WithReaperPtr::make_unchecked`].
    fn require_valid(&self) -> ReaperResult<()> {
        if !self.should_check() {
            return Ok(());
        }
        let ptr = self.get_pointer();
        match Reaper::get().validate_ptr(ptr) {
            true => Ok(()),
            false => Err(Box::new(ReaperError::NullPtr)),
        }
    }

    /// Return [ReaperError::NullPtr] if check failed.
    ///
    /// # Note
    ///
    /// Will not check if turned off by
    /// [`WithReaperPtr::make_unchecked`].
    fn require_valid_2(&self, project: &Project) -> ReaperResult<()> {
        if !self.should_check() {
            return Ok(());
        }
        let ptr = self.get_pointer();
        match Reaper::get().validate_ptr_2(project, ptr) {
            true => Ok(()),
            false => Err(Box::new(ReaperError::NullPtr)),
        }
    }

    /// Perform function with only one validity check.
    ///
    /// Returns [ReaperError::NullPtr] if the first check
    /// failed. Also propagates any error returned from
    /// function.
    fn with_valid_ptr(
        &mut self,
        mut f: impl FnMut(&mut Self) -> ReaperResult<()>,
    ) -> ReaperResult<()> {
        self.require_valid()?;
        self.make_unchecked();
        (f)(self)?;
        self.make_checked();
        Ok(())
    }
}
