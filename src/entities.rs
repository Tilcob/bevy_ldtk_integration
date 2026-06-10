//! Snapshot types for imported LDtk entity instances.

use bevy::prelude::*;
use std::collections::HashMap;

use crate::catalog::LdtkTileMetadata;
use crate::fields::{LdtkFieldAccess, LdtkFieldValue};

/// All data needed to spawn a single LDtk entity instance, passed to registered
/// spawner callbacks.
#[derive(Debug, Clone, Default)]
pub struct LdtkEntitySpawnContext {
    /// Instance-unique IID for this entity.
    pub entity_iid: String,
    /// LDtk identifier of the entity definition (e.g. `"Enemy"`).
    pub entity_identifier: String,
    /// Identifier of the world this entity belongs to, if known.
    pub world_identifier: Option<String>,
    /// Identifier of the level this entity lives in, if known.
    pub level_identifier: Option<String>,
    /// Identifier of the layer this entity lives on, if known.
    pub layer_identifier: Option<String>,
    /// World-space position of the entity's pivot point in pixels.
    pub position: Vec2,
    /// Column/row position of the entity in the layer grid (in grid units).
    pub grid_position: IVec2,
    /// Pixel dimensions of this entity instance.
    pub size: Vec2,
    /// Normalised pivot point, where `(0,0)` is top-left and `(1,1)` is bottom-right.
    pub pivot: Vec2,
    /// Tags defined on this entity instance in the LDtk editor.
    pub tags: Vec<String>,
    /// Optional visual tile assigned to this entity in the LDtk editor.
    pub tile: Option<LdtkTileMetadata>,
    /// Custom field values defined on this entity instance.
    pub field_values: HashMap<String, LdtkFieldValue>,
}

impl LdtkFieldAccess for LdtkEntitySpawnContext {
    fn field_values(&self) -> &HashMap<String, LdtkFieldValue> {
        &self.field_values
    }
}

/// Snapshot of an LDtk entity instance stored in
/// [`LdtkEntityCatalog`](crate::LdtkEntityCatalog) and also attached as a Bevy
/// [`Component`] to the spawned entity.
#[derive(Debug, Clone, Component, Default)]
pub struct LdtkImportedEntity {
    /// Instance-unique IID for this entity.
    pub entity_iid: String,
    /// LDtk identifier of the entity definition (e.g. `"Chest"`).
    pub entity_identifier: String,
    /// Identifier of the world this entity belongs to, if known.
    pub world_identifier: Option<String>,
    /// Identifier of the level this entity lives in, if known.
    pub level_identifier: Option<String>,
    /// Identifier of the layer this entity lives on, if known.
    pub layer_identifier: Option<String>,
    /// World-space position of the entity's pivot point in pixels.
    pub position: Vec2,
    /// Column/row position of the entity in the layer grid (in grid units).
    pub grid_position: IVec2,
    /// Pixel dimensions of this entity instance.
    pub size: Vec2,
    /// Normalised pivot point, where `(0,0)` is top-left and `(1,1)` is bottom-right.
    pub pivot: Vec2,
    /// Tags defined on this entity instance in the LDtk editor.
    pub tags: Vec<String>,
    /// Optional visual tile assigned to this entity in the LDtk editor.
    pub tile: Option<LdtkTileMetadata>,
    /// Custom field values defined on this entity instance.
    pub field_values: HashMap<String, LdtkFieldValue>,
}

impl LdtkFieldAccess for LdtkImportedEntity {
    fn field_values(&self) -> &HashMap<String, LdtkFieldValue> {
        &self.field_values
    }
}

/// Bevy [`Component`] that marks an entity as originating from an LDtk entity
/// instance, carrying its definition name and location identifiers.
#[derive(Debug, Clone, Component, Default)]
pub struct LdtkEntityMarker {
    /// LDtk identifier of the entity definition (e.g. `"Boss"`).
    pub definition_identifier: String,
    /// Identifier of the level the entity was spawned from, if known.
    pub level_identifier: Option<String>,
    /// Identifier of the world the entity was spawned from, if known.
    pub world_identifier: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_field_helpers_read_from_snapshot() {
        let mut entity = LdtkImportedEntity::default();
        entity
            .field_values
            .insert("damage".to_string(), LdtkFieldValue::Int(7));
        entity
            .field_values
            .insert("locked".to_string(), LdtkFieldValue::Bool(false));

        assert_eq!(entity.field_i64("damage"), Some(7));
        assert_eq!(entity.field_bool("locked"), Some(false));
        assert_eq!(entity.field_str("missing"), None);
    }
}
