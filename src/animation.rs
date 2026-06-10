//! Tile-animation metadata, the [`LdtkTileAnimator`] component, and the parser
//! for the `anim=`/`frames=` tile custom-data convention.

use bevy::prelude::*;
use std::time::Duration;

/// Composite key that uniquely identifies a tile within a tileset, used to index
/// [`LdtkMapCatalog::tile_animations`](crate::LdtkMapCatalog::tile_animations).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct LdtkTileKey {
    /// UID of the owning tileset; `None` for embedded/internal tilesets.
    pub tileset_uid: Option<i32>,
    /// Zero-based index of the tile within the tileset.
    pub tile_id: i32,
}

/// Frame sequence describing how a tile cycles through alternative tile IDs over
/// time. Attached to tile entities as a Bevy [`Component`].
#[derive(Debug, Clone, Component, Default)]
pub struct LdtkTileAnimation {
    /// Ordered list of frames; each frame specifies a tile ID and its display duration.
    pub frames: Vec<LdtkTileAnimationFrame>,
    /// When `true`, the animation loops back to frame 0 after the last frame; otherwise it holds.
    pub repeat: bool,
}

/// A single frame in an [`LdtkTileAnimation`], pairing a tile graphic with its
/// display duration.
#[derive(Debug, Clone, Default)]
pub struct LdtkTileAnimationFrame {
    /// Index of the tile in the tileset to display for this frame.
    pub tile_id: i32,
    /// How long this frame is shown, in seconds.
    pub duration: f32,
}

/// Bevy [`Component`] that drives frame advancement for an animated tile entity,
/// holding the animation data, current frame index, and an internal [`Timer`].
#[derive(Debug, Clone, Component)]
pub struct LdtkTileAnimator {
    /// The animation definition being played.
    pub animation: LdtkTileAnimation,
    /// Zero-based index of the frame that is currently displayed.
    pub frame_index: usize,
    /// Countdown timer set to the current frame's duration.
    pub timer: Timer,
}

impl LdtkTileAnimator {
    /// Creates a new animator for `animation`, initialising the timer to the
    /// duration of the first frame (clamped to a minimum of 0.001 s).
    pub fn new(animation: LdtkTileAnimation) -> Self {
        let duration = animation
            .frames
            .first()
            .map(|frame| frame.duration)
            .unwrap_or(0.1)
            .max(0.001);

        Self {
            animation,
            frame_index: 0,
            timer: Timer::from_seconds(duration, TimerMode::Repeating),
        }
    }

    /// Returns `true` once a non-repeating animation has reached its last frame.
    pub fn is_finished(&self) -> bool {
        !self.animation.repeat && self.frame_index + 1 >= self.animation.frames.len()
    }

    /// Advances the animation by `delta`. Returns the tile id of the new frame
    /// when the frame changed this tick, otherwise `None`. This is the single
    /// source of truth for frame stepping shared by every animator system.
    ///
    /// A finished non-repeating animation holds its last frame and returns
    /// `None` without ticking the timer.
    pub fn advance(&mut self, delta: Duration) -> Option<i32> {
        if self.animation.frames.is_empty() || self.is_finished() {
            return None;
        }

        self.timer.tick(delta);
        if !self.timer.just_finished() {
            return None;
        }

        self.frame_index += 1;
        if self.frame_index >= self.animation.frames.len() {
            // `is_finished` above guarantees `repeat` here.
            self.frame_index = 0;
        }

        let frame = &self.animation.frames[self.frame_index];
        self.timer = Timer::from_seconds(frame.duration.max(0.001), TimerMode::Repeating);
        Some(frame.tile_id)
    }
}

/// Advances every [`LdtkTileAnimator`] in the world by the frame delta.
pub(crate) fn tick_ldtk_tile_animators(
    time: Res<'_, Time>,
    mut query: Query<'_, '_, &mut LdtkTileAnimator>,
) {
    let delta = time.delta();
    for mut animator in query.iter_mut() {
        // Only mark the component changed on actual frame steps so systems
        // reacting to `Changed<LdtkTileAnimator>` don't fire every tick.
        let changed = animator.bypass_change_detection().advance(delta).is_some();
        if changed {
            animator.set_changed();
        }
    }
}

