//! [`LevelManagerPlugin`]: level transitions with spawn-point resolution,
//! player teleport, and per-level entity cleanup.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_ecs_ldtk::prelude::{LevelEvent, LevelSelection};
use std::time::Duration;

use crate::catalog::{LdtkCollisionCatalog, LdtkMapCatalog, LdtkSpawnPoint};
use crate::components::LdtkPersistent;
use crate::config::LdtkConfig;
use crate::entities::LdtkEntityMarker;
use crate::plugin::LdtkLoadSet;
use crate::state::{LdtkLoadState, LdtkLoadStatus, LdtkRuntimeState, LdtkValidationReport};

pub struct LevelManagerPlugin;

impl Plugin for LevelManagerPlugin {
    fn build(&self, app: &mut App) {
        // The loader-side resources (LdtkRuntimeState, LdtkMapCatalog, LdtkConfig,
        // ...) are owned by GameLdtkPlugin. LevelManagerPlugin only adds its own
        // transition state and guards its systems on those resources existing, so
        // that adding it without GameLdtkPlugin neither panics nor silently does
        // nothing — it logs a clear error at startup (see check_loader_dependency).
        app.init_resource::<CurrentLdtkLevel>()
            .init_resource::<PendingLdtkLevelTransition>()
            .init_resource::<LevelTransitionState>()
            .init_resource::<LdtkLevelManagerConfig>()
            .init_resource::<LdtkPlayerLocator>()
            .add_message::<LevelTransitionRequest>()
            .add_message::<LdtkLevelReadyEvent>()
            .add_message::<LdtkCollisionReadyEvent>()
            .add_systems(Startup, check_loader_dependency)
            .add_systems(
                Update,
                (
                    request_initial_level_transition,
                    handle_transition_requests,
                    finalize_level_transition,
                )
                    .chain()
                    .in_set(LdtkLoadSet::LevelTransitions)
                    .run_if(resource_exists::<LdtkRuntimeState>),
            );

        #[cfg(feature = "tilemap")]
        crate::tilemap_adapter::register(app);
    }
}

/// Emits a clear error at startup if `LevelManagerPlugin` was added without
/// `GameLdtkPlugin`. `LdtkRuntimeState` is initialized exclusively by the loader
/// plugin, so its absence is an unambiguous signal that the dependency is missing
/// — in which case the transition systems are skipped (see `run_if` above) rather
/// than panicking on a missing resource.
fn check_loader_dependency(runtime: Option<Res<'_, LdtkRuntimeState>>) {
    if runtime.is_none() {
        error!(
            "LevelManagerPlugin requires GameLdtkPlugin, which provides LDtk loading and the \
             LdtkMapCatalog. Without it, level transitions are disabled. Add GameLdtkPlugin to \
             your App before LevelManagerPlugin."
        );
    }
}

/// Configuration for [`LevelManagerPlugin`].
#[derive(Debug, Clone, Resource)]
pub struct LdtkLevelManagerConfig {
    /// Tag used to find the default spawn point when no `spawn_id` is given.
    pub default_spawn_tag: String,
    /// Entity identifier used to find the default spawn point when no `spawn_id` is given.
    pub default_spawn_identifier: String,
    /// Enables the experimental `bevy_ecs_tilemap` tile-animation adapter.
    pub enable_tile_animation_adapter: bool,
    /// When `true`, a missing spawn point falls back to `Vec2::ZERO` instead of failing.
    pub allow_missing_spawnpoints: bool,
}

impl Default for LdtkLevelManagerConfig {
    fn default() -> Self {
        Self {
            default_spawn_tag: "PlayerSpawn".to_string(),
            default_spawn_identifier: "PlayerSpawn".to_string(),
            enable_tile_animation_adapter: false,
            allow_missing_spawnpoints: false,
        }
    }
}

/// Optional resource that pins the exact player entity to teleport; overrides
/// the [`LdtkLevelPlayer`] marker search when set.
#[derive(Debug, Clone, Resource, Default)]
pub struct LdtkPlayerLocator {
    /// The player entity to teleport, if explicitly chosen.
    pub entity: Option<Entity>,
}

