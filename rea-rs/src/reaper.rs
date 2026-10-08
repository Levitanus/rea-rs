//! High-level integration with REAPER's plugin API.
//!
//! [`Reaper`] owns the registrations made by a plugin and provides access to
//! REAPER services such as custom actions, timers, windows, Preferences pages,
//! customizable menus, and control surfaces. The global helpers are intended
//! for plugins that expose one process-wide [`Reaper`] instance; applications
//! that need explicit ownership can use [`Reaper::load`] instead.
//!
//! Callbacks are invoked by REAPER and must not outlive the [`Reaper`]
//! instance that registered them. Registration methods retain the callbacks
//! and native registration data until they are explicitly removed or the
//! instance is dropped.

use log::{debug, trace};
use rea_rs_low::{
    create_cpp_to_rust_control_surface, delete_cpp_control_surface, raw,
    register_plugin_destroy_hook, IReaperControlSurface, PluginContext, Swell,
};

use crate::{
    keys::{FVirt, KeyBinding},
    misc_enums::Section,
    ptr_wrappers::{Hwnd, KbdSectionInfo},
    swell_gui::{self, ReaperWindow, WindowHandler, WindowId, WindowSpec},
    ControlSurface, ControlSurfaceWrap, ReaRsError, ReaperResult,
};
use c_str_macro::c_str;
use serde_derive::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::CString,
    ptr::NonNull,
    sync::Arc,
    time::{Duration, Instant},
};

static mut INSTANCE: Option<Reaper> = None;

type ActionCallback = dyn Fn(&mut ActionHook) -> Result<(), anyhow::Error>;

extern "C" fn custom_menu_hook(
    menu_id: *const std::os::raw::c_char,
    menu: *mut std::ffi::c_void,
    flag: i32,
) {
    if menu_id.is_null() || menu.is_null() || !Reaper::is_available() {
        return;
    }
    let menu_id =
        unsafe { std::ffi::CStr::from_ptr(menu_id) }.to_string_lossy();
    let context =
        swell_gui::CustomMenuContext::new(&menu_id, menu as raw::HMENU, flag);
    if Reaper::get().custom_menu_hook_in_flight {
        return;
    }
    let Some((mut callback, disabled, in_flight)) =
        Reaper::get_mut().custom_menu_hook.take()
    else {
        return;
    };
    if disabled {
        Reaper::get_mut().custom_menu_hook = Some((callback, true, in_flight));
        return;
    }
    Reaper::get_mut().custom_menu_hook_in_flight = true;
    let result = swell_gui::events::invoke_callback(
        "REAPER customizable-menu hook",
        &mut || callback(&context),
    );
    let reaper = Reaper::get_mut();
    reaper.custom_menu_hook_in_flight = false;
    reaper.custom_menu_hook =
        Some((callback, result.is_err() || disabled, false));
}

unsafe extern "C" fn create_preferences_page(parent: raw::HWND) -> raw::HWND {
    if !Reaper::is_available() {
        trace!(
            "Preferences page create callback ignored: Reaper is unavailable"
        );
        return std::ptr::null_mut();
    }
    trace!("Preferences page create callback entered: parent={parent:p}");
    // Move the factory out before invoking user code. The factory can call
    // back into `Reaper`, so holding a mutable borrow of the global instance
    // while it runs would alias that access.
    let Some((mut builder, disabled)) =
        Reaper::get_mut().preferences_page_builder.take()
    else {
        trace!("Preferences page create callback has no registered builder");
        return std::ptr::null_mut();
    };
    if disabled {
        Reaper::get_mut().preferences_page_builder = Some((builder, true));
        return std::ptr::null_mut();
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || {
            let window = Reaper::get()
                .create_preferences_page_window(parent)
                .map_err(anyhow::Error::from)?;
            let hwnd = window.hwnd();
            trace!("Preferences page child created: parent={parent:p} hwnd={hwnd:p}");
            let mut guard = PreferencesPageWindowGuard {
                hwnd,
                retained: false,
            };
            let handler = builder(window)?;
            let window_id = handler.window_id();
            if handler.window().hwnd() != hwnd {
                anyhow::bail!(
                    "Preferences page handler must use the supplied window"
                );
            }
            trace!("Preferences page handler built: id={window_id:?} hwnd={hwnd:p}");
            Reaper::get_mut().register_window_handler(handler)?;
            guard.retained = true;
            trace!("Preferences page handler registered: id={window_id:?} hwnd={hwnd:p}");
            Ok::<_, anyhow::Error>(hwnd)
        },
    ));
    if disabled {
        Reaper::get_mut().preferences_page_builder = Some((builder, true));
        return std::ptr::null_mut();
    }
    match result {
        Ok(Ok(hwnd)) => {
            if Reaper::get().preferences_page_builder.is_none() {
                Reaper::get_mut().preferences_page_builder =
                    Some((builder, false));
            }
            return hwnd;
        }
        Ok(Err(error)) => {
            trace!("Preferences page creation failed; returning null HWND");
            let mut error = Some(error);
            let _ = swell_gui::events::invoke_callback(
                "REAPER Preferences page builder",
                &mut || Err::<(), _>(error.take().expect("reported once")),
            );
            if Reaper::get().preferences_page_builder.is_none() {
                Reaper::get_mut().preferences_page_builder =
                    Some((builder, true));
            }
            return std::ptr::null_mut();
        }
        Err(payload) => {
            trace!("Preferences page builder panicked; returning null HWND");
            let message = payload
                .downcast_ref::<&str>()
                .map(|message| (*message).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".to_owned());
            let _ = swell_gui::events::invoke_callback(
                "REAPER Preferences page builder",
                &mut || {
                    Err::<(), _>(anyhow::anyhow!(
                        "callback panicked: {message}"
                    ))
                },
            );
            if Reaper::get().preferences_page_builder.is_none() {
                Reaper::get_mut().preferences_page_builder =
                    Some((builder, true));
            }
            std::ptr::null_mut()
        }
    }
}

unsafe extern "C" fn preferences_page_base_proc(
    hwnd: raw::HWND,
    message: raw::UINT,
    wparam: raw::WPARAM,
    lparam: raw::LPARAM,
) -> raw::INT_PTR {
    if !Reaper::is_available() {
        return 0;
    }
    Reaper::get()
        .swell()
        .DefWindowProc(hwnd, message, wparam, lparam) as raw::INT_PTR
}

