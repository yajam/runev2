//! Interpolation system for animatable values.
//!
//! This module provides the `Interpolate` trait and implementations for all
//! animatable value types. Interpolation is the core mechanism that creates
//! smooth transitions between values.
//!
//! # Color Space Handling
//!
//! Color interpolation is done in linear RGB space for perceptually smooth
//! gradients. The colors stored in `AnimatableValue::Color` are expected to
//! be in linear premultiplied format.

use super::types::{AnimatableBoxShadow, AnimatableEdgeInsets, AnimatableTransform, AnimatableValue};

/// Trait for types that can be interpolated between two values.
///
/// # Arguments
/// * `to` - Target value to interpolate towards
/// * `t` - Interpolation factor (0.0 = self, 1.0 = to)
///
/// # Returns
/// Interpolated value between self and to at factor t.
pub trait Interpolate: Sized {
    /// Interpolate between self and another value.
    ///
    /// When t = 0.0, returns self.
    /// When t = 1.0, returns to.
    /// Values between 0.0 and 1.0 return intermediate values.
    fn interpolate(&self, to: &Self, t: f32) -> Self;
}

/// Linear interpolation helper for f64 values.
#[inline]
fn lerp_f64(from: f64, to: f64, t: f32) -> f64 {
    from + (to - from) * t as f64
}

/// Linear interpolation helper for f32 values.
#[inline]
fn lerp_f32(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

impl Interpolate for f64 {
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        lerp_f64(*self, *to, t)
    }
}

impl Interpolate for f32 {
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        lerp_f32(*self, *to, t)
    }
}

impl Interpolate for [f32; 4] {
    /// Interpolate RGBA color values.
    ///
    /// Interpolation is done per-component in linear RGB space.
    /// This provides smooth, perceptually correct color transitions.
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        [
            lerp_f32(self[0], to[0], t),
            lerp_f32(self[1], to[1], t),
            lerp_f32(self[2], to[2], t),
            lerp_f32(self[3], to[3], t),
        ]
    }
}

impl Interpolate for AnimatableEdgeInsets {
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        Self {
            top: lerp_f64(self.top, to.top, t),
            right: lerp_f64(self.right, to.right, t),
            bottom: lerp_f64(self.bottom, to.bottom, t),
            left: lerp_f64(self.left, to.left, t),
        }
    }
}

impl Interpolate for AnimatableTransform {
    /// Interpolate transform values.
    ///
    /// Each component (translate, scale, rotate) is interpolated independently.
    /// For more complex scenarios (like avoiding gimbal lock), quaternion
    /// interpolation could be added in the future.
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        Self {
            translate_x: lerp_f64(self.translate_x, to.translate_x, t),
            translate_y: lerp_f64(self.translate_y, to.translate_y, t),
            scale_x: lerp_f64(self.scale_x, to.scale_x, t),
            scale_y: lerp_f64(self.scale_y, to.scale_y, t),
            rotate: lerp_f64(self.rotate, to.rotate, t),
        }
    }
}

impl Interpolate for AnimatableBoxShadow {
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        Self {
            offset_x: lerp_f64(self.offset_x, to.offset_x, t),
            offset_y: lerp_f64(self.offset_y, to.offset_y, t),
            blur: lerp_f64(self.blur, to.blur, t),
            color: self.color.interpolate(&to.color, t),
        }
    }
}

impl Interpolate for AnimatableValue {
    /// Interpolate between two animatable values.
    ///
    /// Both values must be of the same variant. If they differ, returns self unchanged.
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        match (self, to) {
            (Self::F64 { value: from }, Self::F64 { value: to_val }) => Self::F64 {
                value: from.interpolate(to_val, t),
            },
            (Self::Color { rgba: from }, Self::Color { rgba: to_val }) => Self::Color {
                rgba: from.interpolate(to_val, t),
            },
            (Self::EdgeInsets { insets: from }, Self::EdgeInsets { insets: to_val }) => {
                Self::EdgeInsets {
                    insets: from.interpolate(to_val, t),
                }
            }
            (Self::Transform { transform: from }, Self::Transform { transform: to_val }) => {
                Self::Transform {
                    transform: from.interpolate(to_val, t),
                }
            }
            (Self::BoxShadow { shadow: from }, Self::BoxShadow { shadow: to_val }) => {
                Self::BoxShadow {
                    shadow: from.interpolate(to_val, t),
                }
            }
            // Type mismatch - return self unchanged
            _ => self.clone(),
        }
    }
}

