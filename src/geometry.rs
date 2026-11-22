use serde::{Deserialize, Serialize};

/// Logical rectangle in window or viewport coordinates.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Returns `true` when dimensions are finite and strictly positive.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }

    /// Converts logical coordinates into rounded physical pixel `[x, y, w, h]`.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped non-negative pixel bounds"
    )]
    pub fn to_pixel_box(&self, scale_factor: f32) -> [u32; 4] {
        let scale = if scale_factor.is_finite() && scale_factor > 0.0 {
            scale_factor
        } else {
            1.0
        };
        let px_x = (self.x * scale).max(0.0).round() as u32;
        let px_y = (self.y * scale).max(0.0).round() as u32;
        let px_w = (self.width * scale).max(1.0).round() as u32;
        let px_h = (self.height * scale).max(1.0).round() as u32;
        [px_x, px_y, px_w, px_h]
    }
}
