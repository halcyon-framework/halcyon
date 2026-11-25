//! HAL101 §13: ephemeral complete-run edit projection. No document or clock owner.

use crate::core::text_engine::{GlyphRun, glyph_source_ranges};
use crate::core::text_shaper::TextShaper;
use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    SourceAssociation,
    GlyphEvidence,
    MissingGlyph,
    Geometry,
    RetargetGeometry,
    NoVisibleTravel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Source {
    Old,
    Current,
}

#[derive(Debug)]
struct Part {
    old: Range<f32>,
    new: Range<f32>,
    stable: bool,
}

#[derive(Debug)]
enum Geometry {
    Context(Vec<Part>),
    StationaryAppend(Range<f32>),
}

#[derive(Debug)]
pub(super) struct Projection {
    geometry: Geometry,
    pub travel: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Fragment {
    pub source: Source,
    pub clip: Range<f32>,
    pub offset: f32,
}

impl Projection {
    pub fn build(
        old: &GlyphRun,
        new: &GlyphRun,
        removed: Range<usize>,
        inserted: Range<usize>,
        shaper: Option<&TextShaper>,
        scale: f32,
    ) -> Result<Self, Refusal> {
        let a = removed.start;
        let b = removed.end;
        let c = inserted.end;
        if inserted.start != a
            || a > b
            || a > c
            || old.text.get(..a) != new.text.get(..a)
            || old.text.get(b..) != new.text.get(c..)
            || old.text.get(a..b).is_none()
            || new.text.get(a..c).is_none()
        {
            return Err(Refusal::SourceAssociation);
        }
        if old.binding.is_none()
            || old.binding != new.binding
            || [old.font_size, old.height, old.ascent, old.descent]
                != [new.font_size, new.height, new.ascent, new.descent]
        {
            return Err(Refusal::GlyphEvidence);
        }
        if old
            .glyphs
            .iter()
            .chain(&new.glyphs)
            .any(|glyph| glyph.id == 0)
        {
            return Err(Refusal::MissingGlyph);
        }
        // The text owner's helper reproduces both complete shapes and actual
        // glyph/source ranges. Never derive attribution from glyph ordinal.
        let old_sources = glyph_source_ranges(old, shaper, scale).ok_or(Refusal::GlyphEvidence)?;
        let new_sources = glyph_source_ranges(new, shaper, scale).ok_or(Refusal::GlyphEvidence)?;
        fn groups(sources: &[Range<usize>]) -> BTreeMap<(usize, usize), Vec<usize>> {
            let mut groups = BTreeMap::<_, Vec<_>>::new();
            for (index, source) in sources.iter().enumerate() {
                groups
                    .entry((source.start, source.end))
                    .or_default()
                    .push(index);
            }
            groups
        }
        fn ends(run: &GlyphRun) -> Result<Vec<f32>, Refusal> {
            if !run.width.is_finite() || run.width < 0.0 {
                return Err(Refusal::Geometry);
            }
            let mut result = Vec::with_capacity(run.clusters.len());
            let mut previous = 0.0;
            for (i, cluster) in run.clusters.iter().enumerate() {
                let end = run.clusters.get(i + 1).map_or(run.width, |c| c.x_start);
                if ![cluster.x_start, cluster.x_end, end]
                    .iter()
                    .all(|x| x.is_finite())
                    || cluster.x_start != previous
                    || cluster.x_end < cluster.x_start
                    || end < cluster.x_end
                {
                    return Err(Refusal::Geometry);
                }
                previous = end;
                result.push(end);
            }
            if previous != run.width {
                return Err(Refusal::Geometry);
            }
            Ok(result)
        }
        let old_ends = ends(old)?;
        let new_ends = ends(new)?;
        // Terminal kerning can change the prefix's advance without moving any
        // of its ink. When the appended clusters start after that prefix in
        // visual order, reveal the complete current run at its canonical origin.
        // Moving whole old/current fragments would draw the same prefix glyph
        // at two positions around the moving clip.
        if a == b
            && a == old.text.len()
            && c == new.text.len()
            && new.width > old.width
            && new
                .clusters
                .iter()
                .position(|cluster| cluster.source.start >= a)
                .is_some_and(|split| {
                    new.clusters[..split]
                        .iter()
                        .all(|cluster| cluster.source.end <= a)
                        && new.clusters[split..]
                            .iter()
                            .all(|cluster| cluster.source.start >= a)
                })
            && Self::prefix_glyphs_match(old, new, &old_sources, &new_sources)
        {
            return Ok(Self {
                geometry: Geometry::StationaryAppend(old.width..new.width),
                travel: new.width - old.width,
            });
        }
        let old_groups = groups(&old_sources);
        let new_groups = groups(&new_sources);
        let new_indices: BTreeMap<_, _> = new
            .clusters
            .iter()
            .enumerate()
            .map(|(i, c)| ((c.source.start, c.source.end), i))
            .collect();
        let mut anchors = Vec::new();
        let mut preceding_new = None;
        for (oi, oc) in old.clusters.iter().enumerate() {
            let mapped = if oc.source.end <= a {
                Some((oc.source.start, oc.source.end))
            } else if oc.source.start >= b {
                oc.source
                    .start
                    .checked_sub(b)
                    .and_then(|s| s.checked_add(c))
                    .zip(oc.source.end.checked_sub(b).and_then(|e| e.checked_add(c)))
            } else {
                None
            };
            let Some((start, end)) = mapped else { continue };
            let Some(&ni) = new_indices.get(&(start, end)) else {
                continue;
            };
            // Keep actual visual order. Reordered or contextually reshaped
            // clusters belong to complete old/current replacement subregions.
            if preceding_new.is_some_and(|previous| ni <= previous) {
                continue;
            }
            let nc = &new.clusters[ni];
            let Some(og) = old_groups.get(&(oc.source.start, oc.source.end)) else {
                return Err(Refusal::GlyphEvidence);
            };
            let Some(ng) = new_groups.get(&(start, end)) else {
                return Err(Refusal::GlyphEvidence);
            };
            if old_ends[oi] - oc.x_start != new_ends[ni] - nc.x_start
                || oc.direction != nc.direction
                || og.len() != ng.len()
                || !og.iter().zip(ng).all(|(&o, &n)| {
                    let x = &old.glyphs[o];
                    let y = &new.glyphs[n];
                    (x.id, x.x - oc.x_start, x.y, x.advance)
                        == (y.id, y.x - nc.x_start, y.y, y.advance)
                })
            {
                continue;
            }
            anchors.push((oi, ni));
            preceding_new = Some(ni);
        }
        let mut parts = Vec::new();
        let (mut old_start, mut new_start) = (0.0, 0.0);
        for (oi, ni) in anchors {
            let old_left = old.clusters[oi].x_start;
            let new_left = new.clusters[ni].x_start;
            if old_start != old_left || new_start != new_left {
                parts.push(Part {
                    old: old_start..old_left,
                    new: new_start..new_left,
                    stable: false,
                });
            }
            parts.push(Part {
                old: old_left..old_ends[oi],
                new: new_left..new_ends[ni],
                stable: true,
            });
            old_start = old_ends[oi];
            new_start = new_ends[ni];
        }
        if old_start != old.width || new_start != new.width {
            parts.push(Part {
                old: old_start..old.width,
                new: new_start..new.width,
                stable: false,
            });
        }
        let mut travel = 0.0_f32;
        for part in &parts {
            travel = travel
                .max((part.new.start - part.old.start).abs())
                .max((part.new.end - part.old.end).abs());
            if !part.stable {
                travel = travel
                    .max(part.old.end - part.old.start)
                    .max(part.new.end - part.new.start);
            }
        }
        if !travel.is_finite() {
            return Err(Refusal::Geometry);
        }
        if travel == 0.0 {
            return Err(Refusal::NoVisibleTravel);
        }
        Ok(Self {
            geometry: Geometry::Context(parts),
            travel,
        })
    }

