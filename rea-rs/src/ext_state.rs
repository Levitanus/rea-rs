use crate::{
    utils::{string_from_buf, string_from_const_i8},
    Envelope, GenericSend, Item, KnowsProject, Project, ReaRsError, Reaper,
    SendIntType, Take, Track, TrackSend, WithReaperPtr,
};
use log::debug;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::{ffi::CString, fmt::Debug, marker::PhantomData};

const BUF_SIZE: usize = 4096;

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
///     ExtState::new("test section", "first", Some(10), true, rpr, None)?;
/// assert_eq!(state.get()?.expect("can not get value"), 10);
/// state.set(56)?;
/// assert_eq!(state.get()?.expect("can not get value"), 56);
/// state.delete()?;
/// assert!(state.get()?.is_none());
///
/// let mut pr = rpr.current_project();
/// let mut state: ExtState<u32, Project> =
///     ExtState::new("test section", "first", None, true, &pr, None)?;
/// assert_eq!(state.get()?.expect("can not get value"), 10);
/// state.set(56)?;
/// assert_eq!(state.get()?.expect("can not get value"), 56);
/// state.delete()?;
/// assert!(state.get()?.is_none());
///
/// let tr = pr.get_track(0)?.unwrap();
/// let mut state = ExtState::new("testsection", "first", 45, false, &tr, None)?;
/// assert_eq!(state.get()?.expect("can not get value"), 45);
/// state.set(15)?;
/// assert_eq!(state.get()?.expect("can not get value"), 15);
/// state.delete()?;
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
    value: PhantomData<T>,
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
    ) -> Result<Self, ReaRsError> {
        let value = value.into();
        let mut obj: ExtState<'a, T, O> =
            Self::existing(section, key, persist, object, buf_size);
        match value {
            None => {
                if !persist {
                    obj.delete()?;
                }
            }
            Some(val) => {
                if persist && obj.get()?.is_none() || !persist {
                    obj.set(val.clone())?
                }
            }
        }
        Ok(obj)
    }

    /// Load value from ExtState without making ExtState object.
    pub fn load_value(
        section: impl Into<String>,
        key: impl Into<String>,
        object: &'a O,
        buf_size: impl Into<Option<usize>>,
    ) -> Result<Option<T>, ReaRsError> {
        let buf_size = if let Some(s) = buf_size.into() {
            s
        } else {
            BUF_SIZE
        };
        let obj = Self {
            section: section.into(),
            key: key.into(),
            value: PhantomData,
            persist: false,
            object: object,
            buf_size,
        };
        obj.get()
    }

    /// Make ExtState object assuming the existence of the value.
    pub fn existing<V>(
        section: impl Into<String>,
        key: impl Into<String>,
        persist: bool,
        object: &'a O,
        buf_size: impl Into<Option<usize>>,
    ) -> ExtState<'a, V, O>
    where
        V: Serialize + DeserializeOwned + Clone + Debug,
    {
        let buf_size = if let Some(s) = buf_size.into() {
            s
        } else {
            BUF_SIZE
        };
        ExtState {
            section: section.into(),
            key: key.into(),
            value: PhantomData,
            persist,
            object,
            buf_size,
        }
    }

    pub fn section(&self) -> String {
        self.section.clone().to_string()
    }
    pub fn key(&self) -> String {
        self.key.clone().to_string()
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
        let Some(value) = self.object.get_ext_value(
            &self.section,
            &self.key,
            self.buf_size,
        )?
        else {
            return Ok(None);
        };
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
    pub fn set(&mut self, value: T) -> Result<(), ReaRsError> {
        let value = serde_json::to_string(&value).map_err(|e| {
            ReaRsError::UnexpectedAPI(format!(
                "ExtState serialize error for {}/{}: {}",
                self.section, self.key, e
            ))
        })?;
        debug!("set value: {:#?}", value);
        self.object.set_ext_value(&self.section, &self.key, value)
    }

    /// Erase ext value, but keep the object.
    pub fn delete(&mut self) -> Result<(), ReaRsError> {
        debug!("delete value");
        self.object.delete_ext_value(&self.section, &self.key)
    }
}

pub trait HasExtState {
    fn set_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), ReaRsError>;
    fn get_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        buf_size: usize,
    ) -> Result<Option<String>, ReaRsError>;
    fn delete_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<(), ReaRsError>;
}

