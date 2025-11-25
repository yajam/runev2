//! 2D Transform support for CSS-like transforms.
//!
//! This module provides a `Transform2D` struct that represents a 2D affine transform
//! with support for composition, decomposition, and matrix operations.
//!
//! # Features
//!
//! - **Decomposed representation**: translate, scale, rotate, skew
//! - **Matrix form**: 3x3 affine transformation matrix
//! - **Composition**: Combine multiple transforms
//! - **Transform origin**: Apply transforms relative to a point
//! - **Interpolation**: Smooth animation between transforms
//!
//! # Usage
//!
//! ```ignore
//! use rune_scene::animation::transform::{Transform2D, TransformOrigin};
//!
//! // Create transforms
//! let translate = Transform2D::translate(100.0, 50.0);
//! let scale = Transform2D::scale(2.0, 2.0);
//! let rotate = Transform2D::rotate_deg(45.0);
//!
//! // Compose transforms (applied right to left like CSS)
//! let combined = translate.then(&scale).then(&rotate);
//!
//! // Apply to a point
//! let (x, y) = combined.apply_point(0.0, 0.0);
//!
//! // With transform origin
//! let origin = TransformOrigin::center();
//! let with_origin = scale.with_origin(origin, 100.0, 100.0);
//! ```

use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

use super::interpolate::Interpolate;
use super::types::AnimatableTransform;

/// A 2D affine transformation matrix.
///
/// Stored as a 3x2 matrix (the bottom row [0, 0, 1] is implicit):
/// ```text
/// | a  c  tx |
/// | b  d  ty |
/// | 0  0  1  |
/// ```
///
/// This can represent translation, rotation, scaling, and skewing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transform2D {
    /// Scale X (matrix element a)
    pub a: f64,
    /// Skew Y (matrix element b)
    pub b: f64,
    /// Skew X (matrix element c)
    pub c: f64,
    /// Scale Y (matrix element d)
    pub d: f64,
    /// Translate X (matrix element tx)
    pub tx: f64,
    /// Translate Y (matrix element ty)
    pub ty: f64,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::identity()
    }
}

