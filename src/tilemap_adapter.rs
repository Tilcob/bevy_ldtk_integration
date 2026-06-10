//! Experimental adapter that applies [`LdtkTileAnimator`] state to
//! `bevy_ecs_tilemap` tiles. Only compiled with the `tilemap` feature and only
//! active when [`LdtkLevelManagerConfig::enable_tile_animation_adapter`] is set.

use bevy::prelude::*;
use bevy_ecs_ldtk::prelude::LayerMetadata;
use bevy_ecs_tilemap::prelude::{TilePos, TileTextureIndex, TilemapId};
use std::collections::HashMap;

use crate::animation::{LdtkTileAnimation, LdtkTileAnimator};
use crate::catalog::LdtkMapCatalog;
use crate::level_manager::LdtkLevelManagerConfig;
use crate::plugin::LdtkLoadSet;

/// Wires the adapter systems into `app`; called by
/// [`LevelManagerPlugin`](crate::LevelManagerPlugin).
pub(crate) fn register(app: &mut App) {
    app.init_resource::<LdtkTileAnimationLookup>().add_systems(
        Update,
        (
            rebuild_tile_animation_lookup,
            attach_tile_animators_to_tiles,
            apply_tile_animation_to_tilemap,
        )
            .chain()
            .in_set(LdtkLoadSet::Animation)
            .run_if(resource_exists::<LdtkMapCatalog>),
    );
}

/// Identifies one tilemap layer instance: `(level iid, layer iid)`.
type LayerKey = (String, String);

/// Animations indexed per layer first, then per grid position, so the per-tile
/// hot path is a plain `IVec2` lookup with zero string work.
#[derive(Debug, Clone, Resource, Default)]
struct LdtkTileAnimationLookup {
    by_layer: HashMap<LayerKey, HashMap<IVec2, LdtkTileAnimation>>,
}

fn rebuild_tile_animation_lookup(
    catalog: Res<'_, LdtkMapCatalog>,
    config: Res<'_, LdtkLevelManagerConfig>,
    mut lookup: ResMut<'_, LdtkTileAnimationLookup>,
) {
    if !catalog.is_changed() || !config.enable_tile_animation_adapter {
        return;
    }

    lookup.by_layer = build_tile_animation_lookup(&catalog);
}

fn build_tile_animation_lookup(
    catalog: &LdtkMapCatalog,
) -> HashMap<LayerKey, HashMap<IVec2, LdtkTileAnimation>> {
    let mut lookup: HashMap<LayerKey, HashMap<IVec2, LdtkTileAnimation>> = HashMap::new();

    for level in catalog.levels.values() {
        for tile in &level.tiles {
            let Some(animation) = &tile.animation else {
                continue;
            };
            let Some(layer) = catalog.layers.get(&tile.layer_iid) else {
                continue;
            };
            if layer.grid_size <= 0 {
                continue;
            }

            // `layer_position` is in pixels; convert to grid coordinates.
            let grid_pos = IVec2::new(
                tile.layer_position.x / layer.grid_size,
                tile.layer_position.y / layer.grid_size,
            );

            lookup
                .entry((level.iid.clone(), tile.layer_iid.clone()))
                .or_default()
                .insert(grid_pos, animation.clone());
        }
    }

    lookup
}

/// Attaches an [`LdtkTileAnimator`] to every spawned tile that has an animation
/// in the lookup.
///
/// Cost model: tiles only need (re)examination when the lookup was rebuilt or
/// new tiles spawned, so the system bails out otherwise instead of scanning the
/// full tile set every frame. The per-layer animation map is resolved once per
/// tilemap, keeping the per-tile work to a single `IVec2` hash lookup without
/// any string allocation.
fn attach_tile_animators_to_tiles(
    mut commands: Commands<'_, '_>,
    lookup: Res<'_, LdtkTileAnimationLookup>,
    config: Res<'_, LdtkLevelManagerConfig>,
    catalog: Res<'_, LdtkMapCatalog>,
    new_tiles: Query<'_, '_, (), Added<TilePos>>,
    mut tile_query: Query<
        '_,
        '_,
        (Entity, &TilePos, &TilemapId, &mut TileTextureIndex),
        Without<LdtkTileAnimator>,
    >,
    layer_query: Query<'_, '_, &LayerMetadata>,
) {
    if !config.enable_tile_animation_adapter || lookup.by_layer.is_empty() {
        return;
    }
    if !lookup.is_changed() && new_tiles.is_empty() {
        return;
    }

    // Per-tilemap cache: each tilemap entity resolves to "its" animation map
    // (or None) exactly once per pass.
    let lookup = &*lookup;
    let mut per_tilemap: HashMap<Entity, Option<&HashMap<IVec2, LdtkTileAnimation>>> =
        HashMap::new();

    for (entity, pos, tilemap_id, mut texture_index) in tile_query.iter_mut() {
        let animation_map = *per_tilemap.entry(tilemap_id.0).or_insert_with(|| {
            layer_query.get(tilemap_id.0).ok().and_then(|layer_meta| {
                let level_iid =
                    resolve_level_iid_from_metadata(&catalog, &layer_meta.level_id.to_string());
                lookup.by_layer.get(&(level_iid, layer_meta.iid.clone()))
            })
        });
        let Some(animation_map) = animation_map else {
            continue;
        };
        let Some(animation) = animation_map.get(&IVec2::new(pos.x as i32, pos.y as i32)) else {
            continue;
        };

        if let Some(first) = animation.frames.first()
            && texture_index.0 != first.tile_id as u32
        {
            texture_index.0 = first.tile_id as u32;
        }
        commands
            .entity(entity)
            .insert(LdtkTileAnimator::new(animation.clone()));
    }
}

fn resolve_level_iid_from_metadata(catalog: &LdtkMapCatalog, level_id: &str) -> String {
    // `level_id` may already be an iid, or an identifier we need to translate.
    catalog
        .level_by_id_or_iid(level_id)
        .map(|level| level.iid.clone())
        .unwrap_or_else(|| level_id.to_string())
}

/// Copies the current animator frame into `TileTextureIndex`. Writes only when
/// the index actually differs, so Bevy's change detection (and the tilemap
/// chunk re-upload it triggers) fires on real frame steps instead of every tick.
fn apply_tile_animation_to_tilemap(
    mut query: Query<'_, '_, (&LdtkTileAnimator, &mut TileTextureIndex)>,
) {
    for (animator, mut texture_index) in query.iter_mut() {
        let Some(frame) = animator.animation.frames.get(animator.frame_index) else {
            continue;
        };
        let target = frame.tile_id as u32;
        if texture_index.0 != target {
            texture_index.0 = target;
        }
    }
}