impl HasExtState for Reaper {
    fn set_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        let low = Reaper::get().low();
        unsafe {
            low.SetExtState(
                CString::new(section.into())?.as_ptr(),
                CString::new(key.into())?.as_ptr(),
                CString::new(value.into())?.as_ptr(),
                true,
            )
        }
        Ok(())
    }

    fn get_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        _buf_size: usize,
    ) -> Result<Option<String>, ReaRsError> {
        let low = self.low();
        let section = section.into();
        let key = key.into();
        let has_state = unsafe {
            low.HasExtState(
                CString::new(section.clone())?.as_ptr(),
                CString::new(key.clone())?.as_ptr(),
            )
        };
        match has_state {
            false => Ok(None),
            true => {
                let value = unsafe {
                    low.GetExtState(
                        CString::new(section)?.as_ptr(),
                        CString::new(key)?.as_ptr(),
                    )
                };
                Ok(Some(string_from_const_i8(value)?))
            }
        }
    }

    fn delete_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        unsafe {
            self.low().DeleteExtState(
                CString::new(section.into())?.as_ptr(),
                CString::new(key.into())?.as_ptr(),
                true,
            )
        }
        Ok(())
    }
}

impl HasExtState for Project {
    fn set_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        let low = Reaper::get().low();
        let _result = unsafe {
            low.SetProjExtState(
                self.context().to_raw(),
                CString::new(section.into())?.as_ptr(),
                CString::new(key.into())?.as_ptr(),
                CString::new(value.into())?.as_ptr(),
            )
        };
        Ok(())
    }

    fn get_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        buf_size: usize,
    ) -> Result<Option<String>, ReaRsError> {
        let low = Reaper::get().low();
        let mut buf = vec![0_i8; buf_size];
        let status = unsafe {
            low.GetProjExtState(
                self.context().to_raw(),
                CString::new(section.into())?.as_ptr(),
                CString::new(key.into())?.as_ptr(),
                buf.as_mut_ptr(),
                buf_size as i32,
            )
        };
        if status <= 0 {
            return Ok(None);
        }
        Ok(Some(string_from_buf(&buf)?))
    }

    fn delete_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        unsafe {
            Reaper::get().low().SetProjExtState(
                self.context().to_raw(),
                CString::new(section.into())?.as_ptr(),
                CString::new(key.into())?.as_ptr(),
                CString::new("")?.as_ptr(),
            );
        }
        Ok(())
    }
}

fn get_track_ext_state(
    track: &Track,
    section: impl Into<String>,
    key: impl Into<String>,
    buf_size: usize,
) -> Result<Option<String>, ReaRsError> {
    let category = section_key_to_one_category(section.into(), key.into());
    let mut buf = vec![0_i8; buf_size];
    let result = unsafe {
        Reaper::get().low().GetSetMediaTrackInfo_String(
            track.get()?.as_ptr(),
            CString::new(category)?.as_ptr(),
            buf.as_mut_ptr(),
            false,
        )
    };
    match result {
        false => Ok(None),
        true => Ok(Some(string_from_buf(&buf)?)),
    }
}

fn section_key_to_one_category(section: String, key: String) -> String {
    let mut category = String::from("P_EXT:");
    category += &section;
    category += &key;
    category
}

impl HasExtState for Track {
    fn set_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        unsafe {
            Reaper::get().low().GetSetMediaTrackInfo_String(
                self.get()?.as_ptr(),
                CString::new(category)?.as_ptr(),
                CString::new(value.into())?.into_raw(),
                true,
            )
        };
        Ok(())
    }

    fn get_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        buf_size: usize,
    ) -> Result<Option<String>, ReaRsError> {
        get_track_ext_state(self, section, key, buf_size)
    }

    fn delete_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        unsafe {
            Reaper::get().low().GetSetMediaTrackInfo_String(
                self.get()?.as_ptr(),
                CString::new(category)?.as_ptr(),
                CString::new("")?.into_raw(),
                true,
            )
        };
        Ok(())
    }
}

