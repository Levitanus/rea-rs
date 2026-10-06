use rea_rs_low::raw;
use std::{ffi::CString, ptr};

/// Phase in which REAPER invokes a customizable-menu hook.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustomMenuPhase {
    InitializeDefaults,
    BeforeDisplay,
}

/// Callback-scoped view of a REAPER-owned customizable menu.
pub struct CustomMenuContext<'a> {
    menu_id: &'a str,
    menu: raw::HMENU,
    phase: CustomMenuPhase,
}

impl<'a> CustomMenuContext<'a> {
    pub(crate) fn new(menu_id: &'a str, menu: raw::HMENU, flag: i32) -> Self {
        Self {
            menu_id,
            menu,
            phase: if flag == 0 {
                CustomMenuPhase::InitializeDefaults
            } else {
                CustomMenuPhase::BeforeDisplay
            },
        }
    }

    pub fn menu_id(&self) -> &str {
        self.menu_id
    }
    pub fn phase(&self) -> CustomMenuPhase {
        self.phase
    }

    /// Returns the borrowed native handle. It is valid only during the hook.
    pub fn borrowed_handle(&self) -> raw::HMENU {
        self.menu
    }

    /// Adds a command to this borrowed menu. Use only for flag 0 defaults;
    /// dynamic checked/enabled state must be updated during flag 1.
    pub fn add_command(
        &self,
        id: MenuCommandId,
        label: &str,
    ) -> anyhow::Result<()> {
        let label = CString::new(label)?;
        let mut info = raw::MENUITEMINFO::default();
        info.cbSize = std::mem::size_of::<raw::MENUITEMINFO>() as u32;
        info.fMask = raw::MIIM_ID | raw::MIIM_TYPE;
        info.fType = raw::MF_STRING;
        info.wID = id.0;
        info.dwTypeData = label.as_ptr() as *mut _;
        let swell = Reaper::get().swell();
        let position = unsafe { swell.GetMenuItemCount(self.menu) };
        if position < 0 {
            anyhow::bail!("failed to inspect borrowed menu");
        }
        unsafe {
            swell.InsertMenuItem(self.menu, position, 1, &mut info);
        }
        Ok(())
    }
}

/// Command identifier returned by popup menus and delivered by window menus.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MenuCommandId(pub u32);

/// One entry in a native menu tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MenuItem {
    Command {
        id: MenuCommandId,
        label: String,
        enabled: bool,
        checked: bool,
    },
    Submenu {
        label: String,
        items: Vec<MenuItem>,
    },
    Separator,
}

impl MenuItem {
    pub fn command(
        id: impl Into<MenuCommandId>,
        label: impl Into<String>,
    ) -> Self {
        Self::Command {
            id: id.into(),
            label: label.into(),
            enabled: true,
            checked: false,
        }
    }

    pub fn submenu(label: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self::Submenu {
            label: label.into(),
            items,
        }
    }

    pub fn separator() -> Self {
        Self::Separator
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        if let Self::Command {
            enabled: current, ..
        } = &mut self
        {
            *current = enabled;
        }
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        if let Self::Command {
            checked: current, ..
        } = &mut self
        {
            *current = checked;
        }
        self
    }
}

impl From<u32> for MenuCommandId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

/// Owns a native menu tree. Destroying the root destroys its submenus.
pub struct Menu {
    handle: raw::HMENU,
    items: Vec<MenuItem>,
}

impl Menu {
    pub fn new(
        items: impl IntoIterator<Item = MenuItem>,
    ) -> anyhow::Result<Self> {
        let handle = Reaper::get().swell().CreatePopupMenu();
        if handle.is_null() {
            anyhow::bail!("SWELL failed to create a menu");
        }
        let items: Vec<_> = items.into_iter().collect();
        let menu = Self {
            handle,
            items: items.clone(),
        };
        if let Err(error) = menu.append_items(items) {
            drop(menu);
            return Err(error);
        }
        Ok(menu)
    }

