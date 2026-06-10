//! Plugin configuration: [`LdtkConfig`] and the collision-rule DSL.

use bevy::prelude::*;
use std::collections::HashSet;

/// Runtime configuration for the LDtk integration. Pass it to
/// [`GameLdtkPlugin::new`](crate::GameLdtkPlugin::new), which inserts it as a
/// Bevy resource during plugin build.
#[derive(Debug, Clone, Resource)]
pub struct LdtkConfig {
    /// Root directory that asset paths are resolved relative to (e.g. `"assets"`).
    pub asset_root: String,
    /// Asset-relative path to the `.ldtk` world file to load on startup, if any.
    pub world_asset_path: Option<String>,
    /// Whether external `.ldtkl` level files are discovered and cataloged.
    pub catalog_external_levels: bool,
    /// When `true`, levels adjacent to the active level are loaded proactively.
    pub load_level_neighbors: bool,
    /// IntGrid values that are treated as solid (impassable) by default.
    pub int_grid_solid_values: HashSet<i32>,
    /// Ordered list of collision rules that override the default solid-value behaviour.
    pub collision_rules: Vec<LdtkCollisionRule>,
    /// When non-empty, only layers whose identifiers appear here are processed.
    pub include_layers: HashSet<String>,
    /// Layers whose identifiers appear here are skipped even if in `include_layers`.
    pub exclude_layers: HashSet<String>,
    /// Runs structural validation after the world is cataloged when `true`.
    pub validate_on_load: bool,
    /// When `true`, validation warnings are promoted to errors that abort loading.
    pub strict_validation: bool,
    /// Emits a Bevy warning for every LDtk entity that has no registered spawner.
    pub warn_on_unregistered_entities: bool,
}

impl Default for LdtkConfig {
    fn default() -> Self {
        Self {
            asset_root: String::from("assets"),
            world_asset_path: None,
            catalog_external_levels: true,
            load_level_neighbors: true,
            int_grid_solid_values: HashSet::new(),
            collision_rules: Vec::new(),
            include_layers: HashSet::new(),
            exclude_layers: HashSet::new(),
            validate_on_load: true,
            strict_validation: false,
            warn_on_unregistered_entities: true,
        }
    }
}

impl LdtkConfig {
    /// Sets [`Self::world_asset_path`] and returns `self` for chaining.
    pub fn with_world_asset_path(mut self, path: impl Into<String>) -> Self {
        self.world_asset_path = Some(path.into());
        self
    }

    /// Overrides [`Self::asset_root`] and returns `self` for chaining.
    pub fn with_asset_root(mut self, path: impl Into<String>) -> Self {
        self.asset_root = path.into();
        self
    }

    /// Disables external-level cataloging and returns `self` for chaining.
    pub fn without_external_level_catalog(mut self) -> Self {
        self.catalog_external_levels = false;
        self
    }

    /// Replaces [`Self::int_grid_solid_values`] with `values` and returns `self` for chaining.
    pub fn with_solid_int_grid_values(mut self, values: impl IntoIterator<Item = i32>) -> Self {
        self.int_grid_solid_values = values.into_iter().collect();
        self
    }

    /// Replaces [`Self::collision_rules`] with `rules` and returns `self` for chaining.
    pub fn with_collision_rules(
        mut self,
        rules: impl IntoIterator<Item = LdtkCollisionRule>,
    ) -> Self {
        self.collision_rules = rules.into_iter().collect();
        self
    }

    /// Sets the layer allow-list and returns `self` for chaining.
    pub fn include_layers(mut self, layers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.include_layers = layers.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the layer deny-list and returns `self` for chaining.
    pub fn exclude_layers(mut self, layers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.exclude_layers = layers.into_iter().map(Into::into).collect();
        self
    }

    /// Disables on-load validation and returns `self` for chaining.
    pub fn without_validation(mut self) -> Self {
        self.validate_on_load = false;
        self
    }

    /// Suppresses warnings for unregistered LDtk entities and returns `self` for chaining.
    pub fn without_unregistered_entity_warnings(mut self) -> Self {
        self.warn_on_unregistered_entities = false;
        self
    }

    /// Enables strict validation (warnings become errors) and returns `self` for chaining.
    pub fn with_strict_validation(mut self) -> Self {
        self.strict_validation = true;
        self
    }

    /// Returns `true` when `layer_identifier` passes the include/exclude filter
    /// configured in [`Self::include_layers`] and [`Self::exclude_layers`].
    pub fn should_include_layer(&self, layer_identifier: &str) -> bool {
        (self.include_layers.is_empty() || self.include_layers.contains(layer_identifier))
            && !self.exclude_layers.contains(layer_identifier)
    }
}

/// A single rule that maps an IntGrid value (optionally scoped to a layer) to
/// collision behaviour, overriding the global [`LdtkConfig::int_grid_solid_values`] set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LdtkCollisionRule {
    /// When `Some`, the rule only applies to the named layer; `None` matches every layer.
    pub layer_identifier: Option<String>,
    /// The IntGrid cell value this rule matches against.
    pub value: i32,
    /// When `true`, matching cells are treated as solid (impassable) colliders.
    pub solid: bool,
    /// When `true`, matching cells are treated as sensor (trigger) colliders.
    pub sensor: bool,
    /// Optional tag written to [`LdtkCollisionCell::tag`](crate::LdtkCollisionCell::tag) for sensor cells.
    pub tag: Option<String>,
}

impl LdtkCollisionRule {
    /// Creates a rule that marks `value` as a solid collider on any layer.
    pub fn solid(value: i32) -> Self {
        Self {
            value,
            solid: true,
            ..Default::default()
        }
    }

    /// Creates a rule that marks `value` as a sensor collider with the given `tag` on any layer.
    pub fn sensor(value: i32, tag: impl Into<String>) -> Self {
        Self {
            value,
            sensor: true,
            tag: Some(tag.into()),
            ..Default::default()
        }
    }

    /// Scopes this rule to a single layer and returns `self` for chaining.
    pub fn for_layer(mut self, layer_identifier: impl Into<String>) -> Self {
        self.layer_identifier = Some(layer_identifier.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_layer_filters_are_combined() {
        let config = LdtkConfig::default()
            .include_layers(["Ground", "Entities"])
            .exclude_layers(["Debug"]);

        assert!(config.should_include_layer("Ground"));
        assert!(!config.should_include_layer("Background"));
        assert!(!config.should_include_layer("Debug"));
    }
}