/// Parses the tile custom-data animation convention into an
/// [`LdtkTileAnimation`]. Returns `None` when `data` does not describe an
/// animation.
///
/// Supported syntax (parts separated by `;`):
/// - `anim=<ids>` or `frames=<ids>`: comma-separated tile IDs
/// - `<id>@<seconds>`: per-frame duration override
/// - `fps=<n>` or `duration=<seconds>`: uniform frame duration
/// - `repeat=false|0|no`: hold the last frame instead of looping
pub(crate) fn parse_tile_animation(data: &str) -> Option<LdtkTileAnimation> {
    let normalized = data.trim();
    if normalized.is_empty() {
        return None;
    }

    let lower = normalized.to_lowercase();
    if !lower.contains("anim") && !lower.contains("frame") {
        return None;
    }

    let mut duration = 0.1;
    let mut repeat = true;
    let mut frame_text = normalized;

    for part in normalized.split(';') {
        let trimmed = part.trim();
        if let Some(value) = trimmed.strip_prefix("fps=") {
            if let Ok(fps) = value.trim().parse::<f32>() {
                duration = 1.0 / fps.max(0.001);
            }
        } else if let Some(value) = trimmed.strip_prefix("duration=") {
            if let Ok(seconds) = value.trim().parse::<f32>() {
                duration = seconds.max(0.001);
            }
        } else if let Some(value) = trimmed.strip_prefix("repeat=") {
            repeat = !matches!(value.trim(), "false" | "0" | "no");
        } else if trimmed.contains("anim") || trimmed.contains("frame") {
            frame_text = trimmed;
        }
    }

    let frame_text = frame_text
        .split_once('=')
        .map(|(_, value)| value)
        .or_else(|| frame_text.split_once(':').map(|(_, value)| value))
        .unwrap_or(frame_text);

    let frames = frame_text
        .split(',')
        .filter_map(|entry| {
            let entry = entry.trim();
            if entry.is_empty() {
                return None;
            }

            let (tile_id, frame_duration) = entry
                .split_once('@')
                .map(|(tile, seconds)| {
                    (
                        tile.trim().parse::<i32>().ok(),
                        seconds.trim().parse::<f32>().ok(),
                    )
                })
                .unwrap_or_else(|| (entry.parse::<i32>().ok(), None));

            tile_id.map(|tile_id| LdtkTileAnimationFrame {
                tile_id,
                duration: frame_duration.unwrap_or(duration).max(0.001),
            })
        })
        .collect::<Vec<_>>();

    (!frames.is_empty()).then_some(LdtkTileAnimation { frames, repeat })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_frame_animation(repeat: bool) -> LdtkTileAnimation {
        LdtkTileAnimation {
            frames: vec![
                LdtkTileAnimationFrame {
                    tile_id: 1,
                    duration: 0.05,
                },
                LdtkTileAnimationFrame {
                    tile_id: 2,
                    duration: 0.05,
                },
            ],
            repeat,
        }
    }

    #[test]
    fn parses_animation_custom_data_with_fps() {
        let animation = parse_tile_animation("anim=1,2,3;fps=12").expect("animation");

        assert_eq!(animation.frames.len(), 3);
        assert_eq!(animation.frames[0].tile_id, 1);
        assert!((animation.frames[0].duration - (1.0 / 12.0)).abs() < f32::EPSILON);
        assert!(animation.repeat);
    }

    #[test]
    fn parses_animation_custom_data_with_frame_durations() {
        let animation =
            parse_tile_animation("frames=4@0.1,5@0.25;repeat=false").expect("animation");

        assert_eq!(animation.frames.len(), 2);
        assert_eq!(animation.frames[1].tile_id, 5);
        assert_eq!(animation.frames[1].duration, 0.25);
        assert!(!animation.repeat);
    }

    #[test]
    fn ignores_custom_data_without_animation_marker() {
        assert!(parse_tile_animation("solid=true").is_none());
    }

    #[test]
    fn repeating_animation_wraps_around() {
        let mut animator = LdtkTileAnimator::new(two_frame_animation(true));

        assert_eq!(animator.advance(Duration::from_millis(60)), Some(2));
        assert_eq!(animator.advance(Duration::from_millis(60)), Some(1));
        assert!(!animator.is_finished());
    }

    #[test]
    fn non_repeating_animation_stops_on_last_frame() {
        let mut animator = LdtkTileAnimator::new(two_frame_animation(false));

        assert_eq!(animator.advance(Duration::from_millis(60)), Some(2));
        assert!(animator.is_finished());

        // Once finished, the animator holds the last frame and reports no
        // further changes, no matter how much time passes.
        assert_eq!(animator.advance(Duration::from_millis(60)), None);
        assert_eq!(animator.advance(Duration::from_secs(10)), None);
        assert_eq!(animator.frame_index, 1);
    }
}
