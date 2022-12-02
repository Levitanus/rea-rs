use log::debug;
use rea_rs::{Reaper, ActionKind};
use reaper_low;
use reaper_low::PluginContext;
use reaper_macros::reaper_extension_plugin;
use std::error::Error;
use std::process;

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
    if run_integration_test {
        println!("From REAPER: Entering reaper-rs integration test...");
        reaper_test::execute_integration_test(|result| {
            match result {
                Ok(_) => {
                    println!("From REAPER: reaper-rs integration test executed successfully");
                    process::exit(0)
                }
                Err(reason) => {
                    // We use a particular exit code to distinguish test failure from other possible
                    // exit paths.
                    eprintln!(
                        "From REAPER: reaper-rs integration test failed: {}",
                        reason
                    );
                    process::exit(172)
                }
            }
        });
    }
    reaper.register_action(
        "reaRsIntegrationTests",
        "rea-rs integration tests",
        |_| Ok(reaper_test::execute_integration_test(|_| ())),
        ActionKind::NotToggleable,
    )?;
    Ok(())
}
