use log::debug;
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
    collections::HashMap,
    ffi::CString,
    ptr::NonNull,
    sync::Arc,
    time::{self, Duration, Instant},
};

static mut INSTANCE: Option<Reaper> = None;

type ActionCallback = dyn Fn(&mut ActionHook) -> Result<(), anyhow::Error>;

pub struct Action {
    bindings: Vec<ActionBinding>,
    operation: Box<ActionCallback>,
    kind: ActionKind,
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
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct ActionRegistrationOptions {
    pub sections: ActionSections,
    pub default_key_binding: Option<KeyBinding>,
}

impl ActionRegistrationOptions {
    pub fn new(sections: impl Into<ActionSections>) -> Self {
        Self {
            sections: sections.into(),
            default_key_binding: None,
        }
    }

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

    pub fn section(&self) -> &KbdSectionInfo {
        &self.section
    }

    pub fn command_id(&self) -> CommandId {
        self.command_id
    }

    pub fn val(&self) -> i32 {
        self.val
    }

    pub fn val2(&self) -> i32 {
        self.val2
    }

    pub fn relmode(&self) -> i32 {
        self.relmode
    }

    pub fn hwnd(&self) -> Option<Hwnd> {
        self.hwnd
    }

    pub fn flag(&self) -> i32 {
        self.flag
    }

    pub fn toggle_state(&self) -> Option<bool> {
        self.kind.toggle_state()
    }

    pub fn set_toggle_state(&mut self, state: bool) -> bool {
        self.kind.set_toggle_state(state)
    }
}

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
    Reaper::get().show_console_msg(format!(
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
    let actions = &mut Reaper::get_mut().actions;
    for action in actions.iter_mut() {
        if action.bindings.iter().any(|binding| {
            binding.section.id() == section_id.get()
                && binding.command_id.get() == command_id as u32
        }) {
            let operation = &action.operation;
            let mut hook = ActionHook::new(
                section,
                CommandId::new(command_id as u32),
                val,
                val2,
                relmode,
                NonNull::new(hwnd),
                0,
                &mut action.kind,
            );
            match operation(&mut hook) {
                Ok(_) => (),
                Err(e) => action_error(e),
            }
            return true;
        }
    }
    false
}

extern "C" fn toggle_action_hook(command_id: i32) -> i32 {
    let actions = &Reaper::get().actions;
    for action in actions.iter() {
        if action
            .bindings
            .iter()
            .any(|binding| binding.command_id.get() == command_id as u32)
        {
            return match action.kind.toggle_state() {
                Some(true) => 1,
                Some(false) => 0,
                None => -1,
            };
        }
    }
    -1
}

extern "C" fn timer_f() {
    let timers = &mut Reaper::get_mut().timers;
    for (_, (last_time, timer)) in timers.iter_mut() {
        let now = time::Instant::now();
        if now.duration_since(last_time.clone()) > timer.borrow().interval() {
            match timer.borrow_mut().run() {
                Ok(_) => (),
                Err(e) => action_error(e),
            };
            *last_time = now;
        }
    }
}

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
    timers: HashMap<String, (Instant, Arc<RefCell<dyn Timer>>)>,
    pub(crate) windows: HashMap<WindowId, Box<dyn WindowHandler>>,
    pub(crate) window_routes: HashMap<usize, WindowId>,
    csurfases: HashMap<
        String,
        (
            Box<Box<dyn IReaperControlSurface>>,
            NonNull<dyn IReaperControlSurface>,
            NonNull<raw::IReaperControlSurface>,
        ),
    >,
}
impl Reaper {
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
            timers: HashMap::new(),
            windows: HashMap::new(),
            window_routes: HashMap::new(),
            csurfases: HashMap::new(),
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

    pub fn init_global(context: PluginContext) -> &'static mut Reaper {
        let instance = Self::load(context);
        Self::make_available_globally(instance);
        Self::get_mut()
    }