impl Transform2D {
    /// Create an identity transform (no change).
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// Create a translation transform.
    pub fn translate(tx: f64, ty: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx,
            ty,
        }
    }

    /// Create a uniform scale transform.
    pub fn scale_uniform(s: f64) -> Self {
        Self::scale(s, s)
    }

    /// Create a non-uniform scale transform.
    pub fn scale(sx: f64, sy: f64) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// Create a rotation transform from radians.
    pub fn rotate(angle_rad: f64) -> Self {
        let cos = angle_rad.cos();
        let sin = angle_rad.sin();
        Self {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// Create a rotation transform from degrees.
    pub fn rotate_deg(angle_deg: f64) -> Self {
        Self::rotate(angle_deg * PI / 180.0)
    }

    /// Create a skew transform.
    ///
    /// # Arguments
    /// * `skew_x` - Horizontal skew angle in radians
    /// * `skew_y` - Vertical skew angle in radians
    pub fn skew(skew_x: f64, skew_y: f64) -> Self {
        Self {
            a: 1.0,
            b: skew_y.tan(),
            c: skew_x.tan(),
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// Create a skew transform from degrees.
    pub fn skew_deg(skew_x_deg: f64, skew_y_deg: f64) -> Self {
        Self::skew(skew_x_deg * PI / 180.0, skew_y_deg * PI / 180.0)
    }

    /// Create a transform from decomposed components.
    ///
    /// Follows CSS transform order: translate -> rotate -> scale (-> skew)
    /// Matrix multiplication: M = T * R * S * Sk
    pub fn from_decomposed(decomposed: &DecomposedTransform) -> Self {
        let cos = decomposed.rotate.cos();
        let sin = decomposed.rotate.sin();

        // Combined matrix: translate * rotate * scale
        // Skew is typically 0 for simple transforms, we'll apply it if non-zero
        let a = cos * decomposed.scale_x;
        let b = sin * decomposed.scale_x;
        let c = -sin * decomposed.scale_y;
        let d = cos * decomposed.scale_y;

        let mut result = Self {
            a,
            b,
            c,
            d,
            tx: decomposed.translate_x,
            ty: decomposed.translate_y,
        };

        // Apply skew if present
        if decomposed.skew_x.abs() > f64::EPSILON || decomposed.skew_y.abs() > f64::EPSILON {
            let skew = Self::skew(decomposed.skew_x, decomposed.skew_y);
            result = result.then(&skew);
        }

        result
    }

    /// Compose this transform with another (this * other).
    ///
    /// The resulting transform applies `other` first, then `self`.
    pub fn then(&self, other: &Self) -> Self {
        Self {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            tx: self.a * other.tx + self.c * other.ty + self.tx,
            ty: self.b * other.tx + self.d * other.ty + self.ty,
        }
    }

    /// Pre-multiply: compose other before this (other * self).
    pub fn pre_multiply(&self, other: &Self) -> Self {
        other.then(self)
    }

    /// Apply this transform to a point.
    pub fn apply_point(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.tx,
            self.b * x + self.d * y + self.ty,
        )
    }

    /// Apply this transform to a vector (ignores translation).
    pub fn apply_vector(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y, self.b * x + self.d * y)
    }

    /// Calculate the determinant.
    pub fn determinant(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }

    /// Check if this transform is invertible.
    pub fn is_invertible(&self) -> bool {
        self.determinant().abs() > f64::EPSILON
    }

    /// Compute the inverse transform.
    ///
    /// Returns `None` if the transform is not invertible (determinant is zero).
    pub fn inverse(&self) -> Option<Self> {
        let det = self.determinant();
        if det.abs() <= f64::EPSILON {
            return None;
        }

        let inv_det = 1.0 / det;
        Some(Self {
            a: self.d * inv_det,
            b: -self.b * inv_det,
            c: -self.c * inv_det,
            d: self.a * inv_det,
            tx: (self.c * self.ty - self.d * self.tx) * inv_det,
            ty: (self.b * self.tx - self.a * self.ty) * inv_det,
        })
    }

    /// Decompose this transform into translate, rotate, scale, and skew components.
    ///
    /// This follows the CSS decomposition algorithm for 2D transforms.
    /// Matrix layout: | a  c  tx |
    ///                | b  d  ty |
    /// Where: a = sx*cos(θ), b = sx*sin(θ), c = -sy*sin(θ), d = sy*cos(θ)
    pub fn decompose(&self) -> DecomposedTransform {
        let mut result = DecomposedTransform::identity();

        // Extract translation
        result.translate_x = self.tx;
        result.translate_y = self.ty;

        // Extract scale X from first column (a, b)
        result.scale_x = (self.a * self.a + self.b * self.b).sqrt();

        // Extract rotation from normalized first column
        if result.scale_x != 0.0 {
            result.rotate = self.b.atan2(self.a);
        }

        // Extract scale Y from second column after removing rotation
        // For the standard rotation matrix: c = -sy*sin(θ), d = sy*cos(θ)
        // We can compute sy by checking the second column with rotation removed
        let cos = result.rotate.cos();
        let sin = result.rotate.sin();

        // For a matrix: | a  c |   | sx*cos  -sy*sin |
        //               | b  d | = | sx*sin   sy*cos |
        // We can extract sy by computing what d would be given our extracted values
        // d = sy * cos(θ), so sy = d / cos(θ) if cos(θ) != 0
        // Or c = -sy * sin(θ), so sy = -c / sin(θ) if sin(θ) != 0
        if cos.abs() > sin.abs() {
            result.scale_y = self.d / cos;
        } else if sin.abs() > f64::EPSILON {
            result.scale_y = -self.c / sin;
        } else {
            // Identity-like, just use magnitude
            result.scale_y = (self.c * self.c + self.d * self.d).sqrt();
        }

        // Compute skew from residual - for now assume no skew
        result.skew_x = 0.0;
        result.skew_y = 0.0;

        result
    }

    /// Apply a transform with a specific origin point.
    ///
    /// This translates to the origin, applies the transform, then translates back.
    pub fn with_origin(&self, origin: TransformOrigin, width: f64, height: f64) -> Self {
        let (ox, oy) = origin.resolve(width, height);
        Self::translate(ox, oy)
            .then(self)
            .then(&Self::translate(-ox, -oy))
    }

    /// Check if this is approximately an identity transform.
    pub fn is_identity(&self, epsilon: f64) -> bool {
        (self.a - 1.0).abs() < epsilon
            && self.b.abs() < epsilon
            && self.c.abs() < epsilon
            && (self.d - 1.0).abs() < epsilon
            && self.tx.abs() < epsilon
            && self.ty.abs() < epsilon
    }

    /// Convert to a 3x3 matrix array (row-major).
    pub fn to_matrix3x3(&self) -> [[f64; 3]; 3] {
        [
            [self.a, self.c, self.tx],
            [self.b, self.d, self.ty],
            [0.0, 0.0, 1.0],
        ]
    }

    /// Convert to a 4x4 matrix array (row-major) for 3D rendering.
    ///
    /// The 2D transform is embedded in the XY plane with Z=0.
    pub fn to_matrix4x4(&self) -> [[f64; 4]; 4] {
        [
            [self.a, self.c, 0.0, self.tx],
            [self.b, self.d, 0.0, self.ty],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    /// Create from an AnimatableTransform.
    ///
    /// Uses the same matrix construction as `from_decomposed`: translate * rotate * scale
    pub fn from_animatable(t: &AnimatableTransform) -> Self {
        let cos = t.rotate.cos();
        let sin = t.rotate.sin();

        Self {
            a: cos * t.scale_x,
            b: sin * t.scale_x,
            c: -sin * t.scale_y,
            d: cos * t.scale_y,
            tx: t.translate_x,
            ty: t.translate_y,
        }
    }

    /// Convert to an AnimatableTransform by decomposition.
    pub fn to_animatable(&self) -> AnimatableTransform {
        let decomposed = self.decompose();
        AnimatableTransform {
            translate_x: decomposed.translate_x,
            translate_y: decomposed.translate_y,
            scale_x: decomposed.scale_x,
            scale_y: decomposed.scale_y,
            rotate: decomposed.rotate,
        }
    }
}

impl From<AnimatableTransform> for Transform2D {
    fn from(t: AnimatableTransform) -> Self {
        Self::from_animatable(&t)
    }
}

impl From<Transform2D> for AnimatableTransform {
    fn from(t: Transform2D) -> Self {
        t.to_animatable()
    }
}

/// Decomposed 2D transform components.
///
/// This representation is useful for animation because components can be
/// interpolated independently for smooth animations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DecomposedTransform {
    pub translate_x: f64,
    pub translate_y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    /// Rotation in radians.
    pub rotate: f64,
    /// Horizontal skew in radians.
    pub skew_x: f64,
    /// Vertical skew in radians.
    pub skew_y: f64,
}

impl Default for DecomposedTransform {
    fn default() -> Self {
        Self::identity()
    }
}

impl DecomposedTransform {
    /// Create an identity decomposed transform.
    pub fn identity() -> Self {
        Self {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotate: 0.0,
            skew_x: 0.0,
            skew_y: 0.0,
        }
    }

    /// Convert to a Transform2D matrix.
    pub fn to_matrix(&self) -> Transform2D {
        Transform2D::from_decomposed(self)
    }

    /// Create from an AnimatableTransform.
    pub fn from_animatable(t: &AnimatableTransform) -> Self {
        Self {
            translate_x: t.translate_x,
            translate_y: t.translate_y,
            scale_x: t.scale_x,
            scale_y: t.scale_y,
            rotate: t.rotate,
            skew_x: 0.0,
            skew_y: 0.0,
        }
    }

    /// Convert to an AnimatableTransform (skew is lost).
    pub fn to_animatable(&self) -> AnimatableTransform {
        AnimatableTransform {
            translate_x: self.translate_x,
            translate_y: self.translate_y,
            scale_x: self.scale_x,
            scale_y: self.scale_y,
            rotate: self.rotate,
        }
    }
}

impl Interpolate for DecomposedTransform {
    fn interpolate(&self, to: &Self, t: f32) -> Self {
        let t = t as f64;
        Self {
            translate_x: self.translate_x + (to.translate_x - self.translate_x) * t,
            translate_y: self.translate_y + (to.translate_y - self.translate_y) * t,
            scale_x: self.scale_x + (to.scale_x - self.scale_x) * t,
            scale_y: self.scale_y + (to.scale_y - self.scale_y) * t,
            rotate: interpolate_angle(self.rotate, to.rotate, t),
            skew_x: self.skew_x + (to.skew_x - self.skew_x) * t,
            skew_y: self.skew_y + (to.skew_y - self.skew_y) * t,
        }
    }
}

/// Interpolate between two angles, taking the shortest path.
fn interpolate_angle(from: f64, to: f64, t: f64) -> f64 {
    let mut diff = to - from;

    // Normalize to [-PI, PI]
    while diff > PI {
        diff -= 2.0 * PI;
    }
    while diff < -PI {
        diff += 2.0 * PI;
    }

    from + diff * t
}

/// Transform origin specification.
///
/// Determines the point around which transforms are applied.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransformOrigin {
    /// Origin at absolute pixel coordinates.
    Absolute { x: f64, y: f64 },
    /// Origin at percentage of element size.
    Percentage { x: f64, y: f64 },
    /// Named origin positions.
    Named(NamedOrigin),
}

/// Named transform origin positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamedOrigin {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Default for TransformOrigin {
    fn default() -> Self {
        Self::center()
    }
}