/// A custom REAPER action and its section-specific command bindings.
///
/// Actions are created by [`Reaper::register_action`]. The callback receives
/// an [`ActionHook`] containing the invocation context and can inspect or
/// update toggle state through [`Action::kind_mut`].
pub struct Action {
    bindings: Vec<ActionBinding>,
    operation: Box<ActionCallback>,
    kind: ActionKind,
    disabled: bool,
    in_flight: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActionBinding {
    section: Section,
    command_id: CommandId,
}

struct ActionRegistration {
    _section: Section,
    _id: CString,
    _name: CString,
    registration: raw::custom_action_register_t,
}

#[allow(dead_code)]
struct DefaultKeyBindingRegistration {
    _description: CString,
    registration: raw::gaccel_register_t,
    global_text: bool,
}

struct PreferencesPageRegistration {
    _id: CString,
    _display_name: CString,
    registration: raw::prefs_page_register_t,
}

struct PreferencesPageWindowGuard {
    hwnd: raw::HWND,
    retained: bool,
}

struct RetiredControlSurface {
    id: String,
    rust_owner: Box<Box<dyn IReaperControlSurface>>,
    cpp_owner: Option<NonNull<raw::IReaperControlSurface>>,
}

impl Drop for RetiredControlSurface {
    fn drop(&mut self) {
        if let Some(cpp_owner) = self.cpp_owner.take() {
            unsafe { delete_cpp_control_surface(cpp_owner) };
        }
        let _ = &self.id;
        let _ = &self.rust_owner;
    }
}

impl Drop for PreferencesPageWindowGuard {
    fn drop(&mut self) {
        if !self.retained && Reaper::is_available() {
            if let Some(window_id) =
                Reaper::get().window_id_for_hwnd(self.hwnd)
            {
                let _ =
                    Reaper::get_mut().unregister_window_handler(&window_id);
            }
            let swell = Reaper::get().swell();
            if unsafe { swell.IsWindow(self.hwnd) } {
                unsafe { swell.DestroyWindow(self.hwnd) };
            }
        }
    }
}

/// Selects the REAPER action sections in which a custom action is registered.
///
/// `Global` registers the action in every built-in section currently supported
/// by this crate. REAPER does not make a custom action globally available by
/// registering it only with `gaccel_global`; section registrations are needed
/// for editor-independent invocation.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum ActionSections {
    #[default]
    Global,
    Section(Section),
    Sections(Vec<Section>),
}

impl ActionSections {
    fn into_vec(self) -> Vec<Section> {
        let sections = match self {
            Self::Global => vec![
                Section::Main,
                // Section::MainAlt,
                Section::MediaExplorer,
                Section::MidiEditor,
                Section::MidiEventListEditor,
                Section::MidiInlineEditor,
            ],
            Self::Section(section) => vec![section],
            Self::Sections(sections) => sections,
        };

        let mut unique_sections = Vec::with_capacity(sections.len());
        for section in sections {
            if !unique_sections
                .iter()
                .any(|existing: &Section| existing.id() == section.id())
            {
                unique_sections.push(section);
            }
        }
        unique_sections
    }
}

impl From<Section> for ActionSections {
    fn from(section: Section) -> Self {
        Self::Section(section)
    }
}

impl From<Vec<Section>> for ActionSections {
    fn from(sections: Vec<Section>) -> Self {
        Self::Sections(sections)
    }
}

/// Options controlling custom action registration.
///
/// By default, an action is registered in all supported REAPER sections and
/// has no default keyboard shortcut. Use [`ActionRegistrationOptions::new`]
/// to choose sections and
/// [`ActionRegistrationOptions::with_default_key_binding`] to add a default
/// binding for the Main section.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct ActionRegistrationOptions {
    pub sections: ActionSections,
    pub default_key_binding: Option<KeyBinding>,
}

impl ActionRegistrationOptions {
    /// Creates registration options for the supplied action sections.
    pub fn new(sections: impl Into<ActionSections>) -> Self {
        Self {
            sections: sections.into(),
            default_key_binding: None,
        }
    }

    /// Adds a default keyboard shortcut to the action's Main-section binding.
    pub fn with_default_key_binding(
        mut self,
        key_binding: KeyBinding,
    ) -> Self {
        self.default_key_binding = Some(key_binding);
        self
    }
}

impl From<ActionSections> for ActionRegistrationOptions {
    fn from(sections: ActionSections) -> Self {
        Self::new(sections)
    }
}
impl Action {
    /// Invokes the action callback with the supplied REAPER invocation
    /// context.
    pub fn call(&self, hook: &mut ActionHook) -> Result<(), anyhow::Error> {
        (self.operation)(hook)
    }

    pub fn command_id(&self) -> CommandId {
        self.bindings
            .first()
            .expect("an action must have at least one binding")
            .command_id
    }

    pub fn command_id_for(&self, section: &Section) -> Option<CommandId> {
        self.bindings
            .iter()
            .find(|binding| binding.section == *section)
            .map(|binding| binding.command_id)
    }

    pub fn kind(&self) -> &ActionKind {
        &self.kind
    }

    pub fn kind_mut(&mut self) -> &mut ActionKind {
        &mut self.kind
    }
}

/// Context passed to a custom action callback.
///
/// The values mirror the arguments supplied by REAPER's `hookcommand2`
/// callback. The hook also exposes the action's mutable toggle state.
pub struct ActionHook<'a> {
    section: KbdSectionInfo,
    command_id: CommandId,
    val: i32,
    val2: i32,
    relmode: i32,
    hwnd: Option<Hwnd>,
    flag: i32,
    kind: &'a mut ActionKind,
}

impl<'a> ActionHook<'a> {
    fn new(
        section: KbdSectionInfo,
        command_id: CommandId,
        val: i32,
        val2: i32,
        relmode: i32,
        hwnd: Option<Hwnd>,
        flag: i32,
        kind: &'a mut ActionKind,
    ) -> Self {
        Self {
            section,
            command_id,
            val,
            val2,
            relmode,
            hwnd,
            flag,
            kind,
        }
    }

    /// Returns the keyboard/action section in which the action was invoked.
    ///
    /// The section identifies the action context, such as the main, MIDI
    /// editor, or media explorer section. Its numeric ID is available through
    /// [`KbdSectionInfo::unique_id`].
    pub fn section(&self) -> &KbdSectionInfo {
        &self.section
    }

    /// Returns the command ID of the invoked action.
    pub fn command_id(&self) -> CommandId {
        self.command_id
    }

    /// Returns the primary event value supplied by REAPER.
    ///
    /// For MIDI CC events this is in the range `0..=127`. For MIDI pitch,
    /// OSC, and mouse-wheel events, interpret it together with [`Self::val2`]
    /// according to the event source.
    pub fn val(&self) -> i32 {
        self.val
    }

    /// Returns the secondary event value supplied by REAPER.
    ///
    /// For MIDI CC events this is `-1`. For MIDI pitch and OSC events, a
    /// non-negative value combines with [`Self::val`] as
    /// `(val2 | (val << 7)) / 16383.0` to produce the normalized value.
    pub fn val2(&self) -> i32 {
        self.val2
    }

    /// Returns the relative-mode value for the event.
    ///
    /// `0` means absolute mode; `1`, `2`, and `3` identify REAPER's relative
    /// adjustment modes.
    pub fn relmode(&self) -> i32 {
        self.relmode
    }

    /// Returns the source window, when REAPER supplied one.
    ///
    /// The value is `None` when the native `HWND` is null, which is common for
    /// MIDI and other non-window input.
    pub fn hwnd(&self) -> Option<Hwnd> {
        self.hwnd
    }

    /// Returns the event flag passed through the action hook.
    pub fn flag(&self) -> i32 {
        self.flag
    }

    /// Returns the current toggle state, if this action is toggleable.
    pub fn toggle_state(&self) -> Option<bool> {
        self.kind.toggle_state()
    }

    /// Sets the action's toggle state and reports whether it is toggleable.
    pub fn set_toggle_state(&mut self, state: bool) -> bool {
        self.kind.set_toggle_state(state)
    }
}

