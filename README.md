# bevy_ldtk_integration

A Bevy plugin for LDtk level loading in 2D games. Rendering and asset loading are handled by [`bevy_ecs_ldtk`](https://github.com/Trouv/bevy_ecs_ldtk); this crate adds a game-oriented API on top: runtime state, metadata catalogs, typed entity registration, IntGrid collision rules, and level transitions with spawn-point resolution.

**Versions:** Bevy `0.18`, bevy_ecs_ldtk `0.14`, Rust edition 2024

> **Naming note:** the package is `bevy_ldtk_integration`, but the library target is named `ldtk_integration` for backwards compatibility — imports are written as `use ldtk_integration::...`.

---

## Table of contents

- [Installation](#installation)
- [Feature flags](#feature-flags)
- [Quick start](#quick-start)
- [GameLdtkPlugin](#gameldtkplugin)
- [LevelManagerPlugin](#levelmanagerplugin)
- [Entity registration](#entity-registration)
- [Collision](#collision)
- [Layer filters](#layer-filters)
- [Tile animations](#tile-animations)
- [Load state and validation](#load-state-and-validation)
- [API reference](#api-reference)
- [Examples](#examples)
- [Tests](#tests)

---

## Installation

```toml
# Your game's Cargo.toml
[dependencies]
ldtk_integration = { package = "bevy_ldtk_integration", git = "https://github.com/Tilcob/ldtk_integration.git" }
bevy = "0.18.1"
```

Or from a local checkout:

```toml
ldtk_integration = { package = "bevy_ldtk_integration", path = "../bevy_ldtk_integration" }
```

---

## Feature flags

| Feature | Default | Description |
|---------|---------|-------------|
| `tilemap` | ✅ on | Tile-animation adapter for `bevy_ecs_tilemap` |
| `external-level-fs` | ✅ on | Reads external `.ldtkl` level files from the filesystem (not WASM) |

WASM build without filesystem access:

```toml
ldtk_integration = { package = "bevy_ldtk_integration", path = "...", default-features = false, features = ["tilemap"] }
```

---

## Quick start

LDtk files live relative to `assets/`. A file at `assets/worlds/map.ldtk` is referenced as `"worlds/map.ldtk"`.

```rust
use bevy::prelude::*;
use ldtk_integration::{GameLdtkPlugin, LdtkConfig};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GameLdtkPlugin::new(
            LdtkConfig::default()
                .with_world_asset_path("worlds/map.ldtk")
                .with_solid_int_grid_values([1]),
        ))
        .run();
}
```

---

## GameLdtkPlugin

The core plugin. Always required.

```rust
use ldtk_integration::{GameLdtkPlugin, LdtkConfig};

app.add_plugins(GameLdtkPlugin::new(LdtkConfig::default()));
// or with defaults:
app.add_plugins(GameLdtkPlugin::default());
```

Registers these resources: `LdtkConfig`, `LdtkRuntimeState`, `LdtkLoadState`, `LdtkValidationReport`, `LdtkMapCatalog`, `LdtkCollisionCatalog`, `LdtkEntityCatalog`, `LdtkCommandQueue`, `LdtkEntityRegistry`, `LdtkExternalLevelSource`.

### LdtkConfig

Builder API for configuring the plugin:

```rust
LdtkConfig::default()
    // Path to the .ldtk file, relative to assets/
    .with_world_asset_path("worlds/map.ldtk")

    // Base path for external .ldtkl levels (default: "assets")
    .with_asset_root("assets")

    // Mark specific IntGrid values as solid
    // (without this, every value != 0 counts as solid)
    .with_solid_int_grid_values([1, 2])

    // Precise collision rules (override int_grid_solid_values)
    .with_collision_rules([
        LdtkCollisionRule::solid(1).for_layer("Collision"),
        LdtkCollisionRule::sensor(2, "water").for_layer("Gameplay"),
    ])

    // Only catalog these layers
    .include_layers(["Collision", "Entities"])

    // Skip these layers
    .exclude_layers(["Debug", "Notes"])

    // Don't read external .ldtkl files
    .without_external_level_catalog()

    // Promote validation warnings to errors (sets LdtkLoadState to Error)
    .with_strict_validation()

    // Disable validation entirely
    .without_validation()

    // No warning log for unregistered LDtk entities
    .without_unregistered_entity_warnings()
```

### Commands

All commands are available on `Commands` via `LdtkCommandExt`:

```rust
use ldtk_integration::LdtkCommandExt;

// Load a world (replaces a running world)
commands.spawn_ldtk_world("worlds/map.ldtk");

// Switch the level (LevelSelection only, no player teleport)
commands.change_ldtk_level("Level_01");

// Alias for change_ldtk_level
commands.change_level("Level_01");

// Reload the current world
commands.reload_ldtk_world();

// Unload the world
commands.unload_ldtk_world();

// Level transition with spawn point (requires LevelManagerPlugin)
commands.transition_to_ldtk_level("Level_02", Some("Entrance_A"));
commands.transition_to_ldtk_level("Level_02", None::<String>);
```

### App extensions

Entity registration via `LdtkAppExt`:

```rust
use ldtk_integration::LdtkAppExt;

// Register a bundle (inserted via Default::default())
app.register_ldtk_entity::<PlayerBundle>("Player");

// Restrict a bundle to a layer + entity identifier
app.register_ldtk_entity_for_layer::<ChestBundle>("Objects", "Chest");

// Register a spawner function
app.register_ldtk_entity_spawner("Door", my_door_spawner);

// Restrict a spawner to a layer + entity identifier
app.register_ldtk_entity_spawner_for_layer("Objects", "Key", my_key_spawner);
```

---

## LevelManagerPlugin

Optional plugin for level transitions with spawn-point logic and automatic entity cleanup. Requires `GameLdtkPlugin`.

```rust
use ldtk_integration::{GameLdtkPlugin, LevelManagerPlugin};

app.add_plugins(GameLdtkPlugin::default())
   .add_plugins(LevelManagerPlugin);
```

### Triggering a transition

```rust
// Switch to a level; the player lands at "Entrance_A"
commands.transition_to_ldtk_level("Dungeon_02", Some("Entrance_A"));

// Pick the spawn point automatically (PlayerSpawn → first spawn point → fallback)
commands.transition_to_ldtk_level("Dungeon_02", None::<String>);
```

### Spawn-point resolution

The manager searches in this order:

1. Entity with `identifier == spawn_id` or tag `spawn_id` (when given; case-insensitive)
2. Entity with `identifier == "PlayerSpawn"` or tag `"PlayerSpawn"` (configurable)
3. First spawn point in the level
4. `Vec2::ZERO` when `allow_missing_spawnpoints: true`
5. `LevelTransitionStatus::Failed` when no spawn point was found

Any LDtk entity whose identifier contains `"spawn"` (case-insensitive) or that carries a `spawn` tag counts as a spawn point.

### Player teleport

```rust
// Option A: marker component
commands.spawn((Player, LdtkLevelPlayer, Transform::default(), GlobalTransform::default()));

// Option B: explicit resource (overrides the marker search)
commands.insert_resource(LdtkPlayerLocator { entity: Some(player_entity) });
```

### Configuration

```rust
app.insert_resource(LdtkLevelManagerConfig {
    default_spawn_tag: "PlayerSpawn".to_string(),
    default_spawn_identifier: "PlayerSpawn".to_string(),
    allow_missing_spawnpoints: false,
    enable_tile_animation_adapter: false,
});
```

### Events

| Event | Payload | When |
|-------|---------|------|
| `LdtkLevelReadyEvent` | `level_identifier`, `spawn_id`, `position` | Player was teleported |
| `LdtkCollisionReadyEvent` | `level_identifier`, `cells` | Collision data for the level is ready |
| `LdtkMapLoadedEvent` | `world_identifier` | World fully loaded |
| `LdtkLevelActivatedEvent` | `level_identifier` | Level activated via `change_ldtk_level` |
| `LdtkWorldUnloadedEvent` | — | World unloaded |

### Transition state

```rust
fn watch_state(state: Res<LevelTransitionState>) {
    match state.status {
        LevelTransitionStatus::Idle => {}
        LevelTransitionStatus::WaitingForSpawn => { /* show loading screen */ }
        LevelTransitionStatus::Ready => { /* hide loading screen */ }
        LevelTransitionStatus::Failed => {
            error!("Transition failed: {:?}", state.error);
        }
    }
}
```

### Persistence and cleanup

On a level switch, every entity is despawned that:
- carries `LdtkEntityMarker` referencing the old level, **or**
- carries `LdtkLevelScoped { level_identifier }` referencing the old level

Exceptions (never despawned):
- entities with `LdtkPersistent`
- entities with `LdtkLevelPlayer`

```rust
// Entity survives level switches
commands.entity(my_entity).insert(LdtkPersistent);

// Entity is despawned when leaving "Level_01"
commands.entity(my_entity).insert(LdtkLevelScoped {
    level_identifier: "Level_01".to_string(),
});
```

---

## Entity registration

### Bundle (simple)

```rust
#[derive(Bundle, Default)]
struct ChestBundle {
    chest: Chest,
    sprite: Sprite,
}

app.register_ldtk_entity::<ChestBundle>("Chest");
```

An `LdtkEntityMarker` is added automatically. The entity's `Transform` is **not** touched by this crate — `bevy_ecs_ldtk` already places every entity instance at its correct world position.

### Spawner (flexible)

```rust
app.register_ldtk_entity_spawner("Door", |world: &mut World, entity: Entity, ctx: &LdtkEntitySpawnContext| {
    let target_level = ctx.field_str("target_level").unwrap_or("").to_string();
    let target_spawn = ctx.field_str("target_spawn").unwrap_or("").to_string();

    world.entity_mut(entity).insert(Door { target_level, target_spawn });
});
```

### LdtkEntitySpawnContext

Contains everything about the LDtk entity at spawn time:

| Field | Type | Description |
|-------|------|-------------|
| `entity_iid` | `String` | Unique LDtk ID of the entity instance |
| `entity_identifier` | `String` | Definition name (e.g. `"Door"`) |
| `world_identifier` | `Option<String>` | Name of the LDtk world |
| `level_identifier` | `Option<String>` | Name of the level |
| `layer_identifier` | `Option<String>` | Name of the layer |
| `position` | `Vec2` | Pixel position in the world |
| `grid_position` | `IVec2` | Grid position within the layer |
| `size` | `Vec2` | Entity size in pixels |
| `pivot` | `Vec2` | Pivot point (0.0–1.0) |
| `tags` | `Vec<String>` | LDtk entity tags |
| `tile` | `Option<LdtkTileMetadata>` | Optional visual tile of the entity |
| `field_values` | `HashMap<String, LdtkFieldValue>` | All custom fields |

### Field access

`LdtkEntitySpawnContext` and `LdtkImportedEntity` both implement `LdtkFieldAccess`:

```rust
ctx.field("my_field")              // Option<&LdtkFieldValue>
ctx.field_bool("active")           // Option<bool>
ctx.field_i64("damage")            // Option<i64>
ctx.field_f64("speed")             // Option<f64>
ctx.field_str("label")             // Option<&str>
```

### LdtkFieldValue

```rust
pub enum LdtkFieldValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Color(Color),
    Point(Option<IVec2>),
    Tile(Option<LdtkTilesetRect>),
    EntityRef(LdtkEntityReference),
    Array(Vec<LdtkFieldValue>),
    Null,
}
```

---

## Collision

### Configuration

```rust
LdtkConfig::default()
    // These values become solid (default when no rules are set: every value != 0)
    .with_solid_int_grid_values([1, 2])

    // Or precise rules per layer and value
    .with_collision_rules([
        LdtkCollisionRule::solid(1).for_layer("Collision"),
        LdtkCollisionRule::sensor(2, "water").for_layer("Gameplay"),
        LdtkCollisionRule::sensor(3, "damage"),  // applies to all layers
    ])
```

### Reading at runtime

```rust
fn build_colliders(
    mut commands: Commands,
    catalog: Res<LdtkCollisionCatalog>,
    mut ready: MessageReader<LdtkCollisionReadyEvent>,
) {
    for event in ready.read() {
        let cells: Vec<_> = catalog.cells.iter()
            .filter(|cell| cell.level_identifier == event.level_identifier)
            .collect();

        for cell in cells {
            if cell.solid {
                // Create a Rapier/Avian collider at cell.grid_position
            }
            if cell.sensor {
                // Create a sensor trigger with cell.tag
            }
        }
    }
}
```

### LdtkCollisionCell

| Field | Type | Description |
|-------|------|-------------|
| `level_identifier` | `String` | Level containing the cell |
| `level_iid` | `String` | LDtk IID of the level |
| `layer_identifier` | `String` | Layer name |
| `grid_position` | `IVec2` | Grid position |
| `value` | `i32` | IntGrid value |
| `solid` | `bool` | Physically solid |
| `sensor` | `bool` | Sensor/trigger |
| `tag` | `Option<String>` | Semantic tag (e.g. `"water"`) |

Entities whose IntGrid cell matched a rule automatically receive `LdtkCollider { solid, sensor }`.

---

## Layer filters

```rust
// Only catalog these layers
LdtkConfig::default().include_layers(["Collision", "Entities", "Gameplay"])

// Skip these layers (combinable with include_layers)
LdtkConfig::default().exclude_layers(["Debug", "Notes", "Editor"])
```

Filtered layers neither appear in the `LdtkMapCatalog` nor are their entities or tiles processed.

---

## Tile animations

LDtk has no native tile animation. This crate reads a convention from tile custom data:

```
anim=1,2,3;fps=8
frames=1@0.1,2@0.1,3@0.2;repeat=false
```

**Format:**
- `anim=<ids>` or `frames=<ids>`: comma-separated tile IDs
- `fps=<n>`: frames per second (uniform duration)
- `<id>@<seconds>`: individual duration per frame
- `repeat=false`: animation holds the last frame (default: `true`)

Discovered animations live in `LdtkMapCatalog::tile_animations` and `LdtkTileMetadata::animation`. `LdtkTileAnimator` ticks automatically via `GameLdtkPlugin`; non-repeating animations stop on their last frame.

**Tilemap adapter** (experimental, requires the `tilemap` feature):

```rust
app.insert_resource(LdtkLevelManagerConfig {
    enable_tile_animation_adapter: true,
    ..Default::default()
});
```

Applies running `LdtkTileAnimator` state to `bevy_ecs_tilemap`'s `TileTextureIndex`.

---

## Load state and validation

```rust
fn debug_ldtk(
    load: Res<LdtkLoadState>,
    report: Res<LdtkValidationReport>,
) {
    match load.status {
        LdtkLoadStatus::NotLoaded => {}
        LdtkLoadStatus::Loading => {}
        LdtkLoadStatus::Ready => {
            info!("{} level(s) loaded", load.stats.levels);
        }
        LdtkLoadStatus::Error => {
            error!("LDtk error: {:?}", load.errors);
        }
    }

    for warning in &report.warnings {
        warn!("[{}] {}", warning.code, warning.message);
    }
}
```

### LdtkLoadStats

```rust
pub struct LdtkLoadStats {
    pub worlds: usize,
    pub levels: usize,
    pub layers: usize,
    pub tilesets: usize,
    pub tiles: usize,
    pub entities: usize,
    pub spawn_points: usize,
    pub collision_cells: usize,
    pub tile_animations: usize,
}
```

### Validation codes

| Code | Meaning |
|------|---------|
| `external_level_not_cataloged` | External `.ldtkl` level without layer data in the catalog |
| `external_level_wasm_unsupported` | External level not readable on WASM |
| `missing_spawn_point` | Level has no spawn point |
| `unregistered_entity` | LDtk entity has no registered bundle/spawner |
| `missing_tileset_path` | Layer references a tileset without a path |
| `transition_level_missing` | Transition target not in the catalog |
| `transition_spawn_missing` | Requested spawn point not found |

`LdtkConfig::with_strict_validation()` treats every code as an error and sets `LdtkLoadStatus::Error`.

---

## API reference

### Resources

| Resource | Description |
|----------|-------------|
| `LdtkConfig` | Configuration (world path, collision, filters, validation) |
| `LdtkRuntimeState` | Active world, active level, loaded level IIDs |
| `LdtkLoadState` | Status (NotLoaded/Loading/Ready/Error), statistics, warning/error lists |
| `LdtkValidationReport` | Structured warnings and errors with code and message |
| `LdtkMapCatalog` | Worlds, levels, layers, tilesets, tiles, spawn points, entity snapshots, tile animations |
| `LdtkCollisionCatalog` | IntGrid cells with collision type, per-layer summaries |
| `LdtkEntityCatalog` | IID → Bevy entity mapping, entity snapshots |
| `LdtkEntityRegistry` | Registered spawners/bundles |
| `LdtkExternalLevelSource` | Pluggable IO strategy for external levels |
| `LdtkCommandQueue` | Internal command queue (use `LdtkCommandExt` instead) |
| `CurrentLdtkLevel` | Current level identifier + IID (LevelManagerPlugin) |
| `PendingLdtkLevelTransition` | In-flight transition (LevelManagerPlugin) |
| `LevelTransitionState` | Transition status + error message (LevelManagerPlugin) |
| `LdtkLevelManagerConfig` | LevelManagerPlugin configuration |
| `LdtkPlayerLocator` | Explicit player entity for teleporting (optional) |

### Components

| Component | Description |
|-----------|-------------|
| `LdtkWorldRoot` | Marks the root entity of the loaded LDtk world |
| `LdtkEntityMarker` | On every spawned LDtk entity: definition ID, level, world |
| `LdtkImportedEntity` | Snapshot of all LDtk fields on the Bevy entity |
| `LdtkCollider` | `{ solid: bool, sensor: bool }` — set by the collision capture |
| `LdtkTileAnimation` | Animation frames + repeat flag |
| `LdtkTileAnimator` | Running animation state (frame_index, timer) |
| `LdtkPersistent` | Opt-in: entity survives level switches |
| `LdtkLevelPlayer` | Marks the player entity for automatic teleporting |
| `LdtkLevelScoped` | Entity is despawned when the given level is left |

### System sets (in order)

```
LdtkLoadSet::Commands         ← process_ldtk_commands
LdtkLoadSet::Catalog          ← refresh_map_catalog, sync_level_events
LdtkLoadSet::Capture          ← collision, entity_instances, entity_behaviors
LdtkLoadSet::LevelTransitions ← handle_requests, finalize_transition
LdtkLoadSet::Animation        ← tile_animators
```

Your own systems can be ordered relative to these sets:

```rust
app.add_systems(Update,
    my_system.after(LdtkLoadSet::Catalog).before(LdtkLoadSet::Capture)
);
```

### LdtkMapCatalog — key methods

```rust
// Look up a level by identifier or IID (O(1))
catalog.level_by_id_or_iid("Level_01")
catalog.level_by_id_or_iid("abc-123-iid")

// IID → identifier (O(1))
catalog.identifier_for_iid("abc-123-iid")

// Entity snapshot by instance IID (O(1))
catalog.entity_snapshot_by_iid("entity-iid")

// Manual insertion (keeps the secondary indexes in sync)
catalog.insert_level_info(level_info)

catalog.is_empty()
```

Note: `LdtkMapCatalog::layers` is keyed by layer instance **IID** — layer identifiers are not unique across levels.

### Swapping the external level source

For WASM or custom IO:

```rust
use ldtk_integration::{ExternalLevelSource, LdtkExternalLevelSource};

struct MyLevelSource;

impl ExternalLevelSource for MyLevelSource {
    fn load(&self, asset_root: &str, world_path: &str, rel_path: &str) -> Option<String> {
        // Your logic: HTTP fetch, embedded bytes, etc.
        None
    }
}

app.insert_resource(LdtkExternalLevelSource(Some(Box::new(MyLevelSource))));
```

---

## Examples

### `examples/basic_transitions.rs`

Minimal setup: loads the bundled `assets/worlds/AutoLayers_5_Advanced.ldtk` sample and chains two level transitions.

```
cargo run --example basic_transitions
```

### `examples/stealth_doors.rs`

Full setup for a stealth-puzzle game:

- `GameLdtkPlugin` with collision rules
- `LevelManagerPlugin` with player teleporting
- Spawning a `Door` entity from LDtk fields (`target_level`, `target_spawn`)
- Triggering a level transition when entering a door
- Consuming events (`LdtkLevelReadyEvent`, `LdtkCollisionReadyEvent`)
- Logging transition failures

```
cargo run --example stealth_doors
```

The example expects a map at **`assets/worlds/stealth.ldtk`** with:

| What | Requirement |
|------|-------------|
| Levels | at least **2** (doors need a target) |
| Entity `Door` | string fields `target_level` (identifier of the target level) and `target_spawn` (spawn identifier there); give it an editor tile so it is visible |
| Spawn points | a `PlayerSpawn` entity (or any entity tagged `spawn`) in **every** level |
| IntGrid layer `Collision` | value **1** = wall (solid) |
| IntGrid layer `Gameplay` | value **2** = vision zone (sensor, tag `vision_zone`) |

Without the map the app starts, but no levels load and all events stay silent.

---

## Tests

```powershell
cargo test                         # unit + integration tests (headless, no window)
cargo test --no-default-features   # without tilemap + fs (WASM path)
cargo fmt                          # formatting
cargo clippy --all-targets         # lints
```

Coverage: field helpers, layer filters, tile-animation parser (incl. non-repeat stop), tile-ID math, collision rules, spawn-point resolution, transition state, catalog indexes (levels and entities), registry resolve priority, validation logic, plus a headless end-to-end load test against the bundled sample world.
