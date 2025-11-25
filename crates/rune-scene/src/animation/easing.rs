//! Easing functions for animation timing.
//!
//! This module implements CSS-compatible timing functions:
//! - Linear
//! - Ease, EaseIn, EaseOut, EaseInOut (standard CSS curves)
//! - CubicBezier (custom bezier curves)
//! - Steps (stepped animations)
//!
//! # Usage
//!
//! ```
//! use rune_scene::animation::easing::{EasingFunction, StepPosition};
//!
//! let ease = EasingFunction::Ease;
//! let progress = ease.evaluate(0.5); // Get eased progress at 50%
//!
//! let custom = EasingFunction::cubic_bezier(0.4, 0.0, 0.2, 1.0);
//! let progress = custom.evaluate(0.5);
//! ```

use serde::{Deserialize, Serialize};

/// Position for stepped animations.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepPosition {
    /// Jump at the start of each interval (CSS `jump-start` / `start`).
    Start,
    /// Jump at the end of each interval (CSS `jump-end` / `end`).
    End,
    /// Jump at both start and end (CSS `jump-both`).
    Both,
    /// No jump at start or end (CSS `jump-none`).
    None,
}

impl Default for StepPosition {
    fn default() -> Self {
        Self::End
    }
}

/// Easing function for animation timing.
///
/// Easing functions map a linear progress value (0.0 to 1.0) to an eased
/// output value, controlling the rate of change over time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EasingFunction {
    /// Linear interpolation (no easing).
    Linear,

    /// CSS `ease` - Slow start, fast middle, slow end.
    /// Equivalent to `cubic-bezier(0.25, 0.1, 0.25, 1.0)`.
    Ease,

    /// CSS `ease-in` - Slow start, accelerating.
    /// Equivalent to `cubic-bezier(0.42, 0, 1, 1)`.
    EaseIn,

    /// CSS `ease-out` - Fast start, decelerating.
    /// Equivalent to `cubic-bezier(0, 0, 0.58, 1)`.
    EaseOut,

    /// CSS `ease-in-out` - Slow start and end, fast middle.
    /// Equivalent to `cubic-bezier(0.42, 0, 0.58, 1)`.
    EaseInOut,

    /// Custom cubic bezier curve.
    /// Parameters: (x1, y1, x2, y2) - control points.
    /// x values must be in [0, 1], y values can be any float.
    CubicBezier {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },

    /// Stepped animation with discrete jumps.
    /// `steps` is the number of intervals (must be >= 1).
    Steps {
        count: u32,
        position: StepPosition,
    },
}

impl Default for EasingFunction {
    fn default() -> Self {
        Self::Ease
    }
}

impl EasingFunction {
    /// Evaluate the easing function at the given progress.
    ///
    /// # Arguments
    /// * `t` - Progress value from 0.0 to 1.0
    ///
    /// # Returns
    /// Eased progress value (may be outside 0.0-1.0 for some bezier curves)
    pub fn evaluate(&self, t: f32) -> f32 {
        // Clamp input to valid range
        let t = t.clamp(0.0, 1.0);

        match self {
            Self::Linear => t,
            Self::Ease => cubic_bezier(0.25, 0.1, 0.25, 1.0, t),
            Self::EaseIn => cubic_bezier(0.42, 0.0, 1.0, 1.0, t),
            Self::EaseOut => cubic_bezier(0.0, 0.0, 0.58, 1.0, t),
            Self::EaseInOut => cubic_bezier(0.42, 0.0, 0.58, 1.0, t),
            Self::CubicBezier { x1, y1, x2, y2 } => cubic_bezier(*x1, *y1, *x2, *y2, t),
            Self::Steps { count, position } => stepped(*count, *position, t),
        }
    }

    /// Create a custom cubic bezier easing function.
    ///
    /// # Arguments
    /// * `x1`, `y1` - First control point
    /// * `x2`, `y2` - Second control point
    ///
    /// # Panics
    /// Panics if x1 or x2 are outside [0, 1].
    pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&x1) && (0.0..=1.0).contains(&x2),
            "Bezier x values must be in [0, 1]"
        );
        Self::CubicBezier { x1, y1, x2, y2 }
    }

    /// Create a stepped easing function.
    ///
    /// # Arguments
    /// * `steps` - Number of steps (must be >= 1)
    /// * `position` - When the jump occurs
    ///
    /// # Panics
    /// Panics if steps is 0.
    pub fn steps(steps: u32, position: StepPosition) -> Self {
        assert!(steps >= 1, "Steps must be at least 1");
        Self::Steps {
            count: steps,
            position,
        }
    }
}

