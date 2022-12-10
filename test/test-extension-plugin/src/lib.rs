use log::debug;
use rea_rs::{ActionKind, Reaper, RegisteredAction};
use reaper_low;
use reaper_low::PluginContext;
use reaper_macros::reaper_extension_plugin;
use reaper_medium::ControlSurface;
use reaper_test::TestStepResult;
use std::error::Error;
use std::process;

#[derive(Debug)]
struct Listener {
    action: RegisteredAction,
}

impl ControlSurface for Listener {
    fn run(&mut self) {
        Reaper::get().perform_action(self.action.command_id, 0, None);
    }
}

fn test_func(result: TestStepResult) {
    match result {
        Ok(_) => {
            println!("From REAPER: reaper-rs integration test executed successfully");
            process::exit(0)
        }
        Err(reason) => {
            // We use a particular exit code to distinguish test
            // failure from other possible
            // exit paths.
            eprintln!(
                "From REAPER: reaper-rs integration test failed: {}",
                reason
            );
            process::exit(172)
        }
    }
}

#[reaper_extension_plugin]
fn main(context: PluginContext) -> Result<(), Box<dyn Error>> {
    let run_integration_test =
        std::env::var("RUN_REAPER_RS_INTEGRATION_TEST").is_ok();
    if run_integration_test {
        println!(
            "From REAPER: Launching reaper-rs reaper-test-extension-plugin..."
        );
    }
    // let medium = Reaper::get();
    // (reaper_low::Reaper::load(plugin_context));
    env_logger::init();
    Reaper::load(context);
    let reaper = Reaper::get_mut();

    debug!("Loaded reaper-rs integration test plugin");
    let action = match run_integration_test {
        false => reaper.register_action(
            "reaRsIntegrationTests",
            "rea-rs integration tests",
            |_| Ok(reaper_test::execute_integration_test(|_| ())),
            ActionKind::NotToggleable,
        )?,
        true => reaper.register_action(
            "reaRsIntegrationTests",
            "rea-rs integration tests",
            |_| Ok(reaper_test::execute_integration_test(test_func)),
            ActionKind::NotToggleable,
        )?,
    };
    if run_integration_test {
        reaper
            .medium_session_mut()
            .plugin_register_add_csurf_inst(Box::new(Listener { action }))?;
    }
    Ok(())
}
