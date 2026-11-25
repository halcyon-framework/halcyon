//! SPEC-HAL-049: Font Resolution Pipeline
//!
//! Deterministic font resolution from logical styles to concrete LoadedFontFace objects.
//! Works only with fonts registered in FontManager; never consults OS APIs.

use crate::platform::font_manager::{
    FontFaceCatalog, FontFamily, FontManager, FontStack, FontWeight, ResolvedFontFace,
};

enum RegistryView<'a> {
    Manager(&'a FontManager),
    Catalog(&'a FontFaceCatalog),
}

/// Font resolver for deterministic style-to-face mapping (SPEC-049 §5).
///
/// Borrows a FontManager and provides pure resolution logic.
/// Never touches OS APIs; all faces come from the FontManager registry.
pub struct FontResolver<'a> {
    view: RegistryView<'a>,
}

impl<'a> FontResolver<'a> {
    /// Create a new FontResolver backed by a FontManager.
    pub fn new(font_manager: &'a FontManager) -> Self {
        Self {
            view: RegistryView::Manager(font_manager),
        }
    }

    pub fn with_catalog(catalog: &'a FontFaceCatalog) -> Self {
        Self {
            view: RegistryView::Catalog(catalog),
        }
    }

    fn faces(&self) -> Vec<ResolvedFontFace> {
        let mut faces: Vec<ResolvedFontFace> = match self.view {
            RegistryView::Manager(manager) => manager
                .faces()
                .map(|face| ResolvedFontFace::from_loaded(face))
                .collect(),
            RegistryView::Catalog(catalog) => {
                catalog.faces().map(|face| face.resolved.clone()).collect()
            }
        };
        faces.sort_by_key(|face: &ResolvedFontFace| face.font_id);
        faces
    }

    fn find(&self, family: &str, weight: FontWeight, italic: bool) -> Option<ResolvedFontFace> {
        match self.view {
            RegistryView::Manager(manager) => manager
                .find(family, weight, italic)
                .map(|face| ResolvedFontFace::from_loaded(&face)),
            RegistryView::Catalog(catalog) => catalog.find(family, weight, italic),
        }
    }

    /// Resolve a font stack to a concrete face (SPEC-049 §5 resolution pipeline).
    ///
    /// Resolution stages:
    /// 1. Primary matching within FontManager (stack order)
    /// 2. Generic fallback families (SansSerif → SystemUI → Serif → Monospace)
    /// 3. Last-resort fallback (highest coverage face)
    pub fn resolve(
        &self,
        stack: &FontStack,
        weight: FontWeight,
        italic: bool,
    ) -> Option<ResolvedFontFace> {
        // Stage 1: Primary matching within stack
        for family in stack.iter() {
            if let Some(face) = self.resolve_family(family, weight, italic) {
                return Some(face);
            }
        }

        // Stage 2: Generic fallback families
        let fallback_order = [
            FontFamily::SansSerif,
            FontFamily::SystemUI,
            FontFamily::Serif,
            FontFamily::Monospace,
        ];

        for family in &fallback_order {
            if let Some(face) = self.resolve_family(family, weight, italic) {
                return Some(face);
            }
        }

        // Stage 3: Last-resort fallback - any face with best coverage
        self.find_best_coverage_face(weight, italic)
    }

    /// Find a fallback face that covers a specific codepoint (SPEC-049 §5 Stage 4).
    ///
    /// Used during shaping when a glyph is missing from the primary face.
    /// Searches all registered faces for one that covers the codepoint.
    pub fn find_fallback_for_char(
        &self,
        ch: char,
        weight: FontWeight,
        italic: bool,
    ) -> Option<ResolvedFontFace> {
        // Collect faces that cover this codepoint
        let mut candidates: Vec<ResolvedFontFace> = self
            .faces()
            .into_iter()
            .filter(|face| face.coverage_map.covers(ch))
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Sort by style match quality, then by coverage count
        candidates.sort_by(|a, b| {
            let a_score = self.style_match_score(a, weight, italic);
            let b_score = self.style_match_score(b, weight, italic);
            // Higher score is better, so reverse
            b_score
                .cmp(&a_score)
                .then_with(|| a.font_id.cmp(&b.font_id))
        });

        candidates.first().cloned()
    }

    /// Coverage only filters trials. The paragraph owner must still shape the
    /// entire source cluster; cmap coverage alone cannot attest a shaped run.
    pub(crate) fn fallback_candidates_for_cluster(
        &self,
        text: &str,
        weight: FontWeight,
        italic: bool,
    ) -> Vec<ResolvedFontFace> {
        let mut candidates: Vec<_> = self
            .faces()
            .into_iter()
            .filter(|face| text.chars().any(|ch| face.coverage_map.covers(ch)))
            .collect();
        candidates.sort_by(|a, b| {
            self.style_match_score(b, weight, italic)
                .cmp(&self.style_match_score(a, weight, italic))
                .then_with(|| a.font_id.cmp(&b.font_id))
        });
        candidates
    }

    /// Resolve a single FontFamily to a LoadedFontFace.
    fn resolve_family(
        &self,
        family: &FontFamily,
        weight: FontWeight,
        italic: bool,
    ) -> Option<ResolvedFontFace> {
        match family {
            FontFamily::Named(name) => self.find(name, weight, italic),
            FontFamily::Serif => self.find_generic_family("Serif", weight, italic),
            FontFamily::SansSerif => self.find_generic_family("SansSerif", weight, italic),
            FontFamily::Monospace => self.find_generic_family("Monospace", weight, italic),
            FontFamily::SystemUI => self.find_generic_family("SystemUI", weight, italic),
        }
    }

