# LDtk Integration for Bevy

Ein fokussiertes Bevy-Plugin zum Laden von LDtk-Maps in Spielen. Das Rendering und Asset-Loading kommen von `bevy_ecs_ldtk`; dieses Crate legt darueber eine spielnahe API fuer Runtime-State, Kataloge, Entities, Collision, Validierung und Tile-Animations-Metadaten.

## Features

- Laedt LDtk Worlds ueber Bevy `AssetServer` und `bevy_ecs_ldtk`
- Verwaltet `LevelSelection`, aktive Welt, aktive Level und geladene Level
- Baut einen `LdtkMapCatalog` mit Worlds, Levels, Layers, Tilesets, Tiles, Spawnpoints und Entity-Snapshots
- Erfasst LDtk Custom Fields inklusive Tile-, EntityRef-, Array- und Point-Feldern
- Bietet typed Field-Helper wie `field_str`, `field_i64`, `field_bool`
- Registriert LDtk Entities per Bundle oder eigener Spawner-Funktion
- Erfasst IntGrid Collision mit konfigurierbaren Regeln pro Layer und Wert
- Unterstuetzt Layer-Filter fuer Katalog/Spiel-Import
- Fuehrt Load-State und Validation-Report als Ressourcen (optional strict)
- Katalogisiert ausgelagerte `.ldtkl` Level-Dateien aus dem Asset-Ordner (Desktop-Only)
- Liest Tile-Animations-Metadaten aus LDtk Tile Custom Data
- Bietet einen optionalen LevelManager mit Transition-API, Spawnpoint-Logik und Cleanup
- Liefert `LdtkCollisionReadyEvent` fuer Physics-Adapter ohne harte Abhaengigkeit

## Installation als Dependency

Wenn das Repository lokal neben deinem Spiel liegt:

```toml
[dependencies]
ldtk_integration = { path = "../ldtk_integration" }
```

Oder per Git:

```toml
[dependencies]
ldtk_integration = { git = "https://github.com/<user>/<repo>.git" }
```

Tilemap-Adapter optional deaktivieren:

```toml
[dependencies]
ldtk_integration = { path = "../ldtk_integration", default-features = false }
```

Die Bevy-Version muss zur Dependency passen:

```toml
bevy = "0.18.1"
bevy_ecs_ldtk = "0.14.0"
serde_json = "1"
```

## Schnellstart

LDtk-Dateien liegen in Bevy relativ zu `assets/`. Wenn deine Datei unter `assets/worlds/map.ldtk` liegt, laedt man sie als `worlds/map.ldtk`.

```rust
use bevy::prelude::*;
use ldtk_integration::{GameLdtkPlugin, LdtkConfig};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GameLdtkPlugin::new(
            LdtkConfig::default()
                .with_world_asset_path("worlds/map.ldtk")
                .with_asset_root("assets")
                .with_solid_int_grid_values([1]),
        ))
        .run();
}
```

Alternativ kannst du eine Welt per Command laden:

```rust
use bevy::prelude::*;
use ldtk_integration::LdtkCommandExt;

fn load_map(mut commands: Commands) {
    commands.spawn_ldtk_world("worlds/map.ldtk");
}
```

Level wechseln:

```rust
fn change_level(mut commands: Commands) {
    commands.change_ldtk_level("Level_1");
}
```

## Level Manager

Der LevelManager ist ein separates Plugin und kann optional zugeschaltet werden:

```rust
use bevy::prelude::*;
use ldtk_integration::{GameLdtkPlugin, LevelManagerPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GameLdtkPlugin::default())
        .add_plugins(LevelManagerPlugin)
        .run();
}
```

Levelwechsel mit Spawnpoint:

```rust
use bevy::prelude::*;
use ldtk_integration::LdtkCommandExt;

fn transition_level(mut commands: Commands) {
    commands.transition_to_ldtk_level("Level_1", Some("Spawn_A"));
}
```

Spawnpoint-Konventionen:

- Wenn `spawn_id` gesetzt ist, wird der Spawnpoint mit Identifier oder Tag gefunden.
- Ohne `spawn_id` wird zuerst ein Spawnpoint mit Identifier/Tag `PlayerSpawn` verwendet.
- Fallback: erster Spawnpoint des Levels.
- Wenn kein Spawnpoint existiert, wird ein Warnhinweis im `LdtkValidationReport` gesetzt und die Transition markiert als `Failed`.
- Optional: `LdtkLevelManagerConfig { allow_missing_spawnpoints: true }` erlaubt einen Fallback auf `(0, 0)`.

Player-Positionierung:

- Markiere deine Spieler-Entity mit `LdtkLevelPlayer` oder setze `LdtkPlayerLocator { entity: Some(player) }`.
- Beim erfolgreichen Transition-Finish wird der Spieler auf den Spawnpoint gesetzt.

Persistenz und Cleanup:

- Entities mit `LdtkPersistent` werden beim Levelwechsel nicht entfernt.
- LDtk-Entities tragen automatisch `LdtkEntityMarker`, sind aber **nicht** persistent (opt-in).
- Eigene Entities koennen mit `LdtkLevelScoped { level_identifier }` fuer Cleanup markiert werden.

## Entity-Registrierung

Einfluss auf Persistenz:

- Standard: der Entity-Spawn aus LDtk erzeugt **keinen** `LdtkPersistent` Marker.
- Opt-in: setze `LdtkPersistent` im Spawner/Bundle, wenn die Entity Level-uebergreifend bleiben soll.

Einfache Registrierung per Bundle:

