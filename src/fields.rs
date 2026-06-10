//! Typed LDtk field values and the shared [`LdtkFieldAccess`] accessor trait.

use bevy::prelude::*;
use std::collections::HashMap;

/// Fully qualified cross-reference to another LDtk entity instance.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LdtkEntityReference {
    /// IID of the referenced entity instance.
    pub entity_iid: String,
    /// IID of the layer that contains the referenced entity.
    pub layer_iid: String,
    /// IID of the level that contains the referenced entity.
    pub level_iid: String,
    /// IID of the world that contains the referenced entity.
    pub world_iid: String,
}

/// A rectangular region inside a tileset, used by tile-typed LDtk field values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LdtkTilesetRect {
    /// UID of the tileset this rectangle refers to.
    pub tileset_uid: i32,
    /// Top-left corner of the rectangle in pixels within the tileset image.
    pub position: IVec2,
    /// Width and height of the rectangle in pixels.
    pub size: IVec2,
}

/// Typed representation of any LDtk field value, covering all primitive and
/// composite types that LDtk supports.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum LdtkFieldValue {
    /// A boolean field value.
    Bool(bool),
    /// An integer field value stored as `i64` to accommodate all LDtk int ranges.
    Int(i64),
    /// A floating-point field value stored as `f64`.
    Float(f64),
    /// A string, file-path, or enum field value.
    String(String),
    /// A color field value represented as a Bevy [`Color`].
    Color(Color),
    /// A point field value; `None` when the field is set to null in the editor.
    Point(Option<IVec2>),
    /// A tile-reference field value; `None` when unset.
    Tile(Option<LdtkTilesetRect>),
    /// A cross-reference to another entity instance.
    EntityRef(LdtkEntityReference),
    /// An array field containing zero or more [`LdtkFieldValue`] elements.
    Array(Vec<LdtkFieldValue>),
    /// A null / unset field value.
    #[default]
    Null,
}

impl LdtkFieldValue {
    /// Returns the inner `bool` if this is a [`Self::Bool`] variant, otherwise `None`.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the inner `i64` if this is a [`Self::Int`] variant, otherwise `None`.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns the inner value as `f64`; accepts both [`Self::Float`] and [`Self::Int`].
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(*value),
            Self::Int(value) => Some(*value as f64),
            _ => None,
        }
    }

    /// Returns a `&str` borrow if this is a [`Self::String`] variant, otherwise `None`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// Returns `Some(inner)` if this is a [`Self::Point`] variant (inner may itself be `None`),
    /// otherwise returns `None`.
    pub fn as_point(&self) -> Option<Option<IVec2>> {
        match self {
            Self::Point(value) => Some(*value),
            _ => None,
        }
    }

    /// Returns `Some(inner)` if this is a [`Self::Tile`] variant (inner may itself be `None`),
    /// otherwise returns `None`.
    pub fn as_tile(&self) -> Option<Option<&LdtkTilesetRect>> {
        match self {
            Self::Tile(value) => Some(value.as_ref()),
            _ => None,
        }
    }
}

/// Shared typed accessors for anything that carries LDtk field instances.
/// Implemented by both the live snapshot
/// ([`LdtkImportedEntity`](crate::LdtkImportedEntity)) and the spawn-time
/// context ([`LdtkEntitySpawnContext`](crate::LdtkEntitySpawnContext)) so the
/// lookup logic lives in exactly one place.
pub trait LdtkFieldAccess {
    /// Returns a reference to the underlying field-value map.
    fn field_values(&self) -> &HashMap<String, LdtkFieldValue>;

    /// Looks up a field by `identifier`, returning `None` when absent.
    fn field(&self, identifier: &str) -> Option<&LdtkFieldValue> {
        self.field_values().get(identifier)
    }

    /// Returns the boolean value of `identifier`, or `None` if absent or of a different type.
    fn field_bool(&self, identifier: &str) -> Option<bool> {
        self.field(identifier).and_then(LdtkFieldValue::as_bool)
    }

    /// Returns the integer value of `identifier` as `i64`, or `None` if absent or of a different type.
    fn field_i64(&self, identifier: &str) -> Option<i64> {
        self.field(identifier).and_then(LdtkFieldValue::as_i64)
    }

    /// Returns the numeric value of `identifier` as `f64` (accepts int and float fields),
    /// or `None` if absent or incompatible.
    fn field_f64(&self, identifier: &str) -> Option<f64> {
        self.field(identifier).and_then(LdtkFieldValue::as_f64)
    }

