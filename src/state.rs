//! Load, runtime, and validation state resources.

use bevy::prelude::*;
use std::collections::HashSet;

/// Bevy resource that tracks the current lifecycle state of the LDtk world.
#[derive(Debug, Clone, Resource, Default)]
pub struct LdtkLoadState {
    /// Current phase of the load pipeline.
    pub status: LdtkLoadStatus,
    /// Identifier of the world that is loaded or being loaded, if known.
    pub world_identifier: Option<String>,
    /// Non-fatal warnings accumulated during the last load.
    pub warnings: Vec<String>,
    /// Fatal errors accumulated during the last load.
    pub errors: Vec<String>,
    /// Counters describing what was loaded in the last successful pass.
    pub stats: LdtkLoadStats,
}

impl LdtkLoadState {
    /// Returns `true` when the world has finished loading without errors.
    pub fn is_ready(&self) -> bool {
        self.status == LdtkLoadStatus::Ready
    }
}

/// Phase of the LDtk world load pipeline.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LdtkLoadStatus {
    /// No world has been requested yet.
    #[default]
    NotLoaded,
    /// A world load is in progress.
    Loading,
    /// The world loaded successfully and is ready to use.
    Ready,
    /// Loading failed; see [`LdtkLoadState::errors`] for details.
    Error,
}

/// Counters populated after a successful world load, useful for profiling and
/// validation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LdtkLoadStats {
    /// Number of LDtk worlds cataloged.
    pub worlds: usize,
    /// Number of levels cataloged across all worlds.
    pub levels: usize,
    /// Number of layers cataloged across all levels.
    pub layers: usize,
    /// Number of tilesets cataloged.
    pub tilesets: usize,
    /// Total tile instances across all levels and layers.
    pub tiles: usize,
    /// Total entity instances across all levels.
    pub entities: usize,
    /// Number of spawn points extracted from entity layers.
    pub spawn_points: usize,
    /// Number of IntGrid cells with collision data.
    pub collision_cells: usize,
    /// Number of animated tiles found in tilesets.
    pub tile_animations: usize,
}

/// Bevy resource that accumulates validation issues found after loading; cleared
/// and repopulated on each reload.
#[derive(Debug, Clone, Resource, Default)]
pub struct LdtkValidationReport {
    /// Non-fatal issues that do not abort loading.
    pub warnings: Vec<LdtkValidationIssue>,
    /// Fatal issues that abort loading when
    /// [`LdtkConfig::strict_validation`](crate::LdtkConfig::strict_validation) is set.
    pub errors: Vec<LdtkValidationIssue>,
}

impl LdtkValidationReport {
    /// Removes all warnings and errors from the report.
    pub fn clear(&mut self) {
        self.warnings.clear();
        self.errors.clear();
    }

    /// Returns `true` if any errors have been recorded.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Records an issue, routing it to [`Self::errors`] when `strict` is set and
    /// to [`Self::warnings`] otherwise. This is the single place that encodes the
    /// "strict promotes warnings to errors" policy, so callers no longer repeat
    /// the branch at every check.
    pub fn push(&mut self, strict: bool, code: impl Into<String>, message: impl Into<String>) {
        let issue = LdtkValidationIssue::new(code, message);
        if strict {
            self.errors.push(issue);
        } else {
            self.warnings.push(issue);
        }
    }
}

/// A single validation issue with a short machine-readable `code` and a
/// human-readable `message`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LdtkValidationIssue {
    /// Short identifier for the issue class (e.g. `"missing_tileset"`).
    pub code: String,
    /// Human-readable description of the specific problem.
    pub message: String,
}

impl LdtkValidationIssue {
    /// Severity is not stored on the issue itself; it is conveyed by which list
    /// of [`LdtkValidationReport`] the issue ends up in. Use
    /// [`LdtkValidationReport::push`] rather than constructing-and-placing by hand.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Bevy resource that tracks what is currently active at runtime — which world
/// file is open, which level is active, and which levels are loaded.
#[derive(Debug, Clone, Resource, Default)]
pub struct LdtkRuntimeState {
    /// Asset-relative path of the `.ldtk` file that is currently open.
    pub active_world_path: Option<String>,
    /// LDtk identifier of the active world.
    pub active_world_identifier: Option<String>,
    /// Bevy [`Entity`] that is the root of the spawned world hierarchy.
    pub active_world_root: Option<Entity>,
    /// LDtk identifier of the level currently focused by the camera / game logic.
    pub active_level: Option<String>,
    /// Current level-transition phase.
    pub transition: LdtkTransitionState,
    /// Identifiers of all levels that are currently spawned in the world.
    pub loaded_levels: HashSet<String>,
}

/// Phase of a level transition driven by the
/// [`LdtkLoadSet::LevelTransitions`](crate::LdtkLoadSet::LevelTransitions) systems.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum LdtkTransitionState {
    /// No transition is in progress.
    #[default]
    Idle,
    /// A new level has been requested and is being loaded.
    Loading,
    /// The requested level has been loaded and activated.
    Active,
}
