//! The deferred command queue and the [`Commands`]/[`App`] extension traits
//! that form the crate's imperative API surface.

use bevy::prelude::*;

use crate::entities::LdtkEntitySpawnContext;
use crate::events::LdtkSpawnWorldEvent;
use crate::level_manager::LevelTransitionRequest;
use crate::registry::LdtkEntityRegistry;

/// Commands that can be submitted to the [`LdtkCommandQueue`] to drive the LDtk
/// runtime.
#[derive(Debug, Clone)]
pub enum LdtkCommand {
    /// Load and spawn the world at `world_path`.
    SpawnWorld {
        /// Asset-relative path to the `.ldtk` file.
        world_path: String,
    },
    /// Transition to the level identified by `level_identifier`.
    ChangeLevel {
        /// LDtk identifier of the target level.
        level_identifier: String,
    },
    /// Reload the currently active world from disk.
    ReloadWorld,
    /// Despawn the active world and reset runtime state.
    UnloadWorld,
}

/// Bevy resource acting as a deferred queue for [`LdtkCommand`]s, which are
/// processed at the start of each frame by the
/// [`LdtkLoadSet::Commands`](crate::LdtkLoadSet::Commands) systems.
///
/// Prefer the [`LdtkCommandExt`] methods over pushing into this queue directly.
#[derive(Debug, Clone, Resource, Default)]
pub struct LdtkCommandQueue {
    /// Commands waiting to be processed this frame.
    pub pending: Vec<LdtkCommand>,
}

/// Extension methods on [`Commands`] for controlling the LDtk world at runtime.
///
/// All commands are queued and processed in
/// [`LdtkLoadSet::Commands`](crate::LdtkLoadSet::Commands) on the next frame,
/// so they are safe to call from any system.
pub trait LdtkCommandExt {
    /// Loads an LDtk world from `world_path` (relative to `assets/`).
    ///
    /// If a world is already loaded it is despawned first. Resets
    /// [`LdtkLoadState`](crate::LdtkLoadState),
    /// [`LdtkMapCatalog`](crate::LdtkMapCatalog), and the active level.
    fn spawn_ldtk_world(&mut self, world_path: impl Into<String>);

    /// Switches the active level by LDtk identifier.
    ///
    /// Updates `LevelSelection` and emits
    /// [`LdtkLevelActivatedEvent`](crate::LdtkLevelActivatedEvent).
    /// Does **not** teleport the player — use [`Self::transition_to_ldtk_level`]
    /// for that.
    fn change_ldtk_level(&mut self, level_identifier: impl Into<String>);

    /// Alias for [`Self::change_ldtk_level`].
    fn change_level(&mut self, level_identifier: impl Into<String>) {
        self.change_ldtk_level(level_identifier);
    }

    /// Reloads the currently active world from disk.
    ///
    /// Equivalent to calling [`Self::spawn_ldtk_world`] with the same path
    /// that was used last. Does nothing if no world is loaded.
    fn reload_ldtk_world(&mut self);

    /// Despawns the active world and resets all LDtk state.
    ///
    /// Emits [`LdtkWorldUnloadedEvent`](crate::LdtkWorldUnloadedEvent).
    fn unload_ldtk_world(&mut self);

    /// Requests a level transition via `LevelManagerPlugin`.
    ///
    /// Emits a `LevelTransitionRequest` message. The level manager resolves
    /// the spawn point, teleports the player entity marked with
    /// `LdtkLevelPlayer`, cleans up scoped entities, and emits
    /// `LdtkLevelReadyEvent` when done.
    ///
    /// `spawn_id` — the LDtk entity identifier or tag of the desired spawn
    /// point. Pass `None` to use the default spawn point (`"PlayerSpawn"`).
    ///
    /// Requires `LevelManagerPlugin` to be registered.
    fn transition_to_ldtk_level(
        &mut self,
        level_identifier: impl Into<String>,
        spawn_id: Option<impl Into<String>>,
    );
}

impl<'w, 's> LdtkCommandExt for Commands<'w, 's> {
    fn spawn_ldtk_world(&mut self, world_path: impl Into<String>) {
        let world_path = world_path.into();
        self.queue(move |world: &mut World| {
            world
                .resource_mut::<LdtkCommandQueue>()
                .pending
                .push(LdtkCommand::SpawnWorld {
                    world_path: world_path.clone(),
                });
            world.write_message(LdtkSpawnWorldEvent { world_path });
        });
    }

    fn change_ldtk_level(&mut self, level_identifier: impl Into<String>) {
        let level_identifier = level_identifier.into();
        self.queue(move |world: &mut World| {
            world
                .resource_mut::<LdtkCommandQueue>()
                .pending
                .push(LdtkCommand::ChangeLevel { level_identifier });
        });
    }

