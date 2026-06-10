//! Registration of Bevy bundles and spawner callbacks for LDtk entity
//! definitions, resolved at runtime when entity instances are encountered.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::entities::{LdtkEntityMarker, LdtkEntitySpawnContext};

/// Composite key used to look up a registered spawner in [`LdtkEntityRegistry`],
/// supporting both exact (layer + entity) and wildcard (entity-only or default)
/// matches.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LdtkEntityRegistryKey {
    /// When `Some`, the spawner only applies to this layer; `None` matches any layer.
    pub layer_identifier: Option<String>,
    /// When `Some`, the spawner only applies to this entity definition; `None` is the catch-all default.
    pub entity_identifier: Option<String>,
}

/// Type alias for a boxed spawner callback stored in [`LdtkEntityRegistry`].
pub type LdtkEntitySpawner =
    Box<dyn Fn(&mut World, Entity, &LdtkEntitySpawnContext) + Send + Sync + 'static>;

/// Bevy resource that maps LDtk entity definitions to Bevy spawner callbacks,
/// resolved at runtime when entity instances are encountered.
#[derive(Resource, Default)]
pub struct LdtkEntityRegistry {
    /// Registered spawners keyed by [`LdtkEntityRegistryKey`].
    pub spawners: HashMap<LdtkEntityRegistryKey, LdtkEntitySpawner>,
}

impl LdtkEntityRegistry {
    /// Registers `B::default()` as the bundle to insert for any entity matching
    /// `identifier`, regardless of which layer it is on.
    pub fn register_bundle<B>(&mut self, identifier: impl Into<String>)
    where
        B: Bundle + Default + Send + Sync + 'static,
    {
        self.register_bundle_for_layer_optional::<B>(None, Some(identifier.into()));
    }

    /// Registers `B::default()` as the bundle to insert for entities matching
    /// both `layer_identifier` and `identifier`.
    pub fn register_bundle_for_layer<B>(
        &mut self,
        layer_identifier: impl Into<String>,
        identifier: impl Into<String>,
    ) where
        B: Bundle + Default + Send + Sync + 'static,
    {
        self.register_bundle_for_layer_optional::<B>(
            Some(layer_identifier.into()),
            Some(identifier.into()),
        );
    }

    /// Registers `B::default()` as the fallback bundle for any unmatched entity
    /// on `layer_identifier`.
    pub fn register_default_bundle_for_layer<B>(&mut self, layer_identifier: impl Into<String>)
    where
        B: Bundle + Default + Send + Sync + 'static,
    {
        self.register_bundle_for_layer_optional::<B>(Some(layer_identifier.into()), None);
    }

    /// Registers `B::default()` as the global fallback bundle for any entity not
    /// matched by a more specific registration.
    pub fn register_default_bundle<B>(&mut self)
    where
        B: Bundle + Default + Send + Sync + 'static,
    {
        self.register_bundle_for_layer_optional::<B>(None, None);
    }

    /// Low-level registration that accepts optional layer and entity identifiers
    /// directly; prefer the typed helpers above for clarity.
    pub fn register_bundle_for_layer_optional<B>(
        &mut self,
        layer_identifier: Option<String>,
        entity_identifier: Option<String>,
    ) where
        B: Bundle + Default + Send + Sync + 'static,
    {
        let key = LdtkEntityRegistryKey {
            layer_identifier,
            entity_identifier,
        };

        self.spawners.insert(
            key,
            Box::new(
                move |world: &mut World, entity: Entity, context: &LdtkEntitySpawnContext| {
                    // NOTE: do NOT insert a `Transform` here. `bevy_ecs_ldtk`
                    // already gives every entity instance the correct Y-up,
                    // pivot-adjusted transform (see
                    // `calculate_transform_from_entity_instance`). Overwriting it
                    // with `context.position` (raw LDtk Y-down pixel coords)
                    // mirrors the entity vertically and ignores the level's world
                    // translation, placing it in the wrong spot.
                    world.entity_mut(entity).insert((
                        B::default(),
                        LdtkEntityMarker {
                            definition_identifier: context.entity_identifier.clone(),
                            level_identifier: context.level_identifier.clone(),
                            world_identifier: context.world_identifier.clone(),
                        },
                    ));
                },
            ),
        );
    }

    /// Registers a custom `spawner` closure for any entity matching `identifier`,
    /// regardless of layer.
    pub fn register_spawner(
        &mut self,
        identifier: impl Into<String>,
        spawner: impl Fn(&mut World, Entity, &LdtkEntitySpawnContext) + Send + Sync + 'static,
    ) {
        self.register_spawner_for_layer_optional(None, Some(identifier.into()), spawner);
    }

    /// Registers a custom `spawner` closure for entities matching both
    /// `layer_identifier` and `entity_identifier`.
    pub fn register_spawner_for_layer(
        &mut self,
        layer_identifier: impl Into<String>,
        entity_identifier: impl Into<String>,
        spawner: impl Fn(&mut World, Entity, &LdtkEntitySpawnContext) + Send + Sync + 'static,
    ) {
        self.register_spawner_for_layer_optional(
            Some(layer_identifier.into()),
            Some(entity_identifier.into()),
            spawner,
        );
    }