/// A callback scheduled by REAPER's timer facility.
///
/// Register an implementation with [`Reaper::register_timer`]. Returning an
/// error reports the failure through the library's callback error handling;
/// call [`Timer::stop`] to remove the timer from inside `run`.
pub trait Timer {
    fn run(&mut self) -> Result<(), anyhow::Error>;
    fn id_string(&self) -> String;
    fn interval(&self) -> Duration {
        Duration::from_secs(0)
    }
    fn stop(&mut self) {
        match Reaper::get_mut().unregister_timer(self.id_string()) {
            Ok(_) => (),
            Err(e) => action_error(e.into()),
        };
    }
}

fn action_error(error: anyhow::Error) {
    log::error!("{:#?}\n Backtrace: {:#?}", error, error.backtrace());
    let _ = Reaper::get().show_console_msg(format!(
        "Error while performing action: \n{}",
        error.to_string()
    ));
}

extern "C" fn hookcommand2(
    section: *mut raw::KbdSectionInfo,
    command_id: i32,
    val: i32,
    val2: i32,
    relmode: i32,
    hwnd: raw::HWND,
) -> bool {
    let Some(section) = KbdSectionInfo::from_raw(section) else {
        return false;
    };
    let section_id = section.unique_id();
    let matched_index = Reaper::get().actions.iter().position(|action| {
        action.bindings.iter().any(|binding| {
            binding.section.id() == section_id.get()
                && binding.command_id.get() == command_id as u32
        })
    });
    let Some(index) = matched_index else {
        return false;
    };
    let mut action = Reaper::get_mut().actions.remove(index);
    if action.in_flight {
        Reaper::get_mut().actions.insert(index, action);
        return true;
    }
    if action.disabled {
        Reaper::get_mut().actions.insert(index, action);
        return true;
    }
    let action_identity = action
        .bindings
        .iter()
        .map(|binding| (binding.section.id(), binding.command_id.get()))
        .collect::<Vec<_>>();
    let command_id = CommandId::new(command_id as u32);
    let operation_ref = &action.operation;
    let kind = &mut action.kind;
    let mut section = Some(section);
    let mut operation = || {
        let mut hook = ActionHook::new(
            section.take().expect("action callback invoked once"),
            command_id,
            val,
            val2,
            relmode,
            NonNull::new(hwnd),
            0,
            kind,
        );
        operation_ref(&mut hook)
    };
    let _result = dispatch_registered_action(
        &mut action.disabled,
        &mut action.in_flight,
        &mut operation,
    );
    let actions = &mut Reaper::get_mut().actions;
    let replacement_index = actions.iter().position(|candidate| {
        candidate
            .bindings
            .iter()
            .map(|binding| (binding.section.id(), binding.command_id.get()))
            .collect::<Vec<_>>()
            == action_identity
    });
    if let Some(replacement_index) = replacement_index {
        actions[replacement_index].disabled |= action.disabled;
    } else {
        actions.insert(index.min(actions.len()), action);
    }
    true
}

extern "C" fn toggle_action_hook(command_id: i32) -> i32 {
    Reaper::get()
        .actions
        .iter()
        .find(|action| {
            action
                .bindings
                .iter()
                .any(|binding| binding.command_id.get() == command_id as u32)
        })
        .filter(|action| !action.disabled)
        .and_then(|action| action.kind.toggle_state())
        .map_or(-1, |checked| i32::from(checked))
}

extern "C" fn timer_f() {
    // Snapshot identities first: timer callbacks may register/unregister
    // timers or recursively enter the host timer hook.
    let timers: Vec<_> = Reaper::get()
        .timers
        .iter()
        .map(|(id, (last_time, timer))| {
            (id.clone(), *last_time, Arc::clone(timer))
        })
        .collect();

    for (id, last_time, timer) in timers {
        if !Reaper::get_mut().active_timer_callbacks.insert(id.clone()) {
            continue;
        }
        let interval_result = swell_gui::events::invoke_callback(
            "plugin timer interval",
            &mut || Ok(timer.borrow().interval()),
        );
        if Reaper::get_mut().pending_timer_removals.remove(&id) {
            Reaper::get_mut().active_timer_callbacks.remove(&id);
            let _ = Reaper::get_mut().unregister_timer(id.clone());
            continue;
        }
        let interval = match interval_result {
            Ok(interval) => interval,
            Err(_) => {
                Reaper::get_mut().active_timer_callbacks.remove(&id);
                remove_failed_timer(&id, &timer);
                continue;
            }
        };
        let now = Instant::now();
        if now.duration_since(last_time) <= interval {
            Reaper::get_mut().active_timer_callbacks.remove(&id);
            if Reaper::get_mut().pending_timer_removals.remove(&id) {
                let _ = Reaper::get_mut().unregister_timer(id.clone());
            }
            continue;
        }
        let result =
            swell_gui::events::invoke_callback("plugin timer", &mut || {
                timer.borrow_mut().run()
            });
        Reaper::get_mut().active_timer_callbacks.remove(&id);
        if Reaper::get_mut().pending_timer_removals.remove(&id) {
            let _ = Reaper::get_mut().unregister_timer(id.clone());
            continue;
        }
        if result.is_err() {
            remove_failed_timer(&id, &timer);
            continue;
        }

        if let Some((registered_at, registered_timer)) =
            Reaper::get_mut().timers.get_mut(&id)
        {
            if Arc::ptr_eq(registered_timer, &timer) {
                *registered_at = now;
            }
        }
    }
}

fn remove_failed_timer(id: &str, failed_timer: &Arc<RefCell<dyn Timer>>) {
    let reaper = Reaper::get_mut();
    let is_same_timer = reaper
        .timers
        .get(id)
        .is_some_and(|(_, registered)| Arc::ptr_eq(registered, failed_timer));
    if is_same_timer {
        let _ = reaper.unregister_timer(id.to_owned());
    }
}

fn dispatch_registered_action(
    disabled: &mut bool,
    in_flight: &mut bool,
    mut operation: impl FnMut() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    if *disabled || *in_flight {
        return Ok(());
    }
    *in_flight = true;
    let result = swell_gui::events::invoke_callback(
        "registered action",
        &mut operation,
    );
    *in_flight = false;
    if result.is_err() {
        *disabled = true;
    }
    result
}

#[cfg(test)]
mod callback_dispatch_tests {
    use super::{dispatch_registered_action, Action, ActionKind};
    use std::cell::Cell;

    fn action() -> Action {
        Action {
            bindings: Vec::new(),
            operation: Box::new(|_| Ok(())),
            kind: ActionKind::NotToggleable,
            disabled: false,
            in_flight: false,
        }
    }

    #[test]
    fn action_failure_disables_owner_and_later_calls_are_skipped() {
        let mut action = action();
        let calls = Cell::new(0);
        let result = dispatch_registered_action(
            &mut action.disabled,
            &mut action.in_flight,
            || {
                calls.set(calls.get() + 1);
                Err(anyhow::anyhow!("action failed"))
            },
        );
        assert!(result.is_err());
        assert!(action.disabled);
        assert_eq!(calls.get(), 1);
        assert!(dispatch_registered_action(
            &mut action.disabled,
            &mut action.in_flight,
            || Ok(()),
        )
        .is_ok());
        assert_eq!(calls.get(), 1);
    }
}

