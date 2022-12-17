# rea-rs

![linux](https://github.com/Levitanus/rea-rs/actions/workflows/build-linux.yml/badge.svg)
![windows](https://github.com/Levitanus/rea-rs/actions/workflows/build-windows.yml/badge.svg)
![macos](https://github.com/Levitanus/rea-rs/actions/workflows/build-macos.yml/badge.svg)

Easy to use ReaScript API.
While [reaper-rs](https://github.com/helgoboss/reaper-rs) is full-implemented at low-level, and, partially implemented at medium-level, on top of it (mostly, on top of low-level) this crate builds API that is pleasure to use. Actually, for the moment it is the better version of [Reapy](https://github.com/Levitanus/reapy-boost) project.

[See the docs](https://levitanus.github.io/rea-rs-doc/rea_rs/index.html)

The main skeleton of this API is cloned from the Reapy, but reimplemented in a more "rusty" way. Also, a bunch of new functions are added to [Track](https://levitanus.github.io/rea-rs-doc/rea_rs/track/struct.Track.html),
[Item](https://levitanus.github.io/rea-rs-doc/rea_rs/item/struct.Item.html) and [Take](https://levitanus.github.io/rea-rs-doc/rea_rs/take/struct.Take.html), as well as a good new implementation for [ExtState](https://levitanus.github.io/rea-rs-doc/rea_rs/ext_state/struct.ExtState.html) and [midi](https://levitanus.github.io/rea-rs-doc/rea_rs/midi/index.html) was made. I would say, that currently wrapped ~95% of Track, Take,
Item, [AudioAccessor](https://levitanus.github.io/rea-rs-doc/rea_rs/audio_accessor/struct.AudioAccessor.html) and [FX](https://levitanus.github.io/rea-rs-doc/rea_rs/fx/trait.FX.html) original functions; about of 70% for
[Envelope](https://levitanus.github.io/rea-rs-doc/rea_rs/envelope/struct.Envelope.html) and [Source](https://levitanus.github.io/rea-rs-doc/rea_rs/source/struct.Source.html). And the rest is probably, less, then 50%.
It should also be possible to use from VST Plugin, but this has not yet
been tested at all.

Almost everything needed to communicate to crate is re-exported (like [reaper_medium](https://levitanus.github.io/rea-rs-doc/reaper_medium/index.html) and [reaper_low](https://levitanus.github.io/rea-rs-doc/reaper_low/index.html) types), but for comfortably making extension-plugin entry-point it's better to also use reaper-macros dependency:

```toml
[dependencies]
reaper-macros = {git = "https://github.com/Levitanus/reaper-rs", branch = "stable_for_rea-rs"}
```

Until there is no new version of `reaper-rs` which differs from the current master branch a lot, this is the dependency list I highly recommend:

```toml
[dependencies]
rea-rs = {git = "https://github.com/Levitanus/rea-rs"}
reaper-low = "0.1.0"
reaper-macros = "0.1.0"
reaper-medium = "0.1.0"
[patch.crates-io]
reaper-low = {git = "https://github.com/Levitanus/reaper-rs", branch = "stable_for_rea-rs"}
reaper-macros = {git = "https://github.com/Levitanus/reaper-rs", branch = "stable_for_rea-rs"}
reaper-medium = {git = "https://github.com/Levitanus/reaper-rs", branch = "stable_for_rea-rs"}
```

But, actually, all medium- and low-level functionality is still existing in the [Reaper](https://levitanus.github.io/rea-rs-doc/rea_rs/reaper/struct.Reaper.html) object. Just use `Reaper::low`, `Reaper::medium` and `Reaper::medium_session`. The Common entry point should look like this:

```rust
use rea_rs::{errors::ReaperResult, ActionKind, Reaper, PluginContext};
use reaper_macros::reaper_extension_plugin;
use std::error::Error;
#[reaper_extension_plugin]
fn plugin_main(context: PluginContext) -> Result<(), Box<dyn Error>> {
    Reaper::load(context);
    let reaper = Reaper::get_mut();
    let message = "Hello from small extension";
    reaper.show_console_msg(message);
    Ok(())
}
```

Since, there are not many things to be done at the start time of Reaper, there are two common ways to invoke the code: Actions and `ControlSurface`.

```rust
use rea_rs::{
ActionKind, ControlSurface, PluginContext, Reaper, RegisteredAction,
};
use reaper_macros::reaper_extension_plugin;
use std::error::Error;
#[derive(Debug)]
struct Listener {
    action: RegisteredAction,
}
// Full list of function larger.
impl ControlSurface for Listener {
    fn run(&mut self) {
        Reaper::get().perform_action(self.action.command_id, 0, None);
    }
}
fn my_action_func(_flag: i32) -> Result<(), Box<dyn Error>> {
    Reaper::get().show_console_msg("running");
    Ok(())
}
#[reaper_extension_plugin]
fn plugin_main(context: PluginContext) -> Result<(), Box<dyn Error>> {
    Reaper::load(context);
    let reaper = Reaper::get_mut();
    let action = reaper.register_action(
        // This will be capitalized and used as action ID in action window
        "command_name",
        // This is the line user searches action for
        "description",
        my_action_func,
        // Only type currently supported
        ActionKind::NotToggleable,
    )?;
    reaper
        .medium_session_mut()
        .plugin_register_add_csurf_inst(Box::new(Listener { action })).unwrap();
    Ok(())
}
```

There are float values in API. I recommend to use `float_eq` crate.

## API structure

Most of the time, API is used hierarchically: [Reaper](https://levitanus.github.io/rea-rs-doc/rea_rs/reaper/struct.Reaper.html) holds top-level functions and can return [Project](https://levitanus.github.io/rea-rs-doc/rea_rs/project/struct.Project.html), [Item](https://levitanus.github.io/rea-rs-doc/rea_rs/item/struct.Item.html) etc. While [Project](https://levitanus.github.io/rea-rs-doc/rea_rs/project/struct.Project.html) can manipulate by [Track](https://levitanus.github.io/rea-rs-doc/rea_rs/track/struct.Track.html), [Item](https://levitanus.github.io/rea-rs-doc/rea_rs/item/struct.Item.html), [Take](https://levitanus.github.io/rea-rs-doc/rea_rs/take/struct.Take.html). The key point of the hierarchical structure — to be sure safe as long as possible. Since Project is alive, it is safe to refer from track to it. The same with other children. By the same reason, it's almost impossible to mutate two object at a time. If one track is mutable, it responses for the whole underlying objects. And we can be almost sure, that the rest of tracks consist of objects, we left them before. The most part of API is covered by [tests](https://github.com/Levitanus/rea-rs/blob/main/test/test/src/tests.rs), and they are a good set of usage examples.

```rust
use rea_rs::Reaper;
use std::collections::HashMap;
let rpr = Reaper::get();
let captions =
vec!["age(18)", "name(user)", "leave blank", "fate(atheist)"];
let mut answers = HashMap::new();
answers.insert(String::from("age(18)"), String::from("18"));
answers.insert(String::from("name(user)"), String::from("user"));
answers.insert(String::from("leave blank"), String::from(""));
answers.insert(String::from("fate(atheist)"), String::from("atheist"));
let result = rpr.get_user_inputs(
    "Fill values as asked in fields",
    captions,
    None,
).unwrap();
assert_eq!(result, answers);
```

## Better to know about

For the moment, downsides of API are:

- top-level functionality: I'm not sure, that at least a half of little
  reaper functions is wrapped. Like all windowing and theming stuff.
- GUI. As well as with `reapy`, GUI is an issue. In the long perspective, I
  feel that [egui](https://github.com/emilk/egui) backend in the `Win32`
  and `Swell` should be made. But at the moment, possibly, any backend of
  `egui` will suit.
- Thread-safety. It's important to know, that almost nothing of [Reaper](https://levitanus.github.io/rea-rs-doc/rea_rs/reaper/struct.Reaper.html)
  should left the main thread. There are some functions, that are designed
  for audio thread, and some, that are safe to execute from any thread.
  But, basically, here is a rule: if you make a listener, gui or socket
  communication — `Reaper` lives in main thread, and else made by
  `std::sync::mpsc`.
Enjoy the coding!
