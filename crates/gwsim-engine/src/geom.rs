//! Two-dimensional geometry in gwinches (T3.2.2).
//!
//! The field is an open plane with no terrain (Q8). Only `sqrt` is used in
//! the hot path, and range checks compare squared distances, so nothing here
//! depends on platform trigonometry.

use std::ops::{Add, Mul, Neg, Sub};

/// A point or direction on the field.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn distance_squared(self, other: Vec2) -> f32 {
        (self - other).length_squared()
    }

    pub fn distance(self, other: Vec2) -> f32 {
        (self - other).length()
    }

    /// A unit-length vector in the same direction, or zero.
    pub fn normalised(self) -> Vec2 {
        let length = self.length();
        if length <= f32::EPSILON {
            Vec2::ZERO
        } else {
            self * (1.0 / length)
        }
    }

    pub fn dot(self, other: Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// Whether a point is within `radius` of this one. Distances are centre
    /// to centre, and a point exactly on the boundary counts as inside.
    pub fn within(self, other: Vec2, radius: f32) -> bool {
        self.distance_squared(other) <= radius * radius
    }

    /// Whether every coordinate is finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, other: Vec2) -> Vec2 {
        Vec2::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, other: Vec2) -> Vec2 {
        Vec2::new(self.x - other.x, self.y - other.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, factor: f32) -> Vec2 {
        Vec2::new(self.x * factor, self.y * factor)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn band_boundaries_are_inclusive() {
        let origin = Vec2::ZERO;
        for radius in [144.0f32, 166.0, 252.0, 322.0, 1012.0, 1248.0] {
            assert!(
                origin.within(Vec2::new(radius - 1.0, 0.0), radius),
                "inside {radius}"
            );
            assert!(origin.within(Vec2::new(radius, 0.0), radius), "on {radius}");
            assert!(
                !origin.within(Vec2::new(radius + 1.0, 0.0), radius),
                "outside {radius}"
            );
        }
    }

    #[test]
    fn normalising_zero_gives_zero() {
        assert_eq!(Vec2::ZERO.normalised(), Vec2::ZERO);
        let unit = Vec2::new(3.0, 4.0).normalised();
        assert!((unit.length() - 1.0).abs() < 1e-6);
    }
}
