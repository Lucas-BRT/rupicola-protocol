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

/// Visual highlight box attached to a screenshot or a video time segment.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AnnotationBox {
    pub bounds: Rect,
    pub label: String,
    #[serde(default)]
    pub color: Option<String>,
    /// When `None`, the box applies to a still image or the entire video.
    #[serde(default)]
    pub time_range: Option<TimeRange>,
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

    /// Normalizes bounds into `[left%, top%, width%, height%]` for CSS overlays.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "viewport dimensions fit comfortably in f32 mantissa"
    )]
    pub fn to_percent_box(&self, viewport: [u32; 2]) -> Option<[f32; 4]> {
        if viewport[0] == 0 || viewport[1] == 0 || !self.bounds.is_valid() {
            return None;
        }
        let vw = viewport[0] as f32;
        let vh = viewport[1] as f32;
        Some([
            (self.bounds.x / vw) * 100.0,
            (self.bounds.y / vh) * 100.0,
            (self.bounds.width / vw) * 100.0,
            (self.bounds.height / vh) * 100.0,
        ])
    }
}