/// Resource describing the currently active level after a finished transition.
#[derive(Debug, Clone, Resource, Default)]
pub struct CurrentLdtkLevel {
    /// LDtk identifier of the active level.
    pub identifier: Option<String>,
    /// LDtk IID of the active level.
    pub iid: Option<String>,
}

/// Resource describing the transition that is currently in flight.
#[derive(Debug, Clone, Resource, Default)]
pub struct PendingLdtkLevelTransition {
    /// Identifier of the level being transitioned to.
    pub target_level: Option<String>,
    /// Requested spawn point identifier or tag, if any.
    pub spawn_id: Option<String>,
    /// Resolved IID of the target level.
    pub target_level_iid: Option<String>,
}

/// Phase of a level transition managed by [`LevelManagerPlugin`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LevelTransitionStatus {
    /// No transition is in progress.
    #[default]
    Idle,
    /// Waiting for the target level to spawn.
    WaitingForSpawn,
    /// The transition completed and the player was placed.
    Ready,
    /// The transition failed; see [`LevelTransitionState::error`].
    Failed,
}

/// Resource exposing the current transition phase and error, for UI such as
/// loading screens.
#[derive(Debug, Clone, Resource, Default)]
pub struct LevelTransitionState {
    /// Current phase.
    pub status: LevelTransitionStatus,
    /// Human-readable error for the [`LevelTransitionStatus::Failed`] phase.
    pub error: Option<String>,
}

/// Message that requests a level transition; usually emitted via
/// [`LdtkCommandExt::transition_to_ldtk_level`](crate::LdtkCommandExt::transition_to_ldtk_level).
#[derive(Debug, Clone, Message)]
pub struct LevelTransitionRequest {
    /// LDtk identifier (or IID) of the level to switch to.
    pub target_level: String,
    /// Spawn point identifier or tag; `None` resolves the configured default.
    pub spawn_id: Option<String>,
}

/// Message emitted once a transition finished and the player has been placed.
#[derive(Debug, Clone, Message)]
pub struct LdtkLevelReadyEvent {
    /// Identifier of the now-active level.
    pub level_identifier: String,
    /// Spawn id that was requested, if any.
    pub spawn_id: Option<String>,
    /// World-space position the player was teleported to.
    pub position: Vec2,
}

/// Message emitted alongside [`LdtkLevelReadyEvent`] summarising the collision
/// cells captured for the new level.
#[derive(Debug, Clone, Message)]
pub struct LdtkCollisionReadyEvent {
    /// Identifier of the now-active level.
    pub level_identifier: String,
    /// Number of collision cells captured for that level.
    pub cells: usize,
}

/// Marker for the player entity that [`LevelManagerPlugin`] teleports to the
/// resolved spawn point after every transition.
#[derive(Debug, Clone, Component, Default)]
pub struct LdtkLevelPlayer;

/// Scopes an entity to a level: it is despawned when that level is left.
#[derive(Debug, Clone, Component)]
pub struct LdtkLevelScoped {
    /// Identifier of the level this entity belongs to.
    pub level_identifier: String,
}

#[derive(SystemParam)]
struct TransitionResources<'w> {
    pending: ResMut<'w, PendingLdtkLevelTransition>,
    state: ResMut<'w, LevelTransitionState>,
    current: ResMut<'w, CurrentLdtkLevel>,
    runtime: ResMut<'w, LdtkRuntimeState>,
    catalog: Res<'w, LdtkMapCatalog>,
    collision_catalog: Res<'w, LdtkCollisionCatalog>,
    config: Res<'w, LdtkLevelManagerConfig>,
    strict_config: Res<'w, LdtkConfig>,
    load_state: ResMut<'w, LdtkLoadState>,
    validation: ResMut<'w, LdtkValidationReport>,
}