#[cfg(test)]
mod action_registration_tests {
    use super::{ActionKind, ActionRegistrationOptions};
    use crate::misc_enums::Section;

    #[test]
    fn registration_options_deduplicate_sections_before_native_calls() {
        let options = ActionRegistrationOptions {
            sections: super::ActionSections::Sections(vec![
                Section::Main,
                Section::Main,
            ]),
            default_key_binding: None,
        };
        assert_eq!(options.sections.into_vec().len(), 1);
    }

    #[test]
    fn disabled_action_has_no_toggle_state() {
        let mut action = super::Action {
            bindings: Vec::new(),
            operation: Box::new(|_| Ok(())),
            kind: ActionKind::Toggleable(true),
            disabled: true,
            in_flight: false,
        };
        assert!(action.disabled);
        assert_eq!(action.kind.toggle_state(), Some(true));
        action.disabled = false;
        assert!(!action.disabled);
    }
}

/// Owns high-level REAPER registrations and host integrations for a plugin.
///
/// Construct an instance with [`Reaper::load`] or install the process-wide
/// instance with [`Reaper::init_global`]. Dropping the instance unregisters
/// callbacks, actions, timers, windows, Preferences pages, and control
/// surfaces retained by it.
pub struct Reaper {
    low: rea_rs_low::Reaper,
    swell: Swell,
    actions: Vec<Action>,
    hook2: extern "C" fn(
        *mut raw::KbdSectionInfo,
        i32,
        i32,
        i32,
        i32,
        raw::HWND,
    ) -> bool,
    toggle_action_hook: extern "C" fn(i32) -> i32,
    registrations: Vec<ActionRegistration>,
    default_key_bindings: Vec<DefaultKeyBindingRegistration>,
    custom_menu_hook: Option<(
        Box<dyn FnMut(&swell_gui::CustomMenuContext) -> anyhow::Result<()>>,
        bool,
        bool,
    )>,
    custom_menu_hook_registered: bool,
    custom_menu_hook_in_flight: bool,
    preferences_page: Option<Box<PreferencesPageRegistration>>,
    preferences_page_builder: Option<(
        Box<dyn FnMut(ReaperWindow) -> anyhow::Result<Box<dyn WindowHandler>>>,
        bool,
    )>,
    timers: HashMap<String, (Instant, Arc<RefCell<dyn Timer>>)>,
    active_timer_callbacks: HashSet<String>,
    pending_timer_removals: HashSet<String>,
    pub(crate) windows: HashMap<WindowId, Box<dyn WindowHandler>>,
    pub(crate) window_routes: HashMap<usize, WindowId>,
    pub(crate) dispatching_windows: HashSet<WindowId>,
    pub(crate) window_dispatch_depths: HashMap<WindowId, usize>,
    pending_window_removals: HashSet<WindowId>,
    csurfases: HashMap<
        String,
        (
            Box<Box<dyn IReaperControlSurface>>,
            NonNull<dyn IReaperControlSurface>,
            NonNull<raw::IReaperControlSurface>,
        ),
    >,
    retired_csurfases: Vec<RetiredControlSurface>,
}
impl Reaper {
    /// Loads the REAPER and SWELL APIs for a plugin context.
    pub fn load(context: PluginContext) -> Reaper {
        let low = rea_rs_low::Reaper::load(context);
        let actions = Vec::new();
        let hook2 = hookcommand2;
        let swell = Swell::load(context);
        Swell::make_available_globally(swell);
        let toggle_action_hook = toggle_action_hook;
        let swell = Swell::load(context);
        Swell::make_available_globally(swell);
        unsafe {
            low.plugin_register(
                c_str!("hookcommand2").as_ptr(),
                hook2 as *mut _,
            );
            low.plugin_register(
                c_str!("toggleaction").as_ptr(),
                toggle_action_hook as *mut _,
            );
        }
        Self {
            low,
            swell: swell,
            actions,
            hook2,
            toggle_action_hook,
            registrations: Vec::new(),
            default_key_bindings: Vec::new(),
            custom_menu_hook: None,
            custom_menu_hook_registered: false,
            custom_menu_hook_in_flight: false,
            preferences_page: None,
            preferences_page_builder: None,
            timers: HashMap::new(),
            active_timer_callbacks: HashSet::new(),
            pending_timer_removals: HashSet::new(),
            windows: HashMap::new(),
            window_routes: HashMap::new(),
            dispatching_windows: HashSet::new(),
            window_dispatch_depths: HashMap::new(),
            pending_window_removals: HashSet::new(),
            csurfases: HashMap::new(),
            retired_csurfases: Vec::new(),
        }
    }
    fn make_available_globally(reaper: Reaper) {
        static INIT_INSTANCE: std::sync::Once = std::sync::Once::new();
        unsafe {
            INIT_INSTANCE.call_once(|| {
                INSTANCE = Some(reaper);
                register_plugin_destroy_hook(|| INSTANCE = None);
            });
        }
    }

