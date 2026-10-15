//! RGB wallet
//!
//! This module defines the [`Wallet`] related modules.

pub(crate) mod backup;
pub mod idb_store;
#[cfg(feature = "esplora")]
mod incoming_history;
#[cfg(feature = "esplora")]
mod incoming_selection;
mod input_availability;
pub(crate) mod memory_store;
pub(crate) mod offline;
#[cfg(feature = "esplora")]
pub(crate) mod online;
pub mod pending_fill;
pub mod rust_only;
pub mod vss;

pub use offline::*;
#[cfg(feature = "esplora")]
pub use online::*;

use super::*;
