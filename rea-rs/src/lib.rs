//! Easy to use ReaScript API.
//!
//! While [reaper-rs](https://github.com/helgoboss/reaper-rs) is
//! full-implemented at low-level, and, partially implemented at medium-level,
//! on top of it (mostly, on top of low-level) this crate builds API that is
//! pleasure to use.
//!
//! Actually, for the moment it is the better version of
//! [Reapy](https://github.com/Levitanus/reapy-boost) project.
//! The main skeleton of this API is cloned from the Reapy, but reimplemented
//! in a more "rusty" way. Also, a bunch of new functions are added to [Track],
//! [Item] and [Take], as well as a good new implementation for [ExtState] and
//! [midi] was made. I would say, that currently wrapped ~95% of Track, Take,
//! Item, [AudioAccessor] and [FX] original functions; about of 70% for
//! [Envelope] and [Source]. And the rest is probably, less, then 50%.
//!
//! It should also be possible to use from VST Plugin, but this has not yet
//! been tested at all.
//!
//! These are the dependencies:
//! ```ignore
//! [dependencies]
//! rea-rs = "0.1.1"
//! rea-rs-low = "0.1.0" // optional
//! rea-rs-macros = "0.1.0"
//! ```
//!
//! But, actually, all medium- and low-level functionality is still existing in
//! the [Reaper] object. Just use [Reaper::low], [Reaper::medium] and
//! [Reaper::medium_session].
//!
//! The Common entry point should look like this:
//!
//! ```no_run
//! use rea_rs::{errors::ReaperResult, ActionKind, Reaper, PluginContext};
//! use rea_rs_macros::reaper_extension_plugin;
//! use std::error::Error;
//!
//! #[reaper_extension_plugin]
//! fn plugin_main(context: PluginContext) -> Result<(), Box<dyn Error>> {
//!     Reaper::load(context);
//!     let reaper = Reaper::get_mut();
//!     let message = "Hello from small extension";
//!     reaper.show_console_msg(message);
//!     Ok(())
//! }
//! ```
//!
//! Since, there are not many things to be done at the start time of Reaper,
//! there are two common ways to invoke the code: Actions and [ControlSurface].
//!
//! ```no_run
//! use rea_rs::{PluginContext, Reaper, RegisteredAccel, Timer};
//! use rea_rs_macros::reaper_extension_plugin;
//! use std::error::Error;
//!
//! #[derive(Debug)]
//! struct Listener {
//!     action: RegisteredAccel,
//! }
//!
//! // Full list of function larger.
//! impl Timer for Listener {
//!     fn run(&mut self) -> Result<(), Box<dyn Error>> {
//!         Reaper::get().perform_action(self.action.command_id, 0, None);
//!         Ok(())
//!     }
//!     fn id_string(&self) -> String {"test listener".to_string()}
//! }
//!
//! fn my_action_func(_flag: i32) -> Result<(), Box<dyn Error>> {
//!     Reaper::get().show_console_msg("running");
//!     Ok(())
//! }
//!
//! #[reaper_extension_plugin]
//! fn plugin_main(context: PluginContext) -> Result<(), Box<dyn Error>> {
//!     Reaper::load(context);
//!     let reaper = Reaper::get_mut();
//!
//!     let action = reaper.register_action(
//!         // This will be capitalized and used as action ID in action window
//!         "command_name",
//!         // This is the line user searches action for
//!         "description",
//!         my_action_func,
//!         // Only type currently supported
//!         None
//!     )?;
//!
//!     reaper.register_timer(Box::new(Listener{action}));
//!     Ok(())
//! }
//! ```
//!
//! There are float values in API. I recommend to use `float_eq` crate.
//!
//! # API structure.
//!
//! Most of the time, API is used hierarchically: [Reaper] holds top-level
//! functions and can return [Project], [Item] etc. While [Project] can
//! manipulate by [Track], [Item], [Take]. The key point of the hierarchical
//! structure — to be sure safe as long as possible. Since Project is alive, it
//! is safe to refer from track to it. The same with other children. By the
//! same reason, it's almost impossible to mutate two object at a time. If one
//! track is mutable, it responses for the whole underlying objects. And we can
//! be almost sure, that the rest of tracks consist of objects, we left them
//! before.
//!
//! The most part of API is covered by
//! [tests](https://github.com/Levitanus/rea-rs/blob/main/test/test/src/tests.rs),
//! and they are a good set of usage examples.
//!
//! ```no_run
//! use rea_rs::Reaper;
//! use std::collections::HashMap;
//!
//! let rpr = Reaper::get();
//! let captions =
//! vec!["age(18)", "name(user)", "leave blank", "fate(atheist)"];
//! let mut answers = HashMap::new();
//! answers.insert(String::from("age(18)"), String::from("18"));
//! answers.insert(String::from("name(user)"), String::from("user"));
//! answers.insert(String::from("leave blank"), String::from(""));
//! answers.insert(String::from("fate(atheist)"), String::from("atheist"));
//!
//! let result = rpr.get_user_inputs(
//!     "Fill values as asked in fields",
//!     captions,
//!     None,
//! ).unwrap();
//! assert_eq!(result, answers);
//! ```
//!
//! # Better to know about
//!
//! For the moment, downsides of API are:
//! - top-level functionality: I'm not sure, that at least a half of little
//!   reaper functions is wrapped. Like all windowing and theming stuff.
//! - GUI. As well as with `reapy`, GUI is an issue. In the long perspective, I
//!   feel that [egui](https://github.com/emilk/egui) backend in the `Win32`
//!   and `Swell` should be made. But at the moment, possibly, any backend of
//!   `egui` will suit.
//! - Thread-safety. It's important to know, that almost nothing of [Reaper]
//!   should left the main thread. There are some functions, that are designed
//!   for audio thread, and some, that are safe to execute from any thread.
//!   But, basically, here is a rule: if you make a listener, gui or socket
//!   communication — `Reaper` lives in main thread, and else made by
//!   [std::sync::mpsc].
//!
//! Enjoy the coding!

pub use rea_rs_low::PluginContext;

pub mod reaper;
pub use reaper::*;

pub mod ptr_wrappers;
pub mod reaper_pointer;

pub mod simple_functions;
pub use simple_functions::*;

pub mod hardware_functions;
pub use hardware_functions::*;

pub mod project;
pub use project::*;

pub mod utils;
pub use utils::WithReaperPtr;

pub mod misc_enums;
pub use misc_enums::*;

pub mod errors;
pub mod keys;

pub mod misc_types;
pub use misc_types::*;

pub mod ext_state;
pub use ext_state::*;

pub mod marker;
pub use marker::*;

pub mod track;
pub use track::*;

pub mod send;
pub use send::*;

pub mod item;
pub use item::*;

pub mod take;
pub use take::*;

pub mod source;
pub use source::*;

pub mod midi;
pub use midi::*;

pub mod fx;
pub use fx::*;

pub mod audio_accessor;
pub use audio_accessor::*;

pub mod envelope;
pub use envelope::*;

#[cfg(test)]
mod test;