/// Auto-promotes the first level that `bevy_ecs_ldtk` spawns into a full level
/// transition when nothing else has driven one yet. Without this, a world
/// loaded via `LdtkConfig::with_world_asset_path(..)` — or a bare
/// `change_ldtk_level(..)` — would render but never place the player, set
/// `CurrentLdtkLevel`, or fire `LdtkLevelReadyEvent` / `LdtkCollisionReadyEvent`,
/// so a single-level game would start with nothing wired up.
///
/// Runs only while idle (no current level and no pending transition) so it never
/// competes with an explicit `transition_to_ldtk_level` request. The emitted
/// request is picked up by `handle_transition_requests` in the same frame, and
/// `finalize_level_transition` completes it against the same `Spawned` event.
fn request_initial_level_transition(
    mut events: MessageReader<'_, '_, LevelEvent>,
    current: Res<'_, CurrentLdtkLevel>,
    pending: Res<'_, PendingLdtkLevelTransition>,
    catalog: Res<'_, LdtkMapCatalog>,
    mut requests: MessageWriter<'_, LevelTransitionRequest>,
) {
    let idle = current.identifier.is_none() && pending.target_level.is_none();
    let mut requested = false;
    for event in events.read() {
        // Drain every event so the reader cursor stays current even when idle is
        // false; only the first resolvable spawn triggers a request.
        let LevelEvent::Spawned(level_iid) = event else {
            continue;
        };
        if !idle || requested {
            continue;
        }
        if let Some(identifier) = catalog.identifier_for_iid(level_iid.as_str()) {
            requests.write(LevelTransitionRequest {
                target_level: identifier.to_string(),
                spawn_id: None,
            });
            requested = true;
        }
    }
}

fn handle_transition_requests(
    mut requests: MessageReader<'_, '_, LevelTransitionRequest>,
    mut selection: ResMut<'_, LevelSelection>,
    mut pending: ResMut<'_, PendingLdtkLevelTransition>,
    mut state: ResMut<'_, LevelTransitionState>,
    catalog: Res<'_, LdtkMapCatalog>,
    config: Res<'_, LdtkConfig>,
    mut load_state: ResMut<'_, LdtkLoadState>,
    mut validation: ResMut<'_, LdtkValidationReport>,
) {
    for request in requests.read() {
        if let Err(error) = start_transition(
            &mut pending,
            &mut state,
            request.target_level.clone(),
            request.spawn_id.clone(),
            &catalog,
        ) {
            let strict = config.strict_validation;
            state.status = LevelTransitionStatus::Failed;
            state.error = Some(error.clone());
            validation.push(strict, "transition_level_missing", error);
            if strict {
                load_state.status = LdtkLoadStatus::Error;
            }
            continue;
        }
        *selection = LevelSelection::Identifier(request.target_level.clone());
    }
}

fn start_transition(
    pending: &mut PendingLdtkLevelTransition,
    state: &mut LevelTransitionState,
    target_level: String,
    spawn_id: Option<String>,
    catalog: &LdtkMapCatalog,
) -> Result<(), String> {
    let target_level_iid = catalog
        .level_by_id_or_iid(&target_level)
        .map(|info| info.iid.clone())
        .ok_or_else(|| format!("Level '{target_level}' not found in LdtkMapCatalog"))?;

    pending.target_level = Some(target_level);
    pending.spawn_id = spawn_id;
    pending.target_level_iid = Some(target_level_iid);
    state.status = LevelTransitionStatus::WaitingForSpawn;
    state.error = None;
    Ok(())
}