    /// Loads and installs the process-wide [`Reaper`] instance.
    pub fn init_global(context: PluginContext) -> &'static mut Reaper {
        let instance = Self::load(context);
        Self::make_available_globally(instance);
        Self::get_mut()
    }

    /// Returns the low-level REAPER API wrapper owned by this instance.
    pub fn low(&self) -> &rea_rs_low::Reaper {
        &self.low
    }

    /// Registers a callback for REAPER customizable-menu initialization and
    /// display phases. The supplied menu handle is borrowed for the callback.
    pub fn register_custom_menu_hook(
        &mut self,
        callback: impl FnMut(&swell_gui::CustomMenuContext) -> anyhow::Result<()>
            + 'static,
    ) -> anyhow::Result<()> {
        if self.custom_menu_hook_registered {
            anyhow::bail!("a custom menu hook is already registered");
        }
        self.custom_menu_hook = Some((Box::new(callback), false, false));
        self.custom_menu_hook_registered = true;
        unsafe {
            self.low.plugin_register(
                c_str!("hookcustommenu").as_ptr(),
                custom_menu_hook as *mut _,
            );
        }
        Ok(())
    }

    /// Requests that REAPER add its main Extensions menu.
    pub fn ensure_extensions_menu(&self) -> bool {
        self.low.AddExtensionsMainMenu()
    }

    /// Registers one REAPER Preferences page for this plugin. The page
    /// builder receives a non-owning high-level wrapper for a child window
    /// parented by REAPER and must return its boxed [`WindowHandler`].
    /// REAPER's ABI has no per-page userdata or destroy hook, so this API
    /// intentionally permits one page per plugin instance.
    pub fn register_preferences_page(
        &mut self,
        id: &str,
        display_name: &str,
        builder: impl FnMut(ReaperWindow) -> anyhow::Result<Box<dyn WindowHandler>>
            + 'static,
    ) -> anyhow::Result<()> {
        trace!(
            "registering Preferences page: id={id:?} display_name={display_name:?}"
        );
        if self.preferences_page.is_some() {
            anyhow::bail!(
                "only one Preferences page can be registered per plugin"
            );
        }
        let id = CString::new(id)?;
        let display_name = CString::new(display_name)?;
        self.preferences_page_builder = Some((Box::new(builder), false));
        let mut registration = raw::prefs_page_register_t::default();
        registration.idstr = id.as_ptr();
        registration.displayname = display_name.as_ptr();
        registration.create = Some(create_preferences_page);
        // REAPER retains this registration record and writes back fields such
        // as `treeitem` and `hwndCache`. Keep the record at a stable address
        // before handing its pointer to the host.
        registration.par_id = 0;
        registration.par_idstr = std::ptr::null();
        registration.childrenFlag = 0;
        registration.treeitem = std::ptr::null_mut();
        registration.hwndCache = std::ptr::null_mut();
        let mut retained = Box::new(PreferencesPageRegistration {
            _id: id,
            _display_name: display_name,
            registration,
        });
        unsafe {
            let registration_ptr =
                &mut retained.registration as *mut _ as *mut std::ffi::c_void;
            trace!(
                "calling REAPER prefpage registration: registration={registration_ptr:p} create_callback={:p} children_flag={} parent_id={}",
                create_preferences_page as *const (),
                retained.registration.childrenFlag,
                retained.registration.par_id,
            );
            let result = self.low.plugin_register(
                c_str!("prefpage").as_ptr(),
                registration_ptr,
            );
            trace!(
                "REAPER prefpage registration returned {result}: registration={registration_ptr:p} treeitem={:p} hwnd_cache={:p}",
                retained.registration.treeitem,
                retained.registration.hwndCache,
            );
            if result == 0 {
                self.preferences_page_builder = None;
                anyhow::bail!("REAPER rejected Preferences page registration");
            }
        }
        self.preferences_page = Some(retained);
        Ok(())
    }

    fn create_preferences_page_window(
        &self,
        parent: raw::HWND,
    ) -> crate::ReaperResult<ReaperWindow> {
        let parent_valid =
            !parent.is_null() && unsafe { self.swell.IsWindow(parent) };
        trace!(
            "validating Preferences page parent: parent={parent:p} valid={parent_valid}"
        );
        if !parent_valid {
            return Err(ReaRsError::InvalidObject(
                "Preferences page parent is not a valid window",
            ));
        }
        // SWELL synchronously sends WM_CREATE to the supplied procedure when
        // creating this child. Passing None leaves SWELL with a null WndProc
        // and can crash before create_child_window returns.
        // Give the child a distinct base procedure. Handler registration
        // subclasses the HWND and must save a procedure that can safely be
        // called as the fallback; saving `window_proc` itself would recurse
        // once the host route is installed.
        let procedure = Some(preferences_page_base_proc as _);
        trace!(
            "creating Preferences page child with base procedure: parent={parent:p} callback={:p}",
            preferences_page_base_proc as *const (),
        );
        let hwnd = unsafe {
            self.swell.create_child_window(parent, 1, 1, procedure, 0)
        }
        .ok_or(crate::ReaRsError::NullPtr("Preferences page window"))?;
        trace!(
            "created Preferences page child HWND: parent={parent:p} hwnd={hwnd:p}"
        );
        ReaperWindow::from_hwnd(hwnd)
    }
    /// Returns the low-level SWELL API wrapper owned by this instance.
    pub fn swell(&self) -> &rea_rs_low::Swell {
        &self.swell
    }

    /// Creates a new owned top-level window parented to REAPER's main window.
    pub fn create_window(
        &self,
        spec: &WindowSpec,
    ) -> ReaperResult<ReaperWindow> {
        let parent =
            self.low.pointers().GetMainHwnd.as_ref().ok_or_else(|| {
                ReaRsError::UnexpectedAPI("GetMainHwnd not available".into())
            })?;
        let parent = parent();
        if parent.is_null() {
            return Err(ReaRsError::NullPtr("main window"));
        }
        let hwnd = unsafe {
            self.swell.create_window(
                parent,
                &spec.title,
                spec.width,
                spec.height,
                spec.resizable,
                spec.no_minimize,
                spec.no_close,
                self.plugin_context().h_instance(),
                Some(swell_gui::window_proc),
                0,
            )
        }
        .ok_or(ReaRsError::NullPtr("window"))?;
        trace!(
            "native REAPER window created with window_proc: title={:?} hwnd={hwnd:p} parent={parent:p} valid={} visible={} callback={:p}",
            spec.title,
            unsafe { self.swell.IsWindow(hwnd) },
            unsafe { self.swell.IsWindowVisible(hwnd) },
            swell_gui::window_proc as *const (),
        );
        let window = ReaperWindow::owned(
            hwnd,
            spec.allow_show,
            spec.dock_ident.clone(),
            spec.min_size,
        )?;
        let mut client = raw::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe { self.swell.GetClientRect(hwnd, &mut client) };
        log::debug!(
            "created window {:?}: requested={}x{} client={}x{}",
            spec.title,
            spec.width,
            spec.height,
            client.right - client.left,
            client.bottom - client.top,
        );
        // Capture the initial floating geometry before the HWND can ever be
        // reparented into a docker. This is the fallback restore rectangle
        // for the first dock -> float transition.
        window.remember_floating_rect()?;
        trace!(
            "window initial placement captured: title={:?} hwnd={hwnd:p} visible={}",
            spec.title,
            unsafe { self.swell.IsWindowVisible(hwnd) },
        );
        Ok(window)
    }

    /// Registers a handler for either an owned window or a non-owning host
    /// window wrapper. Returns the stable logical ID used for teardown.
    pub fn register_window_handler(
        &mut self,
        handler: Box<dyn WindowHandler>,
    ) -> ReaperResult<WindowId> {
        let window = handler.window();
        let window_id = handler.window_id();
        if self.windows.contains_key(&window_id) {
            return Err(ReaRsError::InvalidObject(
                "window handler ID is already registered",
            ));
        }
        let hwnd = window.hwnd();
        let hwnd_key = hwnd as usize;
        let parent = unsafe { self.swell.GetParent(hwnd) };
        let valid = !hwnd.is_null() && unsafe { self.swell.IsWindow(hwnd) };
        log::debug!(
            "register window handler: id={window_id:?} hwnd={hwnd:p} parent={parent:p} owned={} valid={valid} existing_route={:?}",
            window.is_owned(),
            self.window_routes.get(&hwnd_key),
        );
        if hwnd.is_null()
            || self.window_routes.contains_key(&hwnd_key)
            || !valid
        {
            return Err(ReaRsError::InvalidObject(
                "window is invalid or already attached to a handler",
            ));
        }
        let show_on_register = handler.window().show_on_register;
        let owned = window.is_owned();
        trace!(
            "window registration preparation: id={window_id:?} hwnd={hwnd:p} owned={owned} show_on_register={show_on_register} visible={} parent={parent:p}",
            unsafe { self.swell.IsWindowVisible(hwnd) },
        );
        if owned {
            handler.window().apply_default_layout()?;
            trace!(
                "window initial layout applied: id={window_id:?} hwnd={hwnd:p} valid={} visible={}",
                unsafe { self.swell.IsWindow(hwnd) },
                unsafe { self.swell.IsWindow(hwnd) && self.swell.IsWindowVisible(hwnd) },
            );
        }
        self.windows.insert(window_id.clone(), handler);
        self.window_routes.insert(hwnd_key, window_id.clone());
        if !owned {
            trace!(
                "installing window_proc on host-owned HWND: id={window_id:?} hwnd={hwnd:p} callback={:p}",
                swell_gui::window_proc as *const (),
            );
            let previous = unsafe {
                self.swell.SetWindowLong(
                    hwnd,
                    raw::GWL_WNDPROC,
                    swell_gui::window_proc as *const () as usize as isize,
                )
            };
            trace!(
                "window_proc installed on host-owned HWND: id={window_id:?} hwnd={hwnd:p} previous_proc={previous:#x} current_proc={:#x}",
                unsafe { self.swell.GetWindowLong(hwnd, raw::GWL_WNDPROC) },
            );
            swell_gui::host_proc::remember_host_proc(hwnd, previous);
        }
        log::debug!(
            "window handler registered: id={window_id:?} hwnd={hwnd:p} parent={parent:p} owned={owned} route={:?}",
            self.window_routes.get(&hwnd_key),
        );
        self.begin_window_dispatch(&window_id);
        trace!(
            "window open callback begin: id={window_id:?} hwnd={hwnd:p} valid={} visible={}",
            unsafe { self.swell.IsWindow(hwnd) },
            unsafe { self.swell.IsWindow(hwnd) && self.swell.IsWindowVisible(hwnd) },
        );
        let mut opening_handler = self.windows.remove(&window_id);
        if let Some(handler) = opening_handler.as_mut() {
            let _ = swell_gui::windows::invoke_handler_callback(
                handler.as_mut(),
                "window open",
                |handler| handler.on_open(),
            );
        }
        if let Some(handler) = opening_handler {
            self.windows.insert(window_id.clone(), handler);
        }
        trace!(
            "window open callback complete: id={window_id:?} hwnd={hwnd:p} valid={} visible={} pending_removal={}",
            unsafe { self.swell.IsWindow(hwnd) },
            unsafe { self.swell.IsWindow(hwnd) && self.swell.IsWindowVisible(hwnd) },
            self.pending_window_removals.contains(&window_id),
        );
        if show_on_register {
            if let Some(handler) = self.windows.get(&window_id) {
                trace!(
                    "showing registered window: id={window_id:?} hwnd={hwnd:p} valid={} visible_before={}",
                    unsafe { self.swell.IsWindow(hwnd) },
                    unsafe { self.swell.IsWindow(hwnd) && self.swell.IsWindowVisible(hwnd) },
                );
                if let Err(error) = handler.window().show() {
                    trace!(
                        "showing registered window failed: id={window_id:?} hwnd={hwnd:p} valid={} error={error}",
                        unsafe { self.swell.IsWindow(hwnd) },
                    );
                    self.pending_window_removals.insert(window_id.clone());
                    self.finish_window_dispatch(&window_id);
                    return Err(error);
                }
                trace!(
                    "registered window show complete: id={window_id:?} hwnd={hwnd:p} valid={} visible={}",
                    unsafe { self.swell.IsWindow(hwnd) },
                    unsafe { self.swell.IsWindow(hwnd) && self.swell.IsWindowVisible(hwnd) },
                );
            }
        } else if owned {
            if let Some(handler) = self.windows.get(&window_id) {
                trace!(
                    "hiding registered window by spec: id={window_id:?} hwnd={hwnd:p}",
                );
                let _ = handler.window().hide();
            }
        }
        self.finish_window_dispatch(&window_id);
        Ok(window_id)
    }

    pub(crate) fn window_id_for_hwnd(
        &self,
        hwnd: raw::HWND,
    ) -> Option<WindowId> {
        self.window_routes.get(&(hwnd as usize)).cloned()
    }

    pub fn is_window_registered(&self, window_id: &WindowId) -> bool {
        self.windows.contains_key(window_id)
    }

    /// Unregisters a handler by stable ID. Dropping an owned wrapper destroys
    /// its native window; a host wrapper is detached but never destroys HWND.
    pub fn unregister_window_handler(
        &mut self,
        window_id: &WindowId,
    ) -> ReaperResult<()> {
        trace!(
            "window unregister requested: id={window_id:?} dispatching={} active_callbacks={}",
            self.dispatching_windows.contains(window_id),
            self.windows
                .get(window_id)
                .map(|handler| handler.window().active_callbacks.get())
                .unwrap_or_default(),
        );
        if self.dispatching_windows.contains(window_id)
            || self.windows.get(window_id).is_some_and(|handler| {
                handler.window().active_callbacks.get() > 0
            })
        {
            self.pending_window_removals.insert(window_id.clone());
            return Ok(());
        }
        let Some(mut handler) = self.windows.remove(window_id) else {
            log::debug!(
                "unregister window handler skipped: id={window_id:?} not registered"
            );
            return Ok(());
        };
        let hwnd = handler.window().hwnd();
        let owned = handler.window().is_owned();
        let parent = unsafe { self.swell.GetParent(hwnd) };
        log::debug!(
            "unregister window handler begin: requested_id={window_id:?} handler_id={:?} hwnd={hwnd:p} parent={parent:p} owned={} hwnd_valid={}",
            handler.window_id(),
            owned,
            unsafe { self.swell.IsWindow(hwnd) },
        );
        self.window_routes.remove(&(hwnd as usize));
        if !owned {
            swell_gui::host_proc::detach_host_proc(hwnd);
        }
        handler.window().destroy_structural_children();
        let _ = swell_gui::windows::invoke_handler_cleanup_callback(
            handler.as_mut(),
            "window destroy",
            |handler| handler.on_destroy(),
        );
        if owned {
            handler.window().destroy_owned_native();
        }
        drop(handler);
        log::debug!(
            "unregister window handler complete: id={window_id:?} hwnd={hwnd:p} owned={owned} hwnd_valid_after={}",
            unsafe { self.swell.IsWindow(hwnd) },
        );
        self.pending_window_removals.remove(window_id);
        Ok(())
    }

    pub(crate) fn finish_window_dispatch(&mut self, window_id: &str) {
        let depth = self.window_dispatch_depths.get_mut(window_id);
        if let Some(depth) = depth {
            *depth = depth.saturating_sub(1);
            if *depth > 0 {
                return;
            }
        }
        self.window_dispatch_depths.remove(window_id);
        self.dispatching_windows.remove(window_id);
        if self.pending_window_removals.remove(window_id) {
            let window_id = window_id.to_owned();
            if let Err(error) = self.unregister_window_handler(&window_id) {
                log::error!(
                    "deferred window cleanup failed: id={window_id:?} error={error}"
                );
            }
        }
    }

    pub(crate) fn begin_window_dispatch(&mut self, window_id: &str) {
        *self
            .window_dispatch_depths
            .entry(window_id.to_owned())
            .or_default() += 1;
        self.dispatching_windows.insert(window_id.to_owned());
    }

    /// Compatibility helper: resolves a native HWND to its stable ID.
    #[deprecated(
        note = "use unregister_window_handler with the WindowId returned by registration"
    )]
    pub fn unregister_window_handler_by_hwnd(
        &mut self,
        hwnd: raw::HWND,
    ) -> ReaperResult<()> {
        let id = self.window_id_for_hwnd(hwnd).ok_or_else(|| {
            ReaRsError::Key(format!("{:p}", hwnd), String::new())
        })?;
        self.unregister_window_handler(&id)
    }

    /// Returns a clone of the plugin context used to load this instance.
    pub fn plugin_context(&self) -> PluginContext {
        self.low.plugin_context().clone()
    }

    /// Returns whether a process-wide [`Reaper`] instance is available.
    pub fn is_available() -> bool {
        #[allow(static_mut_refs)]
        unsafe {
            INSTANCE.is_some()
        }
    }
    /// Gives access to the instance which you made available globally before.
    ///
    /// # Panics
    ///
    /// This panics if [`make_available_globally()`] has not been called
    /// before.
    ///
    /// [`make_available_globally()`]: fn.make_available_globally.html
    pub fn get() -> &'static Reaper {
        #[allow(static_mut_refs)]
        unsafe {
            INSTANCE
                .as_ref()
                .expect("call `load(context)` before using `get()`")
        }
    }
    /// Returns mutable access to the process-wide instance.
    ///
    /// # Panics
    ///
    /// Panics when no global instance has been initialized.
    pub fn get_mut() -> &'static mut Reaper {
        #[allow(static_mut_refs)]
        unsafe {
            INSTANCE
                .as_mut()
                .expect("call `load(context)` before using `get()`")
        }
    }

    /// Registers a timer callback, replacing any timer with the same ID.
    pub fn register_timer(&mut self, timer: Arc<RefCell<dyn Timer>>) {
        let string = timer.borrow().id_string();
        self.timers.insert(string, (Instant::now(), timer));
        if self.timers.len() == 1 {
            unsafe {
                self.low().plugin_register(
                    c_str!("timer").as_ptr(),
                    timer_f as *mut _,
                )
            };
        }
    }
    /// Unregisters a timer by its stable string ID.
    pub fn unregister_timer(&mut self, id_string: String) -> ReaperResult<()> {
        // A timer snapshot owns an Arc until its callback returns, so self-
        // unregistration drops registry membership but not the active target.
        if self.active_timer_callbacks.contains(&id_string) {
            self.pending_timer_removals.insert(id_string);
            return Ok(());
        }
        match self.timers.remove(&id_string) {
            Some(_) => {
                if self.timers.len() == 0 {
                    unsafe {
                        self.low().plugin_register(
                            c_str!("timer").as_ptr(),
                            timer_f as *mut _,
                        );
                    }
                }
                Ok(())
            }
            None => Err(crate::ReaRsError::InvalidObject(
                "No timer with the given string",
            )),
        }
    }

    pub fn register_action(
        &mut self,
        id_string: &'static str,
        description: &'static str,
        kind: ActionKind,
        operation: impl Fn(&mut ActionHook) -> Result<(), anyhow::Error> + 'static,
        options: impl Into<Option<ActionRegistrationOptions>>,
    ) -> Result<RegisteredAction, anyhow::Error> {
        let options = options.into().unwrap_or_default();
        let sections = options.sections.into_vec();
        let name = CString::new(description)?;
        let mut bindings = Vec::new();
        for section in sections {
            let binding_id = CString::new(format!(
                "{}_section_{}",
                id_string,
                section.id()
            ))?;
            let is_main_default_binding = section == Section::Main
                && options.default_key_binding.is_some();
            if is_main_default_binding {
                let command_id = unsafe {
                    self.low.plugin_register(
                        c_str!("command_id").as_ptr(),
                        binding_id.as_ptr() as _,
                    )
                };
                if command_id == 0 {
                    return Err(anyhow::anyhow!(
                        "failed to register action command in section {}",
                        section.id()
                    ));
                }
                bindings.push(ActionBinding {
                    section,
                    command_id: CommandId::new(command_id as u32),
                });
                continue;
            }
            let mut registration = raw::custom_action_register_t {
                uniqueSectionId: section.id() as i32,
                idStr: binding_id.as_ptr(),
                name: name.as_ptr(),
                extra: std::ptr::null_mut(),
            };
            let command_id = unsafe {
                self.low.plugin_register(
                    c_str!("custom_action").as_ptr(),
                    &mut registration as *mut _ as _,
                )
            };
            if command_id == 0 {
                return Err(anyhow::anyhow!(
                    "failed to register action in section {}",
                    section.id()
                ));
            }
            bindings.push(ActionBinding {
                section: section.clone(),
                command_id: CommandId::new(command_id as u32),
            });
            self.registrations.push(ActionRegistration {
                _section: section,
                _id: binding_id,
                _name: name.clone(),
                registration,
            });
        }
        if let Some(key_binding) = options.default_key_binding {
            let main_command_id = bindings
                .iter()
                .find(|binding| binding.section == Section::Main)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "a default key binding requires the Main section"
                    )
                })?
                .command_id;
            let description = CString::new(description)?;
            let registration = raw::gaccel_register_t {
                accel: raw::ACCEL {
                    fVirt: key_binding.fvirt.bits(),
                    key: key_binding.key,
                    cmd: main_command_id.get() as u16,
                },
                desc: description.as_ptr(),
            };
            let registration_name = c_str!("gaccel");

            //     if key_binding.fvirt.contains(FVirt::FVIRTKEY) {
            //         c_str!("gaccel_global")
            //     } else {
            //         c_str!("gaccel_globaltext")
            //     };
            unsafe {
                self.low.plugin_register(
                    registration_name.as_ptr(),
                    &registration as *const _ as _,
                );
            }
            self.default_key_bindings
                .push(DefaultKeyBindingRegistration {
                    _description: description,
                    registration,
                    global_text: !key_binding.fvirt.contains(FVirt::FVIRTKEY),
                });
        }
        let action = Action {
            bindings,
            operation: Box::new(operation),
            kind,
            disabled: false,
            in_flight: false,
        };
        self.actions.push(action);
        let command_id = self.actions.last().unwrap().command_id();
        Ok(RegisteredAction { command_id })
    }

    pub fn set_action_toggle_state(
        &mut self,
        command_id: CommandId,
        state: bool,
    ) -> bool {
        self.actions
            .iter_mut()
            .find(|action| {
                action
                    .bindings
                    .iter()
                    .any(|binding| binding.command_id == command_id)
            })
            .is_some_and(|action| action.kind.set_toggle_state(state))
    }

    pub fn register_control_surface(
        &mut self,
        csurf: Arc<RefCell<dyn ControlSurface>>,
    ) {
        let id_string = csurf.borrow().get_type_string();
        if self.csurfases.contains_key(&id_string)
            || self
                .retired_csurfases
                .iter()
                .any(|retired| retired.id == id_string)
        {
            log::error!("control surface already registered or retained: id={id_string:?}");
            return;
        }
        let mut low_cs: Box<dyn IReaperControlSurface> =
            Box::new(ControlSurfaceWrap::new(csurf));
        // Create thin pointer of low_cs before making it a trait
        // object (for being able to restore the original
        // low_cs later).
        let low_cs_thin_ptr = NonNull::new(low_cs.as_mut()).expect("null");
        // Create the C++ counterpart surface (we need to box the Rust side
        // twice in order to obtain a thin pointer for passing it to
        // C++ as callback target).
        let double_boxed_low_cs = Box::new(low_cs);
        let cpp_cs = unsafe {
            create_cpp_to_rust_control_surface(
                double_boxed_low_cs.as_ref().into(),
            )
        };
        debug!(
            "control surface wrapper created: id={id_string:?} rust={:p} cpp={:p}",
            low_cs_thin_ptr.as_ptr(),
            cpp_cs.as_ptr(),
        );
        let s = c_str!("csurf_inst");
        let ret = unsafe {
            self.low().plugin_register(s.as_ptr(), cpp_cs.as_ptr() as _)
        };

        debug!("control surface registered: id={id_string:?} result={ret} cpp={:p}", cpp_cs.as_ptr());
        self.csurfases.insert(
            id_string.clone(),
            (double_boxed_low_cs, low_cs_thin_ptr, cpp_cs),
        );
        debug!(
            "control surface retained: id={id_string:?} active={}",
            self.csurfases.len()
        );
    }

    pub fn has_control_surface(&self, id_string: &String) -> bool {
        self.csurfases.contains_key(id_string)
    }

    pub fn unregister_control_surface(
        &mut self,
        id_string: String,
    ) -> Result<(), ReaRsError> {
        // Detach immediately, but retain both the C++ wrapper and Rust target
        // until plugin teardown. The low-level API cannot identify a safe
        // post-virtual-return point: REAPER may run timers during nested
        // loops. Keep the Rust callback target alive until REAPER has
        // stopped using the detached C++ wrapper.
        let cpp_cs = self
            .csurfases
            .get(&id_string)
            .map(|(_, _, cpp_cs)| *cpp_cs)
            .ok_or_else(|| {
                ReaRsError::Key(
                    id_string.clone(),
                    format!("{:#?}", self.csurfases.keys()),
                )
            })?;
        debug!(
            "control surface unregister begin: id={id_string:?} cpp={:p}",
            cpp_cs.as_ptr(),
        );
        let result = unsafe {
            self.low().plugin_register(
                c_str!("-csurf_inst").as_ptr(),
                cpp_cs.as_ptr() as _,
            )
        };
        debug!(
            "control surface detached and retained until plugin teardown: id={id_string:?} result={result} cpp={:p}",
            cpp_cs.as_ptr(),
        );
        let retained = self
            .csurfases
            .remove(&id_string)
            .expect("control surface checked above");
        self.retired_csurfases.push(RetiredControlSurface {
            id: id_string,
            rust_owner: retained.0,
            cpp_owner: Some(retained.2),
        });
        Ok(())
    }
}
impl Drop for Reaper {
    fn drop(&mut self) {
        let low = self.low().clone();
        unsafe {
            if let Some(page) = self.preferences_page.as_mut() {
                low.plugin_register(
                    c_str!("-prefpage").as_ptr(),
                    &mut page.registration as *mut _ as _,
                );
            }
        }
        let windows = std::mem::take(&mut self.windows);
        self.window_routes.clear();
        for (id, handler) in windows {
            let window = handler.window();
            if !window.is_owned() {
                swell_gui::host_proc::detach_host_proc(window.hwnd());
            } else {
                window.destroy_internal();
            }
            window.destroy_structural_children();
            let mut handler = handler;
            let _ = swell_gui::windows::invoke_handler_cleanup_callback(
                handler.as_mut(),
                "window destroy during plugin teardown",
                |handler| handler.on_destroy(),
            );
            drop(handler);
            let _ = id;
        }
        unsafe {
            if self.custom_menu_hook_registered {
                low.plugin_register(
                    c_str!("-hookcustommenu").as_ptr(),
                    custom_menu_hook as *mut _,
                );
                self.custom_menu_hook_registered = false;
            }
            low.plugin_register(
                c_str!("-hookcommand2").as_ptr(),
                self.hook2 as *mut _,
            );
            low.plugin_register(
                c_str!("-toggleaction").as_ptr(),
                self.toggle_action_hook as *mut _,
            );
            low.plugin_register(c_str!("-timer").as_ptr(), timer_f as *mut _);
            for (_, (_, _, cpp_surface)) in self.csurfases.drain() {
                let _ = low.plugin_register(
                    c_str!("-csurf_inst").as_ptr(),
                    cpp_surface.as_ptr() as _,
                );
            }
        }
        self.preferences_page_builder.take();
        self.preferences_page.take();
        self.custom_menu_hook.take();
        for registration in self.registrations.iter_mut() {
            unsafe {
                low.plugin_register(
                    c_str!("-custom_action").as_ptr(),
                    &mut registration.registration as *mut _ as _,
                )
            };
        }
        for registration in self.default_key_bindings.iter_mut() {
            let registration_name = c_str!("-gaccel_globaltext");
            // if registration.global_text {
            //     c_str!("-gaccel_globaltext")
            // } else {
            //     c_str!("-gaccel_global")
            // };
            unsafe {
                low.plugin_register(
                    registration_name.as_ptr(),
                    &mut registration.registration as *mut _ as _,
                );
            }
        }
    }
}

