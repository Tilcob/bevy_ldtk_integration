//! End-to-end test: boots a headless Bevy app (no window, no GPU), loads the
//! bundled sample world, and checks that the catalogs are populated.
//!
//! Headless setup: `WinitPlugin` is disabled and no primary window is spawned.
//! The `RenderPlugin` stays enabled (surfaceless) because `bevy_ecs_tilemap`
//! hard-requires the `RenderApp` sub-app; on machines without a GPU, wgpu falls
//! back to a software adapter (e.g. WARP on Windows, lavapipe on Linux).

use bevy::prelude::*;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use bevy_ecs_ldtk::prelude::LevelSelection;
use ldtk_integration::{
    GameLdtkPlugin, LdtkCollisionCatalog, LdtkConfig, LdtkEntityCatalog, LdtkLoadState,
    LdtkLoadStatus, LdtkMapCatalog, LdtkValidationReport, LevelManagerPlugin,
};

fn headless_app(config: LdtkConfig) -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<WinitPlugin>()
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..Default::default()
            }),
    )
    .add_plugins(GameLdtkPlugin::new(config))
    .add_plugins(LevelManagerPlugin);

    // Driving the app with `update()` skips the runner, which would normally
    // wait for async plugin init (the renderer) and call finish/cleanup. Do it
    // here, otherwise render resources like `RenderDevice` never materialise.
    while app.plugins_state() == bevy::app::PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();
    app
}

fn drive_app_until(app: &mut App, mut predicate: impl FnMut(&World) -> bool, max_ticks: usize) {
    for _ in 0..max_ticks {
        app.update();
        if predicate(app.world()) {
            return;
        }
        // Asset loading is asynchronous; give the IO task pool a moment.
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[test]
fn loads_ldtk_project_and_catalogs_metadata() {
    let mut app = headless_app(
        LdtkConfig::default().with_world_asset_path("worlds/AutoLayers_5_Advanced.ldtk"),
    );

    drive_app_until(
        &mut app,
        |world| {
            let state = world.resource::<LdtkLoadState>();
            matches!(state.status, LdtkLoadStatus::Ready | LdtkLoadStatus::Error)
        },
        500,
    );

    {
        let load_state = app.world().resource::<LdtkLoadState>();
        assert_eq!(
            load_state.status,
            LdtkLoadStatus::Ready,
            "world failed to load: {:?}",
            load_state.errors
        );
    }

    {
        let catalog = app.world().resource::<LdtkMapCatalog>();
        assert!(!catalog.levels.is_empty());
        assert!(!catalog.layers.is_empty());
    }

    let level_identifier = {
        let catalog = app.world().resource::<LdtkMapCatalog>();
        catalog
            .levels
            .keys()
            .next()
            .cloned()
            .expect("expected at least one level identifier")
    };
    *app.world_mut().resource_mut::<LevelSelection>() =
        LevelSelection::Identifier(level_identifier);
    drive_app_until(
        &mut app,
        |world| !world.resource::<LdtkCollisionCatalog>().cells.is_empty(),
        100,
    );

    {
        let catalog = app.world().resource::<LdtkMapCatalog>();
        let validation = app.world().resource::<LdtkValidationReport>();
        let spawn_count: usize = catalog
            .levels
            .values()
            .map(|level| level.spawn_points.len())
            .sum();
        if spawn_count == 0 {
            assert!(
                validation
                    .warnings
                    .iter()
                    .chain(validation.errors.iter())
                    .any(|issue| issue.code == "missing_spawn_point")
            );
        }

        let entity_catalog = app.world().resource::<LdtkEntityCatalog>();
        let entity_count: usize = catalog
            .levels
            .values()
            .map(|level| level.entities.len())
            .sum();
        if entity_count > 0 {
            assert!(!entity_catalog.snapshots.is_empty());
        }
    }

    {
        let collision = app.world().resource::<LdtkCollisionCatalog>();
        let load_state = app.world().resource::<LdtkLoadState>();
        assert_eq!(collision.cells.len(), load_state.stats.collision_cells);
    }
}
