//! # rea-rs-test
//!
//! Makes testing of REAPER extension plugins easy.
//!
//! This integration test suite was originally written by Benjamin Klum
//! <benjamin.klum@helgoboss.org> for `reaper-rs`. But it was dependent on the
//! `reaper-high` crate, which was not and would not be soon published. And,
//! also, it was deeply integrated into the library.
//!
//! This version incapsulates as much as possible, leaving simple interface to
//! making tests.
//!
//! For testing reaper extension, which itself is of type `cdylib`,
//! you need transform the project folder to workspace. So, basically,
//! project tree would look similar to this:
//!
//! ```bash
//! workspace_directory
//! ├── Cargo.toml
//! ├── README.md
//! ├—— my_lib
//! ├   ├—— src
//! │      └── lib.rs
//! └── test
//!     ├── Cargo.toml
//!     ├── src
//!     │   └── lib.rs
//!     └── tests
//!         └── integration_test.rs
//! ```
//!
//! `test` crate will not be delivered to the end-user, but will be used for
//! testing your library. Since there is a need for patching of reaper-low and
//! reaper-medium, contents of `test/Cargo.toml`:
//!
//! ```toml
//! [package]
//! edition = "2021"
//! name = "reaper-test-extension-plugin"
//! publish = false
//! version = "0.1.0"
//!
//! [dependencies]
//! rea-rs = "0.1.1"
//! rea-rs-macros = "0.1.0"
//! rea-rs-test = "0.1.0"
//! my_lib = {path = "../my_lib"}
//!
//! [lib]
//! crate-type = ["cdylib"]
//! name = "reaper_test_extension_plugin"
//! ```
//!
//! contents of `test/tests/integration_test.rs`:
//!
//! ```no_run
//! use rea_rs_test::{run_integration_test, ReaperVersion};
//! #[test]
//! fn test() {
//!     run_integration_test(ReaperVersion::latest());
//! }
//! ```
//!
//! `test/src/lib.rs` is the file your integration tests are placed in.
//!
//! ```no_run
//! use rea_rs_macros::reaper_extension_plugin;
//! use rea_rs_test::*;
//! use rea_rs::{Reaper, PluginContext};
//! use std::error::Error;
//! fn hello_world(reaper: &mut Reaper) -> TestStepResult {
//!     reaper.show_console_msg("Hello world!");
//!     Ok(())
//! }
//! #[reaper_extension_plugin]
//! fn test_extension(context: PluginContext) -> Result<(), Box<dyn Error>> {
//!     // setup test global environment
//!     let test = ReaperTest::setup(context, "test_action");
//!     // Push single test step.
//!     test.push_test_step(TestStep::new("Hello World!", hello_world));
//!     Ok(())
//! }
//! ```
//!
//! to run integration tests, go to the test folder and type:
//! `cargo build --workspace; cargo test`
//!
//! ## Hint
//!
//! Use crates `log` and `env_logger` for printing to stdio. integration test
//! turns env logger on by itself.

use rea_rs::{PluginContext, Reaper, Timer};
use rea_rs_low::register_plugin_destroy_hook;
use std::{
    cell::RefCell, error::Error, fmt::Debug, panic, process, sync::Arc,
};

pub mod integration_test;
pub use integration_test::*;

static mut INSTANCE: Option<ReaperTest> = None;

pub type TestStepResult = Result<(), Box<dyn Error>>;
pub type TestCallback = dyn Fn(&'static mut Reaper) -> TestStepResult;

pub struct TestStep {
    name: String,
    operation: Box<TestCallback>,
}
impl TestStep {
    pub fn new(
        name: impl Into<String>,
        operation: impl Fn(&'static mut Reaper) -> Result<(), Box<dyn Error>>
            + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            operation: Box::new(operation),
        }
    }
}
impl Debug for TestStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

fn test(_flag: i32) -> Result<(), Box<dyn Error>> {
    ReaperTest::get_mut().test();
    Ok(())
}

struct IntegrationTimer {}
impl Timer for IntegrationTimer {
    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        test(0)?;
        self.stop();
        Ok(())
    }

    fn id_string(&self) -> String {
        "integration_timer".to_string()
    }
}

pub struct ReaperTest {
    steps: Vec<TestStep>,
    is_integration_test: bool,
}
impl ReaperTest {
    fn make_available_globally(r_test: ReaperTest) {
        static INIT_INSTANCE: std::sync::Once = std::sync::Once::new();
        unsafe {
            INIT_INSTANCE.call_once(|| {
                INSTANCE = Some(r_test);
                register_plugin_destroy_hook(|| INSTANCE = None);
            });
        }
    }
    pub fn setup(
        context: PluginContext,
        action_name: &'static str,
    ) -> &'static mut Self {
        let reaper = Reaper::init_global(context);
        let instance = Self {
            steps: Vec::new(),
            is_integration_test: std::env::var("RUN_REAPER_INTEGRATION_TEST")
                .is_ok(),
        };
        let integration = instance.is_integration_test;
        reaper
            .register_action(action_name, action_name, test, None)
            .expect("Can not reigister test action");
        Self::make_available_globally(instance);
        if integration {
            reaper.register_timer(Arc::new(RefCell::new(IntegrationTimer {})))
        }
        ReaperTest::get_mut()
    }

    /// Gives access to the instance which you made available globally before.
    ///
    /// # Panics
    ///
    /// This panics if [`make_available_globally()`] has not been called
    /// before.
    ///
    /// [`make_available_globally()`]: fn.make_available_globally.html
    pub fn get() -> &'static ReaperTest {
        unsafe {
            INSTANCE
                .as_ref()
                .expect("call `load(context)` before using `get()`")
        }
    }
    pub fn get_mut() -> &'static mut ReaperTest {
        unsafe {
            INSTANCE
                .as_mut()
                .expect("call `load(context)` before using `get()`")
        }
    }

    fn test(&mut self) {
        println!("# Testing reaper-rs\n");
        let result = panic::catch_unwind(|| -> TestStepResult {
            // let r_test = ReaperTest::get_mut();
            // let rpr = &mut r_test.reaper;
            // for step in r_test.steps.iter() {
            //     println!("Testing step: {}", step.name);
            //     (step.operation)(rpr)?;
            // }
            ReaperTest::get()
                .steps
                .iter()
                .map(|step| -> Result<(), Box<dyn Error>> {
                    println!("Testing step: {}", step.name);
                    (step.operation)(Reaper::get_mut())?;
                    Ok(())
                })
                .count();
            Ok(())
        });
        let final_result = match result.is_err() {
            false => result.unwrap(),
            true => Err("Reaper panicked!".into()),
        };
        match final_result {
            Ok(_) => {
                println!("From REAPER: reaper-rs integration test executed successfully");
                if self.is_integration_test {
                    process::exit(0)
                }
            }
            Err(reason) => {
                // We use a particular exit code to distinguish test
                // failure from other possible
                // exit paths.
                match self.is_integration_test {
                    true => {
                        eprintln!("From REAPER: reaper-rs integration test failed: {}", reason);
                        process::exit(172)
                    }
                    false => panic!(
                        "From REAPER: reaper-rs integration test failed: {}",
                        reason
                    ),
                }
            }
        }
    }

    pub fn push_test_step(&mut self, step: TestStep) {
        self.steps.push(step);
    }
}