    /// A scalar phase remap is valid only when it preserves every displayed
    /// interval, not merely the total advance. Context replacements with an
    /// independently moving old/current split need additional origin geometry.
    pub fn retarget_geometry_matches(&self, previous: &Self, ratio: f32) -> bool {
        if !ratio.is_finite() || ratio <= 0.0 {
            return false;
        }
        match (&self.geometry, &previous.geometry) {
            (Geometry::StationaryAppend(next), Geometry::StationaryAppend(prior)) => {
                next.start == prior.start
                    && prior.end - prior.start == (next.end - next.start) * ratio
            }
            (Geometry::Context(next), Geometry::Context(prior)) => {
                next.len() == prior.len()
                    && next.iter().zip(prior).all(|(next, prior)| {
                        next.old == prior.old
                            && next.stable == prior.stable
                            && prior.new.start - prior.old.start
                                == (next.new.start - next.old.start) * ratio
                            && prior.new.end - prior.old.end
                                == (next.new.end - next.old.end) * ratio
                            && (next.stable
                                || (next.old.start == next.old.end
                                    && next.new.start == prior.new.start)
                                || (next.new.start == next.new.end
                                    && prior.new.start == prior.new.end)
                                || (ratio == 1.0 && next.new == prior.new))
                    })
            }
            _ => false,
        }
    }