```rust
use bevy::prelude::*;
use ldtk_integration::LdtkAppExt;

#[derive(Component, Default)]
struct Player;

#[derive(Bundle, Default)]
struct PlayerBundle {
    player: Player,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ldtk_integration::GameLdtkPlugin::default())
        .register_ldtk_entity::<PlayerBundle>("Player")
        .run();
}
```

Flexible Registrierung per Spawner:

```rust
use bevy::prelude::*;
use ldtk_integration::{LdtkAppExt, LdtkEntitySpawnContext, LdtkPersistent};

#[derive(Component)]
struct Door {
    target: String,
    locked: bool,
}

fn register_entities(app: &mut App) {
    app.register_ldtk_entity_spawner(
        "Door",
        |world: &mut World, entity: Entity, context: &LdtkEntitySpawnContext| {
            let target = context.field_str("target").unwrap_or_default().to_string();
            let locked = context.field_bool("locked").unwrap_or(false);

            world.entity_mut(entity).insert((Door { target, locked }, LdtkPersistent));
        },
    );
}
```

## Collision-Regeln

Standard: Wenn keine Regeln gesetzt sind, gilt jeder IntGrid-Wert ungleich `0` als solid. Praeziser ist eine Konfiguration:

```rust
use ldtk_integration::{GameLdtkPlugin, LdtkCollisionRule, LdtkConfig};

let config = LdtkConfig::default()
    .with_world_asset_path("worlds/map.ldtk")
    .with_collision_rules([
        LdtkCollisionRule::solid(1).for_layer("Collision"),
        LdtkCollisionRule::sensor(2, "water").for_layer("Collision"),
        LdtkCollisionRule::sensor(3, "damage").for_layer("Gameplay"),
    ]);

app.add_plugins(GameLdtkPlugin::new(config));
```

Collision-Daten stehen in `LdtkCollisionCatalog`. Entities mit passender IntGrid-Zelle bekommen `LdtkCollider`.

Physics-Adapter koennen auf `LdtkCollisionReadyEvent` reagieren und die Daten aus `LdtkCollisionCatalog` nutzen.

## Load-State und Validierung

Das Plugin stellt `LdtkLoadState` und `LdtkValidationReport` bereit:

```rust
use bevy::prelude::*;
use ldtk_integration::{LdtkLoadState, LdtkValidationReport};

fn debug_ldtk(load: Res<LdtkLoadState>, report: Res<LdtkValidationReport>) {
    if load.is_ready() {
        info!("LDtk ready: {:?}", load.stats);
    }

    for warning in &report.warnings {
        warn!("LDtk {}: {}", warning.code, warning.message);
    }
}
```

Validierung warnt unter anderem bei:

- nicht registrierten LDtk Entities
- Levels ohne Spawnpoint
- externen `.ldtkl` Levels, deren Layerdaten nicht katalogisiert werden konnten
- Tileset-Referenzen ohne relativen Pfad

`LdtkConfig::with_strict_validation()` stuft die oben genannten Punkte als Fehler ein und setzt `LdtkLoadState` auf `Error`.

Externe `.ldtkl` Dateien werden fuer den Metadaten-Katalog relativ zum geladenen `.ldtk` unterhalb von `LdtkConfig::asset_root` gelesen. Auf `wasm32` wird dieser Schritt bewusst uebersprungen (Warnung im Validation-Report). Das eigentliche Rendering/Spawning bleibt weiterhin Aufgabe von `bevy_ecs_ldtk`.

## Tile-Animationen

LDtk selbst liefert in den von `bevy_ecs_ldtk 0.14.0` genutzten Rust-Typen keine native Tile-Animation. Dieses Plugin liest deshalb eine einfache Konvention aus Tile Custom Data:

```text
anim=1,2,3;fps=8
frames=1@0.1,2@0.1,3@0.2;repeat=false
```

Gefundene Animationen stehen in:

- `LdtkMapCatalog::tile_animations`
- `LdtkTileMetadata::animation`

Der LevelManager enthaelt einen **experimentellen** Adapter, der fuer Tilemap-Entities mit `TileTextureIndex` die Animationen aktualisiert. Aktivierung:

- Feature `tilemap` (default an)
- `LdtkLevelManagerConfig { enable_tile_animation_adapter: true }`

Wenn die `bevy_ecs_tilemap`-Komponenten nicht vorhanden sind oder das Layout nicht passt, passiert nichts.

## Layer-Filter

Nur bestimmte Layer katalogisieren:

```rust
let config = LdtkConfig::default()
    .include_layers(["Collision", "Entities", "Gameplay"]);
```

Oder Debug-/Editor-Layer ausschliessen:

```rust
let config = LdtkConfig::default()
    .exclude_layers(["Debug", "Notes"]);
```

## Wichtige Ressourcen

- `LdtkRuntimeState`: aktive Welt, aktives Level, geladene Level
- `LdtkLoadState`: Loading/Ready/Error plus Statistiken
- `LdtkMapCatalog`: Worlds, Levels, Layers, Tilesets, Tiles, Entities, Spawnpoints
- `LdtkCollisionCatalog`: IntGrid Collision-Zellen und Layer-Zusammenfassung
- `LdtkEntityCatalog`: Zuordnung LDtk Entity IID zu Bevy Entity
- `LdtkValidationReport`: Warnungen und Fehler fuer Projektkonventionen

## Tests

```powershell
cargo fmt
cargo check
cargo test
```

Aktuell decken Unit-Tests Field-Helper, Layer-Filter, Tile-Animationsparser, Tile-ID-Berechnung, Collision-Regeln sowie Spawnpoint/Transition-Logik ab. Ein Integrationstest laedt eine echte LDtk-Datei und prueft LoadState, Kataloge und Collision-Daten.
