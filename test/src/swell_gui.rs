use log::info;
use rea_rs::{
    ActionKind, Reaper, ReaperWindow, WindowCommand, WindowHandler, WindowSpec,
};
use std::sync::atomic::{AtomicIsize, Ordering};

struct DemoWindow {
    window: ReaperWindow,
}

impl WindowHandler for DemoWindow {
    fn window(&self) -> &ReaperWindow {
        &self.window
    }

    fn on_open(&mut self) {
        info!("window opened");
    }

    fn on_close(&mut self) -> bool {
        info!("window close requested");
        true
    }

    fn on_destroy(&mut self) {
        info!("window destroyed");
    }

    fn on_resize(&mut self, width: i32, height: i32) {
        info!("window resized to {}x{}", width, height);
    }

    fn on_command(&mut self, command: WindowCommand) {
        info!("window command: {:?}", command);
    }
}

static DEMO_WINDOW: AtomicIsize = AtomicIsize::new(0);

pub fn register_actions(reaper: &mut Reaper) -> anyhow::Result<()> {
    reaper.register_action(
        "TestReaRsWindowing",
        "test rea-rs windowing",
        ActionKind::NotToggleable,
        |_| {
            let main_window =
                ReaperWindow::from_hwnd(Reaper::get().low().GetMainHwnd())?;
            info!("main window title: {}", main_window.title()?);

            let old_window = DEMO_WINDOW.load(Ordering::Acquire);
            if old_window != 0 {
                let old_window = ReaperWindow::from_hwnd(
                    old_window as rea_rs_low::raw::HWND,
                );
                if old_window.is_ok() {
                    info!("rea-rs window demo is already open");
                    return Ok(());
                }
                DEMO_WINDOW.store(0, Ordering::Release);
            }

            let window = Reaper::get().create_window(
                &WindowSpec::new("rea-rs window demo")
                    .size(420, 240)
                    .dock_ident("rea_rs_window_demo")
                    .no_close(false)
                    .resizable(true),
            )?;
            let hwnd = window.hwnd() as isize;
            Reaper::get_mut()
                .register_window_handler(Box::new(DemoWindow { window }))?;
            DEMO_WINDOW.store(hwnd, Ordering::Release);
            info!("rea-rs window demo opened");
            Ok(())
        },
        None,
    )?;

    reaper.register_action(
        "TestReaRsWindowDockToggle",
        "test rea-rs windowing: toggle dock/float",
        ActionKind::NotToggleable,
        |_| {
            let hwnd = DEMO_WINDOW.load(Ordering::Acquire);
            if hwnd == 0 {
                info!("open the rea-rs windowing action first");
                return Ok(());
            }
            let window =
                ReaperWindow::from_hwnd_with_dock_ident(
                    hwnd as rea_rs_low::raw::HWND,
                    Some("rea_rs_window_demo".to_string()),
                )?;
            if window.is_docked()? {
                window.float()?;
            } else {
                window.dock(
                    "rea-rs window demo",
                    "rea_rs_window_demo",
                    true,
                )?;
            }
            info!("manual docking toggle requested");
            Ok(())
        },
        None,
    )?;
    Ok(())
}
