//! Headless tests compile the same backend used by the desktop executable.
#![allow(dead_code)]
#[cfg(not(feature = "desktop"))]
#[path = "main.rs"]
mod backend;
