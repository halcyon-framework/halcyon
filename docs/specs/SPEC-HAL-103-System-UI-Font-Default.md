# SPEC-HAL-103: Native SystemUI Font Pipeline & Default OS Font Adoption

System-UI font discovery, loading, and default typography per platform

**Status:** Design – Not Implemented
**Author:** JC

**Depends on:**

* SPEC-HAL-006 – Text & Font System
* SPEC-HAL-048 – Paragraph & Text Layout Engine
* SPEC-HAL-049 – Font Collections & Fallback Rules
* SPEC-HAL-050 – Unified Text Shaping Architecture
* SPEC-HAL-065 – Complex Text Shaping Validation
* SPEC-HAL-090–099 – Typography / Layout / Precision

**Planned follow-ups:**

* SPEC-HAL-104 – Font Fallback Stacks & User-Configurable Typography
* SPEC-HAL-105 – Font Discovery & Diagnostics (Debug Tools / Dev Overlay)

---

## 0. Purpose

This SPEC defines the **native SystemUI font pipeline** for Halcyon:

* How we discover and load **OS-native UI fonts** (Segoe UI / SF / system-ui stack).
* How `FontFamily::SystemUI` is **actually implemented** instead of being an alias for embedded Inter.
* How the **default text engine and renderer** are wired to SystemUI instead of the bundled Inter asset.
* How platform-specific fallbacks behave (Windows / macOS / Linux).

Current state from investigation:

* `system_font_provider` returns **embedded Inter-Regular.ttf**, not OS fonts.
* `FontManagerTextEngine` has **Inter-specific code paths** (`compute_font_id_for_inter` etc.).
* `WgpuRenderer` bootstraps shaping from **Inter data**, bypassing any SystemUI notion.
* `FontFamily::SystemUI` resolves only against fonts **already registered** in `FontManager`, never querying OS.
* Linux path hardcodes DejaVuSans via FreeType; no fontconfig, no discovery.

Goal: introduce a **real SystemUI pipeline** without breaking the existing text engine. Inter remains as a **bundled fallback**, not the primary default.

---

## 1. Role & Non-Goals

### 1.1 Role

SPEC-HAL-103 defines:

* A **platform-neutral contract** for “system UI font”:

    * **Windows:** Segoe UI family (or whatever the OS exposes as UI font).
    * **macOS:** SF system font via CoreText.
    * **Linux:** fontconfig-resolved `system-ui` / default UI font.
* A **SystemFontProvider API** that:

    * Can return raw font bytes + metadata for the default UI font.
    * Can be extended later to expose monospaced UI or weight variants.
* How `FontFamily::SystemUI` should resolve via `FontResolver` + `FontManager`, using real OS fonts.
* How `WgpuRenderer` and `FontManagerTextEngine` bootstrap from SystemUI instead of hardcoded Inter.

### 1.2 Non-Goals

This SPEC does **not** define:

* Arbitrary user font selection / font pickers.
* Full-blown font management UI or configuration files.
* Complex styled stacks like `"system-ui, -apple-system, BlinkMacSystemFont, ..."` (that’s SPEC-HAL-104 territory).
* Fine-grained platform-specific weight/optical-size gymnastics (SF text vs display, etc).

We keep it to:

> **“Give me the OS default UI text font, load it, and use it as `SystemUI`.”**

---

## 2. Core Types & Contracts

### 2.1 System Font Descriptor

New shared type in `src/platform/system_font_provider.rs`:

```rust
/// Platform-neutral description of a single system font face.
pub struct SystemFontDescriptor {
    /// UTF-8 family name (e.g., "Segoe UI", ".SF NS Text", "DejaVu Sans").
    pub family_name: String,

    /// PostScript / full name if available (for stable IDs & debugging).
    pub postscript_name: Option<String>,

    /// OS-specific style properties.
    pub weight: u16,   // 100–900
    pub italic: bool,

    /// Raw font bytes (entire font file or single-face stream).
    pub data: Vec<u8>,
}
```

This replaces the informal “here’s some Inter bytes, trust me” flow.

### 2.2 System Font Provider API

New/updated API:

```rust
pub mod system_font_provider {
    use super::SystemFontDescriptor;

    /// Return the primary system UI font face for text UI.
    pub fn load_default_system_ui_font() -> Option<SystemFontDescriptor> {
        // Platform-specific implementations under cfg(target_os)
    }

    /// Legacy: access embedded Inter as a fallback for tests/tools.
    pub fn load_legacy_embedded_inter() -> SystemFontDescriptor {
        // wraps existing static EMBEDDED_DEFAULT_FONT
    }
}
```

Rules:

* `load_default_system_ui_font()` must **never** silently use Inter. If platform discovery fails, it returns `None`.
* Embedding Inter remains available, but under a clearly named legacy API.

