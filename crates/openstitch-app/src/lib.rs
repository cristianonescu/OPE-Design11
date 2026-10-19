//! OpenStitch desktop editor. [`editor`] holds the UI-independent logic (viewport maths,
//! hit-testing, selection handles, tool state machines, render-model building, inspector and
//! object-list models) so it can be unit-tested without a window; `main.rs` and `ui` are the
//! thin egui shell that feeds it pointer events and runs the commands it returns through
//! [`openstitch_engine::Session::execute`].

pub mod editor;
pub mod png;
