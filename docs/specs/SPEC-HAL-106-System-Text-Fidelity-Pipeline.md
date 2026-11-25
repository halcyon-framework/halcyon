# SPEC-HAL-106: System Text Fidelity Pipeline

SwiftUI-Parity Text Rendering: Optical Sizes, Metrics, Gamma, and Pixel Rules

Status: Design
Author: JC
Target: Renderer + Text Stack
Depends on:
- SPEC-HAL-048 (Paragraph & Text Layout Engine)
- SPEC-HAL-050 (Shaping Integration / Complex Shaping)
- SPEC-HAL-051 (Font Loading & Runtime Injection)
- SPEC-HAL-064 (macOS Subpixel Text Renderer) if present / related
- SPEC-HAL-103 (System UI Font Discovery / Bootstrap)

Related:
- SPEC-HAL-024 (Coordinate Model)
- SPEC-HAL-026 (Dirty Flag / Layout Invalidation)
- SPEC-HAL-0xx (Renderer pipelines, image/glyph atlas, batching)

---

## 0. Goal

Match SwiftUI-level perceived text quality on macOS and Windows under real UI conditions:
- small sizes (6–14)
- mixed sizes in one screen
- fractional layout coordinates (0.5, 0.25 etc)
- scrolling and transforms
- different backgrounds
- both light and dark UI

This SPEC is not “make it nicer”. It defines the exact *fidelity pipeline* required for parity-level results.

---

## 1. Non-Goals

- IME, selection, cursor rendering (separate SPECs)
- Variable-font axis management beyond what is needed for system font selection
- Color font rendering (COLR/CPAL, CBDT/CBLC, sbix) unless needed for system UI
- Arbitrary text effects (stroke, shadow) unless they break fidelity pipeline

---

## 2. What “SwiftUI parity” actually means (practical definition)

SwiftUI parity in Halcyon means:

1) **Stroke edges don’t look “milk glass”**
    - No washed-out stems at small sizes
    - No gray halo from incorrect blending/gamma

2) **Spacing and line rhythm look native**
    - inter-glyph spacing visually stable during scrolling
    - baseline and line height consistent across sizes

3) **Small text remains readable**
    - counters stay open
    - stems retain contrast
    - diagonals don’t staircase excessively

4) **No “wobble”**
    - No jitter when animating/scrolling by fractional offsets
    - No per-frame pixel snapping mismatch between glyph bitmap sampling and quad placement

This SPEC defines the invariants and exact checks to ensure those four outcomes.

---

## 3. Root causes of “not quite SwiftUI” (the real list)

If you have the correct system font and shaping, the remaining fidelity gap is almost always from:

A) **Optical size / font selection**
- Swift uses different optical masters (Text vs Display) or implicit selection rules

B) **Metrics policy mismatch**
- integer-only metrics vs fractional metrics
- inconsistent rounding policy between layout and rasterization
- ascent/descent/leading mismatch vs Apple’s UI metrics

C) **Gamma/linear pipeline mismatch**
- glyph coverage blended in wrong space (sRGB vs linear)
- background compositing done wrong
- atlas stored in a format that forces incorrect sampling

D) **Subpixel / AA path mismatch**
- grayscale AA vs subpixel AA differences
- platform rasterizers differ; Swift uses platform-native pipeline rules

E) **Quad placement mismatch**
- glyph bitmap sampled at fractional UV but quad snapped differently
- baseline alignment wrong by fractions
- per-run rounding differs from per-glyph rounding

This SPEC provides a single “system text fidelity pipeline” that locks all those down.

---

## 4. Architecture Overview

### 4.1 Concepts

- **Font Identity**: (family name, postscript/full name, weight, italic, source)
- **Optical Mode**: (Text, Display)
- **Metrics Mode**: (Integer, Fractional)
- **Raster Mode**: (Grayscale AA, Subpixel AA, None)
- **Compositing Mode**: (Gamma-correct, Legacy)

These modes must be explicit in the code, not implicit behavior.

### 4.2 Required data flow