/// Numeric command identifier assigned by REAPER.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct CommandId {
    id: u32,
}
impl CommandId {
    /// Creates an identifier from REAPER's numeric command ID.
    pub fn new(id: u32) -> Self {
        Self { id }
    }
    /// Returns the underlying numeric command ID.
    pub fn get(&self) -> u32 {
        self.id
    }
}
impl From<u32> for CommandId {
    fn from(id: u32) -> Self {
        Self { id }
    }
}
impl Into<u32> for CommandId {
    fn into(self) -> u32 {
        self.id
    }
}

/// Numeric identifier of a REAPER action section.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
)]
pub struct SectionId {
    id: u32,
}
impl SectionId {
    /// Creates a section identifier from its numeric value.
    pub fn new(id: u32) -> Self {
        Self { id }
    }
    /// Returns the underlying numeric section ID.
    pub fn get(&self) -> u32 {
        self.id
    }
}
impl From<u32> for SectionId {
    fn from(id: u32) -> Self {
        Self { id }
    }
}
impl Into<u32> for SectionId {
    fn into(self) -> u32 {
        self.id
    }
}

#[derive(Debug, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize)]
/// Result of registering a custom action.
///
/// The command ID identifies the binding in the first registered section.
pub struct RegisteredAction {
    // For identifying the registered command (= the functions to be executed)
    pub command_id: CommandId,
}

/// Describes whether an action exposes a toggle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    NotToggleable,
    Toggleable(bool),
}

impl ActionKind {
    /// Returns whether the action supports a toggle state.
    pub fn is_toggleable(self) -> bool {
        matches!(self, Self::Toggleable(_))
    }

    /// Returns the current toggle state, or `None` for a non-toggleable
    /// action.
    pub fn toggle_state(self) -> Option<bool> {
        match self {
            Self::Toggleable(state) => Some(state),
            Self::NotToggleable => None,
        }
    }

    /// Sets the toggle state and reports whether the action is toggleable.
    pub fn set_toggle_state(&mut self, state: bool) -> bool {
        match self {
            Self::Toggleable(current) => {
                *current = state;
                true
            }
            Self::NotToggleable => false,
        }
    }

    /// Flips the toggle state and returns it, or `None` if unsupported.
    pub fn toggle(&mut self) -> Option<bool> {
        match self {
            Self::Toggleable(current) => {
                *current = !*current;
                Some(*current)
            }
            Self::NotToggleable => None,
        }
    }
}

/// Result of registering a default keyboard accelerator.
#[derive(Debug, PartialEq, Eq)]
pub struct RegisteredAccel {
    pub command_id: CommandId,
}