impl TransformOrigin {
    /// Create an origin at the center (50%, 50%).
    pub fn center() -> Self {
        Self::Named(NamedOrigin::Center)
    }

    /// Create an origin at the top-left corner.
    pub fn top_left() -> Self {
        Self::Named(NamedOrigin::TopLeft)
    }

    /// Create an origin at absolute coordinates.
    pub fn absolute(x: f64, y: f64) -> Self {
        Self::Absolute { x, y }
    }

    /// Create an origin at percentage coordinates (0.0 = 0%, 1.0 = 100%).
    pub fn percentage(x: f64, y: f64) -> Self {
        Self::Percentage { x, y }
    }

    /// Resolve the origin to absolute coordinates given element dimensions.
    pub fn resolve(&self, width: f64, height: f64) -> (f64, f64) {
        match self {
            Self::Absolute { x, y } => (*x, *y),
            Self::Percentage { x, y } => (x * width, y * height),
            Self::Named(named) => named.resolve(width, height),
        }
    }
}

impl NamedOrigin {
    /// Resolve to absolute coordinates.
    pub fn resolve(&self, width: f64, height: f64) -> (f64, f64) {
        match self {
            Self::TopLeft => (0.0, 0.0),
            Self::TopCenter => (width * 0.5, 0.0),
            Self::TopRight => (width, 0.0),
            Self::CenterLeft => (0.0, height * 0.5),
            Self::Center => (width * 0.5, height * 0.5),
            Self::CenterRight => (width, height * 0.5),
            Self::BottomLeft => (0.0, height),
            Self::BottomCenter => (width * 0.5, height),
            Self::BottomRight => (width, height),
        }
    }
}

