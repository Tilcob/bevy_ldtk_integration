//! Marker components attached to spawned LDtk worlds, tiles, and entities.

use bevy::prelude::*;

/// Bevy [`Component`] marker placed on the root entity of a spawned LDtk world.
#[derive(Debug, Clone, Component, Default)]
pub struct LdtkWorldRoot;

/// Bevy [`Component`] marker that prevents an entity from being despawned during
/// level transitions.
#[derive(Debug, Clone, Component, Default)]
pub struct LdtkPersistent;

/// Bevy [`Component`] that records the collision role of a spawned tile or
/// entity.
#[derive(Debug, Clone, Component, Default)]
pub struct LdtkCollider {
    /// Whether this collider blocks movement.
    pub solid: bool,
    /// Whether this collider is a sensor (trigger-only, non-blocking).
    pub sensor: bool,
}

// Sub-module with a module-wide allow: rustc reports `deprecated` for the
// derive expansion of a deprecated type even when the item carries
// `#[allow(deprecated)]` itself.
mod deprecated {
    #![allow(deprecated)]

    use bevy::prelude::*;

    /// Bevy [`Component`] intended to link tile entities back to their source
    /// level and IntGrid value.
    #[deprecated(
        since = "0.1.0",
        note = "the plugin never inserts this component; query `LdtkCollider` or read \
                `LdtkCollisionCatalog` instead. It will be removed in a future release."
    )]
    #[derive(Debug, Clone, Component, Default)]
    pub struct LdtkTileCollision {
        /// Identifier of the level that owns this tile.
        pub level_identifier: String,
        /// Index of the tile in its tileset.
        pub tile_id: i32,
        /// Whether the tile is a solid (impassable) collider.
        pub solid: bool,
    }
}

#[allow(deprecated)]
pub use deprecated::LdtkTileCollision;
