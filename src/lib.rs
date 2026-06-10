//! # ldtk_integration
//!
//! A Bevy plugin for loading and managing LDtk levels in 2D games.
//!
//! Rendering and asset loading are handled by [`bevy_ecs_ldtk`]; this crate
//! adds a game-oriented API on top: a world/level/layer catalog, typed entity
//! registration, IntGrid collision rules, level transitions with spawn-point
//! resolution, and tile-animation metadata.
//!
//! ## Quick start
//!
//! ```no_run
//! use bevy::prelude::*;
//! use ldtk_integration::{GameLdtkPlugin, LdtkConfig};
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(GameLdtkPlugin::new(
//!             LdtkConfig::default()
//!                 .with_world_asset_path("worlds/map.ldtk")
//!                 .with_solid_int_grid_values([1]),
//!         ))
//!         .run();
//! }
//! ```
//!
//! ## Feature flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `tilemap` | ✅ | Tile-animation adapter via `bevy_ecs_tilemap` |
//! | `external-level-fs` | ✅ | Read external `.ldtkl` files from disk (not WASM) |
//!
//! ## Crate layout
//!
//! Everything public is re-exported from the crate root. Use
//! `ldtk_integration::prelude::*` or import individual items directly.

// The LDtk extraction/transition systems thread a lot of Bevy state through
// wide signatures and complex `Query` types; this is inherent to the domain.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod animation;
mod capture;
mod catalog;
mod catalog_builder;
mod commands;
mod components;
mod config;
mod entities;
mod events;
mod external;
mod fields;
mod level_manager;
mod plugin;
mod registry;
mod state;
#[cfg(feature = "tilemap")]
mod tilemap_adapter;
mod validation;

// ── Plugins ───────────────────────────────────────────────────────────────────
pub use level_manager::LevelManagerPlugin;
pub use plugin::GameLdtkPlugin;

// ── Command extensions ────────────────────────────────────────────────────────
pub use commands::LdtkAppExt;
pub use commands::LdtkCommandExt;

// ── Config & rules ────────────────────────────────────────────────────────────
pub use config::LdtkCollisionRule;
pub use config::LdtkConfig;

// ── External level source ─────────────────────────────────────────────────────
pub use external::ExternalLevelSource;
#[cfg(feature = "external-level-fs")]
pub use external::FsExternalLevelSource;
pub use external::LdtkExternalLevelSource;
#[cfg(feature = "external-level-fs")]
pub use external::external_level_path;

// ── System sets ───────────────────────────────────────────────────────────────
pub use plugin::LdtkLoadSet;

// ── Resources: load state & catalog ──────────────────────────────────────────
pub use catalog::LdtkCollisionCatalog;
pub use catalog::LdtkEntityCatalog;
pub use catalog::LdtkMapCatalog;
pub use commands::LdtkCommandQueue;
pub use registry::LdtkEntityRegistry;
pub use state::LdtkLoadState;
pub use state::LdtkLoadStats;
pub use state::LdtkLoadStatus;
pub use state::LdtkRuntimeState;
pub use state::LdtkTransitionState;
pub use state::LdtkValidationIssue;
pub use state::LdtkValidationReport;

// ── Catalog data types ────────────────────────────────────────────────────────
pub use animation::LdtkTileAnimation;
pub use animation::LdtkTileAnimationFrame;
pub use animation::LdtkTileAnimator;
pub use animation::LdtkTileKey;
pub use catalog::LdtkCollisionCell;
pub use catalog::LdtkCollisionLayerInfo;
pub use catalog::LdtkDirection;
pub use catalog::LdtkLayerInfo;
pub use catalog::LdtkLayerType;
pub use catalog::LdtkLevelInfo;
pub use catalog::LdtkNeighbor;
pub use catalog::LdtkSpawnPoint;
pub use catalog::LdtkTileMetadata;
pub use catalog::LdtkTilesetInfo;
pub use catalog::LdtkWorldInfo;
pub use catalog::LdtkWorldLayout;

// ── Field values ──────────────────────────────────────────────────────────────
pub use fields::LdtkEntityReference;
pub use fields::LdtkFieldAccess;
pub use fields::LdtkFieldValue;
pub use fields::LdtkTilesetRect;

// ── Entity types ──────────────────────────────────────────────────────────────
pub use entities::LdtkEntityMarker;
pub use entities::LdtkEntitySpawnContext;
pub use entities::LdtkImportedEntity;
pub use registry::LdtkEntityRegistryKey;
pub use registry::LdtkEntitySpawner;

// ── Marker components ─────────────────────────────────────────────────────────
pub use components::LdtkCollider;
pub use components::LdtkPersistent;
#[allow(deprecated)]
pub use components::LdtkTileCollision;
pub use components::LdtkWorldRoot;

// ── Commands ──────────────────────────────────────────────────────────────────
pub use commands::LdtkCommand;

// ── Events ────────────────────────────────────────────────────────────────────
pub use events::LdtkLevelActivatedEvent;
pub use events::LdtkMapLoadedEvent;
pub use events::LdtkSpawnWorldEvent;
pub use events::LdtkValidationFinishedEvent;
pub use events::LdtkWorldUnloadedEvent;

// ── Level manager ─────────────────────────────────────────────────────────────
pub use level_manager::CurrentLdtkLevel;
pub use level_manager::LdtkCollisionReadyEvent;
pub use level_manager::LdtkLevelManagerConfig;
pub use level_manager::LdtkLevelPlayer;
pub use level_manager::LdtkLevelReadyEvent;
pub use level_manager::LdtkLevelScoped;
pub use level_manager::LdtkPlayerLocator;
pub use level_manager::LevelTransitionRequest;
pub use level_manager::LevelTransitionState;
pub use level_manager::LevelTransitionStatus;
pub use level_manager::PendingLdtkLevelTransition;
pub use level_manager::advance_tile_animation;

/// Re-exports the most commonly used items for glob imports.
///
/// ```no_run
/// use ldtk_integration::prelude::*;
/// ```
pub mod prelude {
    pub use super::*;
}

/// Backwards-compatibility shim for the pre-0.1 module layout
/// (`ldtk_integration::ldtk::core::*` etc.). Prefer the crate-root re-exports.
#[doc(hidden)]
pub mod ldtk {
    pub mod core {
        pub use crate::animation::*;
        pub use crate::catalog::*;
        #[allow(deprecated)]
        pub use crate::components::*;
        pub use crate::config::*;
        pub use crate::entities::*;
        pub use crate::events::*;
        pub use crate::external::*;
        pub use crate::fields::*;
        pub use crate::plugin::LdtkLoadSet;
        pub use crate::registry::*;
        pub use crate::state::*;

        pub use crate::commands::{LdtkCommand, LdtkCommandQueue};
    }

    pub mod plugins {
        pub use crate::plugin::GameLdtkPlugin;
    }

    pub mod commands {
        pub use crate::commands::{LdtkAppExt, LdtkCommandExt};
    }

    pub mod level_manager {
        pub use crate::level_manager::*;
    }
}