/// A transform stack for applying multiple transforms in sequence.
///
/// This is useful when building up complex transforms from multiple sources
/// (e.g., CSS transform functions).
#[derive(Debug, Clone, Default)]
pub struct TransformStack {
    transforms: Vec<Transform2D>,
}

impl TransformStack {
    /// Create a new empty transform stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a transform onto the stack.
    pub fn push(&mut self, transform: Transform2D) {
        self.transforms.push(transform);
    }

    /// Push a translation.
    pub fn translate(&mut self, tx: f64, ty: f64) {
        self.push(Transform2D::translate(tx, ty));
    }

    /// Push a scale.
    pub fn scale(&mut self, sx: f64, sy: f64) {
        self.push(Transform2D::scale(sx, sy));
    }

    /// Push a uniform scale.
    pub fn scale_uniform(&mut self, s: f64) {
        self.push(Transform2D::scale_uniform(s));
    }

    /// Push a rotation in radians.
    pub fn rotate(&mut self, angle_rad: f64) {
        self.push(Transform2D::rotate(angle_rad));
    }

    /// Push a rotation in degrees.
    pub fn rotate_deg(&mut self, angle_deg: f64) {
        self.push(Transform2D::rotate_deg(angle_deg));
    }

    /// Push a skew.
    pub fn skew(&mut self, skew_x: f64, skew_y: f64) {
        self.push(Transform2D::skew(skew_x, skew_y));
    }

