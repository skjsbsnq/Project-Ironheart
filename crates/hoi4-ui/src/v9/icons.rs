//! V9 icon-bank entry point.
//!
//! The loader remains asset-backed and cache-compatible; this module gives V9
//! panels a stable import path while old callers keep using `crate::icons`.

pub use crate::icons::{image_from_handle, show_icon, IconBank};
