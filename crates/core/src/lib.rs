//! senda-core — domain types shared by every Senda crate.
//!
//! This crate has no business logic. It only defines the structs and enums
//! that flow through the system: [`Agent`], [`Repo`], [`Automation`], etc.
//! All types derive [`serde::Serialize`] / [`Deserialize`] and (when relevant)
//! [`specta::Type`] so they can cross the Tauri IPC boundary into TypeScript.

pub mod agent;
pub mod automation;
pub mod cli;
pub mod repo;
pub mod run;

pub use agent::*;
pub use automation::*;
pub use cli::*;
pub use repo::*;
pub use run::*;
