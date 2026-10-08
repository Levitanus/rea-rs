# rea-rs

![linux](https://github.com/Levitanus/rea-rs/actions/workflows/build-linux.yml/badge.svg)
![windows](https://github.com/Levitanus/rea-rs/actions/workflows/build-windows.yml/badge.svg)
![macos](https://github.com/Levitanus/rea-rs/actions/workflows/build-macos.yml/badge.svg)

`rea-rs` is a high-level Rust API for working with REAPER from an extension.
It builds on [reaper-rs](https://github.com/helgoboss/reaper-rs) and the
REAPER SDK, and its design was inspired by
[ReaPy](https://github.com/Levitanus/reapy-boost). I wanted the everyday
parts of that API to feel at home in Rust, so `rea-rs` uses ownership and
lifetimes to manage host objects, typed values to make common operations
clearer, and fallible methods when REAPER can report an error. It covers
projects, tracks, media items and takes, MIDI, effects, envelopes, sources,
and SWELL GUI.

[API documentation](https://levitanus.github.io/rea-rs-doc/rea_rs/index.html)
· [Crate on crates.io](https://crates.io/crates/rea-rs)
· [1.0.0 API audit and migration notes](RELEASE_1.0.0_API_AUDIT.md)

It's a pleasure to see people building things with this crate. If you're new
to it, start with the examples below and the API docs; please open an issue
when something is unclear or you run into a missing wrapper.

`rea-rs` is the high-level crate in this workspace. For most extensions,
start with `rea-rs` and the macros; add the low-level crate only when you
need direct access to the REAPER SDK bindings:

```toml
[dependencies]
rea-rs = "1.0.0"
rea-rs-macros = "1.0.0"
rea-rs-low = "1.0.0" # optional: raw REAPER/SDK bindings
```

You can also reach lower-level functionality through `Reaper::low`,
`Reaper::medium`, and `Reaper::medium_session`.

## API status

The breaking 1.0 API is the one to build against; old pre-1.0 names are not
kept as compatibility aliases. Not every function in REAPER's native API is wrapped, and coverage varies by
area.

## API structure

The easiest way to find your way around is to start at `Reaper`. Initialize
it with the plug-in's `PluginContext` and use it for host services, the
current project, and registering actions, timers, control surfaces, and
windows. From a `Project`, work with `Track`s and `Item`s; an `Item` gives you
its active `Take`. For more focused tasks, there are also `Midi`, `FX`,
`Envelope`, `Source`, `AudioAccessor`, and `ExtState` APIs. The `swell_gui`
module is where the windowing, drawing, and widget types live.

Most host operations return `ReaperResult<T>`; callback interfaces commonly
use `anyhow::Result`. These errors are there to help you handle the cases
where a native REAPER operation cannot complete, so propagate or handle them
instead of silently ignoring them.

One important thing to know: REAPER objects do not always share the lifetime
of the Rust handle or of the project/item that produced them. A host object
can be removed while a Rust wrapper still exists, leaving its native pointer
stale. `rea-rs` checks pointer validity through the `WithReaperPtr` trait;
ordinary object methods go through `get()` and perform that check internally,
so you usually don't need to call the validation yourself.

Pointer validation has a cost. If you're doing a batch of work with one
object, `WithReaperPtr::with_valid_ptr` checks it once, then temporarily
disables the repeated checks while your closure runs, and restores checking
afterward (including if the closure panics). Use this only when the object is
expected to remain valid for the whole closure; don't remove, replace, or
otherwise invalidate it during that batch.

## Basic use: read and set the selected item name

To rename a selected media item, you work with its active take: that's where
REAPER stores the displayed name. Here's a small example that reads the first
selected item's current name and adds a suffix. It does nothing if no item is
selected:

```rust,no_run
use rea_rs::Reaper;

fn rename_first_selected_item() -> anyhow::Result<()> {
    let project = Reaper::get().current_project();
    if let Some(item) = project.get_selected_item(0)? {
        let mut take = item.active_take()?;
        let current_name = take.name()?;
        take.set_name(format!("{current_name} (edited)"))?;
    }
    Ok(())
}
```

## Extension entry point and callbacks

When REAPER loads an extension, the macro below exposes `plugin_main` as its
entry point. This is a compact tour of the usual registrations: an Action for
something the user can trigger, a `Timer` for periodic work, a
`ControlSurface` for REAPER's control-surface callbacks, and a
`WindowHandler` for your SWELL window. REAPER calls each callback when it is
needed, after initialization has finished.

```rust,no_run
use rea_rs::{
  ActionHook, ActionKind, ActionRegistrationOptions, ControlSurface,
  PluginContext, Reaper, ReaperWindow, Section, SwellId, Timer,
  WindowHandler, WindowSpec,
};
use rea_rs_macros::reaper_extension_plugin;
use std::{cell::RefCell, sync::Arc, time::Duration};

#[derive(Debug)]
struct DemoSurface;

impl ControlSurface for DemoSurface {
    fn get_type_string(&self) -> String { "REARSRUSTDEMO".into() }
    fn get_desc_string(&self) -> String { "rea-rs example surface".into() }
}

struct DemoTimer;

impl Timer for DemoTimer {
    fn run(&mut self) -> anyhow::Result<()> {
        Reaper::get().show_console_msg("rea-rs timer tick")?;
        Ok(())
    }
    fn id_string(&self) -> String { "rea-rs-example-timer".into() }
    fn interval(&self) -> Duration { Duration::from_secs(1) }
}

struct DemoWindow { window: ReaperWindow }

impl WindowHandler for DemoWindow {
    fn window_id(&self) -> rea_rs::WindowId { "rea-rs-example-window".into() }
    fn window(&self) -> &ReaperWindow { &self.window }
    fn on_timer(&mut self, _id: SwellId) -> anyhow::Result<()> {
        // Handle a window timer here if one was started with start_timer().
        Ok(())
    }
}

#[reaper_extension_plugin]
fn plugin_main(context: PluginContext) -> anyhow::Result<()> {
    Reaper::init_global(context);
    let reaper = Reaper::get_mut();

    reaper.register_action(
        "ReaRsExampleAction",
        "rea-rs: example action",
        ActionKind::NotToggleable,
        |_hook: &mut ActionHook| {
            Reaper::get().show_console_msg("action invoked")?;
            Ok(())
        },
        ActionRegistrationOptions::new(Section::Main),
    )?;
    reaper.register_timer(Arc::new(RefCell::new(DemoTimer)));
    reaper.register_control_surface(Arc::new(RefCell::new(DemoSurface)));

    let window = reaper.create_window(&WindowSpec::new("rea-rs example"))?;
    reaper.register_window_handler(Box::new(DemoWindow { window }))?;
    Ok(())
}
```

REAPER drives `ControlSurface::run` and your plug-in `Timer`; use a timer for
periodic work instead of polling from another thread. The registered
`WindowHandler` receives window events and any SWELL window-timer messages
you start. One important rule: keep most REAPER and GUI calls on REAPER's
UI/main thread. If worker threads need to communicate with host code, pass
messages back to that thread.

## GUI example

For a larger example, take a look at
[`test/src/swell_gui.rs`](test/src/swell_gui.rs). It registers an action that
opens a dockable widget gallery, implements both `ControlSurface` and
`WindowHandler`, and keeps the UI in sync with project state. It also shows a
MIDI-editor paint-over action. The example's opening comments explain how to
run it and where to start when adapting it for your own extension.

The host test extension in [`test/src/lib.rs`](test/src/lib.rs) has more
examples, and [`rea-rs-test/README.md`](rea-rs-test/README.md) explains how
to run the host-based integration suite. For comparing floating-point
values, I recommend taking a look at
[`float_eq`](https://crates.io/crates/float_eq).

## Testing with `rea-rs-test`

`rea-rs-test` runs an extension test plug-in inside a real REAPER process.
The basic setup is to keep your normal library/plugin separate from a small,
non-published `cdylib` test plug-in, then add a host-side Cargo integration
test. In the test plug-in's entry point, call `ReaperTest::setup` and add
named `TestStep`s. Each step receives the initialized `Reaper`; return an
error when an assertion or host operation fails:

```rust,no_run
use rea_rs::{PluginContext, Reaper};
use rea_rs_macros::reaper_extension_plugin;
use rea_rs_test::{ReaperTest, TestStep, TestStepResult};

fn check_something(reaper: &mut Reaper) -> TestStepResult {
    reaper.show_console_msg("Running an integration-test step")?;
    // Add assertions and calls into your library here.
    Ok(())
}

#[reaper_extension_plugin]
fn test_extension(context: PluginContext) -> TestStepResult {
    let tests = ReaperTest::setup(context, "my test action");
    tests.push_test_step(TestStep::new("Check something", check_something));
    Ok(())
}
```

The setup registers the named REAPER Action as the manual entry point for
your test steps. When the runner launches REAPER, it sets
`RUN_REAPER_INTEGRATION_TEST`; `ReaperTest::setup` notices this and schedules
the steps to run automatically. Outside the runner, open REAPER's Actions
list and invoke the registered test action yourself. The runner checks both
the plug-in's PASS/FAIL result and REAPER's process termination, so a host
crash is not mistaken for a passing test.

The host-side integration test selects which REAPER build to use. The runner
currently provides `ReaperVersion::V6_71`, `V6_73`, `V7_78`, and `V7_82`;
`ReaperVersion::latest()` currently means 7.82. To check another host version,
use that variant in `run_integration_test` and run the integration test again.
This lets the same test steps exercise your extension against different
supported REAPER versions.

For example, this workspace uses `test/` for the test plug-in and runner
invocation. From the repository root, run
`cargo test -p reaper-test-extension-plugin --test integration_test`.
The first run downloads the selected REAPER build into `target/reaper` and
needs network access. Linux needs REAPER's GUI/runtime dependencies; macOS
uses the configured disk image and `hdiutil`; Windows silently installs an
isolated portable x64 REAPER copy under `target/reaper`.

There are also editor shortcuts for the same workflow. The Sublime Text
project (`rea-rs.sublime-project`) defines build systems to build/copy the
test plug-in, launch the downloaded REAPER manually, and run the host-driven
integration test. VS Code's `.vscode/tasks.json` provides the corresponding
tasks; open the repository root and choose **Terminal → Run Task**. These
convenience build/copy and launch tasks are Linux-specific; the Cargo runner
handles hosted tests on Linux, macOS, and Windows. See
[`rea-rs-test/README.md`](rea-rs-test/README.md) for the workspace layout and
more details.

## Compatibility and limitations

The crate manifest currently declares Rust 1.82 as its minimum. REAPER host
operations and most `Reaper` state are not thread-safe, so follow each API's
threading contract and keep UI work on the host thread. There are still less
common REAPER functions and GUI details that aren't covered yet; the API
docs and audit are the best places to check what is available today.
