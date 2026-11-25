# SPEC-HAL-063: Image Loading, Caching & Async Raster Assets

**Unified Pipeline for UI Images, Icons, Emoji, and Inline Assets**

Status: Design – Not Implemented
Author: JC
Depends on:

* SPEC-HAL-001 (Core Architecture)
* SPEC-HAL-027 (SceneGraph Batching)
* SPEC-HAL-039 (Scene Diff Compression)
* SPEC-HAL-048–050 (Text & Glyph Atlas Infrastructure)

Planned follow-ups:

* SPEC-HAL-0xx (High-Resolution Icon Packs)
* SPEC-HAL-0xx (Themeable Vector Assets)

---

## Implementation Status

**Not implemented**

Image loading, caching, and the async asset pipeline described in this SPEC are not implemented. The renderer does not currently support image rendering.

---

## 1. Summary

This SPEC defines a unified **image-loading subsystem** for Halcyon, covering:

* PNG/JPEG/WebP file loading
* Runtime-provided byte buffers (memory images)
* Network-loaded images (only if provided by the app)
* Caching and deduplication
* CPU-side decode + optional GPU-side resizing
* Async loading pipeline
* Stable `ImageId` references for all widgets
* Integration with the scene graph and renderer
* Efficient atlas-based GPU uploads where possible

The goal is to make images a **first-class citizen** in Halcyon: reusable, cheap, async-friendly, and consistent across all platforms.

This SPEC is about UI assets.
It is **not** about photo editors, document formats, or HTML embedding.

---

## 2. Motivation

Halcyon needs predictable image handling for:

* Icons
* Avatars
* UI decorations
* Thumbnails inside widgets
* Inline emoji/text attachments
* Simple inline images inside RichTextBlock
* Charts needing bitmap overlays

Current situation:

* Ad-hoc image decoding
* No centralized caching
* No stable identity model
* Widgets manually reloading resources
* No async task integration
* No GPU upload scheduling

This SPEC establishes a stable, minimal, robust pipeline for image assets.

---

## 3. Requirements & Invariants

### 3.1 Functional Requirements

* R1: Load images from file paths, memory buffers, or user-provided loaders.
* R2: Deduplicate identical images by content hash.
* R3: Provide `ImageId` stable handles for widgets.
* R4: Decode image bytes on a worker thread.
* R5: Upload to GPU asynchronously once decoded.
* R6: Allow synchronous loading for critical assets.
* R7: Cached images survive scene diffs.
* R8: Renderer handles missing/not-yet-loaded images gracefully.

### 3.2 Invariants

* I1: GPU uploads must never block the UI thread.
* I2: Decoders run outside the frame loop.
* I3: All images normalize to RGBA8.
* I4: All dimensions stored in logical pixels; scaling applied during paint.
* I5: Scene diff never duplicates GPU textures unnecessarily.

---

## 4. Data Structures

### 4.1 ImageId

A stable integer handle:

```
pub struct ImageId(u32);
```

Uniquely identifies a logical asset, not a GPU texture pointer.

### 4.2 ImageSource

```
enum ImageSource {
    File(PathBuf),
    Bytes(Vec<u8>),
    User(Box<dyn Fn() -> Vec<u8> + Send + Sync>),
}
```

### 4.3 ImageState

```
enum ImageState {
    PendingDecode,
    Decoding,
    Decoded(CpuImage),
    UploadPending(CpuImage),
    Ready(GpuImage),
    Failed,
}
```

### 4.4 CpuImage

Raw RGBA8 + width + height.

### 4.5 GpuImage

Texture handle + format + size.

---

## 5. Loading Pipeline Overview

There are three stages:

### Stage 1 — Request

Widget or app requests:

```
image_loader.request(ImageSource::File("icon.png"));
```

Returns `ImageId`.

State: `PendingDecode`.

### Stage 2 — CPU Decode (Async Worker)

Worker threads decode using `image` crate or platform decoders.

On success: → `Decoded(CpuImage)`
On failure: → `Failed`

### Stage 3 — GPU Upload (Renderer Thread)

During render sync:

* Find images in `UploadPending`
* Create GPU texture
* Upload bytes
* Transition to `Ready(GpuImage)`

Renderer caches GPU images until evicted.

---

## 6. Caching & Deduplication

### 6.1 CPU Cache

Images are deduplicated by:

* source path
* content hash (for bytes)

If you request the same file twice → same ImageId.

### 6.2 GPU Cache

GpuImage stored until:

* LRU eviction (optional)
* explicit unload
* app shutdown

Scene diff references ImageId, not textures, ensuring stability.

---

## 7. Handling Missing/Loading Images

If widget draws an ImageId whose state is:

### Pending / Decoding

Renderer draws placeholder rectangle or nothing, depending on widget style.

### Decoded / UploadPending

Renderer schedules upload next frame.

### Failed

Renderer may draw a fallback X-icon or nothing.

This avoids jank and ensures stable layout even when loading asynchronously.

---

## 8. Widget Integration

Widgets can ask for:

```
ImageWidget::new(image_id)
    .fit(ImageFit::Cover)
    .radius(6.0)
    .opacity(0.8)
```

RichTextBlock may embed small inline images:

* treated like glyphs
* measured as part of layout
* drawn after glyph pass

ScrollViews automatically handle images like any other rendering quad.

---

## 9. Scaling & DPI Rules

* Stored images are DPI-agnostic.
* Scaling is always physical pixel → logical pixel conversion.
* High-DPI monitors render via renderer scaling, not by re-decoding images.
* Optional: allow multi-resolution assets later (future SPEC).

---

## 10. Async Behavior & Frame Loop

Loading never blocks:

* CPU decode done in worker thread.
* GPU upload scheduled on next render pass.
* Scene diffs automatically detect when an ImageId transitions to Ready.

Widgets do not need to poll — the system notifies via:

```
request_redraw()
```

after each stage transition.

---

## 11. Error Handling

Decoder errors set ImageState::Failed.

Widget may:

* display fallback graphic
* shrink to zero
* reserve layout regardless

Errors do not crash the engine.

---

## 12. Security Considerations

* Only trusted sources load images (app code chooses source).
* No network loading unless app provides a loader.
* Decoder does not allow arbitrary formats beyond image crate defaults.
* No remote execution or embedded metadata parsing.

---

## 13. Migration Plan

### Phase 0

Introduce ImageId + ImageLoader skeleton.

### Phase 1

CPU decode worker pool.

### Phase 2

GPU upload integration with renderer.

### Phase 3

SceneGraph integration (new node type for images).

### Phase 4

Inline image support in RichTextBlock.

### Phase 5

Public APIs for widget integration.

---

## 14. Testing Strategy

* Load PNG/JPEG/WebP of various sizes
* Reuse the same ImageId across multiple widgets
* Stress test parallel loads
* GPU upload on heavy scenes
* Placeholder behavior
* Memory leak checks
* Eviction behavior (if implemented)
* Inline images in RichTextBlock
* High-DPI rendering correctness
