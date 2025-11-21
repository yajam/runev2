//! Intermediate representation for Rune packages.

#![allow(clippy::all)]

#[cfg(feature = "cssv2")]
pub(crate) mod css;
pub mod data;
pub mod html;
pub mod logic;
pub mod package;
pub mod schema;
pub mod view;

pub use package::{RuneManifest, RunePackage, TableOfContents};

// Re-export selected CSS v2 types and helpers for tools/binaries without exposing the whole module.
#[cfg(feature = "cssv2")]
pub use css::{
    BackgroundLayer2, ComputedStyle2, Display2, StyleSheet, apply_ua_defaults,
    build_stylesheet_from_html,
};
