//! Pluggable IO for external `.ldtkl` level files.
//!
//! Injecting an [`ExternalLevelSource`] keeps blocking filesystem IO out of the
//! plugin core: the desktop default reads from disk, WASM gets nothing, and a
//! consumer can supply their own (e.g. an async-prefetched cache) by replacing
//! the [`LdtkExternalLevelSource`] resource.

use bevy::prelude::*;

/// Strategy for fetching the JSON of an external `.ldtkl` level file during
/// catalog construction.
pub trait ExternalLevelSource: Send + Sync + 'static {
    /// Returns the raw JSON text of the external level, or `None` if it cannot
    /// be provided. `world_path` is the asset-relative path of the `.ldtk` file,
    /// `rel_path` the level's `external_rel_path` relative to that file.
    fn load(&self, asset_root: &str, world_path: &str, rel_path: &str) -> Option<String>;
}

/// Resource holding the active external-level loader. `None` disables external
/// level cataloging (the default on targets without filesystem access).
#[derive(Resource, Default)]
pub struct LdtkExternalLevelSource(pub Option<Box<dyn ExternalLevelSource>>);

impl LdtkExternalLevelSource {
    /// Returns a reference to the inner [`ExternalLevelSource`], if one is set.
    pub fn source(&self) -> Option<&dyn ExternalLevelSource> {
        self.0.as_deref()
    }
}

/// Joins the asset root, the world file's directory and the level's relative
/// path into the on-disk location of an external `.ldtkl` file.
#[cfg(feature = "external-level-fs")]
pub fn external_level_path(
    asset_root: &str,
    active_world_path: &str,
    external_path: &str,
) -> std::path::PathBuf {
    let world_dir = std::path::Path::new(active_world_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new(""));
    std::path::Path::new(asset_root)
        .join(world_dir)
        .join(external_path)
}

/// Default [`ExternalLevelSource`] that reads external levels synchronously from
/// the filesystem. Only available with the `external-level-fs` feature.
#[cfg(feature = "external-level-fs")]
pub struct FsExternalLevelSource;

#[cfg(feature = "external-level-fs")]
impl ExternalLevelSource for FsExternalLevelSource {
    fn load(&self, asset_root: &str, world_path: &str, rel_path: &str) -> Option<String> {
        let full_path = external_level_path(asset_root, world_path, rel_path);
        std::fs::read_to_string(full_path).ok()
    }
}

#[cfg(all(test, feature = "external-level-fs"))]
mod tests {
    use super::*;

    #[test]
    fn builds_external_level_path_relative_to_world_file() {
        let path = external_level_path(
            "assets",
            "worlds/SeparateLevelFiles.ldtk",
            "SeparateLevelFiles/World_Level_0.ldtkl",
        );

        assert_eq!(
            path,
            std::path::PathBuf::from("assets")
                .join("worlds")
                .join("SeparateLevelFiles")
                .join("World_Level_0.ldtkl")
        );
    }
}
