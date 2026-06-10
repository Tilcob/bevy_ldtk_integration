//! [`GameLdtkPlugin`]: resource/system wiring and the command-queue processor.

use bevy::prelude::*;
use bevy_ecs_ldtk::prelude::*;
use std::path::Path;

use crate::animation::tick_ldtk_tile_animators;
use crate::capture::{
    apply_registered_entity_behaviors, capture_collision_data, capture_entity_instances,
    sync_level_lifecycle_events,
};
use crate::catalog::{LdtkCollisionCatalog, LdtkEntityCatalog, LdtkMapCatalog};
use crate::catalog_builder::refresh_map_catalog_from_project;
use crate::commands::{LdtkCommand, LdtkCommandQueue};
use crate::components::LdtkWorldRoot;
use crate::config::LdtkConfig;
use crate::events::{
    LdtkLevelActivatedEvent, LdtkMapLoadedEvent, LdtkSpawnWorldEvent, LdtkValidationFinishedEvent,
    LdtkWorldUnloadedEvent,
};
use crate::external::LdtkExternalLevelSource;
use crate::registry::LdtkEntityRegistry;
use crate::state::{
    LdtkLoadState, LdtkLoadStats, LdtkLoadStatus, LdtkRuntimeState, LdtkTransitionState,
    LdtkValidationReport,
};

/// Explicit ordering for the LDtk systems so dependent stages run in a
/// deterministic sequence instead of relying on tuple insertion order.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum LdtkLoadSet {
    /// Processes queued [`LdtkCommand`]s before any catalog or level work runs.
    Commands,
    /// Builds or refreshes the [`LdtkMapCatalog`] from the loaded world JSON.
    Catalog,
    /// Captures entity and tile snapshots into runtime catalogs.
    Capture,
    /// Drives level-transition logic (load, activate, unload).
    LevelTransitions,
    /// Advances [`LdtkTileAnimator`](crate::LdtkTileAnimator) timers and swaps
    /// sprite indices.
    Animation,
}

/// The core plugin: wraps `bevy_ecs_ldtk`, builds the metadata catalogs, applies
/// collision rules, and drives the command queue.
#[derive(Default)]
pub struct GameLdtkPlugin {
    /// Configuration inserted as the [`LdtkConfig`] resource during build.
    pub config: LdtkConfig,
}

impl GameLdtkPlugin {
    /// Creates the plugin with the given configuration.
    pub fn new(config: LdtkConfig) -> Self {
        Self { config }
    }
}

impl Plugin for GameLdtkPlugin {
    fn build(&self, app: &mut App) {
        let level_spawn_behavior = LevelSpawnBehavior::UseWorldTranslation {
            load_level_neighbors: self.config.load_level_neighbors,
        };

        app.insert_resource(self.config.clone())
            .insert_resource(LevelSelection::default())
            .insert_resource(LdtkSettings {
                level_spawn_behavior,
                ..Default::default()
            })
            .init_resource::<LdtkRuntimeState>()
            .init_resource::<LdtkLoadState>()
            .init_resource::<LdtkValidationReport>()
            .init_resource::<LdtkMapCatalog>()
            .init_resource::<LdtkCollisionCatalog>()
            .init_resource::<LdtkEntityCatalog>()
            .init_resource::<LdtkCommandQueue>()
            .init_resource::<LdtkEntityRegistry>()
            .add_message::<LdtkSpawnWorldEvent>()
            .add_message::<LdtkMapLoadedEvent>()
            .add_message::<LdtkLevelActivatedEvent>()
            .add_message::<LdtkWorldUnloadedEvent>()
            .add_message::<LdtkValidationFinishedEvent>()
            .add_plugins(bevy_ecs_ldtk::LdtkPlugin)
            .configure_sets(
                Update,
                (
                    LdtkLoadSet::Commands,
                    LdtkLoadSet::Catalog,
                    LdtkLoadSet::Capture,
                    LdtkLoadSet::LevelTransitions,
                    LdtkLoadSet::Animation,
                )
                    .chain(),
            );

        // External level loader: filesystem-backed by default (desktop), absent
        // when the `external-level-fs` feature is off (e.g. WASM builds).
        #[cfg(feature = "external-level-fs")]
        app.insert_resource(LdtkExternalLevelSource(Some(Box::new(
            crate::external::FsExternalLevelSource,
        ))));
        #[cfg(not(feature = "external-level-fs"))]
        app.init_resource::<LdtkExternalLevelSource>();

        app.add_systems(Startup, spawn_configured_world)
            .add_systems(
                Update,
                (
                    process_ldtk_commands.in_set(LdtkLoadSet::Commands),
                    refresh_map_catalog_from_project.in_set(LdtkLoadSet::Catalog),
                    sync_level_lifecycle_events.in_set(LdtkLoadSet::Catalog),
                    (
                        capture_collision_data,
                        capture_entity_instances,
                        apply_registered_entity_behaviors,
                    )
                        .chain()
                        .in_set(LdtkLoadSet::Capture),
                    tick_ldtk_tile_animators.in_set(LdtkLoadSet::Animation),
                ),
            );
    }
}

