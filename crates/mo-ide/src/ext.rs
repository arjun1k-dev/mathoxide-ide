//! Dependency façade for the IDE — the only place that names the UI crates.
//!
//! Every other module in this crate imports GUI types through `crate::ext`
//! (`use crate::ext::egui::{...}`), never `use egui::...` / `use eframe::...`
//! directly. On an egui/eframe/egui-wgpu version bump, fix paths and shim
//! changed APIs here first; most edits should stay in this one file.
//!
//! Note: method-level calls on values (e.g. `ui.button(...)`) still resolve
//! against the real crates — this seam centralizes *import paths and type
//! identities*, which is where the bulk of upgrade churn lands.

pub use egui;
pub use eframe;
pub use egui_wgpu;
pub use wgpu;
pub use image;
