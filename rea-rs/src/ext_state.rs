use crate::{
    utils::{as_c_str, as_string, make_c_string_buf, WithNull},
    GenericSend, Mutable, ProbablyMutable, Project, Reaper, SendIntType,
    Track, TrackSend, WithReaperPtr,
};
use serde::de::DeserializeOwned;
pub use serde::{Deserialize, Serialize};
use std::{
    ffi::{CStr, CString},
    fmt::Debug,
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
/// as well as to project or other objects ext data.
/// Currently supported: [Project], [Track], [TrackSend]
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
///
/// let mut pr = rpr.current_project();
/// let mut state: ExtValue<u32, Project> =
///     ExtValue::new("test section", "first", None, true, &pr);
/// assert_eq!(state.get().expect("can not get value"), 10);
/// state.set(56);
/// assert_eq!(state.get().expect("can not get value"), 56);
/// state.delete();
/// assert!(state.get().is_none());
/// // We need drop here, as state should drop the value
/// // if persistence is false. This will borrow pr on the drop.
/// drop(state);
///
/// let tr = pr.get_track_mut(0).unwrap();
/// let mut state = ExtValue::new("testsection", "first", 45, false, &tr);
/// assert_eq!(state.get().expect("can not get value"), 45);
/// state.set(15);
/// assert_eq!(state.get().expect("can not get value"), 15);
/// state.delete();
/// assert_eq!(state.get(), None);
/// ```
#[derive(Debug, PartialEq)]
pub struct ExtValue<
    'a,
    T: Serialize + DeserializeOwned + Clone + Debug,
    O: HasExtState,
> {
    section: String,
    key: String,
    value: Option<T>,
    persist: bool,
    object: &'a O,
    buf_size: usize,
}
impl<'a, T: Serialize + DeserializeOwned + Clone + Debug, O: HasExtState>
    ExtValue<'a, T, O>
{
    /// Create ext state object.
    ///
    /// If Some(value) provided, but persist is true,
    /// only first call to the [ExtValue::new] will
    /// initialize it. Later will keep previous value.
    pub fn new(
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<Option<T>>,
        persist: bool,
        object: &'a O,
    ) -> Self {
        let value = value.into();
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
        let result = self.object.get_ext_value(section, key, self.buf_size);
        let value_obj = match result {
            None => return None,
            Some(value) => value,
        };
        let value = value_obj.as_bytes();
        let value: T = rmp_serde::decode::from_slice(value)
            .expect("This value was not serialized by ExtState");
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
impl<'a, T: Serialize + DeserializeOwned + Clone + Debug, O: HasExtState> Drop
    for ExtValue<'a, T, O>
{
    fn drop(&mut self) {
        if !self.persist {
            self.delete();
        }
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

fn get_track_ext_state<'a, T: ProbablyMutable>(
    track: &Track<'a, T>,
    section: &CStr,
    key: &CStr,
    buf_size: usize,
) -> Option<CString> {
    let mut category = section_key_to_one_category(section, key);
    let buf = make_c_string_buf(buf_size).into_raw();
    let result = unsafe {
        Reaper::get().low().GetSetMediaTrackInfo_String(
            track.get().as_ptr(),
            as_c_str(category.with_null()).as_ptr(),
            buf,
            false,
        )
    };
    match result {
        false => None,
        true => Some(unsafe { CString::from_raw(buf) }),
    }
}

fn section_key_to_one_category(section: &CStr, key: &CStr) -> String {
    let mut category = String::from("P_EXT:");
    let section =
        as_string(section.as_ptr()).expect("Can not convert to string");
    let key = as_string(key.as_ptr()).expect("Can not convert to string");
    category += &section;
    category += &key;
    category
    // String::from("P_EXT:xyz")
}

impl<'a> HasExtState for Track<'a, Mutable> {
    fn set_ext_value(&self, section: &CStr, key: &CStr, value: *mut i8) {
        let mut category = section_key_to_one_category(section, key);
        unsafe {
            Reaper::get().low().GetSetMediaTrackInfo_String(
                self.get().as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
                value,
                true,
            )
        };
    }

    fn get_ext_value(
        &self,
        section: &CStr,
        key: &CStr,
        buf_size: usize,
    ) -> Option<CString> {
        get_track_ext_state(self, section, key, buf_size)
    }

    fn delete_ext_value(&self, section: &CStr, key: &CStr) {
        let mut category = section_key_to_one_category(section, key);
        unsafe {
            Reaper::get().low().GetSetMediaTrackInfo_String(
                self.get().as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
                CString::new("").unwrap().into_raw(),
                true,
            )
        };
    }
}

impl<'a> HasExtState for TrackSend<'a, Mutable> {
    fn set_ext_value(&self, section: &CStr, key: &CStr, value: *mut i8) {
        let mut category = section_key_to_one_category(section, key);
        unsafe {
            Reaper::get().low().GetSetTrackSendInfo_String(
                self.parent_track().get().as_ptr(),
                self.as_int(),
                self.index() as i32,
                as_c_str(category.with_null()).as_ptr(),
                value,
                true,
            );
        }
    }

    fn get_ext_value(
        &self,
        section: &CStr,
        key: &CStr,
        buf_size: usize,
    ) -> Option<CString> {
        let mut category = section_key_to_one_category(section, key);
        let buf = make_c_string_buf(buf_size).into_raw();
        let result = unsafe {
            Reaper::get().low().GetSetTrackSendInfo_String(
                self.parent_track().get().as_ptr(),
                self.as_int(),
                self.index() as i32,
                as_c_str(category.with_null()).as_ptr(),
                buf,
                false,
            )
        };
        match result {
            false => None,
            true => Some(unsafe { CString::from_raw(buf) }),
        }
    }

    fn delete_ext_value(&self, section: &CStr, key: &CStr) {
        let mut category = section_key_to_one_category(section, key);
        unsafe {
            Reaper::get().low().GetSetTrackSendInfo_String(
                self.parent_track().get().as_ptr(),
                self.as_int(),
                self.index() as i32,
                as_c_str(category.with_null()).as_ptr(),
                CString::new("").unwrap().into_raw(),
                true,
            )
        };
    }
}