    /// Find a face matching a generic family classification.
    ///
    /// This searches for faces that have been tagged with generic family names
    /// or whose family names suggest the classification.
    fn find_generic_family(
        &self,
        generic: &str,
        weight: FontWeight,
        italic: bool,
    ) -> Option<ResolvedFontFace> {
        // First, try exact match on generic name
        if let Some(face) = self.find(generic, weight, italic) {
            return Some(face);
        }

        // Search for faces whose names suggest the generic category
        let candidates: Vec<_> = self
            .faces()
            .into_iter()
            .filter(|face| self.matches_generic_category(&face.family_name, generic))
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Find best style match
        let mut best: Option<(&ResolvedFontFace, i32)> = None;
        for face in &candidates {
            let score = self.style_match_score(face, weight, italic);
            if best.is_none() || score > best.as_ref().unwrap().1 {
                best = Some((face, score));
            }
        }

        best.map(|(face, _)| face.clone())
    }

    /// Check if a family name suggests a generic category.
    fn matches_generic_category(&self, family_name: &str, generic: &str) -> bool {
        let name_lower = family_name.to_lowercase();
        match generic {
            "Serif" => {
                name_lower.contains("serif") && !name_lower.contains("sans")
                    || name_lower.contains("times")
                    || name_lower.contains("georgia")
                    || name_lower.contains("palatino")
            }
            "SansSerif" => {
                name_lower.contains("sans")
                    || name_lower.contains("arial")
                    || name_lower.contains("helvetica")
                    || name_lower.contains("inter")
                    || name_lower.contains("roboto")
                    || name_lower.contains("noto sans")
            }
            "Monospace" => {
                name_lower.contains("mono")
                    || name_lower.contains("courier")
                    || name_lower.contains("consolas")
                    || name_lower.contains("menlo")
                    || name_lower.contains("code")
            }
            "SystemUI" => {
                name_lower.contains("system")
                    || name_lower.contains("segoe")
                    || name_lower.contains("sf pro")
                    || name_lower.contains("cantarell")
            }
            _ => false,
        }
    }

    /// Calculate a style match score for weight/italic matching.
    ///
    /// Higher score = better match.
    fn style_match_score(&self, face: &ResolvedFontFace, weight: FontWeight, italic: bool) -> i32 {
        let mut score = 0;

        // Italic match is worth more
        if face.italic == italic {
            score += 1000;
        }

        // Weight distance (closer = higher score)
        let weight_diff = (face.weight.to_value() as i32 - weight.to_value() as i32).abs();
        score += 900 - weight_diff; // Max 900 for exact match

        // Coverage bonus (more coverage = higher score)
        score += (face.coverage_map.coverage_count() / 100) as i32;

        score
    }

    /// Find the face with the best coverage as last resort.
    fn find_best_coverage_face(
        &self,
        weight: FontWeight,
        italic: bool,
    ) -> Option<ResolvedFontFace> {
        let mut best: Option<(ResolvedFontFace, i32)> = None;

        for face in self.faces() {
            let score = self.style_match_score(&face, weight, italic);
            if best.is_none() || score > best.as_ref().unwrap().1 {
                best = Some((face, score));
            }
        }

        best.map(|(face, _)| face)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn registered_textblock_fallback_ties_follow_stable_face_id() {
        let bytes = std::sync::Arc::new(
            std::fs::read("/System/Library/Fonts/Supplemental/Arial Unicode.ttf").unwrap(),
        );
        for names in [["Coverage A", "Coverage B"], ["Coverage B", "Coverage A"]] {
            let mut manager = FontManager::new();
            let a = manager
                .load_font_memory(bytes.clone(), Some(names[0].to_owned()))
                .unwrap();
            let b = manager
                .load_font_memory(bytes.clone(), Some(names[1].to_owned()))
                .unwrap();
            let expected = a.min(b);
            assert_eq!(
                FontResolver::new(&manager)
                    .find_fallback_for_char('\u{770b}', FontWeight::Regular, false)
                    .unwrap()
                    .font_id,
                expected
            );
            assert_eq!(
                FontResolver::with_catalog(&manager.face_catalog())
                    .find_fallback_for_char('\u{770b}', FontWeight::Regular, false)
                    .unwrap()
                    .font_id,
                expected
            );
        }
    }

    #[test]
    fn test_empty_font_manager_returns_none() {
        let manager = FontManager::new();
        let resolver = FontResolver::new(&manager);

        let stack = FontStack::single(FontFamily::named("Inter"));
        let result = resolver.resolve(&stack, FontWeight::Regular, false);
        assert!(result.is_none());
    }

    #[test]
    fn test_font_family_creation() {
        let named = FontFamily::named("Inter");
        assert_eq!(named, FontFamily::Named("Inter".to_string()));

        let sans = FontFamily::SansSerif;
        assert_eq!(sans, FontFamily::SansSerif);
    }

    #[test]
    fn test_font_stack_building() {
        let stack = FontStack::single(FontFamily::named("Inter"))
            .with_fallback(FontFamily::SansSerif)
            .with_fallback(FontFamily::SystemUI);

        assert_eq!(stack.0.len(), 3);
        assert_eq!(stack.0[0], FontFamily::Named("Inter".to_string()));
        assert_eq!(stack.0[1], FontFamily::SansSerif);
        assert_eq!(stack.0[2], FontFamily::SystemUI);
    }
}