1) Text shaping produces glyph indices + advances
2) Layout computes glyph positions in *logical space* using chosen metrics mode
3) Glyph rasterization occurs in *platform space* with consistent DPI and metrics mode
4) Glyph atlas stores coverage in a format compatible with gamma-correct blending
5) Renderer places quads with the same rounding policy used for rasterization
6) Compositing blends glyph coverage onto target in correct color space

---

## 5. Terminology

- **Coverage**: alpha-like value representing pixel coverage of glyph shape
- **Linear space**: linear RGB (not gamma-encoded)
- **sRGB**: gamma-encoded RGB space
- **AA**: anti-aliasing
- **Subpixel AA**: RGB stripe-based coverage
- **Fractional metrics**: glyph positions/advances allowed to be non-integers
- **Optical size**: typographic master used for size ranges (Text vs Display)

---

## 6. Platform baseline expectations

### 6.1 macOS

SwiftUI (and AppKit) commonly uses:
- SF system font with internal selection rules
- grayscale AA in many contexts (subpixel AA depends on system settings and layer type)
- high-quality hinting + platform raster rules
- compositing consistent with CoreGraphics expectations

### 6.2 Windows

Native UI is typically:
- Segoe UI
- ClearType/subpixel AA in many contexts
- DirectWrite metrics and rendering mode decisions

Halcyon must match native expectations per OS.

---

## 7. Implementation requirements (must-have)

### 7.1 Optical size selection (Text vs Display)

#### 7.1.1 Requirement
Introduce an **OpticalMode** and select the appropriate font face or variation for the system UI font.

#### 7.1.2 Policy
- Default: **Text optical mode** for typical UI sizes (6–20)
- Display mode for large headings (exact threshold defined below)

#### 7.1.3 Threshold (hard requirement)
- OpticalMode::Text for size < 20
- OpticalMode::Display for size >= 20
  This is a starting policy; may be tuned but must remain explicit.

#### 7.1.4 macOS specifics
If the system font family exposes separate faces or PostScript names for Text vs Display, map them explicitly.
Examples often seen:
- `.SFNS` variants
- `SF Pro Text` / `SF Pro Display` (names vary)
  You must detect what is actually present on the machine via CoreText and record the mapping.

#### 7.1.5 Windows specifics
Segoe UI may be variable; optical axis may not exist. Still implement mode plumbing but selection may be identity.

#### 7.1.6 Deliverables
- `enum OpticalMode { Text, Display }`
- `FontIdentity` stores optical mode
- `FontManagerTextEngine` selects the correct face per size/optical mode
- Logs (debug-only): chosen optical mode per run when font is resolved

---

### 7.2 Metrics mode (Fractional vs Integer) must be unified end-to-end

#### 7.2.1 Requirement
A single metrics policy must drive:
- shaping advances
- layout glyph positioning
- rasterizer positioning assumptions
- quad placement rounding

No “fractional here, integer there” drift.

#### 7.2.2 Policy (macOS)
- Default: Fractional metrics ON for UI text (matches typical Apple rhythm)
- Pixel snapping must be applied at *final placement* only, and must be consistent

#### 7.2.3 Policy (Windows)
- Use DirectWrite’s recommended metrics (often fractional)
- If ClearType is used, placement policy must align with subpixel layout

#### 7.2.4 Deliverables
- `enum MetricsMode { Integer, Fractional }`
- MetricsMode stored in text run / paragraph layout config
- A single rounding function used everywhere:
    - `fn snap_x(x: f32, mode: SnapMode) -> f32`
    - `fn snap_y(y: f32, mode: SnapMode) -> f32`

---

### 7.3 Pixel snapping and rounding rules (the “no wobble” contract)

#### 7.3.1 Requirement
Define EXACT rounding rules for:
- baseline Y
- glyph origin X
- line origin Y
- atlas sampling UV alignment

#### 7.3.2 Hard rules
1) **Do not snap during shaping**.
2) **Do not snap advances per glyph** unless OS rendering mode requires it.
3) Snap only at:
    - line origin and baseline
    - final quad placement
