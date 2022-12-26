use rea_rs::{PluginContext, Reaper};
use rea_rs_macros::reaper_extension_plugin;
use reaper_test::*;
use std::error::Error;

fn hello_world(reaper: &mut Reaper) -> TestStepResult {
    reaper.show_console_msg("Hello world!");
    Ok(())
}

#[reaper_extension_plugin]
fn test_extension(context: PluginContext) -> Result<(), Box<dyn Error>> {
    let test = ReaperTest::setup(context, "test_action");
    test.push_test_step(TestStep::new("Hello World!", hello_world));
    Ok(())
}