    /// Returns a `&str` borrow of `identifier`'s value, or `None` if absent or non-string.
    fn field_str(&self, identifier: &str) -> Option<&str> {
        self.field(identifier).and_then(LdtkFieldValue::as_str)
    }
}

impl From<&bevy_ecs_ldtk::ldtk::ReferenceToAnEntityInstance> for LdtkEntityReference {
    fn from(value: &bevy_ecs_ldtk::ldtk::ReferenceToAnEntityInstance) -> Self {
        Self {
            entity_iid: value.entity_iid.clone(),
            layer_iid: value.layer_iid.clone(),
            level_iid: value.level_iid.clone(),
            world_iid: value.world_iid.clone(),
        }
    }
}

impl From<&bevy_ecs_ldtk::ldtk::TilesetRectangle> for LdtkTilesetRect {
    fn from(value: &bevy_ecs_ldtk::ldtk::TilesetRectangle) -> Self {
        Self {
            tileset_uid: value.tileset_uid,
            position: IVec2::new(value.x, value.y),
            size: IVec2::new(value.w, value.h),
        }
    }
}

impl From<&bevy_ecs_ldtk::ldtk::FieldInstance> for LdtkFieldValue {
    fn from(value: &bevy_ecs_ldtk::ldtk::FieldInstance) -> Self {
        use bevy_ecs_ldtk::ldtk::FieldValue;

        match &value.value {
            FieldValue::Int(v) => Self::Int(i64::from(v.unwrap_or_default())),
            FieldValue::Float(v) => Self::Float(f64::from(v.unwrap_or_default())),
            FieldValue::Bool(v) => Self::Bool(*v),
            FieldValue::String(v) => Self::String(v.clone().unwrap_or_default()),
            FieldValue::Color(v) => Self::Color(*v),
            FieldValue::FilePath(v) => Self::String(v.clone().unwrap_or_default()),
            FieldValue::Enum(v) => Self::String(v.clone().unwrap_or_default()),
            FieldValue::Tile(v) => Self::Tile(v.as_ref().map(LdtkTilesetRect::from)),
            FieldValue::EntityRef(v) => Self::EntityRef(
                v.as_ref()
                    .map(LdtkEntityReference::from)
                    .unwrap_or_default(),
            ),
            FieldValue::Point(v) => Self::Point(*v),
            FieldValue::Ints(v) => Self::Array(
                v.iter()
                    .map(|entry| entry.map(|i| Self::Int(i64::from(i))).unwrap_or(Self::Null))
                    .collect(),
            ),
            FieldValue::Floats(v) => Self::Array(
                v.iter()
                    .map(|entry| {
                        entry
                            .map(|f| Self::Float(f64::from(f)))
                            .unwrap_or(Self::Null)
                    })
                    .collect(),
            ),
            FieldValue::Bools(v) => Self::Array(v.iter().map(|entry| Self::Bool(*entry)).collect()),
            FieldValue::Strings(v) | FieldValue::FilePaths(v) | FieldValue::Enums(v) => {
                Self::Array(
                    v.iter()
                        .map(|entry| {
                            entry
                                .as_ref()
                                .map(|text| Self::String(text.clone()))
                                .unwrap_or(Self::Null)
                        })
                        .collect(),
                )
            }
            FieldValue::Colors(v) => {
                Self::Array(v.iter().map(|entry| Self::Color(*entry)).collect())
            }
            FieldValue::Tiles(v) => Self::Array(
                v.iter()
                    .map(|entry| Self::Tile(entry.as_ref().map(LdtkTilesetRect::from)))
                    .collect(),
            ),
            FieldValue::EntityRefs(v) => Self::Array(
                v.iter()
                    .map(|entry| {
                        entry
                            .as_ref()
                            .map(|reference| Self::EntityRef(LdtkEntityReference::from(reference)))
                            .unwrap_or(Self::Null)
                    })
                    .collect(),
            ),
            FieldValue::Points(v) => {
                Self::Array(v.iter().map(|entry| Self::Point(*entry)).collect())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_helpers_return_typed_values() {
        assert_eq!(LdtkFieldValue::Bool(true).as_bool(), Some(true));
        assert_eq!(LdtkFieldValue::Int(42).as_i64(), Some(42));
        assert_eq!(LdtkFieldValue::Int(42).as_f64(), Some(42.0));
        assert_eq!(LdtkFieldValue::Float(1.5).as_f64(), Some(1.5));
        assert_eq!(
            LdtkFieldValue::String("door_a".to_string()).as_str(),
            Some("door_a")
        );
        assert_eq!(LdtkFieldValue::Null.as_bool(), None);
    }
}
