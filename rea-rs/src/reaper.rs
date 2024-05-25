use rea_rs_low::{
    create_cpp_to_rust_control_surface,
    raw::{self, gaccel_register_t},
    register_plugin_destroy_hook, IReaperControlSurface, PluginContext, Swell,
};

use crate::{
    errors::ReaperStaticResult, keys::KeyBinding, ControlSurface,
    ControlSurfaceWrap, ReaRsError,
};
use c_str_macro::c_str;
use serde_derive::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    ffi::CString,
    ptr::NonNull,
    sync::Arc,
    time::{self, Duration, Instant},
};

static mut INSTANCE: Option<Reaper> = None;

type ActionCallback = dyn Fn(i32) -> Result<(), Box<dyn Error>>;

pub struct Action {
    command_id: CommandId,
    operation: Box<ActionCallback>,
}
impl Action {
    pub fn call(&self, flag: i32) -> Result<(), Box<dyn Error>> {
        (self.operation)(flag)
    }
}

pub trait Timer {
    fn run(&mut self) -> Result<(), Box<dyn Error>>;
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

fn action_error(error: Box<dyn Error>) {
    Reaper::get()
        .show_message_box(
            "Error while performing action",
            error.to_string(),
            crate::MessageBoxType::Ok,
        )
        .expect("Can not show error message box");
}

extern "C" fn action_hook(command_id: i32, flag: i32) -> bool {
    let actions = &Reaper::get().actions;
    for action in actions.iter() {
        if action.command_id.get() == command_id as u32 {
            match action.call(flag) {
                Ok(_) => (),
                Err(e) => action_error(e),
            };
            return true;
        }
    }
    false
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
    hook: extern "C" fn(i32, i32) -> bool,
    accels: Vec<Gaccel>,
    timers: HashMap<String, (Instant, Arc<RefCell<dyn Timer>>)>,
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
        let hook = action_hook;
        unsafe {
            low.plugin_register(
                c_str!("hookcommand").as_ptr(),
                hook as *mut _,
            );
        }
        Self {
            low,
            swell: Swell::load(context),
            actions,
            hook,
            accels: Vec::new(),
            timers: HashMap::new(),
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
    pub fn plugin_context(&self) -> PluginContext {
        self.low.plugin_context().clone()
    }

    pub fn is_available() -> bool {
        unsafe { INSTANCE.is_some() }
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
        unsafe {
            INSTANCE
                .as_ref()
                .expect("call `load(context)` before using `get()`")
        }
    }
    pub fn get_mut() -> &'static mut Reaper {
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
    pub fn unregister_timer(
        &mut self,
        id_string: String,
    ) -> ReaperStaticResult<()> {
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
            None => Err(crate::errors::ReaperError::InvalidObject(
                "No timer with the given string",
            )),
        }
    }

    /// Register action in the section and set default keybinding to it
    pub fn register_gaccel(
        &mut self,
        id_string: &'static str,
        description: &'static str,
        key_binding: impl Into<Option<KeyBinding>>,
    ) -> Result<RegisteredAccel, Box<dyn Error>> {
        let kb: Option<KeyBinding> = key_binding.into();
        let low = self.low();
        let id_string = id_string.replace(" ", "_");
        let id_string = CString::new(id_string.as_str())?;

        let command_id = unsafe {
            low.plugin_register(
                c_str!("command_id").as_ptr(),
                id_string.as_ptr() as _,
            )
        };
        let accel = match kb {
            Some(kb) => raw::ACCEL {
                fVirt: kb.fvirt.bits(),
                key: kb.key,
                cmd: command_id as u16,
            },
            None => raw::ACCEL {
                fVirt: 0,
                key: 0,
                cmd: command_id as u16,
            },
        };
        // let mut description = description.to_string();
        let desc = CString::new(description)?;
        let reg_str = c_str!("gaccel");
        let mut gaccel = raw::gaccel_register_t {
            accel,
            desc: desc.as_c_str().as_ptr(),
        };
        unsafe {
            low.plugin_register(
                reg_str.as_ptr(),
                &mut gaccel as *mut raw::gaccel_register_t as _,
            )
        };
        self.accels.push(Gaccel {
            _desc: desc,
            gaccel,
        });
        let reg = RegisteredAccel {
            command_id: CommandId::new(command_id as u32),
        };
        Ok(reg)
    }

    pub fn register_action(
        &mut self,
        id_string: &'static str,
        description: &'static str,
        operation: impl Fn(i32) -> Result<(), Box<dyn Error>> + 'static,
        key_binding: impl Into<Option<KeyBinding>>,
    ) -> Result<RegisteredAccel, Box<dyn Error>> {
        let accel =
            self.register_gaccel(id_string, description, key_binding)?;
        let action = Action {
            command_id: accel.command_id,
            operation: Box::new(operation),
        };
        self.actions.push(action);

        Ok(accel)
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
        println!("made cpp pointer");
        let s = c_str!("csurf_inst");
        let ret = unsafe {
            self.low().plugin_register(s.as_ptr(), cpp_cs.as_ptr() as _)
        };

        println!("registered: {ret}");
        self.csurfases
            .insert(id_string, (double_boxed_low_cs, low_cs_thin_ptr, cpp_cs));
        println!("added to HashMap: {:#?}", self.csurfases);
    }

    pub fn has_control_surface(&self, id_string: &String) -> bool {
        self.csurfases.contains_key(id_string)
    }

    pub fn unregister_control_surface(
        &mut self,
        id_string: String,
    ) -> Result<(), ReaRsError> {
        let (_, _, cpp_cs) =
            self.csurfases.remove(&id_string).ok_or(ReaRsError::Key(
                id_string,
                format!("{:#?}", self.csurfases.keys()),
            ))?;
        unsafe {
            self.low().plugin_register(
                c_str!("-csurf_inst").as_ptr(),
                cpp_cs.as_ptr() as _,
            );
        }
        Ok(())
    }
}
impl Drop for Reaper {
    fn drop(&mut self) {
        let low = self.low().clone();
        unsafe {
            low.plugin_register(
                c_str!("-hookcommand").as_ptr(),
                self.hook as *mut _,
            );
        }
        for accel in self.accels.iter_mut() {
            unsafe {
                low.plugin_register(
                    c_str!("-gaccel").as_ptr(),
                    &mut accel.gaccel as *mut raw::gaccel_register_t as _,
                )
            };
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

pub enum ActionKind {
    NotToggleable,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RegisteredAccel {
    pub command_id: CommandId,
}

struct Gaccel {
    _desc: CString,
    gaccel: gaccel_register_t,
}
