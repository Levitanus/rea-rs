use log::{debug, info};
use rea_rs::{errors::ReaperResult, ActionKind, Reaper};
use reaper_low::PluginContext;
use reaper_macros::reaper_extension_plugin;
use std::error::Error;

#[reaper_extension_plugin]
fn main(context: PluginContext) -> Result<(), Box<dyn Error>> {
    env_logger::init();
    info!("Started small Extension!");
    Reaper::load(context);
    let reaper = Reaper::get_mut();
    let message = "Hello from small extension";
    debug!("Try to show console message with: {:?}", message);
    reaper.show_console_msg(message);
    reaper.register_action(
        "_SMALL_TEST_CONSOLE_MSG",
        "small_test: console_msg",
        |_| -> ReaperResult<()> {
            debug!("Try to show console message from action");
            let rpr = Reaper::get();
            let mut pr = rpr.current_project();
            let mut tr = pr.get_track_mut(0).unwrap();
            let item = tr.get_item(0).unwrap();
            let take = item.active_take();
            debug!("take name: {}", take.name());
            let pitch_mode = take.pitch_mode().unwrap();
            debug!(
                "take: {:?}, pitch_mode shifter: {}, parameter: {}",
                take,
                pitch_mode.shifter(),
                pitch_mode.parameter()
            );
            Ok(())
        },
        ActionKind::NotToggleable,
    )?;

    Ok(())
}