fn finalize_level_transition(
    mut commands: Commands<'_, '_>,
    mut events: MessageReader<'_, '_, LevelEvent>,
    mut ready_messages: MessageWriter<'_, LdtkLevelReadyEvent>,
    mut collision_messages: MessageWriter<'_, LdtkCollisionReadyEvent>,
    mut resources: TransitionResources<'_>,
    locator: Res<'_, LdtkPlayerLocator>,
    player_query: Query<'_, '_, Entity, With<LdtkLevelPlayer>>,
    mut transform_query: Query<'_, '_, &mut Transform>,
    cleanup_query: Query<
        '_,
        '_,
        (Entity, Option<&LdtkEntityMarker>, Option<&LdtkLevelScoped>),
        (Without<LdtkPersistent>, Without<LdtkLevelPlayer>),
    >,
) {
    let Some(target_level) = resources.pending.target_level.clone() else {
        return;
    };

    for event in events.read() {
        let LevelEvent::Spawned(level_iid) = event else {
            continue;
        };
        let level_iid = level_iid.as_str().to_string();
        let level_identifier = level_identifier_from_iid(&resources.catalog, &level_iid);

        let matches_pending = resources
            .pending
            .target_level_iid
            .as_ref()
            .is_some_and(|iid| iid == &level_iid)
            || level_identifier
                .as_ref()
                .is_some_and(|identifier| identifier == &target_level);
        if !matches_pending {
            continue;
        }

        let level_identifier = level_identifier.unwrap_or_else(|| target_level.clone());
        let spawn = match resolve_spawn_point(
            &resources.catalog,
            &level_identifier,
            resources.pending.spawn_id.as_deref(),
            &resources.config,
        ) {
            Ok(spawn) => spawn,
            Err(message) => {
                let strict = resources.strict_config.strict_validation;
                resources.state.status = LevelTransitionStatus::Failed;
                resources.state.error = Some(message.clone());
                resources
                    .validation
                    .push(strict, "transition_spawn_missing", message);
                if strict {
                    resources.load_state.status = LdtkLoadStatus::Error;
                }
                resources.pending.target_level = None;
                resources.pending.target_level_iid = None;
                resources.pending.spawn_id = None;
                return;
            }
        };

        if resources.current.identifier.as_deref() != Some(&level_identifier)
            && let Some(old_identifier) = resources.current.identifier.as_deref()
        {
            cleanup_level_entities(&mut commands, &cleanup_query, old_identifier);
        }

        // Capture the spawn id before clearing `pending`: the ready event below
        // still needs it, but every `pending` field must be reset so a second
        // `Spawned` event in the same frame (neighbor streaming) cannot re-match
        // a stale `target_level_iid`/`spawn_id` and teleport twice.
        let spawn_id = resources.pending.spawn_id.clone();
        resources.current.identifier = Some(level_identifier.clone());
        resources.current.iid = Some(level_iid.clone());
        resources.runtime.active_level = Some(level_identifier.clone());
        resources.state.status = LevelTransitionStatus::Ready;
        resources.state.error = None;
        resources.pending.target_level = None;
        resources.pending.target_level_iid = None;
        resources.pending.spawn_id = None;

        // Teleport the player to the resolved spawn point. Prefer the explicit
        // locator entity, fall back to the first `LdtkLevelPlayer`, and warn
        // loudly if neither resolves to a live Transform instead of silently
        // leaving the player in place.
        let player_entity = locator
            .entity
            .filter(|&entity| transform_query.contains(entity))
            .or_else(|| {
                player_query
                    .iter()
                    .find(|&entity| transform_query.contains(entity))
            });
        match player_entity.and_then(|entity| transform_query.get_mut(entity).ok()) {
            Some(mut transform) => {
                transform.translation =
                    Vec3::new(spawn.position.x, spawn.position.y, transform.translation.z);
            }
            None => {
                warn!(
                    "No LdtkLevelPlayer/locator entity with a Transform found — \
                     skipping teleport for level '{level_identifier}'."
                );
            }
        }

        ready_messages.write(LdtkLevelReadyEvent {
            level_identifier: level_identifier.clone(),
            spawn_id,
            position: spawn.position,
        });

        let collision_cells = resources
            .collision_catalog
            .cells
            .iter()
            .filter(|cell| cell.level_identifier == level_identifier)
            .count();
        collision_messages.write(LdtkCollisionReadyEvent {
            level_identifier,
            cells: collision_cells,
        });

        // One transition completes per finalize pass; stop so additional
        // `Spawned` events this frame are not mistaken for this transition.
        break;
    }
}