/// Evaluate a cubic bezier curve at time t.
///
/// This implementation uses Newton-Raphson iteration to find the t parameter
/// on the bezier curve corresponding to the input progress, then evaluates
/// the y coordinate at that point.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, progress: f32) -> f32 {
    // Handle edge cases
    if progress <= 0.0 {
        return 0.0;
    }
    if progress >= 1.0 {
        return 1.0;
    }

    // Find the t parameter that gives us the desired x value
    let t = solve_bezier_x(x1, x2, progress);

    // Evaluate the y coordinate at t
    bezier_y(y1, y2, t)
}

/// Solve for t in the bezier x equation using Newton-Raphson iteration.
fn solve_bezier_x(x1: f32, x2: f32, target_x: f32) -> f32 {
    // Initial guess
    let mut t = target_x;

    // Newton-Raphson iteration
    for _ in 0..8 {
        let x = bezier_x(x1, x2, t) - target_x;
        if x.abs() < 1e-6 {
            break;
        }

        let dx = bezier_x_derivative(x1, x2, t);
        if dx.abs() < 1e-6 {
            break;
        }

        t -= x / dx;
        t = t.clamp(0.0, 1.0);
    }

    t
}

/// Calculate x coordinate on the bezier curve at parameter t.
/// Bezier formula: x(t) = 3(1-t)²t·x1 + 3(1-t)t²·x2 + t³
#[inline]
fn bezier_x(x1: f32, x2: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;

    3.0 * mt2 * t * x1 + 3.0 * mt * t2 * x2 + t3
}

/// Calculate y coordinate on the bezier curve at parameter t.
#[inline]
fn bezier_y(y1: f32, y2: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;

    3.0 * mt2 * t * y1 + 3.0 * mt * t2 * y2 + t3
}

/// Calculate derivative of x with respect to t.
/// dx/dt = 3(1-t)²·x1 + 6(1-t)t·(x2-x1) + 3t²·(1-x2)
#[inline]
fn bezier_x_derivative(x1: f32, x2: f32, t: f32) -> f32 {
    let mt = 1.0 - t;
    3.0 * mt * mt * x1 + 6.0 * mt * t * (x2 - x1) + 3.0 * t * t * (1.0 - x2)
}

