/// 2D point in logical pixel space
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    pub fn distance(&self, other: Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// 2D size in logical pixel space
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub const fn zero() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
        }
    }

    pub fn area(&self) -> f32 {
        self.width * self.height
    }
}

/// Rectangle in logical pixel space (floating-point coordinates)
/// Origin: top-left corner
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RectF {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// SPEC-HAL-093 v3: Snap rect to pixel grid for consistent 1px border rendering.
    /// Origins floor, far edges ceil, dimensions derived from endpoints.
    pub fn snap_to_pixels(self) -> Self {
        let x0 = self.x.floor();
        let y0 = self.y.floor();
        let x1 = (self.x + self.width).ceil();
        let y1 = (self.y + self.height).ceil();
        Self::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// SPEC-HAL-093 v3: Snap rect to physical pixel grid for crisp 1px border
    /// rendering on fractional-DPI displays. At 150% DPI: logical coordinates
    /// are multiplied by 1.5, floored/ceiled to physical pixel boundaries,
    /// then converted back to logical coords. At 100% DPI this is identical
    /// to `snap_to_pixels`.
    pub fn snap_to_physical_pixels(self, scale: f32) -> Self {
        let x0 = (self.x * scale).floor() / scale;
        let y0 = (self.y * scale).floor() / scale;
        let x1 = ((self.x + self.width) * scale).ceil() / scale;
        let y1 = ((self.y + self.height) * scale).ceil() / scale;
        Self::new(x0, y0, x1 - x0, y1 - y0)
    }

    pub const fn from_pos_size(pos: Point, size: Size) -> Self {
        Self {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
        }
    }

    pub const fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        }
    }

    pub const fn pos(&self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }

    pub const fn size(&self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    pub const fn left(&self) -> f32 {
        self.x
    }

    pub const fn top(&self) -> f32 {
        self.y
    }

    pub const fn right(&self) -> f32 {
        self.x + self.width
    }

    pub const fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub const fn center(&self) -> Point {
        Point {
            x: self.x + self.width / 2.0,
            y: self.y + self.height / 2.0,
        }
    }

    /// Check if point is inside rectangle
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.right()
            && point.y >= self.y
            && point.y <= self.bottom()
    }

    /// Check if x, y coordinates are inside rectangle
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }

    /// Check if two rectangles intersect
    pub fn intersects(&self, other: &RectF) -> bool {
        self.left() < other.right()
            && self.right() > other.left()
            && self.top() < other.bottom()
            && self.bottom() > other.top()
    }

    /// Compute intersection of two rectangles
    pub fn intersection(&self, other: &RectF) -> Option<RectF> {
        if [
            self.x,
            self.y,
            self.width,
            self.height,
            other.x,
            other.y,
            other.width,
            other.height,
        ]
        .iter()
        .any(|value| !value.is_finite())
        {
            return None;
        }
        // Form logical far edges before binary32 rounding. Reconstructing
        // dimensions from rounded f32 edges can enlarge even an identical clip.
        let left = f64::from(self.x).max(f64::from(other.x));
        let top = f64::from(self.y).max(f64::from(other.y));
        let right = (f64::from(self.x) + f64::from(self.width))
            .min(f64::from(other.x) + f64::from(other.width));
        let bottom = (f64::from(self.y) + f64::from(self.height))
            .min(f64::from(other.y) + f64::from(other.height));
        if !(left < right && top < bottom) {
            return None;
        }
        // Greatest binary32 extent inside the logical intersection, with no
        // epsilon, outward edge rounding or change to either original rectangle.
        let inward = |extent: f64| {
            let packed = extent as f32;
            if f64::from(packed) > extent {
                packed.next_down()
            } else {
                packed
            }
        };
        Some(RectF::new(
            left as f32,
            top as f32,
            inward(right - left),
            inward(bottom - top),
        ))
    }

    /// Expand rectangle by padding on all sides
    pub fn expand(&self, padding: f32) -> RectF {
        RectF::new(
            self.x - padding,
            self.y - padding,
            self.width + 2.0 * padding,
            self.height + 2.0 * padding,
        )
    }

    /// Shrink rectangle by padding on all sides
    pub fn shrink(&self, padding: f32) -> RectF {
        self.expand(-padding)
    }

    /// Translate rectangle by offset
    pub fn translate(&self, dx: f32, dy: f32) -> RectF {
        RectF::new(self.x + dx, self.y + dy, self.width, self.height)
    }
}