    pub fn handle(&self) -> raw::HMENU {
        self.handle
    }

    pub(crate) fn duplicate(&self) -> anyhow::Result<Self> {
        Self::new(self.items.clone())
    }

    /// Relinquishes the handle when the native window backend owns its
    /// destruction (for example, generic SWELL destroys its attached menu
    /// during HWND teardown).
    pub(crate) fn relinquish_native_ownership(&mut self) {
        self.handle = ptr::null_mut();
    }

    pub fn popup_at(
        &self,
        owner: raw::HWND,
        x: i32,
        y: i32,
    ) -> Option<MenuCommandId> {
        if owner.is_null() || self.handle.is_null() {
            return None;
        }
        let selected = unsafe {
            Reaper::get().swell().TrackPopupMenu(
                self.handle,
                (raw::TPM_RETURNCMD
                    | raw::TPM_LEFTALIGN
                    | raw::TPM_TOPALIGN
                    | raw::TPM_RIGHTBUTTON) as i32,
                x,
                y,
                0,
                owner,
                ptr::null(),
            )
        };
        (selected > 0).then_some(MenuCommandId(selected as u32))
    }

    fn append_items(
        &self,
        items: impl IntoIterator<Item = MenuItem>,
    ) -> anyhow::Result<()> {
        for item in items {
            self.append_item(item)?;
        }
        Ok(())
    }

    fn append_item(&self, item: MenuItem) -> anyhow::Result<()> {
        let swell = Reaper::get().swell();
        let (mut info, label) = match item {
            MenuItem::Separator => {
                let mut info = raw::MENUITEMINFO::default();
                info.cbSize = std::mem::size_of::<raw::MENUITEMINFO>() as u32;
                info.fMask = raw::MIIM_TYPE;
                info.fType = raw::MF_SEPARATOR;
                (info, None)
            }
            MenuItem::Command {
                id,
                label,
                enabled,
                checked,
            } => {
                let label = CString::new(label)?;
                let mut info = raw::MENUITEMINFO::default();
                info.cbSize = std::mem::size_of::<raw::MENUITEMINFO>() as u32;
                info.fMask = raw::MIIM_ID | raw::MIIM_STATE | raw::MIIM_TYPE;
                info.fType = raw::MF_STRING;
                info.fState = if enabled {
                    raw::MF_ENABLED
                } else {
                    raw::MF_GRAYED
                } | if checked {
                    raw::MF_CHECKED
                } else {
                    raw::MF_UNCHECKED
                };
                info.wID = id.0;
                info.dwTypeData = label.as_ptr() as *mut _;
                (info, Some(label))
            }
            MenuItem::Submenu { label: text, items } => {
                let label = CString::new(text)?;
                let submenu = swell.CreatePopupMenu();
                if submenu.is_null() {
                    anyhow::bail!("SWELL failed to create a submenu");
                }
                let child = Self {
                    handle: submenu,
                    items: Vec::new(),
                };
                if let Err(error) = child.append_items(items) {
                    drop(child);
                    return Err(error);
                }
                std::mem::forget(child); // Root menu owns its inserted
                                         // submenu.
                let mut info = raw::MENUITEMINFO::default();
                info.cbSize = std::mem::size_of::<raw::MENUITEMINFO>() as u32;
                info.fMask = raw::MIIM_SUBMENU | raw::MIIM_TYPE;
                info.fType = raw::MF_STRING;
                info.hSubMenu = submenu;
                info.dwTypeData = label.as_ptr() as *mut _;
                (info, Some(label))
            }
        };
        let _keep_label_alive = label;
        unsafe { swell.InsertMenuItem(self.handle, i32::MAX, 0, &mut info) };
        Ok(())
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        if !self.handle.is_null() && Reaper::is_available() {
            unsafe {
                Reaper::get().swell().DestroyMenu(self.handle);
            }
        }
    }
}

use crate::Reaper;
