//! Private R-025 source qualification and controlled geometric-shadow family.
//! No application shader, light default, public source ingress or second output
//! owner is introduced. D=0 never constructs any object in this module.

use super::context::DynamicStorageLease3D;
use super::pbr::{MaterialSceneSource3D, PbrError3D};
use super::snapshot::CommittedMaterialPacket3D;
use super::{ModelPlacement3, PerspectiveFar, Projection3, ViewDescriptor3, WorldPoint3};
use bytemuck::{Pod, Zeroable};
use glam::{DMat3, DMat4, DVec3, DVec4};
use std::sync::Arc;

pub(crate) const POOL_BYTES: u64 = 54_525_952;
pub(crate) const NEAR: f64 = 0.01;

fn point64(p: WorldPoint3) -> DVec3 {
    DVec3::new(p.x(), p.y(), p.z())
}
fn point_bits(p: WorldPoint3) -> [u64; 3] {
    [p.x(), p.y(), p.z()].map(f64::to_bits)
}
fn orientation_bits(q: super::Orientation3) -> [u32; 4] {
    [q.x(), q.y(), q.z(), q.w()].map(f32::to_bits)
}

pub(super) fn same_view_bits(a: ViewDescriptor3, b: ViewDescriptor3) -> bool {
    point_bits(a.pose().translation()) == point_bits(b.pose().translation())
        && orientation_bits(a.pose().orientation()) == orientation_bits(b.pose().orientation())
        && match (a.projection(), b.projection()) {
            (Projection3::Perspective(a), Projection3::Perspective(b)) => {
                a.vertical_fov_radians().to_bits() == b.vertical_fov_radians().to_bits()
                    && a.near().to_bits() == b.near().to_bits()
                    && match (a.far(), b.far()) {
                        (PerspectiveFar::Finite(a), PerspectiveFar::Finite(b)) => {
                            a.to_bits() == b.to_bits()
                        }
                        (PerspectiveFar::Infinite, PerspectiveFar::Infinite) => true,
                        _ => false,
                    }
            }
            (Projection3::Orthographic(a), Projection3::Orthographic(b)) => {
                [a.vertical_span(), a.near(), a.far()].map(f32::to_bits)
                    == [b.vertical_span(), b.near(), b.far()].map(f32::to_bits)
            }
            _ => false,
        }
}

pub(super) fn orientation64(q: super::Orientation3) -> glam::DQuat {
    glam::DQuat::from_xyzw(
        f64::from(q.x()),
        f64::from(q.y()),
        f64::from(q.z()),
        f64::from(q.w()),
    )
}

fn source(field: &'static str, cause: &'static str) -> PbrError3D {
    PbrError3D::Direct { field, cause }
}
fn checked(v: f64, field: &'static str) -> Result<f32, PbrError3D> {
    let f = v as f32;
    if !v.is_finite() || !f.is_finite() || (v != 0.0 && f == 0.0) {
        return Err(source(field, "not finite/representable in f32"));
    }
    Ok(f)
}
fn unit(v: DVec3, field: &'static str) -> Result<DVec3, PbrError3D> {
    let m = v.abs().max_element();
    if !m.is_finite() || m == 0.0 {
        return Err(source(field, "undefined direction"));
    }
    let n = (v / m).normalize();
    if !n.is_finite() {
        return Err(source(field, "undefined unit direction"));
    }
    Ok(n)
}
fn matrix(m: DMat4) -> Result<[[f32; 4]; 4], PbrError3D> {
    let mut result = [[0.0; 4]; 4];
    for (dst, src) in result.iter_mut().zip(m.to_cols_array_2d()) {
        for (d, s) in dst.iter_mut().zip(src) {
            *d = checked(s, "shadow/metric matrix")?;
        }
    }
    Ok(result)
}

/// Directed binary32 enclosures. Every endpoint is a representable f32 value
/// promoted exactly to f64. Each operation includes both its exact result and
/// its rounded result, so contraction to FMA cannot escape the enclosure.
#[derive(Clone, Copy, Debug)]
struct F32Interval {
    lo: f64,
    hi: f64,
}

impl F32Interval {
    const ZERO: Self = Self { lo: 0.0, hi: 0.0 };
    const ONE: Self = Self { lo: 1.0, hi: 1.0 };

    fn rounded(lo: f64, hi: f64) -> Result<Self, PbrError3D> {
        if !lo.is_finite()
            || !hi.is_finite()
            || lo > hi
            || lo < -f64::from(f32::MAX)
            || hi > f64::from(f32::MAX)
        {
            return Err(source(
                "directional f32 coverage",
                "unbounded/overflowing IEEE interval",
            ));
        }
        let mut lower = lo as f32;
        let mut upper = hi as f32;
        if f64::from(lower) > lo {
            lower = lower.next_down();
        }
        if f64::from(upper) < hi {
            upper = upper.next_up();
        }
        let mut result = Self {
            lo: f64::from(lower),
            hi: f64::from(upper),
        };
        // Gradual underflow is enclosed by the directed endpoints. Also enclose
        // flushing a subnormal input/result to zero; no relative-error model is
        // silently applied in the subnormal range.
        let minimum = f64::from(f32::MIN_POSITIVE);
        if (lo != 0.0 && lo.abs() < minimum) || (hi != 0.0 && hi.abs() < minimum) {
            result.lo = result.lo.min(0.0);
            result.hi = result.hi.max(0.0);
        }
        if !result.lo.is_finite() || !result.hi.is_finite() {
            return Err(source(
                "directional f32 coverage",
                "nonfinite directed endpoint",
            ));
        }
        Ok(result)
    }

    fn scalar(v: f64) -> Result<Self, PbrError3D> {
        Self::rounded(v, v)
    }

    fn mul(self, rhs: Self) -> Result<Self, PbrError3D> {
        // Products of two promoted binary32 endpoints need at most48 significant
        // bits and are exact in binary64, including binary32 subnormal endpoints.
        let products = [
            self.lo * rhs.lo,
            self.lo * rhs.hi,
            self.hi * rhs.lo,
            self.hi * rhs.hi,
        ];
        Self::rounded(
            products.into_iter().fold(f64::INFINITY, f64::min),
            products.into_iter().fold(f64::NEG_INFINITY, f64::max),
        )
    }

    fn add(self, rhs: Self) -> Result<Self, PbrError3D> {
        let lo = self.lo + rhs.lo;
        let hi = self.hi + rhs.hi;
        // Account for the binary64 computation used to form the directed bound.
        // A zero sum of binary32 endpoints is exact cancellation (or exact0+0).
        Self::rounded(
            if lo == 0.0 { 0.0 } else { lo.next_down() },
            if hi == 0.0 { 0.0 } else { hi.next_up() },
        )
    }

    fn hull(self, rhs: Self) -> Self {
        Self {
            lo: self.lo.min(rhs.lo),
            hi: self.hi.max(rhs.hi),
        }
    }
}

fn interval_dot(row: [f32; 4], input: [F32Interval; 4]) -> Result<F32Interval, PbrError3D> {
    // All15 subsets bound every binary addition tree/order of the four terms.
    // Including the unrounded product/sum in each node also bounds FMA trees;
    // this is not certification of one host SIMD reduction order.
    let mut subsets = [F32Interval::ZERO; 16];
    for i in 0..4 {
        subsets[1 << i] = F32Interval::scalar(f64::from(row[i]))?.mul(input[i])?;
    }
    for mask in 1usize..16 {
        if mask.count_ones() == 1 {
            continue;
        }
        let mut enclosure: Option<F32Interval> = None;
        let mut left = (mask - 1) & mask;
        while left != 0 {
            let right = mask ^ left;
            if right != 0 {
                let value = subsets[left].add(subsets[right])?;
                enclosure = Some(enclosure.map_or(value, |old| old.hull(value)));
            }
            left = (left - 1) & mask;
        }
        subsets[mask] = enclosure.expect("nontrivial dot subset");
    }
    Ok(subsets[15])
}

fn triangle_depth_hull(
    row: [f32; 4],
    triangle: &[[F32Interval; 3]],
) -> Result<F32Interval, PbrError3D> {
    let hull = triangle[1..].iter().fold(triangle[0], |a, b| {
        [a[0].hull(b[0]), a[1].hull(b[1]), a[2].hull(b[2])]
    });
    interval_dot(row, [hull[0], hull[1], hull[2], F32Interval::ONE])
}

// The component hull contains each original corner interval. Directed add,
// multiply, hull and every subset of interval_dot preserve inclusion, including
// the existing subnormal/flush-to-zero enclosure. Thus strict containment of
// this larger dot enclosure is sufficient for all three original predicates.
// An error or inconclusive hull proves nothing: run the exact original corner
// dots in their original order, stopping at the same first failed predicate.
fn triangle_depth_certificate(row: [f32; 4], triangle: &[[F32Interval; 3]]) -> (bool, bool) {
    if triangle_depth_hull(row, triangle).is_ok_and(|z| z.lo > 0.0 && z.hi < 1.0) {
        return (true, true);
    }
    (
        triangle.iter().all(|p| {
            interval_dot(row, [p[0], p[1], p[2], F32Interval::ONE])
                .is_ok_and(|z| z.lo > 0.0 && z.hi < 1.0)
        }),
        false,
    )
}

const DEPTH_GROUP_TRIANGLES: usize = 16;
const DEPTH_GROUP_CORNERS: usize = 3 * DEPTH_GROUP_TRIANGLES;

fn triangle_sequence_depth_certificate(
    row: [f32; 4],
    corners: &[[F32Interval; 3]],
    #[cfg(any(test, feature = "test-harness"))] counts: &mut [u64; 7],
) -> Result<(), usize> {
    for (index, triangle) in corners.chunks(3).enumerate() {
        let (accepted, _hull) = triangle_depth_certificate(row, triangle);
        #[cfg(any(test, feature = "test-harness"))]
        {
            counts[if _hull { 1 } else { 2 }] += 1;
        }
        if !accepted {
            return Err(index);
        }
    }
    Ok(())
}

// The same componentwise inclusion proof applies to all corners of up to16
// complete adjacent triangles. Failure of this larger enclosure is not a source
// refusal: only the unchanged triangle sequence can reject, at its first member.
fn triangle_group_depth_certificate(
    row: [f32; 4],
    corners: &[[F32Interval; 3]],
    #[cfg(any(test, feature = "test-harness"))] counts: &mut [u64; 7],
) -> Result<(), usize> {
    if triangle_depth_hull(row, corners).is_ok_and(|z| z.lo > 0.0 && z.hi < 1.0) {
        #[cfg(any(test, feature = "test-harness"))]
        {
            counts[4] += 1;
            counts[6] += (corners.len() / 3) as u64;
        }
        return Ok(());
    }
    #[cfg(any(test, feature = "test-harness"))]
    {
        counts[5] += 1;
    }
    triangle_sequence_depth_certificate(
        row,
        corners,
        #[cfg(any(test, feature = "test-harness"))]
        counts,
    )
}

#[cfg(test)]
pub(crate) fn triangle_groups_depth_certificate_for_test(
    row: [f32; 4],
    endpoints: &[[[f64; 2]; 3]],
) -> (
    Result<(), usize>,
    [u64; 7],
    Vec<Result<[f64; 2], PbrError3D>>,
) {
    let corners = endpoints
        .iter()
        .map(|p| p.map(|[lo, hi]| F32Interval { lo, hi }))
        .collect::<Vec<_>>();
    let mut counts = [0; 7];
    let mut hulls = Vec::new();
    let mut result = Ok(());
    for (group, corners) in corners.chunks(3 * DEPTH_GROUP_TRIANGLES).enumerate() {
        hulls.push(triangle_depth_hull(row, corners).map(|z| [z.lo, z.hi]));
        if let Err(index) = triangle_group_depth_certificate(row, corners, &mut counts) {
            result = Err(group * DEPTH_GROUP_TRIANGLES + index);
            break;
        }
    }
    (result, counts, hulls)
}

#[cfg(test)]
pub(crate) fn triangle_depth_certificate_for_test(
    row: [f32; 4],
    endpoints: [[[f64; 2]; 3]; 3],
) -> (
    Result<[f64; 2], PbrError3D>,
    [Result<[f64; 2], PbrError3D>; 3],
    (bool, bool),
) {
    let triangle = endpoints.map(|p| p.map(|[lo, hi]| F32Interval { lo, hi }));
    (
        triangle_depth_hull(row, &triangle).map(|z| [z.lo, z.hi]),
        triangle
            .map(|p| interval_dot(row, [p[0], p[1], p[2], F32Interval::ONE]).map(|z| [z.lo, z.hi])),
        triangle_depth_certificate(row, &triangle),
    )
}

fn metric_position_intervals(
    columns: [[f32; 4]; 4],
    local: DVec3,
) -> Result<[F32Interval; 3], PbrError3D> {
    let input = [
        F32Interval::scalar(local.x)?,
        F32Interval::scalar(local.y)?,
        F32Interval::scalar(local.z)?,
        F32Interval::ONE,
    ];
    Ok([
        interval_dot(columns.map(|c| c[0]), input)?,
        interval_dot(columns.map(|c| c[1]), input)?,
        interval_dot(columns.map(|c| c[2]), input)?,
    ])
}

fn receiver_box(geometry: &super::geometry::PbrGeometrySource3D) -> Option<[F32Interval; 3]> {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    // Include unused vertices too. This immutable-source scan runs once per
    // packet needing the tighter certificate, never once per light or view.
    for vertex in geometry.vertices.iter() {
        let p = [
            vertex.position.x(),
            vertex.position.y(),
            vertex.position.z(),
        ];
        for i in 0..3 {
            let value = f64::from(p[i]);
            if !value.is_finite() {
                return None;
            }
            lo[i] = lo[i].min(value);
            hi[i] = hi[i].max(value);
        }
    }
    Some([
        F32Interval::rounded(lo[0], hi[0]).ok()?,
        F32Interval::rounded(lo[1], hi[1]).ok()?,
        F32Interval::rounded(lo[2], hi[2]).ok()?,
    ])
}

