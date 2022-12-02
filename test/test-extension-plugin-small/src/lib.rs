use log::{debug, info};
use rea_rs::{
    errors::ReaperResult, ActionKind, CommandId, ExtValue, Reaper, UndoFlags,
};
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
            rpr.show_console_msg("Message from action!");
            rpr.show_console_msg(format!(
                "possible undo: {:?}",
                rpr.current_project().next_undo()
            ));
            let mut state =
                ExtValue::new("small test", "first", Some(56), true, None);
            rpr.show_console_msg(format!("{:?}", state.get()));
            state.set(80);
            rpr.show_console_msg(format!("{:?}", state.get()));
            rpr.with_undo_block(
                "New Undo",
                UndoFlags::empty(),
                Some(&rpr.current_project()),
                || -> ReaperResult<()> {
                    rpr.perform_action(CommandId::new(40001), 0, None);
                    Ok(())
                },
            )?;
            rpr.show_console_msg("render format:");
            rpr.show_console_msg(
                rpr.current_project().get_render_format(false)?,
            );
            rpr.show_console_msg("render project directory:");
            rpr.show_console_msg(format!(
                "{:?}",
                rpr.current_project().get_render_directory()?
            ));
            rpr.current_project()
                .set_track_group_name(3, "my group (maybe, flutes)")?;
            Ok(())
        },
        ActionKind::NotToggleable,
    )?;

    Ok(())
}
