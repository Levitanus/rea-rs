use crate::{reaper_pointer::ReaperPointer, Project, ReaRsError, Reaper};
use std::{
    ffi::{c_char, CStr, CString},
    str::Utf8Error,
};

/// Returns self as a null-terminated String. Implemented only for [String].
pub trait WithNull: Clone {
    /// If not `\0` at the end, it will be added.
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
    let value: &str = value.into();
    let vec: Vec<u8> = value.chars().map(|val| val as u8).collect();
    let string: CString = unsafe { CString::from_vec_unchecked(vec) };
    string.into_raw()
}

/// Convert string to CStr pointer for using with low-level.
pub fn as_c_char<'a>(value: impl Into<&'a str>) -> *const c_char {
    let value = String::from(value.into());
    let value = value + "\0";
    let value = value.as_str();
    let value = CStr::from_bytes_with_nul(value.as_bytes()).unwrap();
    value.as_ptr()
}

/// Has hot to contain Null Byte!!!
pub fn as_c_string<'a>(value: &'a String) -> CString {
    let value = CString::new(value.as_bytes()).unwrap();
    value
}

/// Convert null-terminated String to CStr.
///
/// You can use trait [WithNull].
pub fn as_c_str<'a>(value: &'a String) -> &'a CStr {
    let value = CStr::from_bytes_with_nul(value.as_bytes()).unwrap();
    value
}

/// Convert pointer to CStr to String.
pub fn as_string(ptr: *const i8) -> Result<String, Utf8Error> {
    let value: &CStr = unsafe { CStr::from_ptr(ptr) };
    let value = value.to_str()?;
    let value = String::from(value);
    Ok(value)
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
/// - `get()` call should invoke either `require_valid`
/// or `require_valid_2`.
pub trait WithReaperPtr {
    type Ptr: Into<ReaperPointer>;
    /// Get underlying ReaperPointer.
    fn get_pointer(&self) -> Self::Ptr;
    /// Get underlying ReaperPointer with validity check.
    fn get(&self) -> Self::Ptr;
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
    fn require_valid(&self) -> anyhow::Result<()> {
        if !self.should_check() {
            return Ok(());
        }
        let ptr = self.get_pointer();
        match Reaper::get().validate_ptr(ptr) {
            true => Ok(()),
            false => Err(ReaRsError::NullPtr("reaper object").into()),
        }
    }

    /// Return [ReaRsError::NullPtr] if check failed.
    ///
    /// # Note
    ///
    /// Will not check if turned off by
    /// [`WithReaperPtr::make_unchecked`].
    fn require_valid_2(&self, project: &Project) -> anyhow::Result<()> {
        if !self.should_check() {
            return Ok(());
        }
        let ptr = self.get_pointer();
        match Reaper::get().validate_ptr_2(project, ptr) {
            true => Ok(()),
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
