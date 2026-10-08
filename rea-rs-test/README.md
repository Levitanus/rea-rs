# rea-rs-test

`rea-rs-test` makes it easier to test a REAPER extension plug-in in the real
host. The idea is to keep a small, non-published test extension next to your
library, register named test steps from its entry point, and let a Cargo
integration test build and run it inside REAPER.

This integration test suite was originally written by Benjamin Klum <benjamin.klum@helgoboss.org> for `reaper-rs`. But it was dependent on the `reaper-high` crate, which was not and would not be soon published. And, also, it was deeply integrated into the library.

This version keeps the runner separate from the library under test and aims
to leave you with a small interface for writing those tests.

This crate provides a runner for testing a REAPER extension plug-in
(`cdylib`) inside a real REAPER process. In this repository, the plug-in is
the workspace package `reaper-test-extension-plugin` in `test/`, and the
integration-test executable is `test/tests/integration_test.rs`.

For your project, add a non-published `cdylib` test package and a host-side
integration test to its Cargo workspace. The test plug-in depends on
`rea-rs-test` and calls `ReaperTest::setup` while the extension is
initializing. Add each test as a named `TestStep`; a step can use the
initialized `Reaper` to call your library and check its behavior.

`ReaperTest::setup` also registers a REAPER Action. When you load the test
plug-in manually, run that action from REAPER's Actions list to execute the
steps yourself. When the host-side runner starts REAPER, it sets
`RUN_REAPER_INTEGRATION_TEST`, and the test plug-in runs the steps
automatically. It writes a PASS/FAIL result file, which the runner checks
separately from REAPER's process exit status. A successful test needs both a
PASS result and a normal host exit with code 0; a host crash, timeout, missing
result, or malformed result is reported as an error.

Example workspace layout:

```bash
workspace_directory
├── Cargo.toml
├── README.md
├—— my_lib
├   ├—— src
│      └── lib.rs
└── test
    ├── Cargo.toml
    ├── src
    │   └── lib.rs
    └── tests
        └── integration_test.rs
```

The test plug-in is not published. Adapt the package names and paths for your
workspace. In this repository, `rea-rs`, `rea-rs-low`, and `rea-rs-macros`
are path dependencies on sibling workspace packages.

```toml
[package]
edition = "2021"
name = "reaper-test-extension-plugin"
publish = false
version = "1.0.0"

[dependencies]
rea-rs = { path = "../rea-rs" }
rea-rs-macros = { path = "../macros" }
rea-rs-test = { path = "../rea-rs-test" }
my_lib = {path = "../my_lib"}

[lib]
crate-type = ["cdylib"]
name = "reaper_test_extension_plugin"

```

contents of `test/tests/integration_test.rs`:

```rust
use rea_rs_test::{run_integration_test, ReaperVersion};
#[test]
fn main() {
    run_integration_test(ReaperVersion::latest());
}
```

`ReaperVersion::latest()` currently selects REAPER 7.82. To exercise the same
test extension on another bundled host version, choose a specific variant
instead, such as `ReaperVersion::V6_73`, `ReaperVersion::V7_78`, or
`ReaperVersion::V7_82` (`V6_71` is available too). Run the integration test
with each version you want to cover. This is useful for checking behavior
across the REAPER versions your extension supports.

`test/src/lib.rs` is the file your integration tests are placed in.

```rust
use rea_rs_macros::reaper_extension_plugin;
use rea_rs_test::*;
use rea_rs::{PluginContext, Reaper};
fn hello_world(reaper: &mut Reaper) -> TestStepResult {
    reaper.show_console_msg("Hello world!");
    Ok(())
}
#[reaper_extension_plugin]
fn test_extension(context: PluginContext) -> anyhow::Result<()> {
    // setup test global environment
    let test = ReaperTest::setup(context, "test_action");
    // Push single test step.
    test.push_test_step(TestStep::new("Hello World!", hello_world));
    Ok(())
}
```

## Running the integration test in this repository

Run commands from the repository root. A working Linux environment needs
Rust/Cargo, a C++ compiler and REAPER's normal GUI/runtime dependencies. The
runner downloads the configured REAPER build under `target/reaper`, installs
the freshly built test extension in that REAPER copy's `UserPlugins` folder,
starts a new REAPER process, and waits up to 300 seconds. The first run needs
network access to download REAPER. macOS uses the configured disk image and
requires `hdiutil`; Windows is currently reported as unsupported/skipped.

```sh
cargo build -p reaper-test-extension-plugin
cargo test -p reaper-test-extension-plugin --test integration_test
```

The integration-test harness also builds the plug-in package before copying it
so the installed extension matches the current source. To run non-hosted unit
tests and doctests:

```sh
cargo test --workspace --exclude reaper-test-extension-plugin
```

The runner distinguishes plugin-reported PASS/FAIL from REAPER termination.
A missing or malformed result file, a timeout, or a host crash/termination
without a plugin result is an error. A PASS result and normal REAPER exit code
0 are both required for success.

## Editor build/run workflows

The repository includes editor shortcuts so you don't need to remember the
longer build and launch steps. In Sublime Text, the
`rea-rs.sublime-project` defines build systems to build/copy the test
extension, launch the downloaded REAPER instance, and run the host-driven
integration test. `Build & Copy Integration Test` prepares the plug-in;
`Run REAPER` is handy when you want to trigger the registered test Action
yourself; `Run REAPER Integration Test` runs the Cargo harness and starts
REAPER automatically. The manual build/copy and launch paths are Linux-only
and point into `target/reaper`.

In VS Code, open the repository root and choose **Terminal → Run Task**.
`.vscode/tasks.json` contains matching tasks: **Build & Copy REAPER
integration test**, **Launch downloaded REAPER**, and **Run REAPER integration
test**. The build/copy and manual launch tasks are Linux-specific; the Cargo
runner handles the supported Linux/macOS hosted-test setup.

## Hint

Use crates `log` and `env_logger` for printing to stdio. integration test turns env logger on by itself.
