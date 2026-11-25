# SPEC-HAL-085 Compliance Audit Report

**Audit Date:** 2026-01-14
**Scope:** Shader-level contracts only
**Mode:** Read-only, non-destructive
**Reference:** SPEC-HAL-085 — Halcyon Shader Suite v1

---

## 1. Module Set (Section 4)

| Module | Path | Status |
|--------|------|--------|
| common.wgsl | src/renderer/shaders/common.wgsl | **PASS** |
| rect.wgsl | src/renderer/shaders/rect.wgsl | **PASS** |
| shape.wgsl | src/renderer/shaders/shape.wgsl | **PASS** (placeholder) |
| path.wgsl | src/renderer/shaders/path.wgsl | **PASS** |
| text.wgsl | src/renderer/shaders/text.wgsl | **PASS** |
| text_macos.wgsl | src/renderer/shaders/text_macos.wgsl | **PASS** |
| image.wgsl | src/renderer/shaders/image.wgsl | **PASS** |
| clip.wgsl | src/renderer/shaders/clip.wgsl | **PASS** |
| debug.wgsl | src/renderer/shaders/debug.wgsl | **PASS** |

**Verdict:** **PASS** — All 9 canonical modules exist at specified paths.

---

## 2. Global Uniforms (INV-SHADER-UBO-1)

**SPEC Requirement:** 80-byte `Globals` struct at `@group(0) @binding(0)` for all pipelines.

| Component | SPEC Spec | Rust (wgpu_renderer.rs:108) | WGSL Modules |
|-----------|----------|------------------------------|--------------|
| view_proj | mat4x4<f32> (64B) | `[[f32; 4]; 4]` | All match |
| surface_size | vec2<f32> (8B) | `[f32; 2]` | All match |
| time_seconds | f32 (4B) | `f32` | All match |
| flags | u32 (4B) | `u32` | All match |
| **Total** | **80 bytes** | **80 bytes** (verified by test) | **80 bytes** |

**Verdict:** **PASS** — Globals struct byte-exact across Rust and all WGSL modules.

---

## 3. Bind Group 0 (INV-SHADER-BIND-0)

**SPEC Requirement:** `@group(0)` reserved for Globals at `@binding(0)` only. No additional bindings.

| Module | Binding | Status |
|--------|---------|--------|
| rect.wgsl | @group(0) @binding(0) Globals only | **PASS** |
| path.wgsl | @group(0) @binding(0) Globals only | **PASS** |
| text.wgsl | @group(0) @binding(0) Globals only | **PASS** |
| text_macos.wgsl | @group(0) @binding(0) Globals only | **PASS** |
| image.wgsl | @group(0) @binding(0) Globals only | **PASS** |
| clip.wgsl | @group(0) @binding(0) Globals only | **PASS** |
| debug.wgsl | @group(0) @binding(0) Globals only | **PASS** |
| shape.wgsl | No group(0) binding | **AMBIGUOUS** (placeholder) |

**Verdict:** **PASS** (shape.wgsl is documented placeholder)

---

## 4. Entry Points (INV-SHADER-ENTRY-1, INV-SHADER-ENTRY-2)

**SPEC Requirement:** Each module must provide `vs_main` and `fs_main -> @location(0) vec4<f32>`.

| Module | vs_main | fs_main | Status |
|--------|---------|---------|--------|
| rect.wgsl | Line 46 | Line 67 | **PASS** |
| path.wgsl | Line 54 | Line 87 | **PASS** |
| text.wgsl | Line 58 | Line 78 | **PASS** |
| text_macos.wgsl | Line 58 | Line 77 | **PASS** |
| image.wgsl | Line 38 | Line 48 | **PASS** |
| clip.wgsl | Line 41 | Line 52 | **PASS** |
| debug.wgsl | Line 35 | Line 43 | **PASS** |
| shape.wgsl | Line 26 | Line 35 | **PASS** |

**Verdict:** **PASS** — All modules have correct entry points.

---

## 5. Vertex Input Paths (INV-SHADER-VERT-1, INV-SHADER-VERT-2)