4) UV sampling must be stable:
    - atlas rect coordinates must be integer texel-aligned
    - padding must guarantee sampling does not bleed

#### 7.3.3 SnapMode
- `SnapMode::None`
- `SnapMode::Pixel` (snap to nearest physical pixel)
- `SnapMode::HalfPixel` (only if pipeline requires; default off)
- `SnapMode::SubpixelX(n)` (quantize X to 1/n pixel steps)

#### 7.3.4 Recommended defaults
- macOS grayscale AA: `SubpixelX(4)` or Pixel (final decision after measurement)
- Windows ClearType: `SubpixelX(3)` often aligns with RGB stripes

These are not guesses; implement both and select via platform config.

#### 7.3.5 Deliverables
- Central `snap.rs` with all snapping logic
- Unit tests for snapping invariants
- One integration test that scrolls by fractional offsets and asserts stable glyph bbox positions frame-to-frame (internal test)

---

### 7.4 Gamma-correct glyph blending (the “washed-out stems” killer)

#### 7.4.1 Requirement
Glyph coverage must be composited in a gamma-correct manner.

If you blend coverage in sRGB, you get gray, low-contrast stems. Swift doesn’t do that.

#### 7.4.2 Correct model (coverage compositing)
- Glyph atlas stores coverage (A) as a linear scalar (0..1)
- Text color is in sRGB UI space but must be converted to linear for blending
- Background is linear in the render target pipeline (or treated as such)

Blend equation (in linear space):
`out_rgb = src_rgb * A + dst_rgb * (1 - A)`

#### 7.4.3 Implementation options
- Preferred: Render target in sRGB format but sampling/fragment converts to linear and writes out with correct pipeline
- Or: Keep render target linear and convert at final present

The choice must be explicit and consistent with WGPU formats.

#### 7.4.4 Deliverables
- Text pipeline shader updated to gamma-correct compositing
- Atlas format verified (coverage stored in single channel)
- A/B visual test toggle in debug builds:
    - `HAL_TEXT_GAMMA=0/1`

---

### 7.5 Subpixel vs grayscale AA mode selection (platform accurate)

#### 7.5.1 Requirement
Expose `RasterMode`:
- `GrayscaleAA`
- `SubpixelAA`
- `Monochrome`

On macOS, subpixel AA may not be stable or even available in the same way depending on layer type. SwiftUI often ends up grayscale in modern composited pipelines. Halcyon must support grayscale AA as “default correct”.

#### 7.5.2 Deliverables
- `RasterMode` integrated into text raster backend selection
- `TextShaper/Rasterizer` returns coverage in the correct encoding:
    - grayscale: 1 channel coverage
    - subpixel: 3 channel coverage (RGB)
- Renderer supports both:
    - grayscale: simple coverage blend
    - subpixel: per-channel coverage blend (still gamma-correct)

---

### 7.6 Line metrics: ascent/descent/leading must match platform rhythm

#### 7.6.1 Requirement
The line height must be computed from platform font metrics in a way that matches native UI.

Key: SwiftUI line rhythm often does NOT equal “ascent+descent” naive values. It incorporates leading and platform-specific rounding.

#### 7.6.2 Policy
Define a line metrics function:
`fn compute_line_metrics(font: &ResolvedFont, size: f32, metrics_mode: MetricsMode) -> LineMetrics`

LineMetrics includes:
- ascent
- descent
- leading
- line_height
- baseline_offset

#### 7.6.3 Deliverables
- Implement per-platform metrics extraction:
    - macOS: CoreText metrics
    - Windows: DirectWrite metrics
- Store computed LineMetrics in paragraph layout output
- Add a debug overlay mode that draws baseline and line boxes to validate visually

---

## 8. Stage Plan (implementation order)

This SPEC is executed in stages, because mixing these changes blindly is how you lose weeks.

### Stage 1 — Plumbing and mode types
- Add OpticalMode, MetricsMode, RasterMode, CompositingMode
- Thread these through font selection, paragraph layout config, renderer text pipeline config
- No behavior change yet (default values mirror current behavior)

Exit criteria:
- Compiles on macOS + Windows
- Existing examples render identically as before (unless Stage 4/5 already changed font)