---

## 3. Integration Points

### 3.1 FontManagerTextEngine

File: `src/platform/font_manager_text_engine.rs`

Current issues:

* `compute_font_id_for_inter(data)` hardcodes `"Inter", weight=400, italic=false`.
* Text engine initialization assumes Inter as root font.

Required changes:

1. Replace Inter-specific ID pipeline with generic:

   ```rust
   pub struct FontIdentity {
       pub family_name: String,
       pub postscript_name: Option<String>,
       pub weight: u16,
       pub italic: bool,
   }

   impl FontManagerTextEngine {
       pub fn compute_font_id(identity: &FontIdentity) -> u64 { /* stable hash */ }
   }
   ```

2. Add constructor that uses SystemUI:

   ```rust
   impl FontManagerTextEngine {
       pub fn new_with_system_ui() -> Self {
           // 1. Try system_font_provider::load_default_system_ui_font()
           // 2. Register that font in FontManager as the primary face for FontFamily::SystemUI
           // 3. If that fails, fall back to load_legacy_embedded_inter()
       }
   }
   ```

3. Remove or deprecate `compute_font_id_for_inter` and any “Inter-only” assumptions. Inter becomes just another registered face.

### 3.2 WgpuRenderer Bootstrapping

File: `src/renderer/wgpu_renderer.rs`

Current:

* It directly calls `embedded_inter_data()` and `compute_font_id_for_inter()` to create a `TextShaper` from Inter.

Required:

* Switch to using the **same SystemUI font** as `FontManagerTextEngine`:

  ```rust
  // Pseudocode
  let text_engine = FontManagerTextEngine::new_with_system_ui();
  let primary_face = text_engine.primary_system_ui_face(); // returns data + identity
  let shaper = TextShaper::from_data(primary_face.data.clone())
      .expect("Failed to create TextShaper from system UI font");
  ```

* Guarantee: renderer + text engine share the **same primary font bytes** and identity. No split-brain.

### 3.3 FontResolver & FontFamily::SystemUI

File: `src/platform/font_resolver.rs`

Current:

* `FontFamily::SystemUI` is mapped to `find_generic_family("SystemUI")`, which only searches within already-loaded fonts by name pattern.

Required:

* Introduce a **well-defined resolution chain**:

  ```text
  FontFamily::SystemUI
    1. Prefer the font that SystemFontProvider registered as "SystemUI primary"
    2. If missing, fall back to:
       - Windows: family name contains "segoe"
       - macOS: family name indicates SF / system
       - Linux: any font flagged as default UI (from fontconfig)
    3. If still nothing, fall back to legacy Inter-embedded face
  ```

* The “generic family” heuristic stays as a **secondary backup**, not the primary resolution.

---

## 4. Platform Responsibilities

### 4.1 Windows (DirectWrite)

File: `src/platform/native_font_backend_directwrite.rs` and `system_font_provider.rs` under `cfg(target_os = "windows")`.

Contract:

* Use DirectWrite **system font collection** to identify the default UI font, typically `"Segoe UI"`.

High level:

1. Query the system font collection.
2. Locate the UI family (Segoe UI or whatever is configured).
3. Get a face matching Regular, weight=400, non-italic.
4. Obtain a **file reference or stream**, read bytes into `Vec<u8>`.
5. Fill `SystemFontDescriptor`.

If anything fails, `load_default_system_ui_font()` returns `None` and higher layers fall back to embedded Inter.

### 4.2 macOS (CoreText)

File: `src/platform/native_font_backend_coretext.rs` and `system_font_provider.rs` under `cfg(target_os = "macos")`.

Contract:

* Use CoreText to get the standard system UI font (SF).

High level:

1. Create a system UI `CTFont` instance via the proper CoreText API.
2. Obtain its descriptor, locate the backing font data provider.
3. Extract font bytes into `Vec<u8>`.
4. Populate `SystemFontDescriptor` with family and PostScript name.

Again, failure returns `None`, not silent Inter.

### 4.3 Linux (FreeType + fontconfig)

File: `src/platform/native_font_backend_freetype.rs` and `system_font_provider.rs` under `cfg(target_os = "linux")` (and other Unix-y).

Current:

* Hardcoded path `/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf`.

Required:

* Replace the hardcoded path with **fontconfig-based discovery** of the default UI font (or at least default sans).

High level:

1. Use fontconfig to resolve either:

    * `system-ui` generic, or
    * default UI / Sans family.
2. Obtain the resolved font file path.
3. Read bytes into `Vec<u8>`, wrap into `SystemFontDescriptor`.

If fontconfig isn’t available or fails, we can still fall back to DejaVuSans as a **platform-local default**, but `SystemUI` must be wired through the `system_font_provider` contract, not random paths.