fn cleanup_level_entities(
    commands: &mut Commands<'_, '_>,
    query: &Query<
        '_,
        '_,
        (Entity, Option<&LdtkEntityMarker>, Option<&LdtkLevelScoped>),
        (Without<LdtkPersistent>, Without<LdtkLevelPlayer>),
    >,
    level_identifier: &str,
) {
    for (entity, marker, scoped) in query.iter() {
        let marker_level = marker.and_then(|marker| marker.level_identifier.as_deref());
        let scoped_level = scoped.map(|scope| scope.level_identifier.as_str());
        // The query already excludes persistent entities and the player, so
        // those flags are known-false here; the shared predicate still encodes
        // the full rule and is exercised directly by the unit tests.
        if should_cleanup_entity(marker_level, scoped_level, false, false, level_identifier) {
            commands.entity(entity).despawn();
        }
    }
}

fn should_cleanup_entity(
    marker_level: Option<&str>,
    scoped_level: Option<&str>,
    is_persistent: bool,
    is_player: bool,
    target_level: &str,
) -> bool {
    if is_persistent || is_player {
        return false;
    }
    marker_level == Some(target_level) || scoped_level == Some(target_level)
}

fn resolve_spawn_point(
    catalog: &LdtkMapCatalog,
    target_level: &str,
    spawn_id: Option<&str>,
    config: &LdtkLevelManagerConfig,
) -> Result<LdtkSpawnPoint, String> {
    let level = catalog
        .levels
        .get(target_level)
        .ok_or_else(|| format!("Level '{target_level}' not found in LdtkMapCatalog"))?;

    if let Some(spawn_id) = spawn_id {
        // Identifier and tag matching are both case-insensitive so that
        // `transition_to_ldtk_level("L", Some("playerspawn"))` resolves
        // `PlayerSpawn` regardless of casing.
        let found = level.spawn_points.iter().find(|spawn| {
            spawn.identifier.eq_ignore_ascii_case(spawn_id)
                || spawn
                    .tags
                    .iter()
                    .any(|tag| tag.eq_ignore_ascii_case(spawn_id))
        });
        return found
            .cloned()
            .ok_or_else(|| format!("Spawnpoint '{spawn_id}' not found in level '{target_level}'"));
    }

    let default_spawn = level.spawn_points.iter().find(|spawn| {
        spawn
            .identifier
            .eq_ignore_ascii_case(&config.default_spawn_identifier)
            || spawn
                .tags
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case(&config.default_spawn_tag))
    });
    if let Some(spawn) = default_spawn {
        return Ok(spawn.clone());
    }

    if let Some(spawn) = level.spawn_points.first() {
        return Ok(spawn.clone());
    }

    if config.allow_missing_spawnpoints {
        return Ok(LdtkSpawnPoint {
            identifier: String::from("Fallback"),
            position: Vec2::ZERO,
            level_identifier: target_level.to_string(),
            layer_identifier: String::from(""),
            tags: Vec::new(),
        });
    }

    Err(format!("Level '{target_level}' has no spawnpoints"))
}

fn level_identifier_from_iid(catalog: &LdtkMapCatalog, iid: &str) -> Option<String> {
    catalog.identifier_for_iid(iid).map(ToOwned::to_owned)
}

