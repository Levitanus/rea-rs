# rea-rs-test

Makes testing of REAPER extension plugins easy.

This integration test suite was originally written by Benjamin Klum <benjamin.klum@helgoboss.org> for `reaper-rs`. But it was dependent on the `reaper-high` crate, which was not and would not be soon published. And, also, it was deeply integrated into the library.

This version incapsulates as much as possible, leaving simple interface to making tests.

This crate provides a runner for testing a REAPER extension plug-in
(`cdylib`) inside a real REAPER process. In this repository, the plug-in is
the workspace package `reaper-test-extension-plugin` in `test/`, and the
integration-test executable is `test/tests/integration_test.rs`.

For another project, add a non-published `cdylib` test package and a host-side
integration test to its Cargo workspace. The test plug-in must depend on
`rea-rs-test` and call `ReaperTest::setup` during extension initialization.
The runner sets `RUN_REAPER_INTEGRATION_TEST` and a unique result-file path;
the plug-in writes a PASS/FAIL result after test steps complete. REAPER's
process exit status is reported separately from the plugin outcome. Overall
success requires a PASS result and normal host exit code 0.

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

The repository's `rea-rs.sublime-project` includes Linux-specific tasks to
build/copy the test extension and launch the extracted REAPER instance. Use
`Build & Copy Integration Test` to prepare the plug-in. `Run REAPER` launches
the host manually; `Run REAPER Integration Test` runs the cargo harness and
launches REAPER through it. The paths and `.so` destination are specific to
Linux and use `target/reaper`.

VS Code tasks can be run from **Terminal → Run Task** after opening the
repository root. The `Build REAPER integration test` task builds the extension;
the `Run REAPER integration test` task invokes the host-driven Cargo test and
launches REAPER through the runner.

## Hint

Use crates `log` and `env_logger` for printing to stdio. integration test turns env logger on by itself.