/// Snap a logical coordinate to the nearest physical pixel boundary.
/// At 1x: round to integer. At 2x: round to nearest 0.5. At 1.5x: round to nearest 0.667.
#[inline]
pub fn snap_to_physical(v: f32, scale_factor: f32) -> f32 {
    (v * scale_factor).round() / scale_factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_contains() {
        let rect = RectF::new(10.0, 10.0, 100.0, 100.0);
        assert!(rect.contains(Point::new(50.0, 50.0)));
        assert!(!rect.contains(Point::new(5.0, 5.0)));
        assert!(!rect.contains(Point::new(150.0, 150.0)));
    }

    #[test]
    fn test_rect_intersection() {
        let r1 = RectF::new(0.0, 0.0, 100.0, 100.0);
        let r2 = RectF::new(50.0, 50.0, 100.0, 100.0);
        let intersection = r1.intersection(&r2).unwrap();
        assert_eq!(intersection, RectF::new(50.0, 50.0, 50.0, 50.0));
    }

    #[test]
    fn test_rect_intersection_exact_f32_nested_shrink() {
        // Independent arithmetic witness, not the unavailable original native
        // parent/child rectangles from JC's SIGABRT log.
        let rect = RectF::new(
            f32::from_bits(0x4515f513),
            f32::from_bits(0x4532533c),
            f32::from_bits(0x42893387),
            f32::from_bits(0x42d6f831),
        );
        let bits = |r: RectF| [r.x, r.y, r.width, r.height].map(f32::to_bits);
        let same = rect.intersection(&rect).unwrap();
        assert_eq!(bits(same), bits(rect));
        assert_eq!(same.width * same.height, rect.width * rect.height);

        let contained = |inside: RectF, outside: RectF| {
            assert!(inside.width >= 0.0 && inside.height >= 0.0);
            assert!(inside.width <= outside.width && inside.height <= outside.height);
            assert!(f64::from(inside.x) >= f64::from(outside.x));
            assert!(f64::from(inside.y) >= f64::from(outside.y));
            assert!(
                f64::from(inside.x) + f64::from(inside.width)
                    <= f64::from(outside.x) + f64::from(outside.width)
            );
            assert!(
                f64::from(inside.y) + f64::from(inside.height)
                    <= f64::from(outside.y) + f64::from(outside.height)
            );
            assert!(inside.width * inside.height <= outside.width * outside.height);
        };
        for (a, b) in [
            (rect, rect),
            (
                rect,
                RectF::new(rect.x + 24.0, rect.y + 32.0, rect.width, rect.height),
            ),
            (
                RectF::new(-10.25, -20.5, 80.75, 100.125),
                RectF::new(0.125, 0.25, 100.0, 100.0),
            ),
            (RectF::new(-100.0, -100.0, 200.0, 200.0), rect),
            (
                RectF::new(0.0, 0.0, 100.0, 100.0),
                RectF::new(50.0, 50.0, 100.0, 100.0),
            ),
        ] {
            let Some(intersection) = a.intersection(&b) else {
                assert!(
                    a.right() <= b.x || b.right() <= a.x || a.bottom() <= b.y || b.bottom() <= a.y
                );
                continue;
            };
            assert_eq!(bits(intersection), bits(b.intersection(&a).unwrap()));
            contained(intersection, a);
            contained(intersection, b);
            let width = (f64::from(a.x) + f64::from(a.width))
                .min(f64::from(b.x) + f64::from(b.width))
                - f64::from(intersection.x);
            let height = (f64::from(a.y) + f64::from(a.height))
                .min(f64::from(b.y) + f64::from(b.height))
                - f64::from(intersection.y);
            assert!(
                f64::from(intersection.width) <= width
                    && f64::from(intersection.width.next_up()) > width
            );
            assert!(
                f64::from(intersection.height) <= height
                    && f64::from(intersection.height.next_up()) > height
            );
            let mut nested = intersection;
            for _ in 0..64 {
                nested = nested.intersection(&intersection).unwrap();
                assert_eq!(bits(nested), bits(intersection));
                contained(nested, a);
                contained(nested, b);
            }
        }
        assert_eq!(
            RectF::new(0.0, 0.0, 10.0, 10.0).intersection(&RectF::new(5.0, 5.0, 10.0, 10.0)),
            Some(RectF::new(5.0, 5.0, 5.0, 5.0))
        );
        for empty in [
            RectF::new(10.0, 0.0, 1.0, 1.0), // touching, not overlapping
            RectF::new(20.0, 20.0, 1.0, 1.0),
            RectF::zero(),
            RectF::new(0.0, 0.0, -1.0, 1.0),
            RectF::new(f32::NAN, 0.0, 1.0, 1.0),
            RectF::new(0.0, 0.0, f32::INFINITY, 1.0),
        ] {
            let positive = RectF::new(0.0, 0.0, 10.0, 10.0);
            assert!(positive.intersection(&empty).is_none());
            assert!(empty.intersection(&positive).is_none());
        }
    }
}