    pub fn low(&self) -> &rea_rs_low::Reaper {
        &self.low
    }
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
        let window = ReaperWindow::owned(
            hwnd,
            spec.allow_show,
            spec.dock_ident.clone(),
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
        if owned {
            handler.window().apply_default_layout()?;
        }
        self.windows.insert(window_id.clone(), handler);
        self.window_routes.insert(hwnd_key, window_id.clone());
        if !owned {
            let previous = unsafe {
                self.swell.SetWindowLong(
                    hwnd,
                    raw::GWL_WNDPROC,
                    swell_gui::window_proc as *const () as usize as isize,
                )
            };
            swell_gui::host_proc::remember_host_proc(hwnd, previous);
        }
        log::debug!(
            "window handler registered: id={window_id:?} hwnd={hwnd:p} parent={parent:p} owned={owned} route={:?}",
            self.window_routes.get(&hwnd_key),
        );
        if let Some(handler) = self.windows.get_mut(&window_id) {
            handler.on_open();
        }
        if show_on_register {
            if let Some(handler) = self.windows.get(&window_id) {
                if let Err(error) = handler.window().show() {
                    self.unregister_window_handler(&window_id)?;
                    return Err(error);
                }
            }
        }
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
        handler.on_destroy();
        if owned {
            handler.window().destroy_owned_native();
        }
        drop(handler);
        log::debug!(
            "unregister window handler complete: id={window_id:?} hwnd={hwnd:p} owned={owned} hwnd_valid_after={}",
            unsafe { self.swell.IsWindow(hwnd) },
        );
        Ok(())
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

    pub fn plugin_context(&self) -> PluginContext {
        self.low.plugin_context().clone()
    }

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
    pub fn get_mut() -> &'static mut Reaper {
        #[allow(static_mut_refs)]
        unsafe {
            INSTANCE
                .as_mut()
                .expect("call `load(context)` before using `get()`")
        }
    }

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
    pub fn unregister_timer(&mut self, id_string: String) -> ReaperResult<()> {
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
        };
        self.actions.push(action);
        Ok(RegisteredAction {
            command_id: self.actions.last().unwrap().command_id(),
        })
    }

    pub fn register_control_surface(
        &mut self,
        csurf: Arc<RefCell<dyn ControlSurface>>,
    ) {
        let id_string = csurf.borrow().get_type_string();
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
        // Keep the Rust callback target alive until REAPER has stopped using
        // the C++ wrapper. Then destroy the wrapper before dropping its Rust
        // backing object.
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
        unsafe {
            let result = self.low().plugin_register(
                c_str!("-csurf_inst").as_ptr(),
                cpp_cs.as_ptr() as _,
            );
            debug!(
                "control surface detached from REAPER: id={id_string:?} result={result} cpp={:p}",
                cpp_cs.as_ptr(),
            );
            delete_cpp_control_surface(cpp_cs);
        }
        self.csurfases.remove(&id_string);
        debug!("control surface dropped: id={id_string:?}");
        Ok(())
    }
}
impl Drop for Reaper {
    fn drop(&mut self) {
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
            handler.on_destroy();
            drop(handler);
            let _ = id;
        }
        let low = self.low().clone();
        unsafe {
            low.plugin_register(
                c_str!("-hookcommand2").as_ptr(),
                self.hook2 as *mut _,
            );
            low.plugin_register(
                c_str!("-toggleaction").as_ptr(),
                self.toggle_action_hook as *mut _,
            );
        }
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
    pub fn new(id: u32) -> Self {
        Self { id }
    }
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
    pub fn new(id: u32) -> Self {
        Self { id }
    }
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
pub struct RegisteredAction {
    // For identifying the registered command (= the functions to be executed)
    pub command_id: CommandId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    NotToggleable,
    Toggleable(bool),
}

impl ActionKind {
    pub fn is_toggleable(self) -> bool {
        matches!(self, Self::Toggleable(_))
    }

    pub fn toggle_state(self) -> Option<bool> {
        match self {
            Self::Toggleable(state) => Some(state),
            Self::NotToggleable => None,
        }
    }

    pub fn set_toggle_state(&mut self, state: bool) -> bool {
        match self {
            Self::Toggleable(current) => {
                *current = state;
                true
            }
            Self::NotToggleable => false,
        }
    }

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

#[derive(Debug, PartialEq, Eq)]
pub struct RegisteredAccel {
    pub command_id: CommandId,
}