    /// Low-level spawner registration accepting optional identifiers directly;
    /// prefer the typed helpers above for clarity.
    pub fn register_spawner_for_layer_optional(
        &mut self,
        layer_identifier: Option<String>,
        entity_identifier: Option<String>,
        spawner: impl Fn(&mut World, Entity, &LdtkEntitySpawnContext) + Send + Sync + 'static,
    ) {
        self.spawners.insert(
            LdtkEntityRegistryKey {
                layer_identifier,
                entity_identifier,
            },
            Box::new(spawner),
        );
    }

    /// Resolves the best matching spawner for an entity instance using a four-level
    /// priority: exact (layer + entity) > entity-only > layer-only > global default.
    /// Returns `None` when no spawner has been registered for this combination.
    pub fn resolve(
        &self,
        layer_identifier: Option<&str>,
        entity_identifier: &str,
    ) -> Option<&LdtkEntitySpawner> {
        let exact = LdtkEntityRegistryKey {
            layer_identifier: layer_identifier.map(ToOwned::to_owned),
            entity_identifier: Some(entity_identifier.to_string()),
        };
        let entity_only = LdtkEntityRegistryKey {
            layer_identifier: None,
            entity_identifier: Some(entity_identifier.to_string()),
        };
        let layer_only = layer_identifier.map(|layer| LdtkEntityRegistryKey {
            layer_identifier: Some(layer.to_string()),
            entity_identifier: None,
        });
        let default = LdtkEntityRegistryKey {
            layer_identifier: None,
            entity_identifier: None,
        };

        self.spawners
            .get(&exact)
            .or_else(|| self.spawners.get(&entity_only))
            .or_else(|| layer_only.as_ref().and_then(|key| self.spawners.get(key)))
            .or_else(|| self.spawners.get(&default))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Registers a spawner that records `marker` when invoked, so tests can
    /// observe which registration `resolve` picked.
    fn register_marked(
        registry: &mut LdtkEntityRegistry,
        layer: Option<&str>,
        entity: Option<&str>,
        marker: usize,
        observed: &Arc<AtomicUsize>,
    ) {
        let observed = Arc::clone(observed);
        registry.register_spawner_for_layer_optional(
            layer.map(ToOwned::to_owned),
            entity.map(ToOwned::to_owned),
            move |_world, _entity, _context| {
                observed.store(marker, Ordering::SeqCst);
            },
        );
    }

    fn run_resolved(
        registry: &LdtkEntityRegistry,
        layer: Option<&str>,
        entity: &str,
        observed: &Arc<AtomicUsize>,
    ) -> Option<usize> {
        let spawner = registry.resolve(layer, entity)?;
        let mut world = World::new();
        let probe = world.spawn_empty().id();
        spawner(&mut world, probe, &LdtkEntitySpawnContext::default());
        Some(observed.load(Ordering::SeqCst))
    }

    #[test]
    fn resolve_prefers_exact_over_wildcards() {
        let observed = Arc::new(AtomicUsize::new(0));
        let mut registry = LdtkEntityRegistry::default();
        register_marked(&mut registry, None, None, 1, &observed); // default
        register_marked(&mut registry, Some("Objects"), None, 2, &observed); // layer-only
        register_marked(&mut registry, None, Some("Door"), 3, &observed); // entity-only
        register_marked(&mut registry, Some("Objects"), Some("Door"), 4, &observed); // exact

        assert_eq!(
            run_resolved(&registry, Some("Objects"), "Door", &observed),
            Some(4)
        );
    }

    #[test]
    fn resolve_falls_back_entity_then_layer_then_default() {
        let observed = Arc::new(AtomicUsize::new(0));
        let mut registry = LdtkEntityRegistry::default();
        register_marked(&mut registry, None, None, 1, &observed);
        register_marked(&mut registry, Some("Objects"), None, 2, &observed);
        register_marked(&mut registry, None, Some("Door"), 3, &observed);

        // Entity-only beats layer-only.
        assert_eq!(
            run_resolved(&registry, Some("Objects"), "Door", &observed),
            Some(3)
        );
        // Unknown entity on a registered layer falls back to layer-only.
        assert_eq!(
            run_resolved(&registry, Some("Objects"), "Chest", &observed),
            Some(2)
        );
        // Unknown entity on an unknown layer falls back to the global default.
        assert_eq!(
            run_resolved(&registry, Some("Other"), "Chest", &observed),
            Some(1)
        );
    }

    #[test]
    fn resolve_returns_none_without_matching_registration() {
        let observed = Arc::new(AtomicUsize::new(0));
        let mut registry = LdtkEntityRegistry::default();
        register_marked(&mut registry, Some("Objects"), Some("Door"), 4, &observed);

        assert!(registry.resolve(Some("Objects"), "Chest").is_none());
        assert!(registry.resolve(None, "Door").is_none());
    }
}
