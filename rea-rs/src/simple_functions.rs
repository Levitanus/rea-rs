use crate::{
    errors::{ReaperError, ReaperResult},
    misc_enums::ProjectContext,
    reaper_pointer::ReaperPointer,
    utils::{
        as_c_char, as_c_str, as_mut_i8, as_string, make_string_buf, WithNull,
    },
    AutomationMode, CommandId, MessageBoxType, MessageBoxValue, Project,
    Reaper, Section, UndoFlags,
};
use int_enum::IntEnum;
use log::debug;
use std::{
    collections::HashMap, error::Error, ffi::CString, fs::canonicalize,
    marker::PhantomData, path::Path, ptr::NonNull,
};

impl Reaper {
    /// Show message in console.
    ///
    /// # Note
    ///
    /// `\n` will be added in the end.
    pub fn show_console_msg(&self, msg: impl Into<String>) {
        let mut msg: String = msg.into();
        msg.push_str("\n");
        unsafe {
            self.low()
                .ShowConsoleMsg(as_c_str(msg.with_null()).as_ptr())
        };
    }

    pub fn clear_console(&self) {
        self.low().ClearConsole()
    }

    /// Run action by it's command id.
    ///
    /// # Note
    ///
    /// It seems, that flag should be always 0.
    /// If project is None — will perform on current project.
    pub fn perform_action(
        &self,
        action_id: impl Into<CommandId>,
        flag: i32,
        project: Option<&Project>,
    ) {
        let action_id = action_id.into();
        let current: Project;
        let project = match project {
            None => {
                current = self.current_project();
                &current
            }
            Some(pr) => pr,
        };
        unsafe {
            self.low().Main_OnCommandEx(
                action_id.get() as i32,
                flag,
                project.context().to_raw(),
            )
        }
    }

    /// Get project from the current tab.
    pub fn current_project(&self) -> Project {
        Project::new(ProjectContext::CurrentProject)
    }

    /// Open new project tab.
    ///
    /// To open project in new tab use [Reaper::open_project]
    pub fn add_project_tab(&self, make_current_project: bool) -> Project {
        match make_current_project {
            false => {
                let current_project =
                    Project::new(ProjectContext::CurrentProject);
                let project = self.add_project_tab(true);
                current_project.make_current_project();
                project
            }
            true => {
                self.perform_action(CommandId::new(40859), 0, None);
                self.current_project()
            }
        }
    }

    /// Open project from the filename.
    pub fn open_project(
        &self,
        file: &Path,
        in_new_tab: bool,
        make_current_project: bool,
    ) -> Result<Project, &str> {
        let current_project = self.current_project();
        if in_new_tab {
            self.add_project_tab(true);
        }
        if !file.is_file() {
            return Err("path is not file");
        }
        let path = file.to_str().ok_or("can not use this path")?;
        unsafe {
            self.low().Main_openProject(as_mut_i8(path));
        }
        let project = self.current_project();
        if !make_current_project {
            current_project.make_current_project();
        }
        Ok(project)
    }

    /// Add reascript from file and put to the action list.
    ///
    /// commit must be used in the last call,
    /// but it is faster to make it false in a bulk.
    pub fn add_reascript(
        &self,
        file: &Path,
        section: Section,
        commit: bool,
    ) -> Result<CommandId, Box<dyn Error>> {
        Ok(self
            .add_remove_reascript(file, section, commit, true)?
            .expect("should hold CommandId"))
    }

    /// Remove reascript.
    ///
    /// commit must be used in the last call,
    /// but it is faster to make it false in a bulk.
    pub fn remove_reascript(
        &self,
        file: &Path,
        section: Section,
        commit: bool,
    ) -> Result<(), Box<dyn Error>> {
        self.add_remove_reascript(file, section, commit, false)?;
        Ok(())
    }

    fn add_remove_reascript(
        &self,
        file: &Path,
        section: Section,
        commit: bool,
        add: bool,
    ) -> ReaperResult<Option<CommandId>> {
        if !file.is_file() {
            return Err("path is not file!".into());
        }
        let abs = canonicalize(file)?;
        unsafe {
            let id = self.low().AddRemoveReaScript(
                add,
                section.id() as i32,
                as_mut_i8(abs.to_str().ok_or("can not resolve path")?),
                commit,
            );
            if id <= 0 {
                return Err(Box::new(ReaperError::Str(
                    "Failed to add or remove reascript.",
                )));
            }
            match add {
                true => Ok(Some(CommandId::new(id as u32))),
                false => Ok(None),
            }
        }
    }