If even that fails, the ultimate fallback remains embedded Inter.

Where exact C-level API details are needed, they must be implemented in the platform module; this SPEC just fixes the behavior and contracts.
If Claude needs concrete function names and they’re not available with certainty, the correct answer is: **“Exact verified detail not available.”**

---

## 5. Theme & Typography Defaults

File: `src/core/theme.rs`, `TypographySpec`, plus text widgets.

Required behavioral change:

* **Default font family** used by widgets should be `FontFamily::SystemUI` rather than some implicit Inter assumption.
* `TextStyle::default()` / `TypographySpec` remain size/line/Baseline definitions; font family resolves through SystemUI.

Rules:

1. Any widget that currently relies on “whatever the engine uses” should, after this SPEC, effectively land on `SystemUI` as primary.
2. Inter remains usable as an **explicitly named font** if loaded, but not the engine-wide default.

---

## 6. Backwards Compatibility & Tests

### 6.1 Behavior Guarantees

* On all platforms, the default font for simple text widgets becomes the native UI font.
* If OS font discovery fails, engine falls back to **embedded Inter** and logs a warning in debug builds.
* No API breakage for external users:

    * `FontFamily` remains the same.
    * Text widgets still resolve via `FontFamily` + `FontResolver`.

### 6.2 Tests to Add / Adjust

1. **SystemUI bootstrap test** (per platform, behind `#[cfg]`):

    * Confirm that `FontManagerTextEngine::new_with_system_ui()` registers at least one face tagged as SystemUI.
2. **FontResolver SystemUI test**:

    * Ensure `FontFamily::SystemUI` resolves to the SystemUI face if registered.
3. **Fallback test**:

    * Simulate failure in `load_default_system_ui_font()` (e.g., test-only stub) and check fallback to embedded Inter.
4. **No-more-hardcoded-Inter test**:

    * Ensure no production code path still calls `compute_font_id_for_inter` or uses Inter by name except in legacy helper or tests.

---

## 7. Implementation Plan (Stages for Claude)

Every stage must compile and keep existing tests green.
Inter stays as fallback until the final switch, to avoid bricking text.

### Stage 1 – Extract & Rename Legacy Inter Provider

* Move existing `EMBEDDED_DEFAULT_FONT` logic into `load_legacy_embedded_inter()`.
* Introduce `SystemFontDescriptor` and `load_default_system_ui_font()` stub that just returns `None` on all platforms.
* No behavior change yet – text engine still uses legacy Inter helper.

### Stage 2 – Generic FontIdentity & ID Computation

* Introduce `FontIdentity` and `compute_font_id(identity: &FontIdentity)`.
* Replace `compute_font_id_for_inter` internally with `compute_font_id` using a constructed `FontIdentity`.
* No behavior change – still using Inter for everything, but plumbing is now generic.

### Stage 3 – Text Engine Bootstrap via SystemUI Contract

* Add `FontManagerTextEngine::new_with_system_ui()`:

    * Try `load_default_system_ui_font()`.
    * If success:

        * Register that face as the primary SystemUI font in `FontManager`.
    * If failure:

        * Use `load_legacy_embedded_inter()` and register as SystemUI.

* Make existing entry points (examples / renderer) use `new_with_system_ui()` instead of raw Inter helpers.

### Stage 4 – Implement Windows SystemUI Provider

* Implement `load_default_system_ui_font()` for Windows using DirectWrite.
* Wire it to `FontManagerTextEngine::new_with_system_ui()`.
* Add a Windows-only test that at least asserts `load_default_system_ui_font().is_some()` on a typical environment (skippable if env lacks fonts).

### Stage 5 – Implement macOS SystemUI Provider

* Implement `load_default_system_ui_font()` for macOS using CoreText.
* Same integration as Windows.

### Stage 6 – Implement Linux SystemUI Provider

* Implement `load_default_system_ui_font()` using fontconfig discovery (or compatible mechanism).
* If fontconfig is not available, **clearly documented** fallback to DejaVuSans via existing path, but still returning `SystemFontDescriptor` through the new contract.

### Stage 7 – Hook FontFamily::SystemUI & Theme Defaults

* Ensure `FontFamily::SystemUI` resolves directly to the SystemUI face registered by the provider.
* Switch theme / text defaults so that:

    * `TextStyle::default()` → `FontFamily::SystemUI`.
* Remove any leftover hidden coupling to Inter as default.

### Stage 8 – Cleanup & Diagnostics

* Remove or internalize any Inter-special-case functions that are no longer needed.
* Ensure all Inter-specific references are either:

    * Part of legacy fallback, or
    * Test-only.
* Add `debug` log lines (only in debug builds) that clearly state which SystemUI font is active (family + PostScript) at startup.