/// Interpolate between two optional values.
///
/// - If both are Some, interpolate between them.
/// - If only one is Some, returns it at appropriate progress.
/// - If both are None, returns None.
pub fn interpolate_option<T: Interpolate + Clone>(
    from: &Option<T>,
    to: &Option<T>,
    t: f32,
) -> Option<T> {
    match (from, to) {
        (Some(f), Some(t_val)) => Some(f.interpolate(t_val, t)),
        (Some(f), None) => {
            if t < 1.0 {
                Some(f.clone())
            } else {
                None
            }
        }
        (None, Some(t_val)) => {
            if t > 0.0 {
                Some(t_val.clone())
            } else {
                None
            }
        }
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 0.0001;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    fn approx_eq_f32(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.0001
    }

    #[test]
    fn test_f64_interpolation() {
        let from = 0.0_f64;
        let to = 100.0_f64;

        assert!(approx_eq(from.interpolate(&to, 0.0), 0.0));
        assert!(approx_eq(from.interpolate(&to, 0.25), 25.0));
        assert!(approx_eq(from.interpolate(&to, 0.5), 50.0));
        assert!(approx_eq(from.interpolate(&to, 0.75), 75.0));
        assert!(approx_eq(from.interpolate(&to, 1.0), 100.0));
    }

    #[test]
    fn test_f64_negative_interpolation() {
        let from = -50.0_f64;
        let to = 50.0_f64;

        assert!(approx_eq(from.interpolate(&to, 0.0), -50.0));
        assert!(approx_eq(from.interpolate(&to, 0.5), 0.0));
        assert!(approx_eq(from.interpolate(&to, 1.0), 50.0));
    }

    #[test]
    fn test_color_interpolation() {
        let red: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
        let blue: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

        let mid = red.interpolate(&blue, 0.5);
        assert!(approx_eq_f32(mid[0], 0.5)); // R
        assert!(approx_eq_f32(mid[1], 0.0)); // G
        assert!(approx_eq_f32(mid[2], 0.5)); // B
        assert!(approx_eq_f32(mid[3], 1.0)); // A
    }

    #[test]
    fn test_color_alpha_interpolation() {
        let opaque: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
        let transparent: [f32; 4] = [1.0, 1.0, 1.0, 0.0];

        let mid = opaque.interpolate(&transparent, 0.5);
        assert!(approx_eq_f32(mid[3], 0.5));
    }

    #[test]
    fn test_edge_insets_interpolation() {
        let from = AnimatableEdgeInsets {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        };
        let to = AnimatableEdgeInsets {
            top: 20.0,
            right: 40.0,
            bottom: 60.0,
            left: 80.0,
        };

        let mid = from.interpolate(&to, 0.5);
        assert!(approx_eq(mid.top, 10.0));
        assert!(approx_eq(mid.right, 20.0));
        assert!(approx_eq(mid.bottom, 30.0));
        assert!(approx_eq(mid.left, 40.0));
    }

    #[test]
    fn test_transform_interpolation() {
        let from = AnimatableTransform {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotate: 0.0,
        };
        let to = AnimatableTransform {
            translate_x: 100.0,
            translate_y: 200.0,
            scale_x: 2.0,
            scale_y: 2.0,
            rotate: std::f64::consts::PI,
        };

        let mid = from.interpolate(&to, 0.5);
        assert!(approx_eq(mid.translate_x, 50.0));
        assert!(approx_eq(mid.translate_y, 100.0));
        assert!(approx_eq(mid.scale_x, 1.5));
        assert!(approx_eq(mid.scale_y, 1.5));
        assert!(approx_eq(mid.rotate, std::f64::consts::PI / 2.0));
    }

    #[test]
    fn test_box_shadow_interpolation() {
        let from = AnimatableBoxShadow {
            offset_x: 0.0,
            offset_y: 0.0,
            blur: 0.0,
            color: [0.0, 0.0, 0.0, 0.5],
        };
        let to = AnimatableBoxShadow {
            offset_x: 10.0,
            offset_y: 20.0,
            blur: 30.0,
            color: [0.0, 0.0, 0.0, 1.0],
        };

        let mid = from.interpolate(&to, 0.5);
        assert!(approx_eq(mid.offset_x, 5.0));
        assert!(approx_eq(mid.offset_y, 10.0));
        assert!(approx_eq(mid.blur, 15.0));
        assert!(approx_eq_f32(mid.color[3], 0.75));
    }

    #[test]
    fn test_animatable_value_interpolation() {
        // F64
        let from = AnimatableValue::F64 { value: 0.0 };
        let to = AnimatableValue::F64 { value: 100.0 };
        let mid = from.interpolate(&to, 0.5);
        assert_eq!(mid.as_f64(), Some(50.0));

        // Color
        let from = AnimatableValue::Color {
            rgba: [1.0, 0.0, 0.0, 1.0],
        };
        let to = AnimatableValue::Color {
            rgba: [0.0, 0.0, 1.0, 1.0],
        };
        let mid = from.interpolate(&to, 0.5);
        let color = mid.as_color().unwrap();
        assert!(approx_eq_f32(color[0], 0.5));
        assert!(approx_eq_f32(color[2], 0.5));
    }

    #[test]
    fn test_animatable_value_type_mismatch() {
        // When types don't match, return self unchanged
        let from = AnimatableValue::F64 { value: 50.0 };
        let to = AnimatableValue::Color {
            rgba: [1.0, 0.0, 0.0, 1.0],
        };
        let result = from.interpolate(&to, 0.5);
        assert_eq!(result.as_f64(), Some(50.0));
    }

    #[test]
    fn test_interpolate_option() {
        // Both Some
        let from = Some(0.0_f64);
        let to = Some(100.0_f64);
        assert_eq!(interpolate_option(&from, &to, 0.5), Some(50.0));

        // From Some, To None
        let from = Some(100.0_f64);
        let to: Option<f64> = None;
        assert_eq!(interpolate_option(&from, &to, 0.0), Some(100.0));
        assert_eq!(interpolate_option(&from, &to, 0.5), Some(100.0));
        assert_eq!(interpolate_option(&from, &to, 1.0), None);

        // From None, To Some
        let from: Option<f64> = None;
        let to = Some(100.0_f64);
        assert_eq!(interpolate_option(&from, &to, 0.0), None);
        assert_eq!(interpolate_option(&from, &to, 0.5), Some(100.0));
        assert_eq!(interpolate_option(&from, &to, 1.0), Some(100.0));

        // Both None
        let from: Option<f64> = None;
        let to: Option<f64> = None;
        assert_eq!(interpolate_option(&from, &to, 0.5), None);
    }

    #[test]
    fn test_extrapolation() {
        // Values outside 0-1 range should still work (extrapolation)
        let from = 0.0_f64;
        let to = 100.0_f64;

        // t > 1.0 extrapolates beyond
        assert!(approx_eq(from.interpolate(&to, 1.5), 150.0));

        // t < 0.0 extrapolates before
        assert!(approx_eq(from.interpolate(&to, -0.5), -50.0));
    }
}