impl<'a> HasExtState for TrackSend<'a> {
    fn set_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        unsafe {
            Reaper::get().low().GetSetTrackSendInfo_String(
                self.parent_track().get()?.as_ptr(),
                self.as_int(),
                self.index() as i32,
                CString::new(category)?.as_ptr(),
                CString::new(value.into())?.into_raw(),
                true,
            );
        }
        Ok(())
    }

    fn get_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        buf_size: usize,
    ) -> Result<Option<String>, ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        let mut buf = vec![0_i8; buf_size];
        let result = unsafe {
            Reaper::get().low().GetSetTrackSendInfo_String(
                self.parent_track().get()?.as_ptr(),
                self.as_int(),
                self.index() as i32,
                CString::new(category)?.as_ptr(),
                buf.as_mut_ptr(),
                false,
            )
        };
        match result {
            false => Ok(None),
            true => Ok(Some(string_from_buf(&buf)?)),
        }
    }

    fn delete_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        let empty = CString::new("")?;
        unsafe {
            Reaper::get().low().GetSetTrackSendInfo_String(
                self.parent_track().get()?.as_ptr(),
                self.as_int(),
                self.index() as i32,
                CString::new(category)?.as_ptr(),
                empty.into_raw(),
                true,
            )
        };
        Ok(())
    }
}

impl<'a, P: KnowsProject> HasExtState for Envelope<'a, P> {
    fn set_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        unsafe {
            Reaper::get().low().GetSetEnvelopeInfo_String(
                self.get()?.as_ptr(),
                CString::new(category)?.as_ptr(),
                CString::new(value.into())?.into_raw(),
                true,
            );
        }
        Ok(())
    }

    fn get_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        buf_size: usize,
    ) -> Result<Option<String>, ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        let mut buf = vec![0_i8; buf_size];
        let result = unsafe {
            Reaper::get().low().GetSetEnvelopeInfo_String(
                self.get()?.as_ptr(),
                CString::new(category)?.as_ptr(),
                buf.as_mut_ptr(),
                false,
            )
        };
        match result {
            false => Ok(None),
            true => Ok(Some(string_from_buf(&buf)?)),
        }
    }

    fn delete_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        let empty = CString::new("")?;
        unsafe {
            Reaper::get().low().GetSetEnvelopeInfo_String(
                self.get()?.as_ptr(),
                CString::new(category)?.as_ptr(),
                empty.into_raw(),
                true,
            )
        };
        Ok(())
    }
}

impl HasExtState for Item {
    fn set_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        unsafe {
            Reaper::get().low().GetSetMediaItemInfo_String(
                self.get()?.as_ptr(),
                CString::new(category)?.as_ptr(),
                CString::new(value.into())?.into_raw(),
                true,
            );
        }
        Ok(())
    }

    fn get_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        buf_size: usize,
    ) -> Result<Option<String>, ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        let mut buf = vec![0_i8; buf_size];
        let result = unsafe {
            Reaper::get().low().GetSetMediaItemInfo_String(
                self.get()?.as_ptr(),
                CString::new(category)?.as_ptr(),
                buf.as_mut_ptr(),
                false,
            )
        };
        match result {
            false => Ok(None),
            true => Ok(Some(string_from_buf(&buf)?)),
        }
    }

    fn delete_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        let empty = CString::new("")?;
        unsafe {
            Reaper::get().low().GetSetMediaItemInfo_String(
                self.get()?.as_ptr(),
                CString::new(category)?.as_ptr(),
                empty.into_raw(),
                true,
            )
        };
        Ok(())
    }
}

impl HasExtState for Take {
    fn set_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        unsafe {
            Reaper::get().low().GetSetMediaItemTakeInfo_String(
                self.get()?.as_ptr(),
                CString::new(category)?.as_ptr(),
                CString::new(value.into())?.into_raw(),
                true,
            );
        }
        Ok(())
    }

    fn get_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
        buf_size: usize,
    ) -> Result<Option<String>, ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        let mut buf = vec![0_i8; buf_size];
        let result = unsafe {
            Reaper::get().low().GetSetMediaItemTakeInfo_String(
                self.get()?.as_ptr(),
                CString::new(category)?.as_ptr(),
                buf.as_mut_ptr(),
                false,
            )
        };
        match result {
            false => Ok(None),
            true => Ok(Some(string_from_buf(&buf)?)),
        }
    }

    fn delete_ext_value(
        &self,
        section: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<(), ReaRsError> {
        let category = section_key_to_one_category(section.into(), key.into());
        let empty = CString::new("")?;
        unsafe {
            Reaper::get().low().GetSetMediaItemTakeInfo_String(
                self.get()?.as_ptr(),
                CString::new(category)?.as_ptr(),
                empty.into_raw(),
                true,
            )
        };
        Ok(())
    }
}
