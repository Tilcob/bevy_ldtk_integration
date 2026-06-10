//! Minimal demo: loads the bundled `AutoLayers_5_Advanced.ldtk` sample world
//! and chains two level transitions to exercise the level manager.
//!
//! ```text
//! cargo run --example basic_transitions
//! ```

use bevy::prelude::*;
use ldtk_integration::{
    GameLdtkPlugin, LdtkCommandExt, LdtkLevelManagerConfig, LdtkLevelPlayer, LdtkLevelReadyEvent,
    LdtkMapCatalog, LdtkMapLoadedEvent, LevelManagerPlugin,
};

#[derive(Resource, Default)]
struct DemoTransitionState {
    first_level: Option<String>,
    second_level: Option<String>,
    did_initial: bool,
    did_second: bool,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GameLdtkPlugin::default())
        .add_plugins(LevelManagerPlugin)
        .insert_resource(LdtkLevelManagerConfig {
            allow_missing_spawnpoints: true,
            ..Default::default()
        })
        .init_resource::<DemoTransitionState>()
        .add_systems(Startup, (setup_camera, spawn_player, bootstrap_ldtk_world))
        .add_systems(Update, (queue_initial_transition, queue_second_transition))
        .run();
}

fn setup_camera(mut commands: Commands<'_, '_>) {
    commands.spawn((Camera2d, Name::new("Main 2D Camera")));
}

fn spawn_player(mut commands: Commands<'_, '_>) {
    commands.spawn((
        LdtkLevelPlayer,
        Transform::from_translation(Vec3::ZERO),
        GlobalTransform::default(),
        Name::new("Player (LdtkLevelPlayer)"),
    ));
}

fn bootstrap_ldtk_world(mut commands: Commands<'_, '_>) {
    commands.spawn_ldtk_world("worlds/AutoLayers_5_Advanced.ldtk");
}

fn queue_initial_transition(
    mut messages: MessageReader<'_, '_, LdtkMapLoadedEvent>,
    catalog: Res<'_, LdtkMapCatalog>,
    mut state: ResMut<'_, DemoTransitionState>,
    mut commands: Commands<'_, '_>,
) {
    if state.did_initial {
        return;
    }

    if messages.read().next().is_some() {
        let mut levels = catalog.levels.keys().cloned().collect::<Vec<_>>();
        levels.sort();
        state.first_level = levels.first().cloned();
        state.second_level = levels.get(1).cloned();
        if let Some(level) = state.first_level.clone() {
            commands.transition_to_ldtk_level(level, None::<String>);
            state.did_initial = true;
        }
    }
}

fn queue_second_transition(
    mut messages: MessageReader<'_, '_, LdtkLevelReadyEvent>,
    mut state: ResMut<'_, DemoTransitionState>,
    mut commands: Commands<'_, '_>,
) {
    if state.did_second {
        return;
    }

    for _ in messages.read() {
        if let Some(level) = state.second_level.clone() {
            commands.transition_to_ldtk_level(level, None::<String>);
            state.did_second = true;
        }
    }
}