    /// Ask user to select a file.
    ///
    /// extension — extension for file, e.g. "mp3", "txt". Or empty string.
    pub fn browse_for_file(
        &self,
        window_title: impl Into<String>,
        extension: impl Into<String>,
    ) -> Result<Box<Path>, Box<dyn Error>> {
        unsafe {
            let buf = make_string_buf(4096);
            let result = self.low().GetUserFileNameForRead(
                buf,
                as_mut_i8(window_title.into().as_str()),
                as_mut_i8(extension.into().as_str()),
            );
            match result {
                false => Err(Box::new(ReaperError::UserAborted)),
                true => {
                    let filename = CString::from_raw(buf).into_string()?;
                    Ok(Path::new(&filename).into())
                }
            }
        }
    }

    /// Arm or disarm command.
    ///
    /// # Original doc
    ///
    /// arms a command (or disarms if 0 passed) in section
    /// (empty string for main)
    pub fn arm_command(&self, command: CommandId, section: impl Into<String>) {
        unsafe {
            self.low().ArmCommand(
                command.get() as i32,
                as_mut_i8(section.into().as_str()),
            )
        }
    }

    pub fn disarm_command(&self) {
        self.arm_command(CommandId::new(0), "");
    }

    /// Get armed command.
    ///
    /// If string is empty (`len() = 0`), then it's main section.
    pub fn armed_command(&self) -> Option<(CommandId, String)> {
        unsafe {
            let buf = make_string_buf(200);
            let id = self.low().GetArmedCommand(buf, 200);
            let result = CString::from_raw(buf)
                .into_string()
                .unwrap_or(String::from(""));
            match id {
                0 => None,
                _ => Some((CommandId::new(id as u32), String::from(result))),
            }
        }
    }

    /// Reset global peak cache.
    pub fn clear_peak_cache(&self) {
        self.low().ClearPeakCache()
    }

    // TODO: db to slider?

    /// Get ID for action with the given name.
    ///
    /// # Note
    ///
    /// name is the ID string, that was made, when registered as action,
    /// but not the description line.
    ///
    /// If action name doesn't start with underscore, it will be added.
    pub fn get_action_id(
        &self,
        action_name: impl Into<String>,
    ) -> Option<CommandId> {
        unsafe {
            let mut name: String = action_name.into();
            if !name.starts_with("_") {
                name = String::from("_") + &name;
            }
            // debug!("action name: {:?}", name);
            let id = self.low().NamedCommandLookup(as_mut_i8(name.as_str()));
            // debug!("got action id: {:?}", id);
            match id {
                x if x <= 0 => None,
                _ => Some(CommandId::new(id as u32)),
            }
        }
    }

    /// Get action name (string ID) of an action with the given ID.
    pub fn get_action_name(&self, id: CommandId) -> Option<String> {
        debug!("get action name");
        let result = self.low().ReverseNamedCommandLookup(id.get() as i32);
        // debug!("received result: {:?}", result);
        match result.is_null() {
            true => None,
            false => Some(as_string(result).unwrap()),
        }
    }

    /// Return REAPER bin directory (e.g. "C:\\Program Files\\REAPER").
    pub fn get_binary_directory(&self) -> String {
        let result = self.low().GetExePath();
        as_string(result).expect("Can not convert result to string.")
    }

    /// Get globally overrided automation mode.
    ///
    /// None if do not overrides.
    pub fn get_global_automation_mode(&self) -> Option<AutomationMode> {
        let result = self.low().GetGlobalAutomationOverride();
        let mode =
            AutomationMode::from_int(result).expect("should convert to enum.");
        match mode {
            AutomationMode::None => None,
            _ => Some(mode),
        }
    }

    /// Override global automation mode.
    pub fn set_global_automation_mode(&self, mode: AutomationMode) {
        self.low().SetGlobalAutomationOverride(mode.int_value());
    }

