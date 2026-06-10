//! Bevy messages emitted by the loading and transition pipeline.

use bevy::prelude::*;

/// Bevy event fired to request that a world file is loaded and spawned.
#[derive(Debug, Clone, Message)]
pub struct LdtkSpawnWorldEvent {
    /// Asset-relative path to the `.ldtk` file to load.
    pub world_path: String,
}

/// Bevy event fired once after the [`LdtkMapCatalog`](crate::LdtkMapCatalog)
/// has been fully populated for a world.
#[derive(Debug, Clone, Message)]
pub struct LdtkMapLoadedEvent {
    /// LDtk identifier of the world that was loaded.
    pub world_identifier: String,
}

/// Bevy event fired when a level becomes the active level after a transition.
#[derive(Debug, Clone, Message)]
pub struct LdtkLevelActivatedEvent {
    /// LDtk identifier of the level that was activated.
    pub level_identifier: String,
}

/// Bevy event fired after the active world has been fully despawned.
#[derive(Debug, Clone, Message)]
pub struct LdtkWorldUnloadedEvent;

/// Bevy event fired after an on-load validation pass completes, summarising the
/// number of issues found.
#[derive(Debug, Clone, Message)]
pub struct LdtkValidationFinishedEvent {
    /// Number of validation warnings produced.
    pub warnings: usize,
    /// Number of validation errors produced.
    pub errors: usize,
}
