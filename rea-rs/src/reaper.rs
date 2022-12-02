use reaper_low::{
    raw::gaccel_register_t, register_plugin_destroy_hook, PluginContext,
};
use reaper_medium::{HookCommand, OwnedGaccelRegister};
use std::{error::Error, ptr::NonNull};

static mut INSTANCE: Option<Reaper> = None;

type ActionCallback = dyn Fn(i32) -> Result<(), Box<dyn Error>>;

pub struct Action {
    command_id: CommandId,
    command_name: &'static str,
    description: &'static str,
    operation: Box<ActionCallback>,
    kind: ActionKind,
    address: NonNull<gaccel_register_t>,
}
impl Action {
    pub fn call(&self, flag: i32) -> Result<(), Box<dyn Error>> {
        (self.operation)(flag)
    }
}

pub struct ActionHook {
    actions: Vec<Action>,
}
impl ActionHook {
    pub fn new() -> Self {
        return Self {
            actions: Vec::new(),
        };
    }
}
impl HookCommand for ActionHook {
    fn call(command_id: reaper_medium::CommandId, flag: i32) -> bool {
        let rpr = Reaper::get_mut();
        let hook = rpr.action_hook.as_ref().expect("should be hook here");
        for action in hook.actions.iter() {
            if action.command_id == command_id {
                action.call(flag).unwrap();
                return true;
            }
        }
        return false;
    }
}

pub struct Reaper {
    low: reaper_low::Reaper,
    medium_session: reaper_medium::ReaperSession,
    medium: reaper_medium::Reaper,
    action_hook: Option<ActionHook>,
}
impl Reaper {
    /// Makes the given instance available globally.
    ///
    /// After this has been called, the instance can be queried globally using
    /// `get()`.
    ///
    /// This can be called once only. Subsequent calls won't have any effect!
    pub fn make_available_globally(reaper: Reaper) {
        static INIT_INSTANCE: std::sync::Once = std::sync::Once::new();
        unsafe {
            INIT_INSTANCE.call_once(|| {
                INSTANCE = Some(reaper);
                register_plugin_destroy_hook(|| INSTANCE = None);
            });
        }
    }

    pub fn load(context: PluginContext) {
        let low = reaper_low::Reaper::load(context);
        let medium_session = reaper_medium::ReaperSession::new(low);
        let medium = medium_session.reaper().clone();
        reaper_medium::Reaper::make_available_globally(medium.clone());
        let instance = Self {
            low,
            medium_session,
            medium,
            action_hook: None,
        };
        Self::make_available_globally(instance);
    }
    pub fn low(&self) -> &reaper_low::Reaper {
        &self.low
    }
    pub fn medium_session(&self) -> &reaper_medium::ReaperSession {
        &self.medium_session
    }
    pub fn medium(&self) -> &reaper_medium::Reaper {
        &self.medium
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

    pub fn register_action(
        &mut self,
        command_name: &'static str,
        description: &'static str,
        operation: impl Fn(i32) -> Result<(), Box<dyn Error>> + 'static,
        kind: ActionKind,
    ) -> Result<RegisteredAction, Box<dyn Error>> {
        self.check_action_hook();
        let hook = self.action_hook.as_mut().expect("should be hook here");
        let medium = &mut self.medium_session;
        let command_id =
            medium.plugin_register_add_command_id(command_name).unwrap();
        // self.medium_session().plugin_register_add_gaccel()
        let address = medium.plugin_register_add_gaccel(
            OwnedGaccelRegister::without_key_binding(command_id, description),
        )?;
        let command_id = CommandId::from(command_id);
        hook.actions.push(Action {
            command_id,
            command_name,
            description,
            operation: Box::new(operation),
            kind,
            address,
        });
        Ok(RegisteredAction { command_id })
    }

    fn check_action_hook(&mut self) {
        if self.action_hook.is_none() {
            self.action_hook = Some(ActionHook::new());
            self.medium_session
                .plugin_register_add_hook_command::<ActionHook>()
                .expect("can not register hook");
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
impl PartialEq<reaper_medium::CommandId> for CommandId {
    fn eq(&self, other: &reaper_medium::CommandId) -> bool {
        self.get() == other.get()
    }
    fn ne(&self, other: &reaper_medium::CommandId) -> bool {
        self.get() != other.get()
    }
}
impl From<reaper_medium::CommandId> for CommandId {
    fn from(value: reaper_medium::CommandId) -> Self {
        Self { id: value.get() }
    }
}
impl Into<reaper_medium::CommandId> for CommandId {
    fn into(self) -> reaper_medium::CommandId {
        reaper_medium::CommandId::new(self.get())
    }
}

pub struct RegisteredAction {
    // For identifying the registered command (= the functions to be executed)
    pub command_id: CommandId,
}

pub enum ActionKind {
    NotToggleable,
}