**SPEC Requirement:** All pipelines MUST use Path A (QuadVertex+QuadInstance) or Path B (BakedVertex) **exactly as defined**. Additional attributes beyond extra0/extra1/extra are **forbidden**.

### Findings:

| Module | Vertex Format | SPEC Compliance |
|--------|---------------|----------------|
| rect.wgsl | pos(0), color(1) | **PASS** — Path B subset |
| image.wgsl | pos(0), uv(1), color(2) | **PASS** — Path B subset |
| clip.wgsl | pos(0), uv(1), color(2) | **PASS** — Path B subset |
| debug.wgsl | pos(0), color(1) | **PASS** — Path B subset |
| shape.wgsl | pos(0), color(1) | **PASS** — Path B subset |
| **path.wgsl** | pos(0), color(1), transform_0(2), transform_1(3), clip_id(4), stroke_flag(5) | **FAIL** |
| **text.wgsl** | pos(0), uv(1), color(2), screen_uv(3), downsample_u(4) | **FAIL** |
| **text_macos.wgsl** | pos(0), uv(1), color(2), screen_uv(3), downsample_u(4) | **FAIL** |

### Deviation Details:

**path.wgsl (lines 35-43):**
- Uses 6 attribute locations for custom PathVertex + PathInstance format
- `transform_0`, `transform_1`, `clip_id`, `stroke_flag` are NOT in SPEC Path A or Path B
- Rust side (path_pipeline.rs:124-219) confirms this custom format

**text.wgsl / text_macos.wgsl (lines 21-27):**
- Uses 5 attribute locations including `screen_uv` and `downsample_u`
- These extra fields are NOT in SPEC Path A or Path B
- Added for LCD subpixel AA (SPEC-HAL-064 Phase 5, R-010B)

**Verdict:** **FAIL** — 3 modules violate INV-SHADER-VERT-1/VERT-2.

---

## 6. Coordinate Transform (INV-SHADER-COORD-2)

**SPEC Requirement:** Only the vertex shader converts Absolute Logical → NDC using `Globals.view_proj`.

| Module | Uses view_proj in vs_main | Status |
|--------|---------------------------|--------|
| rect.wgsl | `globals.view_proj * vec4(pos, 0, 1)` (line 49) | **PASS** |
| path.wgsl | `globals.view_proj * vec4(logical_pos, 0, 1)` (line 78) | **PASS** |
| image.wgsl | `globals.view_proj * vec4(pos, 0, 1)` (line 40) | **PASS** |
| clip.wgsl | `globals.view_proj * vec4(pos, 0, 1)` (line 43) | **PASS** |
| debug.wgsl | `globals.view_proj * vec4(pos, 0, 1)` (line 37) | **PASS** |
| **text.wgsl** | `vec4(input.position, 0, 1)` — NO view_proj (line 60) | **FAIL** |
| **text_macos.wgsl** | `vec4(input.position, 0, 1)` — NO view_proj (line 60) | **FAIL** |
| shape.wgsl | `vec4(input.position, 0, 1)` — NO view_proj (line 28) | **AMBIGUOUS** (placeholder) |

**Evidence:** text_vertex.rs:14 documents `/// Position in NDC` — CPU provides pre-transformed coordinates.

**Verdict:** **FAIL** — text.wgsl and text_macos.wgsl bypass shader-side coordinate transform.

---

## 7. Premultiplied Alpha Output (INV-SHADER-ALPHA-1)

**SPEC Requirement:** All fragment outputs MUST be premultiplied RGBA: `(r*a, g*a, b*a, a)`.

| Module | Output Math | Status |
|--------|-------------|--------|
| rect.wgsl:74 | `vec4(c.rgb * c.a, c.a)` | **PASS** |
| rect.wgsl:186 | `vec4(c.rgb * final_alpha, final_alpha)` | **PASS** |
| path.wgsl:101 | `vec4(c.rgb * c.a, c.a)` | **PASS** |
| text.wgsl:86 | `vec4(c.rgb * c.a, c.a)` | **PASS** |
| text.wgsl:113-118 | `(r*a*cov, g*a*cov, b*a*cov, a*max_cov)` | **PASS** |
| clip.wgsl:59 | `input.color * coverage` | **PASS** |
| debug.wgsl:47 | `vec4(c.rgb * c.a, c.a)` | **PASS** |
| **image.wgsl:53-58** | See below | **AMBIGUOUS** |