    /// Reusing a current-source draw requires the actual unchanged prefix glyphs
    /// at the same coordinates in the latest complete shape. This is a proof
    /// over admitted source ranges, not a script or byte-count classifier.
    pub fn current_prefix_matches(
        before: &GlyphRun,
        current: &GlyphRun,
        shaper: Option<&TextShaper>,
        scale: f32,
    ) -> bool {
        let (short, long) = if before.text.len() <= current.text.len() {
            (before, current)
        } else {
            (current, before)
        };
        if !long.text.starts_with(&short.text) || short.binding != long.binding {
            return false;
        }
        let Some(short_sources) = glyph_source_ranges(short, shaper, scale) else {
            return false;
        };
        let Some(long_sources) = glyph_source_ranges(long, shaper, scale) else {
            return false;
        };
        Self::prefix_glyphs_match(short, long, &short_sources, &long_sources)
    }

    fn prefix_glyphs_match(
        short: &GlyphRun,
        long: &GlyphRun,
        short_sources: &[Range<usize>],
        long_sources: &[Range<usize>],
    ) -> bool {
        let mut actual = long
            .glyphs
            .iter()
            .zip(long_sources)
            .filter(|(_, source)| source.end <= short.text.len());
        short
            .glyphs
            .iter()
            .zip(short_sources)
            .all(|(glyph, source)| {
                actual.next().is_some_and(|(other, other_source)| {
                    // Each complete run's advances were verified above. Ink
                    // identity uses the bound glyph and its actual position;
                    // terminal kerning may change advance without moving ink.
                    source == other_source
                        && (glyph.id, glyph.x, glyph.y) == (other.id, other.x, other.y)
                })
            })
            && actual.next().is_none()
    }

    /// Emit only visible, adjacent-coalesced whole-run draws. The scratch work
    /// scales with admitted shape clusters, never source bytes times glyphs.
    pub fn fragments(&self, t: f32, visible: Range<f32>) -> Result<Vec<Fragment>, Refusal> {
        if !t.is_finite()
            || !(0.0..=1.0).contains(&t)
            || !visible.start.is_finite()
            || !visible.end.is_finite()
            || visible.start > visible.end
        {
            return Err(Refusal::Geometry);
        }
        // Exact endpoint ink comes from the complete original/current run,
        // including overhang outside its advance width inside the real viewport.
        if t == 0.0 || t == 1.0 {
            return Ok(vec![Fragment {
                source: if t == 0.0 {
                    Source::Old
                } else {
                    Source::Current
                },
                clip: visible,
                offset: 0.0,
            }]);
        }
        let parts = match &self.geometry {
            Geometry::Context(parts) => parts,
            Geometry::StationaryAppend(reveal) => {
                let end = (reveal.start + (reveal.end - reveal.start) * t).min(visible.end);
                if end <= visible.start {
                    return Ok(Vec::new());
                }
                return Ok(vec![Fragment {
                    source: Source::Current,
                    clip: visible.start..end,
                    offset: 0.0,
                }]);
            }
        };
        let mut result: Vec<Fragment> = Vec::new();
        let mut emit = |source, start: f32, end: f32, offset: f32| -> Result<(), Refusal> {
            if ![start, end, offset].iter().all(|v| v.is_finite()) || end < start {
                return Err(Refusal::Geometry);
            }
            let start = start.max(visible.start);
            let end = end.min(visible.end);
            if end <= start {
                return Ok(());
            }
            if let Some(last) = result.last_mut().filter(|last| {
                last.source == source && last.offset == offset && last.clip.end == start
            }) {
                last.clip.end = end;
            } else {
                result.push(Fragment {
                    source,
                    clip: start..end,
                    offset,
                });
            }
            Ok(())
        };
        for part in parts {
            let left = part.old.start + (part.new.start - part.old.start) * t;
            if part.stable {
                emit(
                    Source::Current,
                    left,
                    left + part.new.end - part.new.start,
                    left - part.new.start,
                )?;
            } else {
                let right = part.old.end + (part.new.end - part.old.end) * t;
                let boundary = if part.old.start == part.old.end {
                    left
                } else if part.new.start == part.new.end {
                    right
                } else {
                    (left + (part.old.end - part.old.start) * (1.0 - t)).min(right)
                };
                emit(Source::Old, left, boundary, left - part.old.start)?;
                emit(Source::Current, boundary, right, boundary - part.new.start)?;
            }
        }
        Ok(result)
    }
}