fn spawn_configured_world(mut commands: Commands<'_, '_>, config: Res<'_, LdtkConfig>) {
    if let Some(world_path) = &config.world_asset_path {
        queue_spawn_world(&mut commands, world_path.clone());
    }
}

fn process_ldtk_commands(
    mut commands: Commands<'_, '_>,
    asset_server: Res<'_, AssetServer>,
    mut queue: ResMut<'_, LdtkCommandQueue>,
    mut runtime: ResMut<'_, LdtkRuntimeState>,
    mut load_state: ResMut<'_, LdtkLoadState>,
    mut selection: ResMut<'_, LevelSelection>,
    mut collision_catalog: ResMut<'_, LdtkCollisionCatalog>,
    mut entity_catalog: ResMut<'_, LdtkEntityCatalog>,
    mut level_messages: MessageWriter<'_, LdtkLevelActivatedEvent>,
    mut unload_messages: MessageWriter<'_, LdtkWorldUnloadedEvent>,
) {
    for command in queue.pending.drain(..) {
        match command {
            LdtkCommand::SpawnWorld { world_path } => {
                if let Some(root) = runtime.active_world_root.take() {
                    commands.entity(root).despawn();
                }

                let world_identifier = world_identifier_from_path(&world_path);
                let ldtk_handle = asset_server.load(world_path.clone());
                let root = commands
                    .spawn((
                        LdtkWorldBundle {
                            ldtk_handle: ldtk_handle.into(),
                            level_set: LevelSet::default(),
                            transform: Transform::default(),
                            global_transform: GlobalTransform::default(),
                            visibility: Visibility::default(),
                            inherited_visibility: InheritedVisibility::default(),
                            view_visibility: ViewVisibility::default(),
                        },
                        LdtkWorldRoot,
                        Name::new(format!("LDtk World: {world_identifier}")),
                    ))
                    .id();

                runtime.active_world_path = Some(world_path);
                runtime.active_world_identifier = Some(world_identifier);
                runtime.active_world_root = Some(root);
                runtime.active_level = None;
                runtime.loaded_levels.clear();
                runtime.transition = LdtkTransitionState::Loading;
                load_state.status = LdtkLoadStatus::Loading;
                load_state.world_identifier = runtime.active_world_identifier.clone();
                load_state.warnings.clear();
                load_state.errors.clear();
                load_state.stats = LdtkLoadStats::default();
                *selection = LevelSelection::default();
                // The collision/entity catalogs are populated from `Added<...>`
                // queries as levels (re)spawn. Without clearing them here every
                // (re)spawn would append duplicate, ever-growing data. Covers
                // SpawnWorld and, via queue_spawn_world, ReloadWorld.
                clear_runtime_catalogs(&mut collision_catalog, &mut entity_catalog);
            }
            LdtkCommand::ChangeLevel { level_identifier } => {
                runtime.active_level = Some(level_identifier.clone());
                *selection = LevelSelection::Identifier(level_identifier.clone());
                level_messages.write(LdtkLevelActivatedEvent { level_identifier });
            }
            LdtkCommand::ReloadWorld => {
                if let Some(world_path) = runtime.active_world_path.clone() {
                    queue_spawn_world(&mut commands, world_path);
                }
            }
            LdtkCommand::UnloadWorld => {
                if let Some(root) = runtime.active_world_root.take() {
                    commands.entity(root).despawn();
                }
                runtime.active_world_path = None;
                runtime.active_world_identifier = None;
                runtime.active_level = None;
                runtime.loaded_levels.clear();
                runtime.transition = LdtkTransitionState::Idle;
                load_state.status = LdtkLoadStatus::NotLoaded;
                load_state.world_identifier = None;
                load_state.stats = LdtkLoadStats::default();
                *selection = LevelSelection::default();
                clear_runtime_catalogs(&mut collision_catalog, &mut entity_catalog);
                unload_messages.write(LdtkWorldUnloadedEvent);
            }
        }
    }
}

/// Empties the runtime collision and entity catalogs. Called whenever a world is
/// spawned, reloaded, or unloaded so that the `Added<...>`-driven capture systems
/// rebuild from scratch instead of accumulating stale rows.
fn clear_runtime_catalogs(
    collision_catalog: &mut LdtkCollisionCatalog,
    entity_catalog: &mut LdtkEntityCatalog,
) {
    collision_catalog.cells.clear();
    collision_catalog.layers.clear();
    entity_catalog.by_iid.clear();
    entity_catalog.snapshots.clear();
}

fn queue_spawn_world(commands: &mut Commands<'_, '_>, world_path: String) {
    commands.queue(move |world: &mut World| {
        world
            .resource_mut::<LdtkCommandQueue>()
            .pending
            .push(LdtkCommand::SpawnWorld {
                world_path: world_path.clone(),
            });
        world.write_message(LdtkSpawnWorldEvent { world_path });
    });
}

/// Derives a fallback world identifier from a `.ldtk` file path (its file stem).
pub(crate) fn world_identifier_from_path(world_path: &str) -> String {
    Path::new(world_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(world_path)
        .to_string()
}