fn receiver_distance_lower(
    bounds: [F32Interval; 3],
    placement: ModelPlacement3,
    emitter: WorldPoint3,
    meters_per_scene_unit: f64,
) -> Option<f64> {
    let object = metric_object(placement, emitter, meters_per_scene_unit).ok()?;
    // Object112's existing first 64 bytes are the packed model columns.
    let columns: [[f32; 4]; 4] = bytemuck::pod_read_unaligned(&bytemuck::bytes_of(&object)[..64]);
    let input = [bounds[0], bounds[1], bounds[2], F32Interval::ONE];
    let mut squared = 0.0f64;
    for axis in 0..3 {
        let interval = interval_dot(columns.map(|col| col[axis]), input).ok()?;
        let gap = if interval.lo > 0.0 {
            interval.lo
        } else if interval.hi < 0.0 {
            -interval.hi
        } else {
            0.0
        };
        // Gaps are promoted binary32 endpoints: their squares are exact in
        // binary64. Direct every subsequent sum and the correctly rounded
        // binary64 sqrt downward; zero remains an exact lower bound.
        let sum = squared + gap * gap;
        if !sum.is_finite() || sum < 0.0 {
            return None;
        }
        squared = if sum == 0.0 { 0.0 } else { sum.next_down() };
    }
    let distance = squared.sqrt();
    distance.is_finite().then(|| {
        if distance == 0.0 {
            0.0
        } else {
            distance.next_down()
        }
    })
}

fn nonnegative_upper(value: f64) -> Option<f64> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let upper = if value == 0.0 { 0.0 } else { value.next_up() };
    upper.is_finite().then_some(upper)
}

#[cfg(test)]
pub(crate) struct CascadeDepthProof3D {
    pub(crate) t: f64,
    pub(crate) row: [f32; 4],
    pub(crate) range: [f64; 2],
    pub(crate) corners: [[f64; 2]; 8],
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Directional3D {
    pub(crate) surface_to_source: [f64; 3],
    pub(crate) irradiance: [f32; 3],
    pub(crate) shadow_far_m: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PointLight3D {
    pub(crate) position: WorldPoint3,
    pub(crate) intensity: [f32; 3],
    pub(crate) radius_m: f64,
    pub(crate) range_m: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpotLight3D {
    pub(crate) point: PointLight3D,
    pub(crate) emitter_to_scene: [f64; 3],
    pub(crate) inner_radians: f64,
    pub(crate) outer_radians: f64,
}

/// One complete immutable extension generation. The same captured snapshot and
/// model are the receiver and complete caster set; no camera-culling subset is
/// built. Only the existing private fixture ingress can currently submit this.
pub(crate) struct DirectSource3D {
    pub(crate) identity: u64,
    pub(crate) revision: u64,
    pub(crate) meters_per_scene_unit: f64,
    pub(crate) directional: Option<Directional3D>,
    pub(crate) points: Arc<[PointLight3D]>,
    pub(crate) spots: Arc<[SpotLight3D]>,
    pub(crate) snapshot: Arc<super::snapshot::MaterialSnapshot3D>,
    pub(crate) model: Arc<super::snapshot::MaterialModelState3D>,
}
impl std::fmt::Debug for DirectSource3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectSource3D")
            .field("identity", &self.identity)
            .field("revision", &self.revision)
            .field("k", &self.meters_per_scene_unit)
            .finish()
    }
}

#[cfg(test)]
std::thread_local! {
    static FULL_DIRECT_VALIDATIONS: std::cell::Cell<(usize, usize)> = const {
        std::cell::Cell::new((0, 0))
    };
}

impl DirectSource3D {
    #[cfg(test)]
    pub(crate) fn full_validation_counts_for_test() -> (usize, usize) {
        FULL_DIRECT_VALIDATIONS.with(std::cell::Cell::get)
    }

    pub(crate) fn validate(&self, scene: &MaterialSceneSource3D) -> Result<(), PbrError3D> {
        #[cfg(test)]
        FULL_DIRECT_VALIDATIONS.with(|count| {
            let (validations, scans) = count.get();
            count.set((validations + 1, scans));
        });
        if self.identity == 0
            || self.revision == 0
            || !self.meters_per_scene_unit.is_finite()
            || self.meters_per_scene_unit <= 0.0
        {
            return Err(source(
                "source/calibration",
                "missing identity/revision or positive finite authored k",
            ));
        }
        if !scene
            .direct
            .as_ref()
            .is_some_and(|captured| std::ptr::eq(self, captured.as_ref()))
        {
            return Err(source(
                "source provenance",
                "direct instance is not the exact captured source",
            ));
        }
        if self.points.len() > 4
            || self.spots.len() > 4
            || (self.directional.is_none() && self.points.is_empty() && self.spots.is_empty())
        {
            return Err(source(
                "counts",
                "D present requires1..9 sources within1/4/4 slots",
            ));
        }
        if !Arc::ptr_eq(&self.snapshot, &scene.snapshot) || !Arc::ptr_eq(&self.model, &scene.model)
        {
            return Err(source(
                "casters",
                "complete caster/receiver snapshot/model generation mismatch",
            ));
        }
        let rgb = |v: [f32; 3]| -> Result<(), PbrError3D> {
            if v.iter()
                .all(|c| c.is_finite() && (0.0..=4096.0).contains(c))
            {
                Ok(())
            } else {
                Err(source("light RGB", "outside finite [0,4096]"))
            }
        };
        let direction = |v: [f64; 3]| -> Result<(), PbrError3D> {
            let n = DVec3::from_array(v);
            // Same explicit producer-unit qualification used by the material
            // source; normalize only after this source property is established.
            super::geometry::PbrUnitDirection3D::try_new(v)?;
            let f = n.to_array().map(|v| checked(v, "light direction"));
            for c in f {
                c?;
            }
            Ok(())
        };
        let point = |p: &PointLight3D| -> Result<(), PbrError3D> {
            rgb(p.intensity)?;
            if !p.radius_m.is_finite()
                || !(0.01..=0.5).contains(&p.radius_m)
                || !p.range_m.is_finite()
                || p.range_m <= p.radius_m
                || p.range_m > 50.0
            {
                return Err(source(
                    "point/spot radius/range",
                    "requires .01<=r<=.5 and r<R<=50 metres",
                ));
            }
            let radius = checked(p.radius_m, "radius")?;
            let range = checked(p.range_m, "range")?;
            if range <= radius {
                return Err(source(
                    "point/spot radius/range",
                    "strict support interval collapsed after f32 packing",
                ));
            }
            Ok(())
        };
        if let Some(d) = &self.directional {
            rgb(d.irradiance)?;
            direction(d.surface_to_source)?;
            if !d.shadow_far_m.is_finite() || d.shadow_far_m <= 0.0 || d.shadow_far_m > 50.0 {
                return Err(source(
                    "directional far",
                    "outside positive finite <=50 metres",
                ));
            }
        }
        for p in self.points.iter() {
            point(p)?;
        }
        for s in self.spots.iter() {
            point(&s.point)?;
            direction(s.emitter_to_scene)?;
            if !s.inner_radians.is_finite()
                || !s.outer_radians.is_finite()
                || s.inner_radians < 0.0
                || s.inner_radians >= s.outer_radians
                || s.outer_radians >= std::f64::consts::FRAC_PI_2
            {
                return Err(source("spot cone", "requires0<=inner<outer<pi/2"));
            }
            let ci = checked(s.inner_radians.cos(), "spot inner cosine")?;
            let co = checked(s.outer_radians.cos(), "spot outer cosine")?;
            if co <= 0.0 || ci <= co {
                return Err(source(
                    "spot cone",
                    "cosines not strictly ordered after f32 conversion",
                ));
            }
        }
        // A complete conservative envelope, not a per-pixel estimate. For each
        // material, c,F<=1 and diffuse*cos<=1. With a2=r_eff^4 and nh in[0,1],
        // d=(1-nh^2)+a2*nh^2>=a2, hence D<=1/(pi*r_eff^4). Cancelled Smith
        // gives vl>=2*nl and vv>=r_eff^2, so spec*cos<=1/(2*pi*r_eff^6)
        // <1/(6*r_eff^6). The latter leaves pi/3-1 (>4.7%) coefficient
        // headroom; it does not replace the existing source/normal, actual-host
        // and supported-backend numeric qualifications with an ideal-math claim.
        // Sum EACH source's irradiance bound and independent emission, retaining
        // the guarded zero/noncontributing branches and visibility<=1.
        let mut incident = [0.0f64; 3];
        if let Some(d) = &self.directional {
            for c in 0..3 {
                incident[c] += f64::from(d.irradiance[c]);
            }
        }
        for p in self
            .points
            .iter()
            .chain(self.spots.iter().map(|s| &s.point))
        {
            for c in 0..3 {
                incident[c] += f64::from(p.intensity[c]) / (p.radius_m * p.radius_m);
            }
        }
        for (packet_index, packet) in scene.snapshot.packets.iter().enumerate() {
            let CommittedMaterialPacket3D::PbrMaterial {
                geometry, material, ..
            } = packet
            else {
                return Err(source(
                    "D material family",
                    "mixed legacy positional shading is unsupported",
                ));
            };
            let m = &material.data.source;
            let min_g = if m.roles[2].is_some() {
                #[cfg(test)]
                FULL_DIRECT_VALIDATIONS.with(|count| {
                    let (validations, scans) = count.get();
                    count.set((validations, scans + 1));
                });
                material.data.textures[2]
                    .descriptor()
                    .texels()
                    .chunks_exact(4)
                    .map(|t| f64::from(t[1]) / 255.0)
                    .fold(1.0f64, f64::min)
            } else {
                1.0
            };
            let re = (f64::from(m.roughness) * min_g).max(0.045);
            let response = 1.0 + 1.0 / (6.0 * re.powi(6));
            if (0..3).all(|c| {
                let bound = response * incident[c] + f64::from(m.emissive_rgb[c]);
                bound.is_finite() && bound <= f64::from(scene.hdr.maximum_rgb[c])
            }) {
                // Preserve every old positive, with no receiver geometry work.
                continue;
            }
            let refined = self
                .model
                .placements
                .get(packet_index)
                .and_then(|placement| {
                    let bounds = receiver_box(&geometry.data.source)?;
                    self.receiver_incident_upper(bounds, *placement)
                });
            if !refined.is_some_and(|incident| {
                (0..3).all(|c| {
                    nonnegative_upper(response * incident[c])
                        .and_then(|value| nonnegative_upper(value + f64::from(m.emissive_rgb[c])))
                        .is_some_and(|bound| bound <= f64::from(scene.hdr.maximum_rgb[c]))
                })
            }) {
                return Err(source(
                    "radiance certificate",
                    "complete BRDF*cos*all-source+emission envelope exceeds producer certificate",
                ));
            }
        }
        Ok(())
    }

