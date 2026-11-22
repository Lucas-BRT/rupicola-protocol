use crate::geometry::Rect;
use serde::{Deserialize, Serialize};

/// Inclusive millisecond interval during which a video annotation is visible.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct TimeRange {
    pub start_ms: u64,
    pub end_ms: u64,
}

impl TimeRange {
    /// Returns `true` when `start_ms <= end_ms`.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.start_ms <= self.end_ms
    }

    /// Returns `true` when `timestamp_ms` falls within `[start_ms, end_ms]`.
    #[must_use]
    pub const fn contains(self, timestamp_ms: u64) -> bool {
        timestamp_ms >= self.start_ms && timestamp_ms <= self.end_ms
    }
}

/// Visual shape used to render an annotation overlay.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(tag = "shape", rename_all = "kebab-case")]
pub enum AnnotationShape {
    #[default]
    Rect,
    Ellipse,
    Spotlight,
    Arrow {
        from: [f32; 2],
        to: [f32; 2],
    },
}

/// Placement of the annotation label badge relative to the highlighted region.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LabelPosition {
    #[default]
    Top,
    Bottom,
    Inside,
    Hidden,
}

/// Customizable styling for an annotation box (respected by both DOM overlays and exports).
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AnnotationStyle {
    #[serde(default = "default_stroke_color")]
    pub stroke_color: String,
    #[serde(default = "default_stroke_width")]
    pub stroke_width: f32,
    #[serde(default)]
    pub fill_color: Option<String>,
    #[serde(default = "default_border_radius")]
    pub border_radius: f32,
    #[serde(default)]
    pub dashed: bool,
    #[serde(default)]
    pub label_position: LabelPosition,
}

fn default_stroke_color() -> String {
    "#c24518".to_owned()
}

const fn default_stroke_width() -> f32 {
    2.0
}

const fn default_border_radius() -> f32 {
    6.0
}

impl Default for AnnotationStyle {
    fn default() -> Self {
        Self {
            stroke_color: default_stroke_color(),
            stroke_width: default_stroke_width(),
            fill_color: None,
            border_radius: default_border_radius(),
            dashed: false,
            label_position: LabelPosition::Top,
        }
    }
}

/// Interpolation curve between consecutive [`AnnotationKeyframe`]s.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Easing {
    #[default]
    Linear,
    EaseInOut,
    Step,
}

impl Easing {
    #[must_use]
    pub fn apply(self, t: f32) -> f32 {
        let clamped = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => clamped,
            Self::EaseInOut => clamped * clamped * (3.0 - 2.0 * clamped),
            Self::Step => {
                if clamped >= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

/// Position/size keyframe for tracking moving UI elements across video playback.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct AnnotationKeyframe {
    pub timestamp_ms: u64,
    pub bounds: Rect,
    #[serde(default)]
    pub easing: Easing,
}

/// Visual highlight attached to a screenshot or moving video element.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AnnotationBox {
    pub bounds: Rect,
    pub label: String,
    #[serde(default)]
    pub shape: AnnotationShape,
    #[serde(default)]
    pub style: AnnotationStyle,
    /// When `None`, the box applies to a still image or the entire video.
    #[serde(default)]
    pub time_range: Option<TimeRange>,
    /// Optional sorted keyframes that override `bounds` over time during video playback.
    #[serde(default)]
    pub keyframes: Vec<AnnotationKeyframe>,
}

impl AnnotationBox {
    /// Returns `true` if this annotation should be displayed at `timestamp_ms`.
    #[must_use]
    pub const fn active_at(&self, timestamp_ms: u64) -> bool {
        match self.time_range {
            Some(range) => range.contains(timestamp_ms),
            None => true,
        }
    }

    /// Computes interpolated bounds at `timestamp_ms` using `keyframes` when present.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "video timestamps fit comfortably in f32 mantissa for interpolation"
    )]
    pub fn bounds_at(&self, timestamp_ms: u64) -> Rect {
        if self.keyframes.is_empty() {
            return self.bounds;
        }
        let first = self.keyframes[0];
        if timestamp_ms <= first.timestamp_ms {
            return first.bounds;
        }
        let last = self.keyframes[self.keyframes.len() - 1];
        if timestamp_ms >= last.timestamp_ms {
            return last.bounds;
        }
        for pair in self.keyframes.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if timestamp_ms >= a.timestamp_ms && timestamp_ms <= b.timestamp_ms {
                let span = b.timestamp_ms.saturating_sub(a.timestamp_ms);
                if span == 0 {
                    return b.bounds;
                }
                let raw_t = (timestamp_ms - a.timestamp_ms) as f32 / span as f32;
                let t = a.easing.apply(raw_t);
                return Rect {
                    x: a.bounds.x + (b.bounds.x - a.bounds.x) * t,
                    y: a.bounds.y + (b.bounds.y - a.bounds.y) * t,
                    width: a.bounds.width + (b.bounds.width - a.bounds.width) * t,
                    height: a.bounds.height + (b.bounds.height - a.bounds.height) * t,
                };
            }
        }
        self.bounds
    }

    /// Normalizes bounds at `timestamp_ms` into `[left%, top%, width%, height%]` for CSS overlays.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "viewport dimensions fit comfortably in f32 mantissa"
    )]
    pub fn percent_box_at(&self, timestamp_ms: u64, viewport: [u32; 2]) -> Option<[f32; 4]> {
        let bounds = self.bounds_at(timestamp_ms);
        if viewport[0] == 0 || viewport[1] == 0 || !bounds.is_valid() {
            return None;
        }
        let vw = viewport[0] as f32;
        let vh = viewport[1] as f32;
        Some([
            (bounds.x / vw) * 100.0,
            (bounds.y / vh) * 100.0,
            (bounds.width / vw) * 100.0,
            (bounds.height / vh) * 100.0,
        ])
    }

    /// Convenience wrapper for static screenshots (`timestamp_ms = 0`).
    #[must_use]
    pub fn to_percent_box(&self, viewport: [u32; 2]) -> Option<[f32; 4]> {
        self.percent_box_at(0, viewport)
    }
}
