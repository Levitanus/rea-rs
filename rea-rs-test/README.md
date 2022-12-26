# reaper-test

Makes testing of REAPER extension plugins easy.

This integration test suite was originally written by Benjamin Klum <benjamin.klum@helgoboss.org> for `reaper-rs`. But it was dependent on the `reaper-high` crate, which was not and would not be soon published. And, also, it was deeply integrated into the library.

This version incapsulates as much as possible, leaving simple interface to making tests.

For testing reaper extension, which itself is of type `cdylib`,
you need transform the project folder to workspace. So, basically,
project tree would look similar to this:

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

`test` crate will not be delivered to the end-user, but will be used for
testing your library. Since there is a need for patching of reaper-low and reaper-medium, contents of `test/Cargo.toml`:

```toml
[package]
edition = "2021"
name = "reaper-test-extension-plugin"
publish = false
version = "0.1.0"

[dependencies]
reaper-low = "0.1.0"
reaper-macros = "0.1.0"
reaper-medium = "0.1.0"
reaper-test = "0.1.0"
my_lib = {path = "../my_lib"}

[patch.crates-io]
reaper-low = {git = "https://github.com/Levitanus/reaper-rs", branch = "stable_for_rea-rs"}
reaper-macros = {git = "https://github.com/Levitanus/reaper-rs", branch = "stable_for_rea-rs"}
reaper-medium = {git = "https://github.com/Levitanus/reaper-rs", branch = "stable_for_rea-rs"}
reaper-test = {git = "https://github.com/Levitanus/reaper-test"}

[lib]
crate-type = ["cdylib"]
name = "reaper_test_extension_plugin"

```

contents of `test/tests/integration_test.rs`:

```rust
use reaper_test::{run_integration_test, ReaperVersion};
#[test]
fn main() {
    run_integration_test(ReaperVersion::latest());
}
```

`test/src/lib.rs` is the file your integration tests are placed in.

```rust
use reaper_macros::reaper_extension_plugin;
use reaper_test::*;
use std::error::Error;
fn hello_world(reaper: &ReaperTest) -> TestStepResult {
    reaper.medium().show_console_msg("Hello world!");
    Ok(())
}
#[reaper_extension_plugin]
fn test_extension(context: PluginContext) -> Result<(), Box<dyn Error>> {
    // setup test global environment
    let test = ReaperTest::setup(context, "test_action");
    // Push single test step.
    test.push_test_step(TestStep::new("Hello World!", hello_world));
    Ok(())
}
```

to run integration tests, go to the test folder and type:
`cargo build --workspace; cargo test`

## Hint

Use crates `log` and `env_logger` for ptinting to stdio. integration test turns env logger on by itself.