    fn receiver_incident_upper(
        &self,
        bounds: [F32Interval; 3],
        placement: ModelPlacement3,
    ) -> Option<[f64; 3]> {
        let mut incident = self
            .directional
            .as_ref()
            .map_or([0.0; 3], |d| d.irradiance.map(f64::from));
        for light in self
            .points
            .iter()
            .chain(self.spots.iter().map(|spot| &spot.point))
        {
            let radius_squared = (light.radius_m * light.radius_m).next_down();
            let distance_squared = receiver_distance_lower(
                bounds,
                placement,
                light.position,
                self.meters_per_scene_unit,
            )
            .filter(|distance| *distance > 0.0)
            .map(|distance| (distance * distance).next_down().max(0.0))
            .unwrap_or(0.0);
            // Missing/zero separation retains I/radius². Do not use cone,
            // support cutoff or shadow visibility to reduce this certificate.
            let denominator = radius_squared.max(distance_squared);
            if !denominator.is_finite() || denominator <= 0.0 {
                return None;
            }
            for c in 0..3 {
                let contribution = nonnegative_upper(f64::from(light.intensity[c]) / denominator)?;
                incident[c] = nonnegative_upper(incident[c] + contribution)?;
            }
        }
        Some(incident)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct Light64 {
    pub(crate) a: [f32; 4],
    pub(crate) b: [f32; 4],
    pub(crate) c: [f32; 4],
    pub(crate) d: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct LightBlock608 {
    pub(crate) counts: [u32; 4],
    pub(crate) splits: [f32; 4],
    pub(crate) directional: Light64,
    pub(crate) points: [Light64; 4],
    pub(crate) spots: [Light64; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct ShadowBlock1984 {
    pub(crate) matrices: [[[f32; 4]; 4]; 31],
}
const _: [(); 608] = [(); std::mem::size_of::<LightBlock608>()];
const _: [(); 1984] = [(); std::mem::size_of::<ShadowBlock1984>()];

pub(crate) fn metric_object(
    placement: ModelPlacement3,
    origin: WorldPoint3,
    k: f64,
) -> Result<super::appearance::ObjectBlockBytes, PbrError3D> {
    placement.checked_transform(origin)?;
    let scale = placement.scale();
    let s = DVec3::new(
        f64::from(scale.x()),
        f64::from(scale.y()),
        f64::from(scale.z()),
    ) * k;
    if !s.is_finite() || s.min_element() <= 0.0 {
        return Err(source("metric model", "invalid positive metric scale"));
    }
    let q = orientation64(placement.orientation());
    let t = (point64(placement.translation()) - point64(origin)) * k;
    let model = DMat4::from_scale_rotation_translation(s, q, t);
    let linear = DMat3::from_quat(q) * DMat3::from_diagonal(s);
    let normal = linear.inverse().transpose();
    let mc = matrix(model)?;
    let mut nc = [[0.0f32; 4]; 3];
    for (dst, col) in nc.iter_mut().zip(normal.to_cols_array_2d()) {
        for j in 0..3 {
            dst[j] = checked(col[j], "true metric normal matrix")?;
        }
    }
    let lf = glam::Mat3::from_cols_array_2d(&[
        mc[0][..3].try_into().unwrap(),
        mc[1][..3].try_into().unwrap(),
        mc[2][..3].try_into().unwrap(),
    ]);
    if !lf.determinant().is_finite() || lf.determinant() <= 0.0 {
        return Err(source(
            "metric model",
            "f32 positive invertibility is unqualified",
        ));
    }
    Ok(super::appearance::ObjectBlockBytes::new(mc, nc))
}

pub(crate) fn metric_view(
    view: ViewDescriptor3,
    origin: WorldPoint3,
    k: f64,
    width: u32,
    height: u32,
) -> Result<super::appearance::ViewBlockBytes, PbrError3D> {
    if width == 0 || height == 0 {
        return Err(source("metric view", "inactive extent"));
    }
    let camera = (point64(view.pose().translation()) - point64(origin)) * k;
    let q = orientation64(view.pose().orientation());
    let v = DMat4::from_rotation_translation(q.conjugate(), -(q.conjugate() * camera));
    let aspect = f64::from(width) / f64::from(height);
    let p = match view.projection() {
        Projection3::Perspective(p) => {
            let n = f64::from(p.near()) * k;
            let (a, b) = match p.far() {
                PerspectiveFar::Finite(f) => {
                    let f = f64::from(f) * k;
                    (n / (f - n), n * f / (f - n))
                }
                PerspectiveFar::Infinite => (0.0, n),
            };
            let focal = 1.0 / (f64::from(p.vertical_fov_radians()) * 0.5).tan();
            DMat4::from_cols(
                DVec4::new(focal / aspect, 0.0, 0.0, 0.0),
                DVec4::new(0.0, focal, 0.0, 0.0),
                DVec4::new(0.0, 0.0, a, -1.0),
                DVec4::new(0.0, 0.0, b, 0.0),
            )
        }
        Projection3::Orthographic(p) => {
            let n = f64::from(p.near()) * k;
            let f = f64::from(p.far()) * k;
            let y = 2.0 / (f64::from(p.vertical_span()) * k);
            DMat4::from_cols(
                DVec4::new(y / aspect, 0.0, 0.0, 0.0),
                DVec4::new(0.0, y, 0.0, 0.0),
                DVec4::new(0.0, 0.0, 1.0 / (f - n), 0.0),
                DVec4::new(0.0, 0.0, f / (f - n), 1.0),
            )
        }
    };
    let c = camera.to_array().map(|v| checked(v, "metric camera"));
    Ok(super::appearance::ViewBlockBytes::new(
        matrix(p * v)?,
        [c[0].clone()?, c[1].clone()?, c[2].clone()?],
    ))
}

pub(crate) fn cascade_splits(near: f64, far: f64) -> Result<[f32; 4], PbrError3D> {
    if !near.is_finite() || !far.is_finite() || near <= 0.0 || near >= far || far > 50.0 {
        return Err(source(
            "directional interval",
            "requires0<metric view near<shadow_far<=50",
        ));
    }
    let mut s = [0.0; 4];
    for j in 1..=3 {
        let t = j as f64 / 3.0;
        s[j - 1] = checked(
            0.5 * near * (far / near).powf(t) + 0.5 * (near + (far - near) * t),
            "cascade split",
        )?;
    }
    s[3] = checked(far, "shadow far")?;
    if !(s[0] > near as f32 && s[1] > s[0] && s[2] > s[1]) {
        return Err(source(
            "cascade split",
            "f32 boundaries not strictly increasing",
        ));
    }
    Ok(s)
}

pub(crate) fn point_face(q: DVec3) -> usize {
    let a = q.abs();
    if a.x >= a.y && a.x >= a.z {
        if q.x >= 0.0 { 0 } else { 1 }
    } else if a.y >= a.z {
        if q.y >= 0.0 { 2 } else { 3 }
    } else if q.z >= 0.0 {
        4
    } else {
        5
    }
}
pub(crate) fn point_taps(q: DVec3) -> Result<[DVec3; 9], PbrError3D> {
    let q = unit(q, "point PCF base")?;
    let u = [-DVec3::Z, DVec3::Z, DVec3::X, DVec3::X, DVec3::X, -DVec3::X][point_face(q)];
    let t = unit(u - u.dot(q) * q, "point chart T")?;
    let b = t.cross(q);
    let mut taps = [DVec3::ZERO; 9];
    for y in -1..=1 {
        for x in -1..=1 {
            taps[((y + 1) * 3 + x + 1) as usize] = unit(
                q + (2.0 / 512.0) * (x as f64 * t + y as f64 * b),
                "point PCF tap",
            )?;
        }
    }
    Ok(taps)
}

pub(crate) struct DirectFrame3D {
    pub(crate) light: LightBlock608,
    pub(crate) transforms: ShadowBlock1984,
    pub(crate) active_faces: Vec<usize>,
    #[cfg(test)]
    pub(crate) depth_proofs: Vec<CascadeDepthProof3D>,
}

// Consecutive clock boundaries partition one opt-in CPU call. The bookkeeping
// after a boundary is included in the following segment; no timer runs per
// source triangle. No clock or record exists in an ordinary production build.
#[cfg(any(test, feature = "test-harness"))]
struct DirectProjectionClock3D {
    last: std::time::Instant,
    phases: [std::time::Duration; 13],
}

#[cfg(any(test, feature = "test-harness"))]
impl DirectProjectionClock3D {
    const VALIDATION: usize = 0;
    const BASIS: usize = 1;
    const RECEIVERS: usize = 2;
    const FIT: usize = 3;
    const DEPTH: usize = 6;
    const XY: usize = 9;
    const OTHER: usize = 12;

    fn new() -> Self {
        Self {
            last: std::time::Instant::now(),
            phases: [std::time::Duration::ZERO; 13],
        }
    }

    fn mark(&mut self, phase: usize) {
        let now = std::time::Instant::now();
        self.phases[phase] += now.duration_since(self.last);
        self.last = now;
    }

    fn finish(
        mut self,
        timing: &mut super::raster::PbrPreparationTiming3D,
        source_triangles: usize,
        receiver_triangles: usize,
        cascades: usize,
        counts: [u64; 4],
    ) {
        self.mark(Self::OTHER);
        timing.direct_source_validation += self.phases[Self::VALIDATION];
        timing.direct_source_basis += self.phases[Self::BASIS];
        timing.direct_receiver_clipping += self.phases[Self::RECEIVERS];
        for i in 0..3 {
            timing.direct_cascade_fit[i] += self.phases[Self::FIT + i];
            timing.direct_depth_certification[i] += self.phases[Self::DEPTH + i];
            timing.direct_xy_pcf[i] += self.phases[Self::XY + i];
        }
        timing.direct_light_other += self.phases[Self::OTHER];
        timing.direct_projection_body += self.phases.iter().copied().sum::<std::time::Duration>();
        timing.direct_source_triangles += source_triangles;
        timing.direct_receiver_triangles += receiver_triangles;
        timing.direct_cascades += cascades;
        timing.direct_candidate_attempts += counts[0];
        timing.direct_fast_triangle_visits += counts[1];
        timing.direct_fallback_triangle_visits += counts[2];
        timing.direct_frustum_corner_tests += counts[3];
    }
}

/// Immutable results of the view-independent source/metric checks. The strong
/// source capture pins its exact snapshot, placements, resource generations and
/// context; this is neither a digest receipt nor a camera/cascade cache.
pub(crate) struct QualifiedDirectSource3D {
    source: Arc<DirectSource3D>,
    origin_bits: [u64; 3],
    k_bits: u64,
    validated_radiance: ValidatedDirectRadiance3D,
    source_corners: usize,
    geometry: QualifiedGeometry3D,
}

#[derive(Default)]
struct DirectGeometryArrays3D {
    triangles: Vec<[DVec3; 3]>,
    metric_intervals: Vec<[F32Interval; 3]>,
}

impl DirectGeometryArrays3D {
    fn with_capacity(corners: usize, directional: bool) -> Self {
        Self {
            triangles: Vec::with_capacity(corners / 3),
            metric_intervals: Vec::with_capacity(if directional { corners } else { 0 }),
        }
    }
}

enum QualifiedGeometry3D {
    Flat(DirectGeometryArrays3D),
    Slots(Vec<Arc<QualifiedDirectSlot3D>>),
}

impl QualifiedGeometry3D {
    fn triangles(&self) -> impl Iterator<Item = &[DVec3; 3]> {
        let (flat, slots): (&[_], &[_]) = match self {
            Self::Flat(geometry) => (&geometry.triangles, &[]),
            Self::Slots(slots) => (&[], slots),
        };
        flat.iter()
            .chain(slots.iter().flat_map(|slot| &slot.geometry.triangles))
    }

    #[cfg(test)]
    fn intervals(&self) -> impl Iterator<Item = &[F32Interval; 3]> {
        let (flat, slots): (&[_], &[_]) = match self {
            Self::Flat(geometry) => (&geometry.metric_intervals, &[]),
            Self::Slots(slots) => (&[], slots),
        };
        flat.iter().chain(
            slots
                .iter()
                .flat_map(|slot| &slot.geometry.metric_intervals),
        )
    }

    fn all_depth_groups(&self, mut check: impl FnMut(&[[F32Interval; 3]]) -> bool) -> bool {
        match self {
            Self::Flat(geometry) => geometry
                .metric_intervals
                .chunks(DEPTH_GROUP_CORNERS)
                .all(check),
            Self::Slots(slots) => {
                // Global groups retain the original first/last corner even when
                // they cross packet boundaries. Only one group is staged.
                let mut group = [[F32Interval::ZERO; 3]; DEPTH_GROUP_CORNERS];
                let mut used = 0;
                for slot in slots {
                    let mut remaining = slot.geometry.metric_intervals.as_slice();
                    while !remaining.is_empty() {
                        let count = remaining.len().min(group.len() - used);
                        group[used..used + count].copy_from_slice(&remaining[..count]);
                        remaining = &remaining[count..];
                        used += count;
                        if used == group.len() {
                            if !check(&group) {
                                return false;
                            }
                            used = 0;
                        }
                    }
                }
                used == 0 || check(&group[..used])
            }
        }
    }
}

/// Valid only inside a basis pinned to the exact same snapshot. Its packet
/// leases authenticate the handle generations and their source interpretation.
#[derive(PartialEq, Eq)]
struct DirectSlotKey3D {
    index: usize,
    geometry: (u32, u64),
    material: (u32, u64),
    raster: super::RasterMode,
    translation: [u64; 3],
    orientation: [u32; 4],
    scale: [u32; 3],
}

impl DirectSlotKey3D {
    fn new(
        index: usize,
        packet: &CommittedMaterialPacket3D,
        placement: ModelPlacement3,
    ) -> Option<(Self, usize)> {
        let CommittedMaterialPacket3D::PbrMaterial {
            geometry,
            material,
            raster,
            ..
        } = packet
        else {
            return None;
        };
        let scale = placement.scale();
        Some((
            Self {
                index,
                geometry: geometry.handle.0.slot_generation(),
                material: material.handle.0.slot_generation(),
                raster: *raster,
                translation: point_bits(placement.translation()),
                orientation: orientation_bits(placement.orientation()),
                scale: [scale.x(), scale.y(), scale.z()].map(f32::to_bits),
            },
            geometry.data.source.element_count(),
        ))
    }
}

struct QualifiedDirectSlot3D {
    key: DirectSlotKey3D,
    geometry: DirectGeometryArrays3D,
    _cpu: DynamicStorageLease3D,
}

impl QualifiedDirectSlot3D {
    fn bytes(corners: usize, directional: bool) -> Option<u64> {
        let arc = std::alloc::Layout::new::<[usize; 2]>()
            .extend(std::alloc::Layout::new::<Self>())
            .ok()?
            .0
            .pad_to_align();
        QualifiedDirectSource3D::payload_bytes(corners as u64, directional)
            .ok()?
            .checked_add(arc.size() as u64)
    }

    fn table_bytes(count: usize, directional: bool) -> Option<u64> {
        let bytes = std::alloc::Layout::array::<Arc<Self>>(count).ok()?.size() as u64;
        // Reserve one stack group for each slot-backed basis's frame proof.
        // As with the existing fit reserve, it covers future view proofs of
        // the same retained basis and is separate from shared chunk payload.
        bytes.checked_add(if directional {
            std::mem::size_of::<[[F32Interval; 3]; DEPTH_GROUP_CORNERS]>() as u64
        } else {
            0
        })
    }

    fn new(
        key: DirectSlotKey3D,
        geometry: DirectGeometryArrays3D,
        charge: &mut DynamicStorageLease3D,
    ) -> Arc<Self> {
        let bytes = Self::bytes(
            geometry.triangles.capacity() * 3,
            !geometry.metric_intervals.is_empty(),
        )
        .expect("qualified slot storage was preflighted");
        debug_assert!(
            geometry.metric_intervals.capacity() == 0
                || geometry.metric_intervals.capacity() == geometry.triangles.capacity() * 3
        );
        Arc::new(Self {
            key,
            geometry,
            _cpu: charge.split_retained_cpu(bytes),
        })
    }
}

/// The target has authenticated the predecessor and reserved all new CPU bytes.
/// This borrow never outlives synchronous frame preparation.
pub(super) struct ReservedDirectSlots3D<'a> {
    predecessor: &'a [Arc<QualifiedDirectSlot3D>],
    charge: DynamicStorageLease3D,
    #[cfg(any(test, feature = "test-harness"))]
    bytes: u64,
}

impl ReservedDirectSlots3D<'_> {
    pub(super) fn into_table_charge(self) -> DynamicStorageLease3D {
        self.charge
    }
}

/// The successful full source proof's captured HDR inputs, not a view proof.
#[derive(Clone, Copy)]
struct ValidatedDirectRadiance3D {
    hdr_identity: u64,
    hdr_revision: u64,
    exposure_bits: u32,
    maximum_rgb_bits: [u32; 3],
    samples: u32,
}

impl QualifiedDirectSource3D {
    #[cfg(test)]
    pub(super) fn slot_preflight_for_test(
        &self,
        context: &super::Context3D,
        previous: &MaterialSceneSource3D,
        scene: &MaterialSceneSource3D,
        origin: WorldPoint3,
        holder_bytes: u64,
    ) -> bool {
        let full =
            Self::payload_bytes(self.source_corners as u64, self.source.directional.is_some())
                .unwrap()
                + holder_bytes;
        self.reserve_slot_successor(context, previous, scene, origin, holder_bytes, full)
            .is_some()
    }

    #[cfg(test)]
    pub(super) fn assert_full_parity_for_test(
        &self,
        scene: &MaterialSceneSource3D,
        view: ViewDescriptor3,
        origin: WorldPoint3,
        extent: (u32, u32),
        actual: &DirectFrame3D,
    ) -> (usize, usize) {
        let direct = scene.direct.as_ref().unwrap();
        assert!(self.matches_validated_radiance(direct, &scene.snapshot.context, scene, origin));
        let (expected, flat) = direct
            .prepare_frame_source::<true, false, true>(
                scene, view, origin, extent.0, extent.1, None, None, None, None,
            )
            .unwrap();
        let flat = flat.unwrap();
        assert_eq!(self.source_corners, flat.source_corners);
        let triangle_bits =
            |triangle: &[DVec3; 3]| triangle.map(|p| p.to_array().map(f64::to_bits));
        let interval_bits =
            |point: &[F32Interval; 3]| point.map(|c| [c.lo.to_bits(), c.hi.to_bits()]);
        assert!(
            self.geometry
                .triangles()
                .map(triangle_bits)
                .eq(flat.geometry.triangles().map(triangle_bits))
        );
        assert!(
            self.geometry
                .intervals()
                .map(interval_bits)
                .eq(flat.geometry.intervals().map(interval_bits))
        );
        assert_eq!(
            bytemuck::bytes_of(&actual.light),
            bytemuck::bytes_of(&expected.light)
        );
        assert_eq!(
            bytemuck::bytes_of(&actual.transforms),
            bytemuck::bytes_of(&expected.transforms)
        );
        assert_eq!(actual.active_faces, expected.active_faces);
        let depth_bits = |proof: &CascadeDepthProof3D| {
            (
                proof.t.to_bits(),
                proof.row.map(f32::to_bits),
                proof.range.map(f64::to_bits),
                proof.corners.map(|p| p.map(f64::to_bits)),
            )
        };
        assert!(
            actual
                .depth_proofs
                .iter()
                .map(depth_bits)
                .eq(expected.depth_proofs.iter().map(depth_bits))
        );
        (
            self.geometry.triangles().count(),
            self.geometry.intervals().count(),
        )
    }

    #[cfg(test)]
    pub(crate) fn check_retired_cpu_budget_for_test(
        context: &super::Context3D,
    ) -> Result<(), super::Context3DError> {
        // Exercise the existing Context3D accounting boundary without exposing
        // its private lease or adding a second resource counter for tests.
        drop(context.reserve_dynamic_storage(
            0,
            0,
            0,
            context.limits().policy.max_retained_dynamic_cpu_bytes,
        )?);
        Ok(())
    }

    pub(crate) fn matches(
        &self,
        context: &super::Context3D,
        scene: &MaterialSceneSource3D,
        origin: WorldPoint3,
    ) -> bool {
        self.source.snapshot.context.same_resource_family(context)
            && scene.snapshot.context.same_resource_family(context)
            && scene.direct.as_ref().is_some_and(|direct| {
                Arc::ptr_eq(&self.source, direct)
                    && self.source.identity == direct.identity
                    && self.source.revision == direct.revision
                    && self.k_bits == direct.meters_per_scene_unit.to_bits()
            })
            && Arc::ptr_eq(&self.source.snapshot, &scene.snapshot)
            && Arc::ptr_eq(&self.source.model, &scene.model)
            && self.origin_bits == [origin.x(), origin.y(), origin.z()].map(f64::to_bits)
    }

    pub(crate) fn matches_validated_radiance(
        &self,
        direct: &DirectSource3D,
        context: &super::Context3D,
        scene: &MaterialSceneSource3D,
        origin: WorldPoint3,
    ) -> bool {
        std::ptr::eq(direct, self.source.as_ref())
            && self.matches(context, scene, origin)
            && self.validated_radiance.hdr_identity == scene.hdr.identity
            && self.validated_radiance.hdr_revision == scene.hdr.revision
            && self.validated_radiance.exposure_bits == scene.hdr.exposure_ev.to_bits()
            && self.validated_radiance.maximum_rgb_bits == scene.hdr.maximum_rgb.map(f32::to_bits)
            && self.validated_radiance.samples == scene.samples
    }

    pub(crate) fn payload_bytes(corners: u64, directional: bool) -> Result<u64, PbrError3D> {
        let per_corner = std::mem::size_of::<DVec3>()
            + if directional {
                std::mem::size_of::<[F32Interval; 3]>()
            } else {
                0
            };
        corners.checked_mul(per_corner as u64).ok_or(source(
            "directional source storage",
            "qualified source byte count overflow",
        ))
    }

    pub(super) fn reserve_slot_successor(
        &self,
        context: &super::Context3D,
        previous: &MaterialSceneSource3D,
        scene: &MaterialSceneSource3D,
        origin: WorldPoint3,
        holder_bytes: u64,
        full_bytes: u64,
    ) -> Option<ReservedDirectSlots3D<'_>> {
        let QualifiedGeometry3D::Slots(predecessor) = &self.geometry else {
            return None;
        };
        let direct = scene.direct.as_ref()?;
        if !self.matches(context, previous, origin)
            || !Arc::ptr_eq(&scene.snapshot, &previous.snapshot)
            || !Arc::ptr_eq(&scene.model.snapshot, &scene.snapshot)
            || !Arc::ptr_eq(&direct.snapshot, &scene.snapshot)
            || !Arc::ptr_eq(&direct.model, &scene.model)
            || scene.model.revision <= previous.model.revision
            || direct.revision <= self.source.revision
            || scene.samples != previous.samples
            || scene.hdr.identity != previous.hdr.identity
            || scene.hdr.revision <= previous.hdr.revision
            || scene.hdr.exposure_ev.to_bits() != previous.hdr.exposure_ev.to_bits()
            || scene.hdr.maximum_rgb.map(f32::to_bits) != previous.hdr.maximum_rgb.map(f32::to_bits)
            || !match (&scene.indirect, &previous.indirect) {
                (None, None) => true,
                (Some(current), Some(old)) => Arc::ptr_eq(current, old),
                _ => false,
            }
            || direct.meters_per_scene_unit.to_bits() != self.k_bits
            || !direct.same_authored_lights(&self.source)
            || predecessor.len() != scene.snapshot.packets.len()
            || predecessor.len() != scene.model.placements.len()
        {
            return None;
        }

        let directional = direct.directional.is_some();
        let mut bytes = holder_bytes.checked_add(QualifiedDirectSlot3D::table_bytes(
            predecessor.len(),
            directional,
        )?)?;
        for (index, ((packet, placement), old)) in scene
            .snapshot
            .packets
            .iter()
            .zip(scene.model.placements.iter())
            .zip(predecessor)
            .enumerate()
        {
            let (key, corners) = DirectSlotKey3D::new(index, packet, *placement)?;
            if old.key != key {
                bytes = bytes.checked_add(QualifiedDirectSlot3D::bytes(corners, directional)?)?;
            }
        }

        // Optional work must leave at least the original preparation's CPU
        // headroom for later mandatory reservations. Cold seeding uses only
        // additional capacity after those reservations have succeeded.
        if bytes > full_bytes {
            return None;
        }
        let charge = context.reserve_dynamic_storage(0, 0, 0, bytes).ok()?;
        Some(ReservedDirectSlots3D {
            predecessor,
            charge,
            #[cfg(any(test, feature = "test-harness"))]
            bytes,
        })
    }

    pub(super) fn seed_slots(
        &self,
        context: &super::Context3D,
        holder_bytes: u64,
    ) -> Option<(Self, DynamicStorageLease3D, u64)> {
        let QualifiedGeometry3D::Flat(flat) = &self.geometry else {
            return None;
        };
        let packets = &self.source.snapshot.packets;
        let placements = &self.source.model.placements;
        if packets.is_empty() {
            return None;
        }
        let directional = self.source.directional.is_some();
        let mut bytes = holder_bytes.checked_add(QualifiedDirectSlot3D::table_bytes(
            packets.len(),
            directional,
        )?)?;
        for (index, (packet, placement)) in packets.iter().zip(placements.iter()).enumerate() {
            let (_, corners) = DirectSlotKey3D::new(index, packet, *placement)?;
            bytes = bytes.checked_add(QualifiedDirectSlot3D::bytes(corners, directional)?)?;
        }
        // Both representations coexist during the copy. Reserve before
        // allocating either the slot table or any per-slot payload.
        let mut charge = context.reserve_dynamic_storage(0, 0, 0, bytes).ok()?;
        let mut slots = Vec::with_capacity(packets.len());
        let mut triangles = flat.triangles.as_slice();
        let mut intervals = flat.metric_intervals.as_slice();
        for (index, (packet, placement)) in packets.iter().zip(placements.iter()).enumerate() {
            let (key, corners) = DirectSlotKey3D::new(index, packet, *placement)
                .expect("completed exact packet proof");
            let (slot_triangles, rest) = triangles.split_at(corners / 3);
            triangles = rest;
            let (slot_intervals, rest) = intervals.split_at(if directional { corners } else { 0 });
            intervals = rest;
            slots.push(QualifiedDirectSlot3D::new(
                key,
                DirectGeometryArrays3D {
                    triangles: slot_triangles.to_vec(),
                    metric_intervals: slot_intervals.to_vec(),
                },
                &mut charge,
            ));
        }
        debug_assert!(triangles.is_empty() && intervals.is_empty());
        Some((
            Self {
                source: self.source.clone(),
                origin_bits: self.origin_bits,
                k_bits: self.k_bits,
                validated_radiance: self.validated_radiance,
                source_corners: self.source_corners,
                geometry: QualifiedGeometry3D::Slots(slots),
            },
            charge,
            bytes,
        ))
    }
}

fn clip_into(input: &[DVec3], output: &mut Vec<DVec3>, plane: impl Fn(DVec3) -> f64) {
    output.clear();
    let Some(&last) = input.last() else {
        return;
    };
    let mut a = last;
    let mut da = plane(a);
    for &b in input {
        let db = plane(b);
        if (da >= 0.0) != (db >= 0.0) {
            output.push(a + (b - a) * (da / (da - db)));
        }
        if db >= 0.0 {
            output.push(b);
        }
        a = b;
        da = db;
    }
}

fn reset_clip<const REUSE: bool>(polygon: &mut Vec<DVec3>, triangle: &[DVec3; 3]) {
    #[cfg(test)]
    if !REUSE {
        *polygon = Vec::new();
        *polygon = triangle.to_vec();
        return;
    }
    polygon.clear();
    polygon.extend_from_slice(triangle);
}

fn clip_current<const REUSE: bool>(
    polygon: &mut Vec<DVec3>,
    scratch: &mut Vec<DVec3>,
    plane: impl Fn(DVec3) -> f64,
) {
    #[cfg(test)]
    if !REUSE {
        *polygon = clip_polygon(std::mem::take(polygon), plane);
        return;
    }
    clip_into(polygon, scratch, plane);
    std::mem::swap(polygon, scratch);
}

// Only the allocating test control releases its final polygon per triangle.
// The production buffers remain local to this one frame projection.
fn finish_clip<const REUSE: bool>(_polygon: &mut Vec<DVec3>) {
    #[cfg(test)]
    if !REUSE {
        *_polygon = Vec::new();
    }
}

#[cfg(test)]
fn clip_polygon(mut poly: Vec<DVec3>, plane: impl Fn(DVec3) -> f64) -> Vec<DVec3> {
    if poly.is_empty() {
        return poly;
    }
    let mut out = Vec::new();
    let mut a = *poly.last().unwrap();
    let mut da = plane(a);
    for b in poly.drain(..) {
        let db = plane(b);
        if (da >= 0.0) != (db >= 0.0) {
            out.push(a + (b - a) * (da / (da - db)));
        }
        if db >= 0.0 {
            out.push(b);
        }
        a = b;
        da = db;
    }
    out
}

#[test]
fn r025_clip_scratch_exact_plane_sequences_and_bounded_storage() {
    use super::{Orientation3, PerspectiveProjection, RigidPose3};

    let bits = |polygon: &[DVec3]| {
        polygon
            .iter()
            .map(|p| p.to_array().map(f64::to_bits))
            .collect::<Vec<_>>()
    };
    let tiny = f64::from(f32::from_bits(1));
    let triangles = [
        [[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.0, 0.5, 0.0]],
        [[-4.0, -4.0, 0.0], [4.0, -4.0, 0.0], [0.0, 4.0, 0.0]],
        [[100.0, 0.0, 0.0], [101.0, 0.0, 0.0], [100.0, 1.0, 0.0]],
        [[-1.0, 0.0, 3.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        [[-0.0, 0.0, 2.0], [0.0, -0.0, 2.0], [0.0, 0.0, 2.0]],
        [[-tiny, tiny, 0.0], [tiny, -tiny, -0.0], [-0.0, 0.0, 0.0]],
        [[-0.5, 0.0, -8.0], [0.5, 0.0, -8.0], [0.0, 0.5, -8.0]],
        [[-0.5, -0.5, 1.99], [0.5, -0.5, 1.99], [0.0, 0.5, 1.99]],
    ]
    .map(|triangle| triangle.map(DVec3::from_array));
    let mut polygon = Vec::with_capacity(12);
    let mut scratch = Vec::with_capacity(12);
    let allocations = [polygon.as_ptr(), scratch.as_ptr()];
    let mut compared_cuts = 0;
    let mut maximum_vertices = 0;
    for degrees in [52.0f32, 48.0, 44.0, 48.0] {
        let view = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap(),
                Orientation3::identity(),
            ),
            PerspectiveProjection::try_new(
                degrees.to_radians(),
                0.1,
                PerspectiveFar::Finite(10.0),
            )
            .unwrap()
            .into(),
        );
        let vb = metric_view(view, WorldPoint3::origin(), 1.0, 192, 128).unwrap();
        let columns: [[f32; 4]; 4] =
            bytemuck::pod_read_unaligned(&bytemuck::bytes_of(&vb)[..64]);
        let vp = DMat4::from_cols_array_2d(&columns.map(|c| c.map(f64::from)));
        let pos = DVec3::new(0.0, 0.0, 2.0);
        let axis = -DVec3::Z;
        let (right, up) = basis(axis).unwrap();
        let tan = 0.7f64.tan();
        for (sequence, cuts) in [6, 5, 7].into_iter().enumerate() {
            for (case, triangle) in triangles.iter().enumerate() {
                reset_clip::<true>(&mut polygon, triangle);
                let mut original = triangle.to_vec();
                for cut in 0..cuts {
                    let plane = |p: DVec3| {
                        if sequence == 1 {
                            let h = p - pos;
                            let d = h.dot(axis);
                            match cut {
                                0 => d * tan + h.dot(right),
                                1 => d * tan - h.dot(right),
                                2 => d * tan + h.dot(up),
                                3 => d * tan - h.dot(up),
                                _ => 10.0 - d,
                            }
                        } else if cut == 6 {
                            (p - pos).dot(axis)
                        } else {
                            let c = vp * p.extend(1.0);
                            match cut {
                                0 => c.w + c.x,
                                1 => c.w - c.x,
                                2 => c.w + c.y,
                                3 => c.w - c.y,
                                4 => c.z,
                                _ => c.w - c.z,
                            }
                        }
                    };
                    original = clip_polygon(original, plane);
                    clip_current::<true>(&mut polygon, &mut scratch, plane);
                    assert_eq!(
                        bits(&polygon),
                        bits(&original),
                        "FOV{degrees} sequence{sequence} case{case} cut{cut}"
                    );
                    assert_eq!((polygon.capacity(), scratch.capacity()), (12, 12));
                    assert!(allocations.contains(&polygon.as_ptr()));
                    assert!(allocations.contains(&scratch.as_ptr()));
                    maximum_vertices = maximum_vertices.max(polygon.len());
                    compared_cuts += 1;
                }
            }
        }
    }
    assert_eq!(compared_cuts, 576);
    assert!(
        maximum_vertices > 3,
        "crossing triangles must grow during clipping"
    );
    assert!(maximum_vertices <= 10);
    std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
        "R025_CLIP_EXACT cuts={compared_cuts} max_vertices={maximum_vertices} \
         scratch_allocations=2 scratch_capacity=12 scratch_payload_bytes={}\n",
        2 * 12 * std::mem::size_of::<DVec3>(),
    )).unwrap();
}

fn basis(forward: DVec3) -> Result<(DVec3, DVec3), PbrError3D> {
    let a = forward.abs();
    let axis = if a.x <= a.y && a.x <= a.z {
        DVec3::X
    } else if a.y <= a.z {
        DVec3::Y
    } else {
        DVec3::Z
    };
    let right = unit(forward.cross(axis), "shadow right")?;
    Ok((right, right.cross(forward)))
}
fn perspective_from_axes(
    origin: DVec3,
    right: DVec3,
    up: DVec3,
    forward: DVec3,
    near: f64,
    far: f64,
    focal: f64,
) -> DMat4 {
    let rows = [
        right * focal,
        up * focal,
        forward * (-near / (far - near)),
        forward,
    ];
    DMat4::from_cols(
        DVec4::new(rows[0].x, rows[1].x, rows[2].x, rows[3].x),
        DVec4::new(rows[0].y, rows[1].y, rows[2].y, rows[3].y),
        DVec4::new(rows[0].z, rows[1].z, rows[2].z, rows[3].z),
        DVec4::new(
            -rows[0].dot(origin),
            -rows[1].dot(origin),
            near * far / (far - near) - rows[2].dot(origin),
            -rows[3].dot(origin),
        ),
    )
}

impl DirectSource3D {
    fn same_authored_lights(&self, old: &Self) -> bool {
        let point_matches = |a: &PointLight3D, b: &PointLight3D| {
            point_bits(a.position) == point_bits(b.position)
                && a.intensity.map(f32::to_bits) == b.intensity.map(f32::to_bits)
                && a.radius_m.to_bits() == b.radius_m.to_bits()
                && a.range_m.to_bits() == b.range_m.to_bits()
        };
        self.identity == old.identity
            && match (&self.directional, &old.directional) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    a.surface_to_source.map(f64::to_bits) == b.surface_to_source.map(f64::to_bits)
                        && a.irradiance.map(f32::to_bits) == b.irradiance.map(f32::to_bits)
                        && a.shadow_far_m.to_bits() == b.shadow_far_m.to_bits()
                }
                _ => false,
            }
            && self.points.len() == old.points.len()
            && self
                .points
                .iter()
                .zip(old.points.iter())
                .all(|(a, b)| point_matches(a, b))
            && self.spots.len() == old.spots.len()
            && self.spots.iter().zip(old.spots.iter()).all(|(a, b)| {
                point_matches(&a.point, &b.point)
                    && a.emitter_to_scene.map(f64::to_bits) == b.emitter_to_scene.map(f64::to_bits)
                    && a.inner_radians.to_bits() == b.inner_radians.to_bits()
                    && a.outer_radians.to_bits() == b.outer_radians.to_bits()
            })
    }

    // Called only after complete direct-source validation, and in production
    // only after the target owner has reserved the exact retained CPU charge.
    fn qualify_geometry(
        &self,
        scene: &MaterialSceneSource3D,
        origin: WorldPoint3,
    ) -> Result<QualifiedDirectSource3D, PbrError3D> {
        let source_corners = scene
            .snapshot
            .packets
            .iter()
            .try_fold(0usize, |count, packet| {
                let CommittedMaterialPacket3D::PbrMaterial { geometry, .. } = packet else {
                    unreachable!("D family validated")
                };
                count
                    .checked_add(geometry.data.source.element_count())
                    .ok_or(source(
                        "directional source storage",
                        "source corner count overflow",
                    ))
            })?;
        // Exact capacities preserve the original source/fit reservation.
        let mut arrays =
            DirectGeometryArrays3D::with_capacity(source_corners, self.directional.is_some());
        for (packet, placement) in scene
            .snapshot
            .packets
            .iter()
            .zip(scene.model.placements.iter())
        {
            self.append_qualified_slot(packet, *placement, origin, &mut arrays)?;
        }
        Ok(self.qualified_geometry(
            scene,
            origin,
            source_corners,
            QualifiedGeometry3D::Flat(arrays),
        ))
    }

    fn qualify_slots(
        &self,
        scene: &MaterialSceneSource3D,
        origin: WorldPoint3,
        reserved: &mut ReservedDirectSlots3D<'_>,
        #[cfg(any(test, feature = "test-harness"))] mut timing: Option<
            &mut super::raster::PbrPreparationTiming3D,
        >,
    ) -> Result<QualifiedDirectSource3D, PbrError3D> {
        #[cfg(any(test, feature = "test-harness"))]
        let start = timing.as_ref().map(|_| std::time::Instant::now());
        #[cfg(any(test, feature = "test-harness"))]
        let rebuilt_before = timing
            .as_ref()
            .map_or(std::time::Duration::ZERO, |t| t.direct_slot_rebuild);
        let directional = self.directional.is_some();
        let mut slots = Vec::with_capacity(reserved.predecessor.len());
        let mut source_corners = 0usize;
        for (index, ((packet, placement), old)) in scene
            .snapshot
            .packets
            .iter()
            .zip(scene.model.placements.iter())
            .zip(reserved.predecessor)
            .enumerate()
        {
            let (key, corners) = DirectSlotKey3D::new(index, packet, *placement)
                .expect("reserved exact packet roster");
            source_corners = source_corners.checked_add(corners).ok_or(source(
                "directional source storage",
                "source corner count overflow",
            ))?;
            if old.key == key {
                slots.push(old.clone());
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(t) = timing.as_deref_mut() {
                    t.direct_slots_reused += 1;
                }
            } else {
                #[cfg(any(test, feature = "test-harness"))]
                let slot_start = timing.as_ref().map(|_| std::time::Instant::now());
                let mut arrays = DirectGeometryArrays3D::with_capacity(corners, directional);
                self.append_qualified_slot(packet, *placement, origin, &mut arrays)?;
                slots.push(QualifiedDirectSlot3D::new(
                    key,
                    arrays,
                    &mut reserved.charge,
                ));
                #[cfg(any(test, feature = "test-harness"))]
                if let (Some(t), Some(slot_start)) = (timing.as_deref_mut(), slot_start) {
                    t.direct_slots_rebuilt += 1;
                    t.direct_slot_rebuild += slot_start.elapsed();
                }
            }
        }
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(t), Some(start)) = (timing, start) {
            t.direct_slot_recompose += start.elapsed() - (t.direct_slot_rebuild - rebuilt_before);
            t.direct_slot_reserved_cpu_bytes = reserved.bytes;
        }
        Ok(self.qualified_geometry(
            scene,
            origin,
            source_corners,
            QualifiedGeometry3D::Slots(slots),
        ))
    }

    fn append_qualified_slot(
        &self,
        packet: &CommittedMaterialPacket3D,
        placement: ModelPlacement3,
        origin: WorldPoint3,
        arrays: &mut DirectGeometryArrays3D,
    ) -> Result<(), PbrError3D> {
        let CommittedMaterialPacket3D::PbrMaterial { geometry, .. } = packet else {
            unreachable!("D family validated")
        };
        let k = self.meters_per_scene_unit;
        let object = metric_object(placement, origin, k)?;
        let model_columns: [[f32; 4]; 4] =
            bytemuck::pod_read_unaligned(&bytemuck::bytes_of(&object)[..64]);
        let normal_columns: [[f32; 4]; 3] =
            bytemuck::pod_read_unaligned(&bytemuck::bytes_of(&object)[64..112]);
        let normal_matrix =
            glam::Mat3::from_cols_array_2d(&normal_columns.map(|c| [c[0], c[1], c[2]]));
        let q = orientation64(placement.orientation());
        let s = placement.scale();
        let scale = DVec3::new(f64::from(s.x()), f64::from(s.y()), f64::from(s.z())) * k;
        let translation = (point64(placement.translation()) - point64(origin)) * k;
        for t in 0..geometry.data.source.element_count() / 3 {
            let p = geometry.data.source.triangle_positions(t)?;
            let original = p.map(DVec3::from_array);
            let face = unit(
                (original[1] - original[0]).cross(original[2] - original[0]),
                "source winding face",
            )?
            .as_vec3();
            let transformed_face = normal_matrix * face;
            if !transformed_face.is_finite() || transformed_face.abs().max_element() == 0.0 {
                return Err(source(
                    "geometric face f32",
                    "true normal transform is not finite/nonzero",
                ));
            }
            let p = p.map(|p| q * (DVec3::from_array(p) * scale) + translation);
            if p.iter().any(|p| !p.is_finite()) {
                return Err(source("metric geometry", "nonfinite transformed source"));
            }
            if self.directional.is_some() {
                for local in original {
                    arrays
                        .metric_intervals
                        .push(metric_position_intervals(model_columns, local)?);
                }
            }
            arrays.triangles.push(p);
        }
        Ok(())
    }

    fn qualified_geometry(
        &self,
        scene: &MaterialSceneSource3D,
        origin: WorldPoint3,
        source_corners: usize,
        geometry: QualifiedGeometry3D,
    ) -> QualifiedDirectSource3D {
        QualifiedDirectSource3D {
            source: scene
                .direct
                .as_ref()
                .expect("validated direct capture")
                .clone(),
            origin_bits: point_bits(origin),
            k_bits: self.meters_per_scene_unit.to_bits(),
            validated_radiance: ValidatedDirectRadiance3D {
                hdr_identity: scene.hdr.identity,
                hdr_revision: scene.hdr.revision,
                exposure_bits: scene.hdr.exposure_ev.to_bits(),
                maximum_rgb_bits: scene.hdr.maximum_rgb.map(f32::to_bits),
                samples: scene.samples,
            },
            source_corners,
            geometry,
        }
    }

    pub(crate) fn prepare_frame(
        &self,
        scene: &MaterialSceneSource3D,
        view: ViewDescriptor3,
        origin: WorldPoint3,
        width: u32,
        height: u32,
    ) -> Result<DirectFrame3D, PbrError3D> {
        self.prepare_frame_with_qualified_source(
            scene,
            view,
            origin,
            width,
            height,
            None,
            #[cfg(any(test, feature = "test-harness"))]
            None,
        )
        .map(|(frame, _)| frame)
    }

    /// Geometry and a successful full source proof are reusable only for their
    /// exact captured inputs. Every call executes all original current-view,
    /// clipping, light, cascade-candidate, f32 depth and XY/PCF qualification.
    /// A new basis is returned only with a successful complete frame.
    pub(crate) fn prepare_frame_with_qualified_source(
        &self,
        scene: &MaterialSceneSource3D,
        view: ViewDescriptor3,
        origin: WorldPoint3,
        width: u32,
        height: u32,
        qualified: Option<&QualifiedDirectSource3D>,
        #[cfg(any(test, feature = "test-harness"))] timing: Option<
            &mut super::raster::PbrPreparationTiming3D,
        >,
    ) -> Result<(DirectFrame3D, Option<QualifiedDirectSource3D>), PbrError3D> {
        self.prepare_frame_with_source_proof(
            scene,
            view,
            origin,
            width,
            height,
            qualified,
            None,
            #[cfg(any(test, feature = "test-harness"))]
            timing,
        )
    }

    /// A complete scene admission proves its exact current radiance independently
    /// of an older geometry basis. Neither that basis nor its HDR witness changes.
    pub(crate) fn prepare_frame_with_source_proof(
        &self,
        scene: &MaterialSceneSource3D,
        view: ViewDescriptor3,
        origin: WorldPoint3,
        width: u32,
        height: u32,
        qualified: Option<&QualifiedDirectSource3D>,
        proof: Option<&super::pbr::ValidatedMaterialSceneSource3D>,
        #[cfg(any(test, feature = "test-harness"))] timing: Option<
            &mut super::raster::PbrPreparationTiming3D,
        >,
    ) -> Result<(DirectFrame3D, Option<QualifiedDirectSource3D>), PbrError3D> {
        self.prepare_frame_with_slots(
            scene,
            view,
            origin,
            width,
            height,
            qualified,
            proof,
            None,
            #[cfg(any(test, feature = "test-harness"))]
            timing,
        )
    }

    pub(super) fn prepare_frame_with_slots(
        &self,
        scene: &MaterialSceneSource3D,
        view: ViewDescriptor3,
        origin: WorldPoint3,
        width: u32,
        height: u32,
        qualified: Option<&QualifiedDirectSource3D>,
        proof: Option<&super::pbr::ValidatedMaterialSceneSource3D>,
        reserved: Option<&mut ReservedDirectSlots3D<'_>>,
        #[cfg(any(test, feature = "test-harness"))] timing: Option<
            &mut super::raster::PbrPreparationTiming3D,
        >,
    ) -> Result<(DirectFrame3D, Option<QualifiedDirectSource3D>), PbrError3D> {
        self.prepare_frame_source::<true, true, true>(
            scene,
            view,
            origin,
            width,
            height,
            qualified,
            proof,
            reserved,
            #[cfg(any(test, feature = "test-harness"))]
            timing,
        )
    }

    #[cfg(test)]
    pub(crate) fn prepare_frame_without_depth_groups_for_test(
        &self,
        scene: &MaterialSceneSource3D,
        view: ViewDescriptor3,
        origin: WorldPoint3,
        width: u32,
        height: u32,
    ) -> Result<DirectFrame3D, PbrError3D> {
        self.prepare_frame_source::<false, false, true>(
            scene, view, origin, width, height, None, None, None, None,
        )
        .map(|(frame, _)| frame)
    }

    #[cfg(test)]
    pub(crate) fn prepare_frame_with_full_validation_for_test(
        &self,
        scene: &MaterialSceneSource3D,
        view: ViewDescriptor3,
        origin: WorldPoint3,
        width: u32,
        height: u32,
        qualified: &QualifiedDirectSource3D,
    ) -> Result<DirectFrame3D, PbrError3D> {
        self.prepare_frame_source::<true, false, true>(
            scene,
            view,
            origin,
            width,
            height,
            Some(qualified),
            None,
            None,
            None,
        )
        .map(|(frame, _)| frame)
    }

    #[cfg(test)]
    pub(crate) fn prepare_frame_with_allocating_clip_for_test(
        &self,
        scene: &MaterialSceneSource3D,
        view: ViewDescriptor3,
        origin: WorldPoint3,
        width: u32,
        height: u32,
        qualified: Option<&QualifiedDirectSource3D>,
        timing: Option<&mut super::raster::PbrPreparationTiming3D>,
    ) -> Result<DirectFrame3D, PbrError3D> {
        self.prepare_frame_source::<true, true, false>(
            scene, view, origin, width, height, qualified, None, None, timing,
        )
        .map(|(frame, _)| frame)
    }

    fn prepare_frame_source<
        const GROUPS: bool,
        const REUSE_VALIDATION: bool,
        const REUSE_CLIP: bool,
    >(
        &self,
        scene: &MaterialSceneSource3D,
        view: ViewDescriptor3,
        origin: WorldPoint3,
        width: u32,
        height: u32,
        qualified: Option<&QualifiedDirectSource3D>,
        proof: Option<&super::pbr::ValidatedMaterialSceneSource3D>,
        reserved: Option<&mut ReservedDirectSlots3D<'_>>,
        #[cfg(any(test, feature = "test-harness"))] mut timing: Option<
            &mut super::raster::PbrPreparationTiming3D,
        >,
    ) -> Result<(DirectFrame3D, Option<QualifiedDirectSource3D>), PbrError3D> {
        #[cfg(any(test, feature = "test-harness"))]
        let mut clock = timing.as_ref().map(|_| DirectProjectionClock3D::new());
        let validation_reused = REUSE_VALIDATION
            && (qualified.is_some_and(|q| {
                q.matches_validated_radiance(self, &scene.snapshot.context, scene, origin)
            }) || proof.is_some_and(|proof| {
                proof.matches(&scene.snapshot.context, scene)
                    && scene
                        .direct
                        .as_ref()
                        .is_some_and(|direct| std::ptr::eq(self, direct.as_ref()))
                    && qualified.is_none_or(|q| q.matches(&scene.snapshot.context, scene, origin))
            }));
        if !validation_reused {
            self.validate(scene)?;
        }
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(t) = timing.as_deref_mut() {
            t.direct_source_validation_reused = validation_reused;
        }
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(c) = clock.as_mut() {
            c.mark(DirectProjectionClock3D::VALIDATION);
        }
        let qualified = qualified.filter(|q| q.matches(&scene.snapshot.context, scene, origin));
        let new_qualified = if qualified.is_none() {
            Some(match reserved {
                Some(reserved) => self.qualify_slots(
                    scene,
                    origin,
                    reserved,
                    #[cfg(any(test, feature = "test-harness"))]
                    timing.as_deref_mut(),
                )?,
                None => self.qualify_geometry(scene, origin)?,
            })
        } else {
            None
        };
        let geometry = new_qualified
            .as_ref()
            .or(qualified)
            .expect("qualified source geometry");
        let k = self.meters_per_scene_unit;
        let source_corners = geometry.source_corners;
        let triangles = || geometry.geometry.triangles();
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(c) = clock.as_mut() {
            c.mark(DirectProjectionClock3D::BASIS);
        }
        let mut light = LightBlock608::zeroed();
        let mut transforms = ShadowBlock1984::zeroed();
        #[cfg(test)]
        let mut depth_proofs = Vec::new();
        #[cfg(any(test, feature = "test-harness"))]
        // Candidate attempts, triangle fast/fallback calls, frustum tests,
        // group successes/fallbacks, and triangles covered by group successes.
        let mut hull_counts = [0u64; 7];
        #[cfg(any(test, feature = "test-harness"))]
        let (mut receiver_triangles, mut cascades) = (0, 0);
        let mut active_faces = Vec::new();
        light.counts = [
            u32::from(self.directional.is_some()),
            self.points.len() as u32,
            self.spots.len() as u32,
            0,
        ];
        let camera = (point64(view.pose().translation()) - point64(origin)) * k;
        let camera_rotation = orientation64(view.pose().orientation());
        let vb = metric_view(view, origin, k, width, height)?;
        let columns: [[f32; 4]; 4] = bytemuck::pod_read_unaligned(&bytemuck::bytes_of(&vb)[..64]);
        let vp = DMat4::from_cols_array_2d(&columns.map(|c| c.map(f64::from)));
        // A convex triangle gains at most one vertex per halfspace: the longest
        // current sequence has seven cuts, hence at most ten vertices. Keep two
        // frame-local buffers across all directional/spot triangles. Vec growth
        // still preserves the complete exact polygon for exceptional inputs.
        let capacity = if REUSE_CLIP && (self.directional.is_some() || !self.spots.is_empty()) {
            12
        } else {
            0
        };
        let mut polygon = Vec::with_capacity(capacity);
        let mut clip_scratch = Vec::with_capacity(capacity);
        if let Some(d) = &self.directional {
            let n = match view.projection() {
                Projection3::Perspective(p) => f64::from(p.near()) * k,
                Projection3::Orthographic(p) => f64::from(p.near()) * k,
            };
            light.splits = cascade_splits(n, d.shadow_far_m)?;
            let l = unit(DVec3::from_array(d.surface_to_source), "directional L")?;
            light.directional.a = [
                checked(l.x, "L")?,
                checked(l.y, "L")?,
                checked(l.z, "L")?,
                0.0,
            ];
            light.directional.b[..3].copy_from_slice(&d.irradiance);
            // Clip actual source triangles to the actual camera volume. Only
            // potentially visible receivers impose the finite cascade-far cut.
            #[cfg(any(test, feature = "test-harness"))]
            if let Some(c) = clock.as_mut() {
                c.mark(DirectProjectionClock3D::OTHER);
            }
            for triangle in triangles() {
                reset_clip::<REUSE_CLIP>(&mut polygon, triangle);
                for plane in 0..6 {
                    clip_current::<REUSE_CLIP>(&mut polygon, &mut clip_scratch, |p| {
                        let c = vp * p.extend(1.0);
                        match plane {
                            0 => c.w + c.x,
                            1 => c.w - c.x,
                            2 => c.w + c.y,
                            3 => c.w - c.y,
                            4 => c.z,
                            _ => c.w - c.z,
                        }
                    });
                }
                if polygon
                    .iter()
                    .any(|p| -(camera_rotation.conjugate() * (*p - camera)).z > d.shadow_far_m)
                {
                    return Err(source(
                        "directional coverage",
                        "potentially lit visible receiver beyond all cascades",
                    ));
                }
                finish_clip::<REUSE_CLIP>(&mut polygon);
            }
            #[cfg(any(test, feature = "test-harness"))]
            {
                receiver_triangles += source_corners / 3;
                if let Some(c) = clock.as_mut() {
                    c.mark(DirectProjectionClock3D::RECEIVERS);
                }
            }
            let (right, up) = basis(-l)?;
            let mut start = n;
            #[cfg(any(test, feature = "test-harness"))]
            if let Some(c) = clock.as_mut() {
                c.mark(DirectProjectionClock3D::OTHER);
            }
            for cascade in 0..3 {
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(c) = clock.as_mut() {
                    // Loop control and the previous slice's temporary drops
                    // belong to other work, before this slice starts gathering.
                    c.mark(DirectProjectionClock3D::OTHER);
                }
                let end = f64::from(light.splits[cascade]);
                let mut points =
                    Vec::<DVec3>::with_capacity(source_corners.checked_add(8).ok_or(source(
                        "directional source storage",
                        "frustum corner count overflow",
                    ))?);
                points.extend(triangles().flatten().copied());
                for depth in [start, end] {
                    let y = match view.projection() {
                        Projection3::Perspective(p) => {
                            depth * (f64::from(p.vertical_fov_radians()) * 0.5).tan()
                        }
                        Projection3::Orthographic(p) => f64::from(p.vertical_span()) * k * 0.5,
                    };
                    let x = y * f64::from(width) / f64::from(height);
                    for sx in [-1.0, 1.0] {
                        for sy in [-1.0, 1.0] {
                            points.push(
                                camera + camera_rotation * DVec3::new(sx * x, sy * y, -depth),
                            );
                        }
                    }
                }
                let coordinates = points
                    .iter()
                    .map(|p| DVec3::new(right.dot(*p), up.dot(*p), l.dot(*p)))
                    .collect::<Vec<_>>();
                let mut lo = coordinates
                    .iter()
                    .copied()
                    .fold(DVec3::splat(f64::INFINITY), DVec3::min);
                let mut hi = coordinates
                    .iter()
                    .copied()
                    .fold(DVec3::splat(f64::NEG_INFINITY), DVec3::max);
                for axis in 0..2 {
                    let step = (hi[axis] - lo[axis]) / (1024.0 - 4.0);
                    if !step.is_finite() || step <= 0.0 {
                        return Err(source(
                            "directional footprint",
                            "undefined finite conservative fit",
                        ));
                    }
                    lo[axis] = (lo[axis] / step).floor() * step - 2.0 * step;
                    hi[axis] = (hi[axis] / step).ceil() * step + 2.0 * step;
                }
                if hi.z <= lo.z {
                    return Err(source(
                        "directional depth",
                        "undefined conservative depth interval",
                    ));
                }
                let rx = right * (2.0 / (hi.x - lo.x));
                let uy = up * (2.0 / (hi.y - lo.y));
                let distance = hi.z - lo.z;
                let center = lo.z + distance * 0.5;
                if !distance.is_finite() || !center.is_finite() {
                    return Err(source(
                        "directional f32 coverage",
                        "nonfinite fitted depth envelope",
                    ));
                }
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(c) = clock.as_mut() {
                    c.mark(DirectProjectionClock3D::FIT + cascade);
                }
                let mut selected = None;
                // Architect-selected finite sequence: exact fit, then24..1.
                // The first certified candidate wins; no tolerance/heuristic loop.
                for candidate in 0..25 {
                    #[cfg(any(test, feature = "test-harness"))]
                    {
                        hull_counts[0] += 1;
                    }
                    let t = if candidate == 0 {
                        1.0
                    } else {
                        1.0 - 2.0f64.powi(-(25 - candidate))
                    };
                    let zz = l * (t / distance);
                    let m = DMat4::from_cols(
                        DVec4::new(rx.x, uy.x, zz.x, 0.0),
                        DVec4::new(rx.y, uy.y, zz.y, 0.0),
                        DVec4::new(rx.z, uy.z, zz.z, 0.0),
                        DVec4::new(
                            -(hi.x + lo.x) / (hi.x - lo.x),
                            -(hi.y + lo.y) / (hi.y - lo.y),
                            0.5 - (t / distance) * center,
                            1.0,
                        ),
                    );
                    let Ok(packed) = matrix(m) else {
                        continue;
                    };
                    let row = packed.map(|c| c[2]);
                    #[cfg(test)]
                    let mut range = F32Interval {
                        lo: f64::INFINITY,
                        hi: f64::NEG_INFINITY,
                    };
                    #[cfg(test)]
                    let mut corners = [[0.0; 2]; 8];
                    let mut certified = geometry.geometry.all_depth_groups(|group| {
                        let result = if GROUPS {
                            triangle_group_depth_certificate(
                                row,
                                group,
                                #[cfg(any(test, feature = "test-harness"))]
                                &mut hull_counts,
                            )
                        } else {
                            triangle_sequence_depth_certificate(
                                row,
                                group,
                                #[cfg(any(test, feature = "test-harness"))]
                                &mut hull_counts,
                            )
                        };
                        #[cfg(test)]
                        {
                            // Keep the complete old witness, including its exact
                            // range bits. Unit tests never substitute a hull for
                            // the original per-corner observation.
                            let mut original = Ok(());
                            'triangles: for (index, triangle) in group.chunks(3).enumerate() {
                                for p in triangle {
                                    match interval_dot(row, [p[0], p[1], p[2], F32Interval::ONE]) {
                                        Ok(z) if z.lo > 0.0 && z.hi < 1.0 => range = range.hull(z),
                                        _ => {
                                            original = Err(index);
                                            break 'triangles;
                                        }
                                    }
                                }
                            }
                            assert_eq!(
                                result, original,
                                "group hull changed depth truth or first rejected triangle"
                            );
                        }
                        result.is_ok()
                    });
                    if certified {
                        // All eight view-frustum corners retain the original
                        // conversion, dot predicate and visitation order.
                        for (_i, point) in points[source_corners..].iter().enumerate() {
                            #[cfg(any(test, feature = "test-harness"))]
                            {
                                hull_counts[3] += 1;
                            }
                            let z = (|| {
                                Ok([
                                    F32Interval::scalar(point.x)?,
                                    F32Interval::scalar(point.y)?,
                                    F32Interval::scalar(point.z)?,
                                ])
                            })()
                            .and_then(|p| interval_dot(row, [p[0], p[1], p[2], F32Interval::ONE]));
                            match z {
                                Ok(z) if z.lo > 0.0 && z.hi < 1.0 => {
                                    #[cfg(test)]
                                    {
                                        range = range.hull(z);
                                        corners[_i] = [z.lo, z.hi];
                                    }
                                }
                                _ => {
                                    certified = false;
                                    break;
                                }
                            }
                        }
                    }
                    if certified {
                        #[cfg(test)]
                        depth_proofs.push(CascadeDepthProof3D {
                            t,
                            row,
                            range: [range.lo, range.hi],
                            corners,
                        });
                        selected = Some(packed);
                        break;
                    }
                }
                let packed = selected.ok_or(source(
                    "directional f32 coverage",
                    "no finite certified depth envelope among25 candidates",
                ))?;
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(c) = clock.as_mut() {
                    c.mark(DirectProjectionClock3D::DEPTH + cascade);
                }
                // Preserve the prior actual-f32 XY/PCF check exactly. Depth now
                // has the stronger source-transform + shadow-dot certificate.
                let mf = glam::Mat4::from_cols_array_2d(&packed);
                for p in &points {
                    let p = mf * p.as_vec3().extend(1.0);
                    if !p.is_finite()
                        || p.x.abs() > 1.0 - 2.0 / 1024.0
                        || p.y.abs() > 1.0 - 2.0 / 1024.0
                    {
                        return Err(source(
                            "directional f32 coverage",
                            "source/PCF footprint not contained after packing",
                        ));
                    }
                }
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(c) = clock.as_mut() {
                    c.mark(DirectProjectionClock3D::XY + cascade);
                }
                transforms.matrices[cascade] = packed;
                active_faces.push(cascade);
                start = end;
                #[cfg(any(test, feature = "test-harness"))]
                {
                    cascades += 1;
                    if let Some(c) = clock.as_mut() {
                        c.mark(DirectProjectionClock3D::OTHER);
                    }
                }
            }
        }
        let pack_point = |p: &PointLight3D| -> Result<Light64, PbrError3D> {
            let position = (point64(p.position) - point64(origin)) * k;
            let mut l = Light64::zeroed();
            l.a = [
                checked(position.x, "light position")?,
                checked(position.y, "light position")?,
                checked(position.z, "light position")?,
                checked(p.radius_m, "radius")?,
            ];
            l.b = [
                p.intensity[0],
                p.intensity[1],
                p.intensity[2],
                checked(p.range_m, "range")?,
            ];
            Ok(l)
        };
        for (p, point) in self.points.iter().enumerate() {
            light.points[p] = pack_point(point)?;
            let pos = (point64(point.position) - point64(origin)) * k;
            for triangle in triangles() {
                // The closest source triangle point bounds every displaced tap:
                // d_major_i>=d/sqrt(3). This geometric proof includes interiors.
                let min = closest_triangle_squared(*triangle, pos);
                if !min.is_finite() || min <= 3.0 * NEAR * NEAR {
                    return Err(source(
                        "point near cap",
                        "caster/receiver triangle lacks complete nine-tap near coverage",
                    ));
                }
            }
            let axes = [
                (DVec3::X, -DVec3::Z, DVec3::Y),
                (-DVec3::X, DVec3::Z, DVec3::Y),
                (DVec3::Y, DVec3::X, -DVec3::Z),
                (-DVec3::Y, DVec3::X, DVec3::Z),
                (DVec3::Z, DVec3::X, DVec3::Y),
                (-DVec3::Z, -DVec3::X, DVec3::Y),
            ];
            for (f, (forward, right, up)) in axes.into_iter().enumerate() {
                let index = 7 + 6 * p + f;
                transforms.matrices[index] = matrix(perspective_from_axes(
                    pos,
                    right,
                    up,
                    forward,
                    NEAR,
                    point.range_m,
                    1.0,
                ))?;
                active_faces.push(index);
            }
        }
        for (s, spot) in self.spots.iter().enumerate() {
            light.spots[s] = pack_point(&spot.point)?;
            let axis = unit(DVec3::from_array(spot.emitter_to_scene), "spot axis")?;
            light.spots[s].c = [
                checked(axis.x, "spot axis")?,
                checked(axis.y, "spot axis")?,
                checked(axis.z, "spot axis")?,
                checked(spot.inner_radians.cos(), "spot inner")?,
            ];
            light.spots[s].d[0] = checked(spot.outer_radians.cos(), "spot outer")?;
            let pos = (point64(spot.point.position) - point64(origin)) * k;
            let (right, up) = basis(axis)?;
            let tan = spot.outer_radians.tan();
            for triangle in triangles() {
                reset_clip::<REUSE_CLIP>(&mut polygon, triangle);
                for plane in 0..5 {
                    clip_current::<REUSE_CLIP>(&mut polygon, &mut clip_scratch, |p| {
                        let h = p - pos;
                        let d = h.dot(axis);
                        match plane {
                            0 => d * tan + h.dot(right),
                            1 => d * tan - h.dot(right),
                            2 => d * tan + h.dot(up),
                            3 => d * tan - h.dot(up),
                            _ => spot.point.range_m - d,
                        }
                    });
                }
                if polygon.iter().any(|p| (*p - pos).dot(axis) <= NEAR) {
                    return Err(source(
                        "spot near cap",
                        "relevant triangle intersects the light near cap",
                    ));
                }
                finish_clip::<REUSE_CLIP>(&mut polygon);
            }
            let index = 3 + s;
            let packed = matrix(perspective_from_axes(
                pos,
                right,
                up,
                axis,
                NEAR,
                spot.point.range_m,
                1.0 / tan,
            ))?;
            let projector = glam::Mat4::from_cols_array_2d(&packed);
            for triangle in triangles() {
                reset_clip::<REUSE_CLIP>(&mut polygon, triangle);
                for plane in 0..6 {
                    clip_current::<REUSE_CLIP>(&mut polygon, &mut clip_scratch, |p| {
                        let c = vp * p.extend(1.0);
                        match plane {
                            0 => c.w + c.x,
                            1 => c.w - c.x,
                            2 => c.w + c.y,
                            3 => c.w - c.y,
                            4 => c.z,
                            _ => c.w - c.z,
                        }
                    });
                }
                clip_current::<REUSE_CLIP>(&mut polygon, &mut clip_scratch, |p| {
                    (p - pos).dot(axis)
                });
                let visible = &polygon;
                let mut potentially_lit = false;
                for j in 1..visible.len().saturating_sub(1) {
                    let t = [visible[0] - pos, visible[j] - pos, visible[j + 1] - pos];
                    if closest_triangle_squared(t, DVec3::ZERO) < spot.point.range_m.powi(2)
                        && cone_minimum(t, axis, tan) < 0.0
                    {
                        potentially_lit = true;
                    }
                }
                if !potentially_lit {
                    finish_clip::<REUSE_CLIP>(&mut polygon);
                    continue;
                }
                // Convex projection bounds contain every source-bound covered
                // receiver and all nine tap addresses. An unqualified footprint
                // refuses before writes, even though the sampler clamps.
                for p in visible {
                    let c = projector * p.as_vec3().extend(1.0);
                    let ndc = c.truncate() / c.w;
                    if !c.is_finite()
                        || c.w <= 0.0
                        || !ndc.is_finite()
                        || ndc.x.abs() > 1.0 - 2.0 / 1024.0
                        || ndc.y.abs() > 1.0 - 2.0 / 1024.0
                        || ndc.z < 0.0
                        || ndc.z > 1.0
                    {
                        return Err(source(
                            "spot PCF coverage",
                            "potentially lit visible receiver lacks full f32 nine-tap footprint",
                        ));
                    }
                }
                finish_clip::<REUSE_CLIP>(&mut polygon);
            }
            transforms.matrices[index] = packed;
            active_faces.push(index);
        }
        #[cfg(feature = "test-harness")]
        if self.directional.is_some() {
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
                "R025_DEPTH_HULL source={} revision={} samples={} extent={}x{} view={view:?} candidate_attempts={} fast_triangles={} fallback_triangles={} group_passes={} group_fallbacks={} group_fast_triangles={} full_original_witness={}\n",
                self.identity, self.revision, scene.samples, width, height,
                hull_counts[0], hull_counts[1], hull_counts[2],
                hull_counts[4], hull_counts[5], hull_counts[6], cfg!(test),
            )).ok();
        }
        #[cfg(any(test, feature = "test-harness"))]
        if let (Some(clock), Some(timing)) = (clock, timing) {
            clock.finish(
                timing,
                source_corners / 3,
                receiver_triangles,
                cascades,
                [
                    hull_counts[0],
                    hull_counts[1],
                    hull_counts[2],
                    hull_counts[3],
                ],
            );
        }
        Ok((
            DirectFrame3D {
                light,
                transforms,
                active_faces,
                #[cfg(test)]
                depth_proofs,
            },
            new_qualified,
        ))
    }
}

