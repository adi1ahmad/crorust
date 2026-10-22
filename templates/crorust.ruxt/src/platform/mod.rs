//! Platform Abstraction Layer (PAL)
//!
//! Encapsulates OS windowing, native file pickers, and platform runtime bindings.

pub mod dialog;

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use dialog::{pick_file, poll_picked_file};

#[cfg(target_arch = "wasm32")]
pub use web::*;