    fn reload_ldtk_world(&mut self) {
        self.queue(move |world: &mut World| {
            world
                .resource_mut::<LdtkCommandQueue>()
                .pending
                .push(LdtkCommand::ReloadWorld);
        });
    }

    fn unload_ldtk_world(&mut self) {
        self.queue(move |world: &mut World| {
            world
                .resource_mut::<LdtkCommandQueue>()
                .pending
                .push(LdtkCommand::UnloadWorld);
        });
    }

    fn transition_to_ldtk_level(
        &mut self,
        level_identifier: impl Into<String>,
        spawn_id: Option<impl Into<String>>,
    ) {
        let target_level = level_identifier.into();
        let spawn_id = spawn_id.map(Into::into);
        self.queue(move |world: &mut World| {
            world.write_message(LevelTransitionRequest {
                target_level,
                spawn_id,
            });
        });
    }
}

/// Extension methods on [`App`] for registering LDtk entity definitions.
///
/// Requires [`GameLdtkPlugin`](crate::GameLdtkPlugin) to be registered so
/// that [`LdtkEntityRegistry`] is available as a resource.
pub trait LdtkAppExt {
    /// Registers a [`Bundle`] for the given LDtk entity identifier.
    ///
    /// When `bevy_ecs_ldtk` spawns an entity with this identifier, the bundle
    /// is inserted via [`Default::default()`], together with an
    /// [`LdtkEntityMarker`](crate::LdtkEntityMarker). The entity's [`Transform`]
    /// is **not** touched — `bevy_ecs_ldtk` already places the entity at its
    /// correct world position.
    fn register_ldtk_entity<B>(&mut self, identifier: impl Into<String>) -> &mut Self
    where
        B: Bundle + Default + Send + Sync + 'static;

    /// Registers a [`Bundle`] scoped to a specific layer and entity identifier.
    ///
    /// Takes precedence over a registration without a layer.
    fn register_ldtk_entity_for_layer<B>(
        &mut self,
        layer_identifier: impl Into<String>,
        entity_identifier: impl Into<String>,
    ) -> &mut Self
    where
        B: Bundle + Default + Send + Sync + 'static;

    /// Registers a custom spawner function for the given LDtk entity identifier.
    ///
    /// The spawner receives `&mut World`, the target [`Entity`], and a
    /// [`LdtkEntitySpawnContext`] with all field values and position data.
    /// Use this instead of [`Self::register_ldtk_entity`] when you need to read
    /// LDtk custom fields or insert non-default component values.
    /// The context implements [`LdtkFieldAccess`](crate::LdtkFieldAccess).
    fn register_ldtk_entity_spawner(
        &mut self,
        identifier: impl Into<String>,
        spawner: impl Fn(&mut World, Entity, &LdtkEntitySpawnContext) + Send + Sync + 'static,
    ) -> &mut Self;

    /// Registers a custom spawner scoped to a specific layer and entity identifier.
    ///
    /// Takes precedence over a registration without a layer.
    fn register_ldtk_entity_spawner_for_layer(
        &mut self,
        layer_identifier: impl Into<String>,
        entity_identifier: impl Into<String>,
        spawner: impl Fn(&mut World, Entity, &LdtkEntitySpawnContext) + Send + Sync + 'static,
    ) -> &mut Self;
}

impl LdtkAppExt for App {
    fn register_ldtk_entity<B>(&mut self, identifier: impl Into<String>) -> &mut Self
    where
        B: Bundle + Default + Send + Sync + 'static,
    {
        self.world_mut()
            .resource_mut::<LdtkEntityRegistry>()
            .register_bundle::<B>(identifier);
        self
    }

    fn register_ldtk_entity_for_layer<B>(
        &mut self,
        layer_identifier: impl Into<String>,
        entity_identifier: impl Into<String>,
    ) -> &mut Self
    where
        B: Bundle + Default + Send + Sync + 'static,
    {
        self.world_mut()
            .resource_mut::<LdtkEntityRegistry>()
            .register_bundle_for_layer::<B>(layer_identifier, entity_identifier);
        self
    }

    fn register_ldtk_entity_spawner(
        &mut self,
        identifier: impl Into<String>,
        spawner: impl Fn(&mut World, Entity, &LdtkEntitySpawnContext) + Send + Sync + 'static,
    ) -> &mut Self {
        self.world_mut()
            .resource_mut::<LdtkEntityRegistry>()
            .register_spawner(identifier, spawner);
        self
    }

    fn register_ldtk_entity_spawner_for_layer(
        &mut self,
        layer_identifier: impl Into<String>,
        entity_identifier: impl Into<String>,
        spawner: impl Fn(&mut World, Entity, &LdtkEntitySpawnContext) + Send + Sync + 'static,
    ) -> &mut Self {
        self.world_mut()
            .resource_mut::<LdtkEntityRegistry>()
            .register_spawner_for_layer(layer_identifier, entity_identifier, spawner);
        self
    }
}