fn cone_minimum(t: [DVec3; 3], axis: DVec3, tan: f64) -> f64 {
    let quadratic = |a: DVec3, b: DVec3| a.dot(b) - (1.0 + tan * tan) * a.dot(axis) * b.dot(axis);
    let value = |p: DVec3| quadratic(p, p);
    let mut minimum = t.into_iter().map(value).fold(f64::INFINITY, f64::min);
    for i in 0..3 {
        let a = t[i];
        let edge = t[(i + 1) % 3] - a;
        let aa = quadratic(edge, edge);
        if aa > 0.0 {
            let u = -quadratic(a, edge) / aa;
            if u > 0.0 && u < 1.0 {
                minimum = minimum.min(value(a + u * edge));
            }
        }
    }
    let a = t[0];
    let b = t[1] - a;
    let c = t[2] - a;
    let bb = quadratic(b, b);
    let cc = quadratic(c, c);
    let bc = quadratic(b, c);
    let det = bb * cc - bc * bc;
    if det != 0.0 {
        let ab = quadratic(a, b);
        let ac = quadratic(a, c);
        let u = (-ab * cc + ac * bc) / det;
        let v = (-ac * bb + ab * bc) / det;
        if u >= 0.0 && v >= 0.0 && u + v <= 1.0 {
            minimum = minimum.min(value(a + u * b + v * c));
        }
    }
    minimum
}

