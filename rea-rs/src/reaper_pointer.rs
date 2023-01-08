//! Module originally designed by Benjamin Klum https://github.com/helgoboss for his
//! `reaper-rs` repository. But module was moved here for removing dependency
//! on reaper-medium crate, which is almost not used by the rea-rs.
use crate::ptr_wrappers::Hwnd;

use super::ptr_wrappers::{
    AudioAccessor, MediaItem, MediaItemTake, MediaTrack, PcmSource,
    ReaProject, TrackEnvelope,
};
use c_str_macro::c_str;

use std::{ffi::CStr, os::raw::c_void};

/// Validatable REAPER pointer.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum ReaperPointer {
    MediaTrack(MediaTrack),
    ReaProject(ReaProject),
    MediaItem(MediaItem),
    MediaItemTake(MediaItemTake),
    TrackEnvelope(TrackEnvelope),
    PcmSource(PcmSource),
    AudioAccessor(AudioAccessor),
    Hwnd(Hwnd),
    // If a variant is missing in this enum, you can use this custom one as a
    // resort.
    //
    // Use [`custom()`] to create this variant.
    //
    // [`custom()`]: #method.custom
    // Custom {
    //     type_name: Cow<'a, ReaperStr>,
    //     pointer: *mut c_void,
    // },
}

impl ReaperPointer {
    /// Convenience function for creating a [`Custom`] pointer.
    ///
    /// **Don't** include the trailing asterisk (`*`)! It will be added
    /// automatically.
    ///
    /// [`Custom`]: #variant.Custom
    // pub fn custom(
    //     pointer: *mut c_void,
    //     type_name: impl Into<ReaperStringArg<'a>>,
    // ) -> ReaperPointer<'a> {
    //     ReaperPointer::Custom {
    //         pointer,
    //         type_name: type_name.into().into_inner(),
    //     }
    // }

    pub(crate) fn key_into_raw<'a>(self) -> &'a CStr {
        use ReaperPointer::*;
        match self {
            MediaTrack(_) => c_str!("MediaTrack*"),
            ReaProject(_) => c_str!("ReaProject*"),
            MediaItem(_) => c_str!("MediaItem*"),
            MediaItemTake(_) => c_str!("MediaItem_Take*"),
            TrackEnvelope(_) => c_str!("TrackEnvelope*"),
            PcmSource(_) => c_str!("PCM_source*"),
            AudioAccessor(_) => c_str!("AudioAccessor*"),
            Hwnd(_) => c_str!("HWND*"),
        }
    }

    pub(crate) fn ptr_as_void(&self) -> *mut c_void {
        use ReaperPointer::*;
        match self {
            MediaTrack(p) => p.as_ptr() as *mut _,
            ReaProject(p) => p.as_ptr() as *mut _,
            MediaItem(p) => p.as_ptr() as *mut _,
            MediaItemTake(p) => p.as_ptr() as *mut _,
            TrackEnvelope(p) => p.as_ptr() as *mut _,
            PcmSource(p) => p.as_ptr() as *mut _,
            AudioAccessor(p) => p.as_ptr() as *mut _,
            Hwnd(p) => p.as_ptr() as *mut _,
            // Custom { pointer, .. } => *pointer,
        }
    }
}

/// For just having to pass a NonNull pointer to `validate_ptr_2`. Very
/// convenient!
macro_rules! impl_from_ptr_to_variant {
    ($struct_type: ty, $enum_name: ident) => {
        impl<'a> From<$struct_type> for ReaperPointer {
            fn from(p: $struct_type) -> Self {
                ReaperPointer::$enum_name(p)
            }
        }
    };
}

impl_from_ptr_to_variant!(MediaTrack, MediaTrack);
impl_from_ptr_to_variant!(ReaProject, ReaProject);
impl_from_ptr_to_variant!(MediaItem, MediaItem);
impl_from_ptr_to_variant!(MediaItemTake, MediaItemTake);
impl_from_ptr_to_variant!(TrackEnvelope, TrackEnvelope);
impl_from_ptr_to_variant!(PcmSource, PcmSource);
impl_from_ptr_to_variant!(AudioAccessor, AudioAccessor);
impl_from_ptr_to_variant!(Hwnd, Hwnd);
