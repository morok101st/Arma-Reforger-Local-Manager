//! Ablauf eines Syncs: Mod-Liste lesen und zwischenspeichern. Download und Orchestrierung folgen in T3.

mod cache;
mod list_reader;

pub use cache::{CachedModList, ModListCache};
pub use list_reader::{ServerListError, ServerListReader};