fn closest_triangle_squared(t: [DVec3; 3], p: DVec3) -> f64 {
    // Closed convex triangle distance, including edges/vertices, not a vertex-
    // only radius check that could miss an emitter inside a large triangle.
    let [a, b, c] = t;
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return ap.length_squared();
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return bp.length_squared();
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return (p - (a + ab * (d1 / (d1 - d3)))).length_squared();
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return cp.length_squared();
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return (p - (a + ac * (d2 / (d2 - d6)))).length_squared();
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return (p - (b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6))))).length_squared();
    }
    let inv = 1.0 / (va + vb + vc);
    (p - (a + ab * (vb * inv) + ac * (vc * inv))).length_squared()
}

/// Fixed composition of the SAME R-024 evaluator, with only R-025's admitted
/// additional face interface and controlled direct/shadow entry points.
pub(crate) fn shader_source() -> String {
    shader_source_with_material(super::pbr::PBR_WGSL)
}
pub(super) fn shader_source_with_material(material: &str) -> String {
    let base = material
        .split("@fragment fn pbr_fragment")
        .next()
        .expect("fixed R024 evaluator");
    let base=base.replace("@location(4) tangent:vec4<f32>\n}","@location(4) tangent:vec4<f32>, @location(5) face_normal:vec3<f32>\n}")
        .replace("@location(4) @interpolate(perspective,centroid) tangent:vec4<f32>\n}","@location(4) @interpolate(perspective,centroid) tangent:vec4<f32>,\n @location(5) @interpolate(flat) face_normal:vec3<f32>\n}")
        .replace("o.tangent=v.tangent; return o;","o.tangent=v.tangent; o.face_normal=v.face_normal; return o;");
    format!("{base}\n{DIRECT_WGSL}")
}

