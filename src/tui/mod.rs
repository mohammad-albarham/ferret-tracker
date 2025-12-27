//! TUI module for Ferret
//!
//! This module contains all the terminal user interface components
//! built with Ratatui.

pub mod app;
pub mod detail_view;
pub mod filters;
pub mod help;
pub mod list_view;
pub mod input;
pub mod tree_view;

pub use app::App;