/// Evaluate stepped easing function.
fn stepped(steps: u32, position: StepPosition, t: f32) -> f32 {
    if steps == 0 {
        return t;
    }

    let steps_f = steps as f32;

    match position {
        StepPosition::Start => {
            // Jump at the start of each interval
            (t * steps_f).ceil() / steps_f
        }
        StepPosition::End => {
            // Jump at the end of each interval
            (t * steps_f).floor() / steps_f
        }
        StepPosition::Both => {
            // Add 1 to steps, jumps at both ends
            let total_steps = steps_f + 1.0;
            ((t * total_steps).floor() / steps_f).min(1.0)
        }
        StepPosition::None => {
            // Subtract 1 from effective steps, no jumps at ends
            if steps == 1 {
                // Special case: with 1 step and jump-none, we stay at 0.5
                0.5
            } else {
                let effective_steps = steps_f - 1.0;
                ((t * steps_f).floor() / effective_steps).min(1.0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.001;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_linear() {
        let ease = EasingFunction::Linear;
        assert!(approx_eq(ease.evaluate(0.0), 0.0));
        assert!(approx_eq(ease.evaluate(0.25), 0.25));
        assert!(approx_eq(ease.evaluate(0.5), 0.5));
        assert!(approx_eq(ease.evaluate(0.75), 0.75));
        assert!(approx_eq(ease.evaluate(1.0), 1.0));
    }

    #[test]
    fn test_ease_boundaries() {
        let ease = EasingFunction::Ease;
        assert!(approx_eq(ease.evaluate(0.0), 0.0));
        assert!(approx_eq(ease.evaluate(1.0), 1.0));

        // CSS ease (0.25, 0.1, 0.25, 1.0) starts slowly, then accelerates quickly
        // At t=0.5, output is typically around 0.8 due to the curve shape
        let mid = ease.evaluate(0.5);
        assert!(mid > 0.7 && mid < 0.9, "CSS ease mid-point should be ~0.8, got {}", mid);

        // Verify the curve is monotonically increasing
        let early = ease.evaluate(0.25);
        let late = ease.evaluate(0.75);
        assert!(early < mid, "early ({}) should be less than mid ({})", early, mid);
        assert!(mid < late, "mid ({}) should be less than late ({})", mid, late);
    }

    #[test]
    fn test_ease_in() {
        let ease = EasingFunction::EaseIn;
        assert!(approx_eq(ease.evaluate(0.0), 0.0));
        assert!(approx_eq(ease.evaluate(1.0), 1.0));

        // Ease-in should be slower at start, faster at end
        let early = ease.evaluate(0.25);
        let mid = ease.evaluate(0.5);
        assert!(early < 0.25); // Slower at start
        assert!(mid < 0.5); // Still accelerating
    }

    #[test]
    fn test_ease_out() {
        let ease = EasingFunction::EaseOut;
        assert!(approx_eq(ease.evaluate(0.0), 0.0));
        assert!(approx_eq(ease.evaluate(1.0), 1.0));

        // Ease-out should be faster at start, slower at end
        let early = ease.evaluate(0.25);
        let mid = ease.evaluate(0.5);
        assert!(early > 0.25); // Faster at start
        assert!(mid > 0.5); // Decelerating
    }

    #[test]
    fn test_ease_in_out() {
        let ease = EasingFunction::EaseInOut;
        assert!(approx_eq(ease.evaluate(0.0), 0.0));
        assert!(approx_eq(ease.evaluate(1.0), 1.0));

        // Ease-in-out should be symmetrical
        let mid = ease.evaluate(0.5);
        assert!(approx_eq(mid, 0.5));

        // Check symmetry
        let early = ease.evaluate(0.25);
        let late = ease.evaluate(0.75);
        assert!(approx_eq(early + late, 1.0));
    }

    #[test]
    fn test_custom_bezier() {
        // Material Design standard curve
        let ease = EasingFunction::cubic_bezier(0.4, 0.0, 0.2, 1.0);
        assert!(approx_eq(ease.evaluate(0.0), 0.0));
        assert!(approx_eq(ease.evaluate(1.0), 1.0));

        // Linear equivalent
        let linear_bezier = EasingFunction::CubicBezier {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        assert!(approx_eq(linear_bezier.evaluate(0.5), 0.5));
    }

    #[test]
    fn test_steps_end() {
        let ease = EasingFunction::steps(4, StepPosition::End);

        assert!(approx_eq(ease.evaluate(0.0), 0.0));
        assert!(approx_eq(ease.evaluate(0.24), 0.0));
        assert!(approx_eq(ease.evaluate(0.25), 0.25));
        assert!(approx_eq(ease.evaluate(0.49), 0.25));
        assert!(approx_eq(ease.evaluate(0.5), 0.5));
        assert!(approx_eq(ease.evaluate(0.74), 0.5));
        assert!(approx_eq(ease.evaluate(0.75), 0.75));
        assert!(approx_eq(ease.evaluate(0.99), 0.75));
        assert!(approx_eq(ease.evaluate(1.0), 1.0));
    }

    #[test]
    fn test_steps_start() {
        let ease = EasingFunction::steps(4, StepPosition::Start);

        assert!(approx_eq(ease.evaluate(0.0), 0.0));
        assert!(approx_eq(ease.evaluate(0.01), 0.25));
        assert!(approx_eq(ease.evaluate(0.25), 0.25));
        assert!(approx_eq(ease.evaluate(0.26), 0.5));
        assert!(approx_eq(ease.evaluate(0.5), 0.5));
        assert!(approx_eq(ease.evaluate(0.51), 0.75));
        assert!(approx_eq(ease.evaluate(0.75), 0.75));
        assert!(approx_eq(ease.evaluate(0.76), 1.0));
        assert!(approx_eq(ease.evaluate(1.0), 1.0));
    }

    #[test]
    fn test_clamping() {
        let ease = EasingFunction::Ease;

        // Values outside 0-1 should be clamped
        assert!(approx_eq(ease.evaluate(-0.5), 0.0));
        assert!(approx_eq(ease.evaluate(1.5), 1.0));
    }

    #[test]
    fn test_default() {
        assert_eq!(EasingFunction::default(), EasingFunction::Ease);
        assert_eq!(StepPosition::default(), StepPosition::End);
    }

    #[test]
    #[should_panic(expected = "Bezier x values must be in [0, 1]")]
    fn test_invalid_bezier_x1() {
        EasingFunction::cubic_bezier(-0.1, 0.0, 0.5, 1.0);
    }

    #[test]
    #[should_panic(expected = "Bezier x values must be in [0, 1]")]
    fn test_invalid_bezier_x2() {
        EasingFunction::cubic_bezier(0.5, 0.0, 1.5, 1.0);
    }

    #[test]
    #[should_panic(expected = "Steps must be at least 1")]
    fn test_invalid_steps() {
        EasingFunction::steps(0, StepPosition::End);
    }
}