    /// Push a skew in degrees.
    pub fn skew_deg(&mut self, skew_x_deg: f64, skew_y_deg: f64) {
        self.push(Transform2D::skew_deg(skew_x_deg, skew_y_deg));
    }

    /// Compute the combined transform (applied in push order).
    pub fn to_matrix(&self) -> Transform2D {
        let mut result = Transform2D::identity();
        for t in &self.transforms {
            result = result.then(t);
        }
        result
    }

    /// Clear the stack.
    pub fn clear(&mut self) {
        self.transforms.clear();
    }

    /// Check if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.transforms.is_empty()
    }

    /// Get the number of transforms in the stack.
    pub fn len(&self) -> usize {
        self.transforms.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    fn approx_eq_loose(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn test_identity() {
        let t = Transform2D::identity();
        assert!(t.is_identity(EPSILON));

        let (x, y) = t.apply_point(100.0, 200.0);
        assert!(approx_eq(x, 100.0));
        assert!(approx_eq(y, 200.0));
    }

    #[test]
    fn test_translate() {
        let t = Transform2D::translate(50.0, 100.0);
        let (x, y) = t.apply_point(10.0, 20.0);
        assert!(approx_eq(x, 60.0));
        assert!(approx_eq(y, 120.0));
    }

    #[test]
    fn test_scale() {
        let t = Transform2D::scale(2.0, 3.0);
        let (x, y) = t.apply_point(10.0, 20.0);
        assert!(approx_eq(x, 20.0));
        assert!(approx_eq(y, 60.0));
    }

    #[test]
    fn test_rotate_90() {
        let t = Transform2D::rotate_deg(90.0);
        let (x, y) = t.apply_point(1.0, 0.0);
        assert!(approx_eq_loose(x, 0.0));
        assert!(approx_eq_loose(y, 1.0));
    }

    #[test]
    fn test_rotate_180() {
        let t = Transform2D::rotate_deg(180.0);
        let (x, y) = t.apply_point(1.0, 0.0);
        assert!(approx_eq_loose(x, -1.0));
        assert!(approx_eq_loose(y, 0.0));
    }

    #[test]
    fn test_composition() {
        // Translate then scale
        let t = Transform2D::translate(10.0, 20.0).then(&Transform2D::scale(2.0, 2.0));
        let (x, y) = t.apply_point(0.0, 0.0);
        // Scale is applied first (0,0) -> (0,0), then translate (0,0) -> (10,20)
        assert!(approx_eq(x, 10.0));
        assert!(approx_eq(y, 20.0));

        // Scale then translate (different result)
        let t2 = Transform2D::scale(2.0, 2.0).then(&Transform2D::translate(10.0, 20.0));
        let (x2, y2) = t2.apply_point(0.0, 0.0);
        // Translate first (0,0) -> (10,20), then scale (10,20) -> (20,40)
        assert!(approx_eq(x2, 20.0));
        assert!(approx_eq(y2, 40.0));
    }

    #[test]
    fn test_inverse() {
        let t = Transform2D::translate(50.0, 100.0)
            .then(&Transform2D::scale(2.0, 3.0))
            .then(&Transform2D::rotate_deg(45.0));

        let inv = t.inverse().unwrap();
        let roundtrip = t.then(&inv);
        assert!(roundtrip.is_identity(1e-10));
    }

    #[test]
    fn test_determinant() {
        assert!(approx_eq(Transform2D::identity().determinant(), 1.0));
        assert!(approx_eq(Transform2D::scale(2.0, 3.0).determinant(), 6.0));
        assert!(approx_eq(Transform2D::rotate_deg(45.0).determinant(), 1.0));
    }

    #[test]
    fn test_decompose_identity() {
        let t = Transform2D::identity();
        let d = t.decompose();
        assert!(approx_eq(d.translate_x, 0.0));
        assert!(approx_eq(d.translate_y, 0.0));
        assert!(approx_eq(d.scale_x, 1.0));
        assert!(approx_eq(d.scale_y, 1.0));
        assert!(approx_eq(d.rotate, 0.0));
    }

    #[test]
    fn test_decompose_translate() {
        let t = Transform2D::translate(100.0, 200.0);
        let d = t.decompose();
        assert!(approx_eq(d.translate_x, 100.0));
        assert!(approx_eq(d.translate_y, 200.0));
        assert!(approx_eq(d.scale_x, 1.0));
        assert!(approx_eq(d.scale_y, 1.0));
    }

    #[test]
    fn test_decompose_scale() {
        let t = Transform2D::scale(2.0, 3.0);
        let d = t.decompose();
        assert!(approx_eq(d.scale_x, 2.0));
        assert!(approx_eq(d.scale_y, 3.0));
        assert!(approx_eq(d.translate_x, 0.0));
        assert!(approx_eq(d.translate_y, 0.0));
    }

    #[test]
    fn test_decompose_rotate() {
        let angle = PI / 4.0; // 45 degrees
        let t = Transform2D::rotate(angle);
        let d = t.decompose();
        assert!(approx_eq_loose(d.rotate, angle));
        assert!(approx_eq_loose(d.scale_x, 1.0));
        assert!(approx_eq_loose(d.scale_y, 1.0));
    }

    #[test]
    fn test_decompose_roundtrip() {
        let original = Transform2D::translate(50.0, 100.0)
            .then(&Transform2D::rotate_deg(30.0))
            .then(&Transform2D::scale(2.0, 1.5));

        let decomposed = original.decompose();
        let reconstructed = decomposed.to_matrix();

        // Test that applying to a point gives same result
        let test_point = (100.0, 200.0);
        let (ox, oy) = original.apply_point(test_point.0, test_point.1);
        let (rx, ry) = reconstructed.apply_point(test_point.0, test_point.1);
        assert!(approx_eq_loose(ox, rx));
        assert!(approx_eq_loose(oy, ry));
    }

    #[test]
    fn test_transform_origin() {
        // Scale from center of 100x100 element
        let scale = Transform2D::scale(2.0, 2.0);
        let origin = TransformOrigin::center();
        let with_origin = scale.with_origin(origin, 100.0, 100.0);

        // Center point should remain at center
        let (cx, cy) = with_origin.apply_point(50.0, 50.0);
        assert!(approx_eq(cx, 50.0));
        assert!(approx_eq(cy, 50.0));

        // Corner should move away from center
        let (corner_x, corner_y) = with_origin.apply_point(0.0, 0.0);
        assert!(approx_eq(corner_x, -50.0));
        assert!(approx_eq(corner_y, -50.0));
    }

    #[test]
    fn test_named_origins() {
        let width = 200.0;
        let height = 100.0;

        assert_eq!(NamedOrigin::TopLeft.resolve(width, height), (0.0, 0.0));
        assert_eq!(NamedOrigin::TopCenter.resolve(width, height), (100.0, 0.0));
        assert_eq!(NamedOrigin::TopRight.resolve(width, height), (200.0, 0.0));
        assert_eq!(NamedOrigin::Center.resolve(width, height), (100.0, 50.0));
        assert_eq!(
            NamedOrigin::BottomRight.resolve(width, height),
            (200.0, 100.0)
        );
    }

    #[test]
    fn test_interpolate_decomposed() {
        let from = DecomposedTransform {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotate: 0.0,
            skew_x: 0.0,
            skew_y: 0.0,
        };

        let to = DecomposedTransform {
            translate_x: 100.0,
            translate_y: 200.0,
            scale_x: 2.0,
            scale_y: 3.0,
            rotate: PI / 2.0,
            skew_x: 0.0,
            skew_y: 0.0,
        };

        let mid = from.interpolate(&to, 0.5);
        assert!(approx_eq(mid.translate_x, 50.0));
        assert!(approx_eq(mid.translate_y, 100.0));
        assert!(approx_eq(mid.scale_x, 1.5));
        assert!(approx_eq(mid.scale_y, 2.0));
        assert!(approx_eq(mid.rotate, PI / 4.0));
    }

    #[test]
    fn test_interpolate_angle_shortest_path() {
        // From 350 degrees to 10 degrees should go through 0, not 180
        let from = 350.0 * PI / 180.0;
        let to = 10.0 * PI / 180.0;
        let mid = interpolate_angle(from, to, 0.5);

        // Should be at 0 degrees (or 360)
        let mid_deg = mid * 180.0 / PI;
        // Normalize to [0, 360)
        let normalized = ((mid_deg % 360.0) + 360.0) % 360.0;
        assert!(normalized < 5.0 || normalized > 355.0);
    }

    #[test]
    fn test_transform_stack() {
        let mut stack = TransformStack::new();
        assert!(stack.is_empty());

        stack.translate(100.0, 50.0);
        stack.scale(2.0, 2.0);
        stack.rotate_deg(45.0);

        assert_eq!(stack.len(), 3);

        let combined = stack.to_matrix();
        assert!(!combined.is_identity(EPSILON));

        stack.clear();
        assert!(stack.is_empty());
    }

    #[test]
    fn test_animatable_transform_conversion() {
        let animatable = AnimatableTransform {
            translate_x: 100.0,
            translate_y: 50.0,
            scale_x: 2.0,
            scale_y: 1.5,
            rotate: PI / 4.0,
        };

        let transform2d = Transform2D::from_animatable(&animatable);
        let back = transform2d.to_animatable();

        assert!(approx_eq_loose(back.translate_x, animatable.translate_x));
        assert!(approx_eq_loose(back.translate_y, animatable.translate_y));
        assert!(approx_eq_loose(back.scale_x, animatable.scale_x));
        assert!(approx_eq_loose(back.scale_y, animatable.scale_y));
        assert!(approx_eq_loose(back.rotate, animatable.rotate));
    }

    #[test]
    fn test_skew() {
        let t = Transform2D::skew_deg(45.0, 0.0);
        let (x, y) = t.apply_point(0.0, 1.0);
        // Skew X of 45 degrees means y coordinate is added to x
        assert!(approx_eq_loose(x, 1.0));
        assert!(approx_eq_loose(y, 1.0));
    }

    #[test]
    fn test_to_matrix_formats() {
        let t = Transform2D::translate(10.0, 20.0);

        let m3 = t.to_matrix3x3();
        assert!(approx_eq(m3[0][2], 10.0));
        assert!(approx_eq(m3[1][2], 20.0));
        assert!(approx_eq(m3[2][2], 1.0));

        let m4 = t.to_matrix4x4();
        assert!(approx_eq(m4[0][3], 10.0));
        assert!(approx_eq(m4[1][3], 20.0));
        assert!(approx_eq(m4[2][2], 1.0));
        assert!(approx_eq(m4[3][3], 1.0));
    }
}
