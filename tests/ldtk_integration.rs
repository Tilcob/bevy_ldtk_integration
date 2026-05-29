use bevy::app::{Main, PostStartup, Startup};
use bevy::prelude::*;
use bevy::winit::WinitPlugin;
use bevy_ecs_ldtk::prelude::LevelSelection;
use ldtk_integration::{
    GameLdtkPlugin, LdtkCollisionCatalog, LdtkConfig, LdtkEntityCatalog, LdtkLoadState,
    LdtkLoadStatus, LdtkMapCatalog, LdtkValidationReport, LevelManagerPlugin,
};

fn run_startup(app: &mut App) {
    app.world_mut().run_schedule(Startup);
    app.world_mut().run_schedule(PostStartup);
}

fn update_main(app: &mut App) {
    app.world_mut().run_schedule(Main);
}

fn drive_app_until(app: &mut App, mut predicate: impl FnMut(&World) -> bool, max_ticks: usize) {
    for _ in 0..max_ticks {
        update_main(app);
        if predicate(app.world()) {
            break;
        }
    }
}

#[test]
#[cfg_attr(
    target_os = "windows",
    ignore = "requires full window/render resources in the test harness"
)]
fn loads_ldtk_project_and_catalogs_metadata() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WinitPlugin {
        run_on_any_thread: true,
        ..Default::default()
    }))
    .add_plugins(GameLdtkPlugin::new(
        LdtkConfig::default().with_world_asset_path("worlds/AutoLayers_5_Advanced.ldtk"),
    ))
    .add_plugins(LevelManagerPlugin);

    run_startup(&mut app);
    drive_app_until(
        &mut app,
        |world| {
            let state = world.resource::<LdtkLoadState>();
            matches!(state.status, LdtkLoadStatus::Ready | LdtkLoadStatus::Error)
        },
        200,
    );

    {
        let load_state = app.world().resource::<LdtkLoadState>();
        assert_eq!(load_state.status, LdtkLoadStatus::Ready);
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
    drive_app_until(&mut app, |_| false, 40);

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