    /// Show text inputs to user and get values from them.
    ///
    /// # Note
    ///
    /// default buf size is 1024
    pub fn get_user_inputs<'a>(
        &self,
        title: impl Into<String>,
        captions: Vec<&'a str>,
        buf_size: impl Into<Option<usize>>,
    ) -> ReaperResult<HashMap<String, String>> {
        unsafe {
            let buf_size = match buf_size.into() {
                None => 1024,
                Some(sz) => sz,
            };
            let buf = make_string_buf(buf_size);
            let result = self.low().GetUserInputs(
                as_c_char(title.into().as_str()),
                captions.len() as i32,
                as_mut_i8(captions.join(",").as_str()),
                buf,
                buf_size as i32,
            );
            if result == false {
                return Err(Box::new(ReaperError::UserAborted));
            }
            let mut map = HashMap::new();
            let values =
                as_string(buf).expect("can not retrieve user inputs.");
            for (key, val) in captions.into_iter().zip(values.split(",")) {
                map.insert(String::from(key), String::from(val));
            }
            Ok(map)
        }
    }

    /// Call function while freezing the UI.
    pub fn with_prevent_ui_refresh(&self, f: impl Fn()) {
        self.low().PreventUIRefresh(1);
        (f)();
        self.low().PreventUIRefresh(-1);
    }

    /// Call function in undo block with given name.
    ///
    /// # Note
    ///
    /// Probably, it's better to use `UndoFlags.all()`
    /// by default.
    pub fn with_undo_block(
        &self,
        undo_name: impl Into<String>,
        flags: UndoFlags,
        project: Option<&Project>,
        mut f: impl FnMut() -> ReaperResult<()>,
    ) -> ReaperResult<()> {
        let low = self.low();
        let undo_name: String = undo_name.into();
        match project {
            None => low.Undo_BeginBlock(),
            Some(pr) => unsafe {
                low.Undo_BeginBlock2(pr.context().to_raw());
            },
        }

        (f)()?;
        unsafe {
            // let undo_name = ;
            let flags = flags.bits() as i32;
            match project {
                None => {
                    low.Undo_EndBlock(as_c_char(undo_name.as_str()), flags);
                }
                Some(pr) => low.Undo_EndBlock2(
                    pr.context().to_raw(),
                    as_c_char(undo_name.as_str()),
                    flags,
                ),
            }
        }
        Ok(())
    }

    /// Show message box to user and get result.
    pub fn show_message_box(
        &self,
        title: impl Into<String>,
        text: impl Into<String>,
        box_type: MessageBoxType,
    ) -> ReaperResult<MessageBoxValue> {
        unsafe {
            let low = self.low();
            let status = low.ShowMessageBox(
                as_mut_i8(text.into().as_str()),
                as_mut_i8(title.into().as_str()),
                box_type.int_value(),
            );
            Ok(MessageBoxValue::from_int(status)?)
        }
    }

    /// Redraw the arrange view.
    pub fn update_arrange(&self) {
        self.low().UpdateArrange();
    }

    /// Redraw timeline.
    pub fn update_timeline(&self) {
        self.low().UpdateTimeline();
    }

    /// Open preferences window.
    ///
    /// page should be positive or None.
    ///
    /// if not page — then name will be used.
    pub fn view_prefs(
        &self,
        page: impl Into<Option<u32>>,
        name: impl Into<Option<String>>,
    ) {
        let name = name.into().unwrap_or(String::from(""));
        let page = page.into().unwrap_or(0_u32);
        unsafe {
            self.low().ViewPrefs(page as i32, as_c_char(name.as_str()));
        }
    }

    /// Iter through all opened projects.
    ///
    /// # Warning
    ///
    /// This operation, probably, of O(n²) complexity.
    /// So, it's better not to use it in loop or too often.
    pub fn iter_projects<'a>(&self) -> ProjectIterator {
        ProjectIterator::new(*self.low())
    }

    /// Checks if the given pointer is still valid.
    ///
    /// Returns true if the pointer is a valid object
    /// of the correct type in the current project.
    pub fn validate_ptr<'a>(&self, pointer: impl Into<ReaperPointer>) -> bool {
        let pointer: ReaperPointer = pointer.into();
        unsafe {
            self.low().ValidatePtr(
                pointer.ptr_as_void(),
                pointer.key_into_raw().as_ptr(),
            )
        }
    }

    /// Checks if the given pointer is still valid.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use rea_rs::{Reaper, ProjectContext, WithReaperPtr};
    /// let rpr = Reaper::get();
    /// let pr = rpr.current_project();
    /// let track = pr.get_track(0).ok_or("No track")?;
    /// let track_is_valid = rpr.validate_ptr_2(&pr, track.get_pointer());
    /// assert!(track_is_valid);
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Returns `true` if the pointer is a valid object of the
    /// correct type in the given project.
    /// The project is ignored if the pointer itself is a project.
    pub fn validate_ptr_2<'a>(
        &self,
        project: &Project,
        pointer: impl Into<ReaperPointer>,
    ) -> bool {
        let pointer: ReaperPointer = pointer.into();
        unsafe {
            self.low().ValidatePtr2(
                project.context().to_raw(),
                pointer.ptr_as_void(),
                pointer.key_into_raw().as_ptr(),
            )
        }
    }
}

/// Iterates through all opened projects.
///
/// Should be created by [`Reaper::iter_projects()`]
pub struct ProjectIterator {
    low: rea_rs_low::Reaper,
    index: i32,
    phantom: PhantomData<Project>,
}
impl ProjectIterator {
    fn new(low: rea_rs_low::Reaper) -> Self {
        Self {
            low,
            index: 0,
            phantom: PhantomData::default(),
        }
    }
}
impl Iterator for ProjectIterator {
    type Item = Project;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let raw = self.low.EnumProjects(self.index, as_mut_i8(""), 0);
            let raw = NonNull::new(raw);
            self.index += 1;
            match raw {
                None => None,
                Some(raw) => Some(Project::new(ProjectContext::Proj(raw))),
            }
        }
    }
}
