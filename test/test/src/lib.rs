#[macro_use]
mod assert;
mod tests;
use crate::tests::create_test_steps;
use log::info;
use rea_rs::Reaper;
use std::{error::Error, panic};

pub type TestStepResult = Result<(), Box<dyn Error>>;

type TestOperation = dyn FnOnce() -> TestStepResult;

pub struct TestStep {
    pub name: String,
    pub operation: Box<TestOperation>,
}

pub fn step<Op>(name: impl Into<String>, operation: Op) -> TestStep
where
    Op: FnOnce() -> TestStepResult + 'static,
{
    TestStep {
        name: name.into(),
        operation: Box::new(operation),
    }
}

/// Executes the complete integration test.
///
/// Calls the given callback as soon as finished (either when the first test
/// step failed or when all steps have executed successfully).
pub fn execute_integration_test(
    on_finish: impl Fn(Result<(), Box<dyn Error>>) + 'static,
) {
    let rpr = Reaper::get();
    rpr.clear_console();
    info!("# Testing reaper-rs\n");
    let result = panic::catch_unwind(|| -> TestStepResult {
        let steps: Vec<TestStep> = create_test_steps().collect();
        for step in steps {
            info!("Testing step: {}", step.name);
            (step.operation)()?;
        }
        Ok(())
    });
    let final_result = match result.is_err() {
        false => result.unwrap(),
        true => Err("Reaper panicked!".into()),
    };
    on_finish(final_result)
}
