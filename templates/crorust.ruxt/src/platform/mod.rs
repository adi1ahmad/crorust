//! Platform Abstraction Layer (PAL)
//!
//! Encapsulates OS windowing, native file pickers, and platform runtime bindings.

pub mod dialog;
pub mod svg_loader;

pub use svg_loader::UniversalSvgLoader;

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use dialog::{
    PickedFile, clear_picked_file, deliver_picked_file, deliver_picked_path, pick_file,
    pick_file_async, poll_picked_file_subscriber,
};

#[cfg(target_arch = "wasm32")]
pub use web::*;
