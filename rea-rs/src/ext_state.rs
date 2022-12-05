use crate::{
    utils::{as_c_str, make_c_string_buf, WithNull},
    Project, Reaper,
};
use serde::de::DeserializeOwned;
pub use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};

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
/// # use rea_rs::ExtValue;
/// let mut state =
///     ExtValue::new("test section", "first", Some(10), true, None);
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
> {
    section: String,
    key: String,
    value: Option<T>,
    persist: bool,
    project: Option<&'a Project>,
    buf_size: usize,
}
impl<'a, T: Serialize + DeserializeOwned + Clone + std::fmt::Debug>
    ExtValue<'a, T>
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
        project: Option<&'a Project>,
    ) -> Self {
        let is_some = value.is_some();
        let mut obj = Self {
            section: section.into(),
            key: key.into(),
            value,
            persist,
            project,
            buf_size: 4096,
        };
        if is_some {
            if persist && obj.get().is_none() || !persist {
                obj.set(obj.value.as_ref().unwrap().clone());
            }
        } else {
            if !persist {
                obj.delete();
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
        let result = match &self.project {
            None => self.value_from_reaper(),
            Some(pr) => self.value_from_project(pr),
        };
        result
    }

    fn value_from_reaper(&self) -> Option<T> {
        let low = Reaper::get().low();
        unsafe {
            match low.HasExtState(
                as_c_str(&self.section()).as_ptr(),
                as_c_str(&self.key()).as_ptr(),
            ) {
                false => None,
                true => {
                    let value = low.GetExtState(
                        as_c_str(&self.section()).as_ptr(),
                        as_c_str(&self.key()).as_ptr(),
                    );
                    let value: &[u8] = CStr::from_ptr(value).to_bytes();
                    let value: T = rmp_serde::decode::from_slice(value)
                        .expect("This value was not serialized by ExtState");
                    Some(value)
                }
            }
        }
    }

    fn value_from_project<'b>(&self, project: &Project) -> Option<T> {
        let low = Reaper::get().low();
        let (section_str, key_str) = (self.section(), self.key());
        let (section, key) = (as_c_str(&section_str), as_c_str(&key_str));
        // let mut buf = vec![0_i8; self.buf_size];
        let buf = make_c_string_buf(self.buf_size);
        let ptr = buf.into_raw();
        let status = unsafe {
            low.GetProjExtState(
                project.context().to_raw(),
                section.as_ptr(),
                key.as_ptr(),
                ptr,
                self.buf_size as i32,
            )
        };
        if status <= 0 {
            return None;
        }
        let value = unsafe { CString::from_raw(ptr) };
        let value = value.as_bytes();
        let value: T = rmp_serde::decode::from_slice(value)
            .expect("This value was not serialized by ExtState");
        Some(value)
    }

    /// Set the value to ext state.
    pub fn set(&mut self, value: T) {
        match self.project {
            None => self.value_to_reaper(value),
            Some(pr) => self.value_to_project(pr, value),
        }
    }

    fn value_to_reaper(&self, value: T) {
        let low = Reaper::get().low();
        unsafe {
            let value = rmp_serde::encode::to_vec(&value)
                .expect("can not serialize value");
            let value = CString::from_vec_unchecked(value);
            low.SetExtState(
                as_c_str(&self.section()).as_ptr(),
                as_c_str(&self.key()).as_ptr(),
                value.into_raw(),
                self.persist,
            )
        }
    }

    fn value_to_project(&self, project: &Project, value: T) {
        let low = Reaper::get().low();
        let (section_str, key_str) = (self.section(), self.key());
        let (section, key) = (as_c_str(&section_str), as_c_str(&key_str));
        let mut value = rmp_serde::encode::to_vec(&value)
            .expect("can not serialize value");
        value.push(0);
        let value = CString::from_vec_with_nul(value)
            .expect("can not serialize to string");
        let _result = unsafe {
            low.SetProjExtState(
                project.context().to_raw(),
                section.as_ptr(),
                key.as_ptr(),
                value.into_raw(),
            )
        };
    }

    /// Erase ext value, but keep the object.
    pub fn delete(&mut self) {
        match &self.project {
            None => unsafe {
                Reaper::get().low().DeleteExtState(
                    as_c_str(&self.section()).as_ptr(),
                    as_c_str(&self.key()).as_ptr(),
                    true,
                )
            },
            Some(pr) => unsafe {
                Reaper::get().low().SetProjExtState(
                    pr.context().to_raw(),
                    as_c_str(&self.section()).as_ptr(),
                    as_c_str(&self.key()).as_ptr(),
                    CStr::from_bytes_with_nul_unchecked(&[0_u8]).as_ptr(),
                );
            },
        }
    }
}