### Stage 2 — Optical size selection (macOS priority)
- Implement detection/mapping for Text vs Display system faces on macOS
- Use size threshold rule (Text < 20, Display >= 20)

Exit criteria:
- Logs show correct face identity selection per size bucket
- Visual: large sizes gain “display” feel; small sizes keep crisp UI rhythm

### Stage 3 — Unified snapping + no wobble contract
- Introduce snap.rs and enforce snapping at final placement only
- Add scroll test that moves by fractional steps and validates stability

Exit criteria:
- No wobble under scroll
- No overlap regressions in text_quality_test due to rounding drift

### Stage 4 — Gamma-correct compositing
- Implement linear blending for glyph coverage
- Add debug toggle to compare old vs new

Exit criteria:
- Small sizes gain contrast (stems darken correctly)
- No halos on dark backgrounds

### Stage 5 — Raster mode selection + coverage formats
- Ensure grayscale path is first-class
- Subpixel path implemented (even if disabled by default on macOS)

Exit criteria:
- Both paths render correctly
- No atlas bleed
- No color fringing unless subpixel is enabled

### Stage 6 — Line metrics parity pass
- Replace naive line height with platform-aware line metrics function
- Add baseline/line debug overlay

Exit criteria:
- Line rhythm visually matches native UI within tolerance
- No clipping of ascenders/descenders

---

## 9. Testing Strategy

### 9.1 Visual tests (must exist)
1) `text_quality_test`:
    - sizes 6..55
    - dark background + light background toggle
    - fractional scroll offsets
2) Mixed-run test:
    - one paragraph with multiple sizes, weights, italic
3) UI baseline overlay:
    - draw baseline and line boxes (debug mode)

### 9.2 Snapshot tolerance (practical)
Text rendering is not bit-identical across GPUs, but internal invariants can be tested:
- glyph bbox stability under scroll (positions must be stable)
- atlas rects must be integer aligned
- snapping functions must be deterministic

### 9.3 Diagnostics (debug-only)
- Log resolved font identity:
    - family, full name, postscript name
    - optical mode
- Log raster mode
- Log compositing mode

---

## 10. Data model additions

### 10.1 Types (required)
- `OpticalMode`
- `MetricsMode`
- `RasterMode`
- `CompositingMode`
- `LineMetrics`
- `ResolvedFontIdentity` (includes optical mode and platform source)

### 10.2 Configuration entry points
- `TextStyle` gains optional:
    - optical_mode override (default auto)
    - metrics_mode override (default platform)
    - raster_mode override (default platform)
- Global defaults:
    - per-platform config table, one place

---

## 11. Renderer requirements

### 11.1 Glyph atlas invariants
- atlas rects integer-aligned (x,y,w,h in texels)
- padding >= 1 texel (recommend 2)
- UVs computed from integer rects
- bilinear sampling allowed but must not bleed (hence padding)

### 11.2 Shader requirements
- coverage sampling must be linear scalar(s)
- gamma-correct blending in shader or pipeline
- subpixel mode: coverage is vec3, blend per channel

### 11.3 Output target color space
- Must specify whether the swapchain surface is sRGB
- Must ensure conversion rules are coherent across pipelines

---

## 12. Platform backend requirements

### 12.1 macOS CoreText (Stage 5 of SPEC-HAL-103 was introduced)
- System font selection must support Text/Display identity resolution
- Font data extraction must produce valid font data for shaping/rasterization
- Metrics extraction must use CoreText-provided metrics

### 12.2 Windows DirectWrite
- System font selection already: Segoe UI Regular
- Must support ClearType/grayscale selection paths
- Must supply correct metrics

---

## 13. Performance constraints

- No per-frame font discovery
- Cache resolved font identities per (family/style/weight/optical mode)
- Cache line metrics per (font identity, size, metrics mode)
- Glyph atlas must remain stable; no per-frame rebuild churn from rounding drift

---

## 14. Migration / Compatibility

- Existing code that assumes a single font face must be updated to:
    - resolve a font face per (size, optical mode)