**image.wgsl concern (lines 53-58):**
```wgsl
let final_color = vec4<f32>(
    input.color.rgb * tex_color.a,
    input.color.a * tex_color.a
) * tex_color;
```
This produces: `(color.rgb * tex.a * tex.rgb, color.a * tex.a * tex.a)` — mathematically unusual. Requires verification against intended blend behavior.

**Verdict:** **AMBIGUOUS** — image.wgsl output math needs verification.

---

## 8. Bind Group 1 Layouts (INV-SHADER-BIND-1)

**SPEC Section 7.2 specifies:**
- Text: binding(0)=atlas, binding(1)=sampler, binding(2)=optional UBO
- Image: binding(0)=texture, binding(1)=sampler
- Clip: binding(0)=mask, binding(1)=sampler

| Module | Actual Bindings | SPEC Spec | Status |
|--------|-----------------|----------|--------|
| text.wgsl | 0=atlas, 1=sampler, 2=bg_tex, 3=bg_sampler, 4=text_params | 0=atlas, 1=sampler, 2=UBO | **FAIL** |
| image.wgsl | 0=texture, 1=sampler | 0=texture, 1=sampler | **PASS** |
| clip.wgsl | 0=mask, 1=sampler | 0=mask, 1=sampler | **PASS** |
| rect.wgsl | 0=RoundedClip UBO | "optional material UBO" | **PASS** |
| path.wgsl | 0=clips storage buffer | Not specified | **AMBIGUOUS** |

**text.wgsl deviation:** Has 5 bindings (0-4) vs SPEC's 3. Extra bindings (bg_tex, bg_sampler) added for LCD AA background sampling.

**Verdict:** **FAIL** — text.wgsl exceeds SPEC bind group layout.

---

## 9. SPEC Document Accuracy

| Claim | Reality | Status |
|-------|---------|--------|
| "Path A or Path B exactly as defined" | path.wgsl, text.wgsl use custom formats | **STALE** |
| Text group(1) layout (3 bindings) | text.wgsl has 5 bindings | **STALE** |
| "vertex shader converts Absolute Logical → NDC" | text pipelines receive NDC directly | **STALE** |
| path.wgsl group(1) layout | Not documented in SPEC | **MISSING** |

---

## Summary Checklist

| Invariant | Status |
|-----------|--------|
| Module set complete | **PASS** |
| Globals struct (80 bytes) | **PASS** |
| Group 0 = Globals only | **PASS** |
| Entry points vs_main/fs_main | **PASS** |
| Vertex formats (Path A or B) | **FAIL** (path, text, text_macos) |
| Coordinate transform via view_proj | **FAIL** (text, text_macos) |
| Premultiplied alpha output | **AMBIGUOUS** (image.wgsl) |
| Group 1 layouts per SPEC | **FAIL** (text.wgsl) |
| SPEC document accuracy | **STALE** (3+ claims outdated) |

---

## Audit Verdict

**SPEC-HAL-085 is PARTIALLY COMPLIANT.**

Core invariants (Globals, group 0, entry points) are upheld. However:
- 3 modules violate vertex format constraints
- 2 modules bypass shader-side coordinate transform
- 1 module exceeds bind group layout specification
- SPEC document contains stale claims not matching implementation

---

## Recommended Actions

1. **SPEC Amendment Required:** Update SPEC-HAL-085 to document:
   - Path pipeline vertex format (PathVertex + PathInstance)
   - Text pipeline vertex format (with screen_uv, downsample_u)
   - Text pipeline bind group layout (5 bindings for LCD AA)
   - Text pipeline coordinate convention (CPU-side NDC transform)

2. **Verify image.wgsl:** Confirm premultiplied alpha math is intentional.

3. **shape.wgsl:** Either implement to spec or document as permanent placeholder.

---

**End of Audit Report**