pub(crate) const DIRECT_WGSL: &str = r#"
struct Light64 { a:vec4<f32>, b:vec4<f32>, c:vec4<f32>, d:vec4<f32> }
struct Direct608 { counts:vec4<u32>, splits:vec4<f32>, directional:Light64, points:array<Light64,4>, spots:array<Light64,4> }
struct Shadow1984 { matrices:array<mat4x4<f32>,31> }
@group(3) @binding(0) var<uniform> direct:Direct608;
@group(3) @binding(1) var<uniform> shadow:Shadow1984;
@group(3) @binding(2) var shadow_2d:texture_depth_2d_array;
@group(3) @binding(3) var compare_2d:sampler_comparison;
@group(3) @binding(4) var shadow_cube:texture_depth_cube_array;
@group(3) @binding(5) var compare_cube:sampler_comparison;
fn receiver_bias(z:f32,Ng:vec3<f32>,L:vec3<f32>)->f32 {
 return min(1.0,z+1.0/65536.0+(1.0/4096.0)*max(0.0,1.0-clamp(dot(Ng,L),-1.0,1.0)));
}
fn visibility_2d(p:vec3<f32>,Ng:vec3<f32>,L:vec3<f32>,layer:u32)->f32 {
 let clip=shadow.matrices[layer]*vec4(p,1.0);let ndc=clip.xyz/clip.w;
 let uv=vec2(0.5+0.5*ndc.x,0.5-0.5*ndc.y);let z=receiver_bias(ndc.z,Ng,L);
 var count=0.0;
 for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){
  let observation=compare_2d_tap(uv,layer,z,x,y);
  count+=observation;
 }}
 return count/9.0;
}
fn compare_2d_tap(uv:vec2<f32>,layer:u32,z:f32,x:i32,y:i32)->f32 {
 return textureSampleCompareLevel(shadow_2d,compare_2d,uv+vec2(f32(x),f32(y))/1024.0,i32(layer),z);
}
fn owning_face(q:vec3<f32>)->u32 {
 let a=abs(q);
 if a.x>=a.y && a.x>=a.z {if q.x>=0.0{return 0u;}return 1u;}
 if a.y>=a.z {if q.y>=0.0{return 2u;}return 3u;}
 if q.z>=0.0{return 4u;}return 5u;
}
fn chart_u(face:u32)->vec3<f32> {
 switch face {case 0u:{return vec3(0.0,0.0,-1.0);}case 1u:{return vec3(0.0,0.0,1.0);}
 case 5u:{return vec3(-1.0,0.0,0.0);}default:{return vec3(1.0,0.0,0.0);}}
}
fn visibility_point(h:vec3<f32>,d:f32,Ng:vec3<f32>,L:vec3<f32>,p:u32,far:f32)->f32 {
 let q=unit_nonzero(h);let U=chart_u(owning_face(q));
 let T=unit_nonzero(U-dot(U,q)*q);let B=cross(T,q);var count=0.0;
 for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){
  let tap=unit_nonzero(q+(2.0/512.0)*(f32(x)*T+f32(y)*B));
  // Each actual displaced ray performs its own native cube-face lookup.
  let observation=compare_point_tap(tap,d,Ng,L,p,far);
  count+=observation;
 }}
 return count/9.0;
}
fn point_reference(tap:vec3<f32>,d:f32,far:f32)->f32 {
 let major=d*max(max(abs(tap.x),abs(tap.y)),abs(tap.z));
 return 0.01*(far-major)/(major*(far-0.01));
}
fn compare_point_tap(tap:vec3<f32>,d:f32,Ng:vec3<f32>,L:vec3<f32>,p:u32,far:f32)->f32 {
 return textureSampleCompareLevel(shadow_cube,compare_cube,tap,i32(p),receiver_bias(point_reference(tap,d,far),Ng,L));
}
fn view_depth(p:vec3<f32>)->f32 {
 let wrow=vec3(view.view_projection[0].w,view.view_projection[1].w,view.view_projection[2].w);
 if any(wrow!=vec3(0.0)){return (view.view_projection*vec4(p,1.0)).w;}
 let zrow=vec3(view.view_projection[0].z,view.view_projection[1].z,view.view_projection[2].z);
 return -dot(unit_nonzero(zrow),p-view.camera_relative);
}
fn direct_color(i:FragmentIn,s:Surface,N:vec3<f32>,Ng:vec3<f32>)->vec3<f32> {
 let V=view.camera_relative-i.position;var result=vec3(0.0);
 if direct.counts.x==1u {
  let L=unit_nonzero(direct.directional.a.xyz);let z=view_depth(i.position);
  var cascade=0u;if z>direct.splits.x{cascade=1u;}if z>direct.splits.y{cascade=2u;}
  let visibility=visibility_2d(i.position,Ng,L,cascade);
  result+=reflected_brdf(s.base,s.metallic,s.roughness,N,V,L)*max(0.0,dot(N,L))*direct.directional.b.xyz*visibility;
 }
 for(var p=0u;p<direct.counts.y;p++){
  let light=direct.points[p];let delta=light.a.xyz-i.position;
  let magnitude=max(max(abs(delta.x),abs(delta.y)),abs(delta.z));
  if magnitude>0.0 {
   let scaled=delta/magnitude;let d=magnitude*sqrt(dot(scaled,scaled));
   if d<light.b.w {
    let L=unit_nonzero(delta);let ratio=d/light.b.w;let r2=ratio*ratio;let w=(1.0-r2*r2)*(1.0-r2*r2);
    let incident=light.b.xyz*w/max(d*d,light.a.w*light.a.w);
    let visibility=visibility_point(-delta,d,Ng,L,p,light.b.w);
    result+=reflected_brdf(s.base,s.metallic,s.roughness,N,V,L)*max(0.0,dot(N,L))*incident*visibility;
   }
  }
 }
 for(var p=0u;p<direct.counts.z;p++){
  let light=direct.spots[p];let delta=light.a.xyz-i.position;
  let magnitude=max(max(abs(delta.x),abs(delta.y)),abs(delta.z));
  if magnitude>0.0 {
   let scaled=delta/magnitude;let d=magnitude*sqrt(dot(scaled,scaled));
   if d<light.b.w {
    let L=unit_nonzero(delta);let cosine=clamp(dot(light.c.xyz,-L),-1.0,1.0);
    let t=clamp((cosine-light.d.x)/(light.c.w-light.d.x),0.0,1.0);let cone=t*t*(3.0-2.0*t);
    if cone>0.0 {
     let ratio=d/light.b.w;let r2=ratio*ratio;let w=(1.0-r2*r2)*(1.0-r2*r2);
     let incident=light.b.xyz*w*cone/max(d*d,light.a.w*light.a.w);
     let visibility=visibility_2d(i.position,Ng,L,3u+p);
     result+=reflected_brdf(s.base,s.metallic,s.roughness,N,V,L)*max(0.0,dot(N,L))*incident*visibility;
    }
   }
  }
 }
 return result;
}
@fragment fn direct_fragment(i:FragmentIn,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {
 let s=surface(i);if material.role_mode.y==1u&&s.alpha<material.mr_ao_cutoff.w{discard;}
 let f=frame(i,s.normal_sample,front);
 let Ng=geometric_normal(i,front);
 return vec4(direct_color(i,s,f.N,Ng)+s.emission,1.0);
}
fn geometric_normal(i:FragmentIn,front:bool)->vec3<f32>{
 var Ng=unit_nonzero(object.normal_to_world*i.face_normal);if !front{Ng=-Ng;}return Ng;
}
@fragment fn shadow_fragment(i:FragmentIn) {
 if material.role_mode.y!=1u{return;}
 var base_alpha=1.0;
 if present(0u){base_alpha=textureSample(base_tex,base_sampler,i.uv0).a;}
 let alpha=base_alpha*material.base_rgba.a*i.color.a;
 if alpha<material.mr_ao_cutoff.w{discard;}
}
"#;

#[cfg(test)]
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct DirectProbeInput3D {
    pub(crate) p: [f32; 4],
    pub(crate) geometric: [f32; 4],
    pub(crate) light: [f32; 4],
    pub(crate) mode: [u32; 4],
}
#[cfg(test)]
pub(crate) use super::raster::{direct_compare_probe_for_test, qualify_material_uvs_for_test};

#[cfg(test)]
pub(crate) const PROOF_WGSL: &str = r#"
struct ComparisonInput {p:vec4<f32>,geometric:vec4<f32>,light:vec4<f32>,mode:vec4<u32>}
struct ComparisonOutput {values:array<vec4<f32>,20>}
@group(0) @binding(1) var<storage,read> comparison_inputs:array<ComparisonInput>;
@group(0) @binding(2) var<storage,read_write> comparison_outputs:array<ComparisonOutput>;
@group(0) @binding(3) var<uniform> fixed_depth_config:vec4<f32>;
@group(0) @binding(4) var<uniform> direct_observation_header:vec4<u32>;
// Test-only observations of the actual source interface and SAME production
// evaluation. The four-group production entry has no observation resources.
@fragment fn direct_observe_fragment(i:FragmentIn,@builtin(front_facing) front:bool)->@location(0) vec4<f32>{
 let s=surface(i);if material.role_mode.y==1u&&s.alpha<material.mr_ao_cutoff.w{discard;}
 let f=frame(i,s.normal_sample,front);let Ng=geometric_normal(i,front);
 let color=direct_color(i,s,f.N,Ng)+s.emission;
 let index=u32(i.clip.y)*direct_observation_header.x+u32(i.clip.x);
 if index<arrayLength(&comparison_outputs){
  comparison_outputs[index].values[0]=vec4(Ng,1.0);
  comparison_outputs[index].values[1]=vec4(f.N,select(0.0,1.0,front));
  comparison_outputs[index].values[2]=vec4(i.face_normal,1.0);
  comparison_outputs[index].values[3]=vec4(i.position,1.0);
  comparison_outputs[index].values[4]=i.color;
  comparison_outputs[index].values[5]=vec4(receiver_bias(0.5,Ng,vec3(0.0,0.0,1.0)),receiver_bias(0.5,f.N,vec3(0.0,0.0,1.0)),s.alpha,1.0);
  comparison_outputs[index].values[6]=vec4(color,1.0);
  comparison_outputs[index].values[7]=vec4(f.local_n,length(f.local_n));
  comparison_outputs[index].values[8]=vec4(f.g,length(f.g));
  comparison_outputs[index].values[9]=vec4(i.uv0,0.0,0.0);
 }
 return vec4(color,1.0);
}
@vertex fn fixed_depth_vertex(@builtin(vertex_index) v:u32)->@builtin(position) vec4<f32> {
 let positions=array<vec2<f32>,3>(vec2(-1.0,-1.0),vec2(3.0,-1.0),vec2(-1.0,3.0));return vec4(positions[v],0.5,1.0);
}
// A bounded test-owned DEPTH source, not an illumination provider, final pixel
// upload, shadow implementation or replacement material/tone evaluator.
@fragment fn fixed_depth_fragment(@builtin(position) p:vec4<f32>)->@builtin(frag_depth) f32 {
 if fixed_depth_config.y>=4.0{return 0.0;}
 if fixed_depth_config.y==1.0{return 0.5+1.0/32768.0;}
 if fixed_depth_config.y==2.0{return fixed_depth_config.z;}
 if fixed_depth_config.y==3.0 {
  return 0.125+select(0.0,0.25,p.x>=fixed_depth_config.x*0.5)+select(0.0,0.125,p.y>=fixed_depth_config.x*0.5);
 }
 return select(0.25,0.75,p.x>=fixed_depth_config.x*0.5);
}
@compute @workgroup_size(1) fn comparison_probe(@builtin(global_invocation_id) id:vec3<u32>) {
 if id.x>=arrayLength(&comparison_inputs){return;}
 let p=comparison_inputs[id.x];var total=0.0;var actual=0.0;var q=vec3(0.0);var T=vec3(0.0);var B=vec3(0.0);
 if p.mode.x==2u {
  var i:FragmentIn;i.position=p.p.xyz;
  let s=Surface(vec3(1.0),1.0,1.0,1.0,1.0,vec3(0.0),vec3(0.0,0.0,1.0));
  comparison_outputs[id.x].values[0]=vec4(direct_color(i,s,p.geometric.xyz,p.geometric.xyz),1.0);
  return;
 }
 if p.mode.x==0u{actual=visibility_2d(p.p.xyz,p.geometric.xyz,p.light.xyz,p.mode.y);}
 else{q=unit_nonzero(p.p.xyz);let U=chart_u(owning_face(q));T=unit_nonzero(U-dot(U,q)*q);B=cross(T,q);actual=visibility_point(q*p.p.w,p.p.w,p.geometric.xyz,p.light.xyz,p.mode.y,p.light.w);}
 for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){
  let index=u32((y+1)*3+x+1);var observation=0.0;var tap=vec3(0.0);var reference=0.0;var face=0u;
  if p.mode.x==0u {
   let clip=shadow.matrices[p.mode.y]*vec4(p.p.xyz,1.0);let ndc=clip.xyz/clip.w;
   reference=ndc.z;let uv=vec2(0.5+0.5*ndc.x,0.5-0.5*ndc.y);
   observation=compare_2d_tap(uv,p.mode.y,receiver_bias(reference,p.geometric.xyz,p.light.xyz),x,y);
  }else{
   tap=unit_nonzero(q+(2.0/512.0)*(f32(x)*T+f32(y)*B));reference=point_reference(tap,p.p.w,p.light.w);face=owning_face(tap);
   observation=compare_point_tap(tap,p.p.w,p.geometric.xyz,p.light.xyz,p.mode.y,p.light.w);
  }
  comparison_outputs[id.x].values[1u+index]=vec4(tap,observation);
  comparison_outputs[id.x].values[10u+index]=vec4(f32(face),reference,receiver_bias(reference,p.geometric.xyz,p.light.xyz),0.0);
  total+=observation;
 }}
 comparison_outputs[id.x].values[0]=vec4(actual,total/9.0,receiver_bias(0.5,p.geometric.xyz,p.light.xyz),f32(owning_face(q)));
 comparison_outputs[id.x].values[19]=vec4(T,length(B));
}
"#;