- Default behavior must remain “good” if optical mode selection cannot find a display face:
    - fallback to text face

---

## 15. Failure modes and required behavior

- If system font cannot be extracted:
    - fallback to Inter (legacy) as today
- If display face missing:
    - use text face
- If subpixel AA unsupported:
    - use grayscale AA
- If gamma pipeline cannot be enabled (format limitations):
    - must be surfaced clearly (debug log), keep existing behavior

No silent “almost worked” behavior.

---

## 16. Legal / IP / patents risk (project stance)

This SPEC targets:
- platform APIs (CoreText, DirectWrite)
- standard color-space compositing principles
- standard font metrics usage
- standard glyph atlas and blending techniques

This is not inventing a novel rendering algorithm; it is implementing known, conventional practices.

Hard line:
- Do not copy proprietary shader code or reverse-engineered private frameworks.
- Use documented OS APIs and public specs (OpenType, sRGB/linear blending rules).
- Avoid claiming parity by copying Apple’s internal heuristics; achieve parity by correct math + correct platform metrics.

Risk assessment:
- Implementing gamma-correct blending and metrics policies is standard practice and not a patent trap in itself.
- Subpixel AA has historical patents, but today this is typically implemented via OS pipelines (DirectWrite/CoreGraphics). Halcyon’s approach should prefer using OS-provided raster outputs rather than implementing novel subpixel reconstruction tricks.

If later a truly identical Apple private path is required, stop and reassess. This SPEC does not require it.

---

## 17. Completion criteria

SPEC-HAL-106 is "done" when:
1) On macOS: SF system font (Text + Display selection) renders with contrast and rhythm comparable to SwiftUI in the same sizes.
2) On Windows: Segoe UI renders with expected native clarity.
3) No wobble under fractional scroll.
4) Gamma toggle shows clear improvement; gamma-correct mode becomes default.
5) Debug overlay confirms baseline and line height stability.

---

## 17A. Control Text Placement Enforcement (Binding)

**Status:** BINDING — This is a fidelity enforcement rule.

### 17A.1 Invariant

**INV-TEXT-FIDELITY-3: Metrics-Based Control Placement**
Any control text placement that does not use actual font ascent/descent
metrics is considered a text fidelity violation.

Such placement MUST be rejected during review and is not eligible for
visual correctness claims.

### 17A.2 Rationale

Text fidelity requires that glyphs are placed where they will actually render.
Using `line_height` (a layout abstraction) instead of actual font metrics
(ascent + descent) causes a mismatch between placement intent and render reality.

This manifests as:
- Glyphs touching or crossing control edges
- Inconsistent baseline alignment across controls
- "Almost correct but not quite" visual appearance

### 17A.3 Enforcement

During code review and visual QA:
1. Controls MUST use `compute_control_text_baseline_y()` (SPEC-HAL-092 Section 8A)
2. Controls MUST NOT use `line_height` for vertical text placement
3. Any use of `(control_height - line_height) / 2` for control text is rejected

### 17A.4 Cross-References

- SPEC-HAL-092 Section 8A: Canonical formula
- SPEC-HAL-094 Section 3A: Glyph containment invariant
- `src/core/text_engine.rs`: `compute_control_text_baseline_y()`

---

## 18. Work estimate (with Claude doing implementation)

- Stage 1: 0.5–1 day
- Stage 2: 0.5–1 day
- Stage 3: 2–3 days
- Stage 4: 3–5 days
- Stage 5: 2–4 days
- Stage 6: 1–2 days

Total: 7–10 working days.

---

## 19. Appendix: Required engineering checklist (do-not-skip)

- [ ] Verify resolved font identity logs (postscript/full/family)
- [ ] Verify optical mode selection at threshold boundary
- [ ] Verify atlas rect integer alignment
- [ ] Verify quad placement uses the same snap policy as raster assumptions
- [ ] Verify gamma-correct blend path (toggle A/B)
- [ ] Verify line metrics with baseline overlay
- [ ] Verify fractional scroll stability test
- [ ] Verify fallback paths (no system font -> Inter)