/// Advances `animator` by `delta`, returning the new tile id when the frame
/// changed. Thin wrapper around [`LdtkTileAnimator::advance`] kept for backwards
/// compatibility.
pub fn advance_tile_animation(
    animator: &mut crate::animation::LdtkTileAnimator,
    delta: Duration,
) -> Option<i32> {
    animator.advance(delta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::LdtkLevelInfo;

    fn build_catalog_with_spawnpoints() -> LdtkMapCatalog {
        let mut catalog = LdtkMapCatalog::default();
        let level = LdtkLevelInfo {
            identifier: "Level_A".to_string(),
            spawn_points: vec![
                LdtkSpawnPoint {
                    identifier: "PlayerSpawn".to_string(),
                    position: Vec2::new(10.0, 20.0),
                    tags: vec!["PlayerSpawn".to_string()],
                    level_identifier: "Level_A".to_string(),
                    layer_identifier: "Entities".to_string(),
                },
                LdtkSpawnPoint {
                    identifier: "Alt".to_string(),
                    position: Vec2::new(30.0, 40.0),
                    tags: vec!["Alt".to_string()],
                    level_identifier: "Level_A".to_string(),
                    layer_identifier: "Entities".to_string(),
                },
            ],
            ..Default::default()
        };
        catalog.insert_level_info(level);
        catalog
    }

    fn empty_level_catalog() -> LdtkMapCatalog {
        let mut catalog = LdtkMapCatalog::default();
        catalog.insert_level_info(LdtkLevelInfo {
            identifier: "Level_A".to_string(),
            ..Default::default()
        });
        catalog
    }

    #[test]
    fn resolves_explicit_spawn_id() {
        let catalog = build_catalog_with_spawnpoints();
        let config = LdtkLevelManagerConfig::default();

        let spawn =
            resolve_spawn_point(&catalog, "Level_A", Some("Alt"), &config).expect("spawnpoint");

        assert_eq!(spawn.identifier, "Alt");
    }

    #[test]
    fn resolves_spawn_id_case_insensitively() {
        let catalog = build_catalog_with_spawnpoints();
        let config = LdtkLevelManagerConfig::default();

        // Lower-case query must resolve the `PlayerSpawn` identifier.
        let spawn = resolve_spawn_point(&catalog, "Level_A", Some("playerspawn"), &config)
            .expect("spawnpoint");
        assert_eq!(spawn.identifier, "PlayerSpawn");

        // And the alternate spawn by its identifier, also case-insensitively.
        let alt =
            resolve_spawn_point(&catalog, "Level_A", Some("ALT"), &config).expect("spawnpoint");
        assert_eq!(alt.identifier, "Alt");
    }

    #[test]
    fn falls_back_to_default_spawnpoint() {
        let catalog = build_catalog_with_spawnpoints();
        let config = LdtkLevelManagerConfig::default();

        let spawn = resolve_spawn_point(&catalog, "Level_A", None, &config).expect("spawnpoint");

        assert_eq!(spawn.identifier, "PlayerSpawn");
    }

    #[test]
    fn missing_spawnpoint_returns_error() {
        let catalog = empty_level_catalog();

        let result = resolve_spawn_point(
            &catalog,
            "Level_A",
            Some("Missing"),
            &LdtkLevelManagerConfig::default(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn transition_state_changes_on_request() {
        let catalog = build_catalog_with_spawnpoints();
        let mut pending = PendingLdtkLevelTransition::default();
        let mut state = LevelTransitionState::default();

        let result = start_transition(
            &mut pending,
            &mut state,
            "Level_A".to_string(),
            None,
            &catalog,
        );

        assert!(result.is_ok());
        assert_eq!(state.status, LevelTransitionStatus::WaitingForSpawn);
        assert_eq!(pending.target_level.as_deref(), Some("Level_A"));
    }

    #[test]
    fn transition_state_fails_for_unknown_level() {
        let catalog = build_catalog_with_spawnpoints();
        let mut pending = PendingLdtkLevelTransition::default();
        let mut state = LevelTransitionState::default();

        let result = start_transition(
            &mut pending,
            &mut state,
            "Missing".to_string(),
            None,
            &catalog,
        );

        assert!(result.is_err());
    }

    #[test]
    fn cleanup_decision_respects_persistence() {
        assert!(!should_cleanup_entity(
            Some("Level_A"),
            None,
            true,
            false,
            "Level_A"
        ));
        assert!(!should_cleanup_entity(
            Some("Level_A"),
            None,
            false,
            true,
            "Level_A"
        ));
        assert!(should_cleanup_entity(
            Some("Level_A"),
            None,
            false,
            false,
            "Level_A"
        ));
        assert!(!should_cleanup_entity(
            Some("Level_B"),
            None,
            false,
            false,
            "Level_A"
        ));
    }

    #[test]
    fn allows_fallback_spawnpoint_when_enabled() {
        let catalog = empty_level_catalog();

        let config = LdtkLevelManagerConfig {
            allow_missing_spawnpoints: true,
            ..Default::default()
        };
        let spawn = resolve_spawn_point(&catalog, "Level_A", None, &config).expect("fallback");

        assert_eq!(spawn.identifier, "Fallback");
        assert_eq!(spawn.position, Vec2::ZERO);
    }
}
