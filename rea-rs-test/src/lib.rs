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
//! `test` crate will not be delivered to end-users; it is a non-published
//! extension plug-in used only for host-driven testing. An example package
//! manifest is:
//!
//! ```toml
//! [package]
//! edition = "2021"
//! name = "reaper-test-extension-plugin"
//! publish = false
//! version = "1.0.0"
//!
//! [dependencies]
//! rea-rs = "1.0.0"
//! rea-rs-macros = "1.0.0"
//! rea-rs-test = "1.0.0"
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
//! fn hello_world(reaper: &mut Reaper) -> TestStepResult {
//!     reaper.show_console_msg("Hello world!")?;
//!     Ok(())
//! }
//! #[reaper_extension_plugin]
//! fn test_extension(context: PluginContext) -> Result<(), anyhow::Error> {
//!     // setup test global environment
//!     let test = ReaperTest::setup(context, "test_action");
//!     // Push single test step.
//!     test.push_test_step(TestStep::new("Hello World!", hello_world));
//!     Ok(())
//! }
//! ```
//!
//! Run the integration test from the workspace root with
//! `cargo test -p reaper-test-extension-plugin --test integration_test`.
//!
//! ## Hint
//!
//! Plug-in test steps can use `log` for structured diagnostics. The runner
//! captures REAPER stdout and stderr and prints the capture when a test fails.

use rea_rs::{
    ActionHook, ActionKind, ActionRegistrationOptions, PluginContext, Reaper,
    Timer,
};
use rea_rs_low::register_plugin_destroy_hook;
use std::{
    cell::RefCell,
    fmt::Debug,
    panic::{self, AssertUnwindSafe},
    process,
    sync::Arc,
};

pub mod integration_test;
pub use integration_test::*;

const INTEGRATION_RESULT_PATH_ENV: &str =
    integration_test::INTEGRATION_RESULT_PATH_ENV;

static mut INSTANCE: Option<ReaperTest> = None;

pub type TestStepResult = Result<(), anyhow::Error>;
pub type TestCallback = dyn Fn(&'static mut Reaper) -> TestStepResult;

pub struct TestStep {
    name: String,
    operation: Box<TestCallback>,
}
impl TestStep {
    pub fn new(
        name: impl Into<String>,
        operation: impl Fn(&'static mut Reaper) -> Result<(), anyhow::Error>
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

fn run_tests() -> Result<(), anyhow::Error> {
    ReaperTest::get_mut().test();
    Ok(())
}

fn test(_hook: &mut ActionHook) -> Result<(), anyhow::Error> {
    run_tests()
}

struct IntegrationTimer {}
impl Timer for IntegrationTimer {
    fn run(&mut self) -> Result<(), anyhow::Error> {
        run_tests()?;
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
            .register_action(
                action_name,
                action_name,
                ActionKind::NotToggleable,
                test,
                ActionRegistrationOptions::default(),
            )
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
    #[allow(static_mut_refs)]
    pub fn get() -> &'static ReaperTest {
        unsafe {
            INSTANCE
                .as_ref()
                .expect("call `load(context)` before using `get()`")
        }
    }
    #[allow(static_mut_refs)]
    pub fn get_mut() -> &'static mut ReaperTest {
        unsafe {
            INSTANCE
                .as_mut()
                .expect("call `load(context)` before using `get()`")
        }
    }

    fn test(&mut self) {
        println!("# Testing reaper-rs\n");
        let mut is_err = false;
        for step in ReaperTest::get().steps.iter() {
            println!("Testing step: {}", step.name);
            // let operation = step.operation;
            match panic::catch_unwind(AssertUnwindSafe(
                || -> TestStepResult { (step.operation)(Reaper::get_mut()) },
            )) {
                Ok(result) => match result {
                    Ok(_) => println!("passed!"),
                    Err(e) => {
                        is_err = true;
                        eprintln!("error occured: {}", e)
                    }
                },
                Err(reason) => {
                    is_err = true;
                    eprintln!("paniced: {:?}", reason)
                }
            }
        }
        match is_err {
            false => {
                println!("From REAPER: reaper-rs integration test executed successfully");
                if self.is_integration_test {
                    if let Err(error) = write_integration_test_result("PASS") {
                        eprintln!(
                            "Could not report integration-test success: {error}"
                        );
                        process::exit(173)
                    }
                    process::exit(0)
                }
            }
            true => {
                // We use a particular exit code to distinguish test
                // failure from other possible
                // exit paths.
                match self.is_integration_test {
                    true => {
                        eprintln!(
                            "From REAPER: reaper-rs integration test failed"
                        );
                        if let Err(error) =
                            write_integration_test_result("FAIL")
                        {
                            eprintln!(
                                "Could not report integration-test failure: {error}"
                            );
                        }
                        process::exit(172)
                    }
                    false => panic!(
                        "From REAPER: reaper-rs integration test failed. panic!"
                    ),
                }
            }
        }
    }

    pub fn push_test_step(&mut self, step: TestStep) {
        self.steps.push(step);
    }
}

fn write_integration_test_result(result: &str) -> std::io::Result<()> {
    let result_path = std::env::var_os(INTEGRATION_RESULT_PATH_ENV)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "integration result path environment variable is not set",
            )
        })?;
    let result_path = std::path::PathBuf::from(result_path);
    let temporary_path = result_path.with_extension("result.tmp");
    std::fs::write(&temporary_path, format!("{result}\n"))?;
    std::fs::rename(&temporary_path, &result_path)
}
