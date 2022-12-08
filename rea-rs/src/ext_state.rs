use crate::{
    utils::{as_c_str, make_c_string_buf, WithNull},
    Project, Reaper,
};
use log::debug;
use serde::de::DeserializeOwned;
pub use serde::{Deserialize, Serialize};
use std::{
    ffi::{CStr, CString},
    ptr::null,
};

/// Serializes extension data.
///
/// This struct should be used instead of simple `set_ext_state`
/// and `get_ext_state` calls.
///
/// The features, that make it better are:
/// - It serializes not strings, but anything, can be serialized
/// by [serde] crate.
/// - It provides the similar interface as for global ext state values,
/// as well as to project ext data.
/// - it erases data, if in process of development
/// you decided to turn the persistence off.
/// - it can be initialized with value, but only once, if
/// persistence is needed.
///
/// Also, it is very idiomatic and type-safe, so it ideally suits
/// as gui state etc.
///
/// # Note
///
/// Be careful with ext section and key. If two different states
/// of different types are saved for one key — it will panic at get().
///
/// # Usage
///
/// ```no_run
/// use rea_rs::{ExtValue, HasExtState, Reaper, Project};
/// let rpr = Reaper::get();
/// let mut state =
///     ExtValue::new("test section", "first", Some(10), true, rpr);
/// assert_eq!(state.get().expect("can not get value"), 10);
/// state.set(56);
/// assert_eq!(state.get().expect("can not get value"), 56);
/// state.delete();
/// assert!(state.get().is_none());
/// let pr = rpr.current_project();
/// let mut state: ExtValue<u32, Project> =
///     ExtValue::new("test section", "first", None, true, &pr);
/// assert_eq!(state.get().expect("can not get value"), 10);
/// state.set(56);
/// assert_eq!(state.get().expect("can not get value"), 56);
/// state.delete();
/// assert!(state.get().is_none());
/// ```
#[derive(Debug, PartialEq)]
pub struct ExtValue<
    'a,
    T: Serialize + DeserializeOwned + Clone + std::fmt::Debug,
    O: HasExtState,
> {
    section: String,
    key: String,
    value: Option<T>,
    persist: bool,
    object: &'a O,
    buf_size: usize,
}
impl<
        'a,
        T: Serialize + DeserializeOwned + Clone + std::fmt::Debug,
        O: HasExtState,
    > ExtValue<'a, T, O>
{
    /// Create ext state object.
    ///
    /// If Some(value) provided, but persist is true,
    /// only first call to the [ExtValue::new] will
    /// initialize it. Later will keep previous value.
    pub fn new(
        section: impl Into<String>,
        key: impl Into<String>,
        value: Option<T>,
        persist: bool,
        object: &'a O,
    ) -> Self {
        let mut obj = Self {
            section: section.into(),
            key: key.into(),
            value,
            persist,
            object: object,
            buf_size: 4096,
        };
        match obj.value.as_ref() {
            None => {
                if persist {
                    match obj.get() {
                        None => (),
                        Some(val) => obj.set(val),
                    }
                } else {
                    obj.delete()
                }
            }
            Some(val) => {
                if persist && obj.get().is_none() || !persist {
                    obj.set(val.clone())
                }
            }
        }
        obj
    }

    fn section(&self) -> String {
        self.section.clone().with_null().to_string()
    }
    fn key(&self) -> String {
        self.key.clone().with_null().to_string()
    }

    /// Get value from ext state.
    ///
    /// Returns None if no value saved in REAPER.
    ///
    /// # Panics
    ///
    /// If value of the wrong type stored in the
    /// same section/key.
    pub fn get(&self) -> Option<T> {
        let (section_str, key_str) = (self.section(), self.key());
        let (section, key) = (as_c_str(&section_str), as_c_str(&key_str));
        debug!("get ext value");
        let result = self.object.get_ext_value(section, key, self.buf_size);
        debug!("match value: {:?}", result);
        let value_obj = match result {
            None => return None,
            Some(value) => value,
        };
        let value = value_obj.as_bytes();
        debug!("deserialize value: {:?}", value);
        let value: T = rmp_serde::decode::from_slice(value)
            .expect("This value was not serialized by ExtState");
        debug!("deserialized value: {:?}", value);
        Some(value)
    }

    /// Set the value to ext state.
    pub fn set(&mut self, value: T) {
        let (section_str, key_str) = (self.section(), self.key());
        let (section, key) = (as_c_str(&section_str), as_c_str(&key_str));
        let mut value = rmp_serde::encode::to_vec(&value)
            .expect("can not serialize value");
        value.push(0);
        let value = CString::from_vec_with_nul(value)
            .expect("can not serialize to string");
        self.object.set_ext_value(section, key, value.into_raw())
    }

    /// Erase ext value, but keep the object.
    pub fn delete(&mut self) {
        let (section_str, key_str) = (self.section(), self.key());
        let (section, key) = (as_c_str(&section_str), as_c_str(&key_str));
        self.object.delete_ext_value(section, key)
    }
}

pub trait HasExtState {
    fn set_ext_value(&self, section: &CStr, key: &CStr, value: *mut i8);
    fn get_ext_value(
        &self,
        section: &CStr,
        key: &CStr,
        buf_size: usize,
    ) -> Option<CString>;
    fn delete_ext_value(&self, section: &CStr, key: &CStr);
}

impl HasExtState for Reaper {
    fn set_ext_value(&self, section: &CStr, key: &CStr, value: *mut i8) {
        let low = Reaper::get().low();
        unsafe { low.SetExtState(section.as_ptr(), key.as_ptr(), value, true) }
    }

    fn get_ext_value(
        &self,
        section: &CStr,
        key: &CStr,
        _buf_size: usize,
    ) -> Option<CString> {
        let low = self.low();
        let has_state =
            unsafe { low.HasExtState(section.as_ptr(), key.as_ptr()) };
        match has_state {
            false => None,
            true => {
                let value =
                    unsafe { low.GetExtState(section.as_ptr(), key.as_ptr()) };
                let c_str = unsafe { CStr::from_ptr(value) };
                Some(CString::from(c_str))
            }
        }
    }

    fn delete_ext_value(&self, section: &CStr, key: &CStr) {
        unsafe {
            self.low()
                .DeleteExtState(section.as_ptr(), key.as_ptr(), true)
        }
    }
}

impl HasExtState for Project {
    fn set_ext_value(&self, section: &CStr, key: &CStr, value: *mut i8) {
        let low = Reaper::get().low();
        let _result = unsafe {
            low.SetProjExtState(
                self.context().to_raw(),
                section.as_ptr(),
                key.as_ptr(),
                value,
            )
        };
    }

    fn get_ext_value(
        &self,
        section: &CStr,
        key: &CStr,
        buf_size: usize,
    ) -> Option<CString> {
        let low = Reaper::get().low();
        let buf = make_c_string_buf(buf_size);
        let ptr = buf.into_raw();
        let status = unsafe {
            low.GetProjExtState(
                self.context().to_raw(),
                section.as_ptr(),
                key.as_ptr(),
                ptr,
                buf_size as i32,
            )
        };
        if status <= 0 {
            return None;
        }
        unsafe { Some(CString::from_raw(ptr)) }
    }

    fn delete_ext_value(&self, section: &CStr, key: &CStr) {
        unsafe {
            Reaper::get().low().SetProjExtState(
                self.context().to_raw(),
                section.as_ptr(),
                key.as_ptr(),
                null(),
            );
        }
    }
}
