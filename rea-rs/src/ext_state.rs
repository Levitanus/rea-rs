use crate::{
    utils::{as_c_str, as_string, make_c_string_buf, WithNull},
    Envelope, GenericSend, Item, KnowsProject, Mutable, ProbablyMutable,
    Project, ReaRsError, Reaper, SendIntType, Take, Track, TrackSend,
    WithReaperPtr,
};
use log::debug;
use serde::de::DeserializeOwned;
use serde::Serialize;
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
/// Currently supported: [Project], [Track], [TrackSend], [Envelope], [Item],
/// [Take]
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
/// use rea_rs::{ExtState, HasExtState, Reaper, Project};
/// let rpr = Reaper::get();
/// let mut state =
///     ExtState::new("test section", "first", Some(10), true, rpr, None);
/// assert_eq!(state.get()?.expect("can not get value"), 10);
/// state.set(56);
/// assert_eq!(state.get()?.expect("can not get value"), 56);
/// state.delete();
/// assert!(state.get()?.is_none());
///
/// let mut pr = rpr.current_project();
/// let mut state: ExtState<u32, Project> =
///     ExtState::new("test section", "first", None, true, &pr, None);
/// assert_eq!(state.get()?.expect("can not get value"), 10);
/// state.set(56);
/// assert_eq!(state.get()?.expect("can not get value"), 56);
/// state.delete();
/// assert!(state.get()?.is_none());
///
/// let tr = pr.get_track_mut(0).unwrap();
/// let mut state = ExtState::new("testsection", "first", 45, false, &tr, None);
/// assert_eq!(state.get()?.expect("can not get value"), 45);
/// state.set(15);
/// assert_eq!(state.get()?.expect("can not get value"), 15);
/// state.delete();
/// assert_eq!(state.get()?, None);
/// Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, PartialEq)]
pub struct ExtState<
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
    ExtState<'a, T, O>
{
    /// Create ext state object.
    ///
    /// If Some(value) provided, but persist is true,
    /// only first call to the [ExtState::new] will
    /// initialize it. Later will keep previous value.
    pub fn new(
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<Option<T>>,
        persist: bool,
        object: &'a O,
        buf_size: impl Into<Option<usize>>,
    ) -> Self {
        let value = value.into();
        let buf_size = if let Some(s) = buf_size.into() {
            s
        } else {
            4096
        };
        let mut obj = Self {
            section: section.into(),
            key: key.into(),
            value,
            persist,
            object: object,
            buf_size,
        };
        match obj.value.as_ref() {
            None => {
                if persist {
                    match obj.get() {
                        Err(_) | Ok(None) => (),
                        Ok(Some(val)) => obj.set(val),
                    }
                } else {
                    obj.delete()
                }
            }
            Some(val) => {
                if persist && obj.get().unwrap_or(None).is_none() || !persist {
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
    /// # Error
    ///
    /// If value of the wrong type stored in the
    /// same section/key.
    pub fn get(&self) -> Result<Option<T>, ReaRsError> {
        let (section_str, key_str) = (self.section(), self.key());
        let (section, key) = (as_c_str(&section_str), as_c_str(&key_str));
        let result = self.object.get_ext_value(section, key, self.buf_size);
        let value_obj = match result {
            None => return Ok(None),
            Some(value) => value,
        };
        let value = value_obj.to_string_lossy();
        // let value = value_obj.as_bytes();
        // let value: T = rmp_serde::decode::from_slice(value)
        //     .expect("This value was not serialized by ExtState");
        // let value: T = serde_pickle::from_slice(value, Default::default())
        //     .expect("This value was not serialized by ExtState");
        debug!("got value: {:#?}", value);
        let value: T = match serde_json::from_str(&*value) {
            Ok(v) => v,
            Err(e) => {
                return Err(ReaRsError::ExtStateDeserializtion(
                    self.section.clone(),
                    self.key.clone(),
                    e.into(),
                ))
            }
        };
        Ok(Some(value))
    }

    /// Set the value to ext state.
    pub fn set(&mut self, value: T) {
        let (section_str, key_str) = (self.section(), self.key());
        let (section, key) = (as_c_str(&section_str), as_c_str(&key_str));
        // let mut value = serde_pickle::to_vec(&value, Default::default())
        //     .expect("can not serialize value");
        // let mut value = rmp_serde::encode::to_vec(&value)
        //     .expect("can not serialize value");
        // value.push(0);
        // let value = CString::from_vec_with_nul(value)
        //     .expect("can not serialize to string");
        let value =
            serde_json::to_string(&value).expect("Can not serialize value!");
        debug!("set value: {:#?}", value);
        let value = CString::new(value.as_str())
            .expect("Can not convert ExtValue String to CString");
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

impl<'a, P: KnowsProject> HasExtState for Envelope<'a, P, Mutable> {
    fn set_ext_value(&self, section: &CStr, key: &CStr, value: *mut i8) {
        let mut category = section_key_to_one_category(section, key);
        unsafe {
            Reaper::get().low().GetSetEnvelopeInfo_String(
                self.get().as_ptr(),
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
            Reaper::get().low().GetSetEnvelopeInfo_String(
                self.get().as_ptr(),
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
            Reaper::get().low().GetSetEnvelopeInfo_String(
                self.get().as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
                CString::new("").unwrap().into_raw(),
                true,
            )
        };
    }
}

impl<'a> HasExtState for Item<'a, Mutable> {
    fn set_ext_value(&self, section: &CStr, key: &CStr, value: *mut i8) {
        let mut category = section_key_to_one_category(section, key);
        unsafe {
            Reaper::get().low().GetSetMediaItemInfo_String(
                self.get().as_ptr(),
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
            Reaper::get().low().GetSetMediaItemInfo_String(
                self.get().as_ptr(),
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
            Reaper::get().low().GetSetMediaItemInfo_String(
                self.get().as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
                CString::new("").unwrap().into_raw(),
                true,
            )
        };
    }
}

impl<'a> HasExtState for Take<'a, Mutable> {
    fn set_ext_value(&self, section: &CStr, key: &CStr, value: *mut i8) {
        let mut category = section_key_to_one_category(section, key);
        unsafe {
            Reaper::get().low().GetSetMediaItemTakeInfo_String(
                self.get().as_ptr(),
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
            Reaper::get().low().GetSetMediaItemTakeInfo_String(
                self.get().as_ptr(),
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
            Reaper::get().low().GetSetMediaItemTakeInfo_String(
                self.get().as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
                CString::new("").unwrap().into_raw(),
                true,
            )
        };
    }
}
