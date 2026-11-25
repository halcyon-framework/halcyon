# SPEC-HAL-263B: Image Registration Lifetime Correction

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

**Status:** Adopted / Binding; Implemented  
**Date:** 2026-09-13  
**Amendment Type:** D — Emergency Correction  
**Amends with literal replacement text:** SPEC-HAL-263 §§5.2, 7.1, 9.1, and 10  
**Depends on:** SPEC-HAL-002, SPEC-HAL-040, SPEC-HAL-084, SPEC-HAL-085A, SPEC-HAL-150, SPEC-HAL-263, SPEC-HAL-263A

---

## 1. Purpose

SPEC-HAL-263 gives an Image registration an explicit lifetime from registration
until unregistration, but its former §10 also grouped ordinary pipeline
revalidation with device loss and required `image_textures.clear()`. That rule
could accept an Image batch during Compile, delete the accepted registration
when text-atlas or target-scale work advanced the shared pipeline generation,
then reach Execute without the bind group that Compile had validated.

This amendment separates durable Image registration identity from derived
pipeline-layout state. It changes no Image pixels, sampling contract, scene
identity, custom-render ownership, or device-loss recovery mechanism.

---

## 2. Binding invariants

### INV-263B-001 — Registration lifetime is explicit

On a live renderer/device, an admitted Image registration remains present from
the successful `register_image_*()` or `register_image_from_view*()` call until
the matching `unregister_image()` call or renderer/device destruction.

Glyph-atlas growth, target scale-factor change, surface-format pipeline
revalidation, and any other same-device pipeline-generation change MUST NOT
remove or replace the registered texture/view identity.

**363/364 scoped amendment:** unregister_image ends the current public registration exactly as
before. A previously acquired SPEC-HAL-364 content-generation lease MAY continue
through a distinct private historical binding owned by SPEC-HAL-364/040.
It SHALL NOT resurrect ordinary lookup of the unregistered public texture_id
or substitute another generation. A bare texture_id or retained view pointer
is insufficient.

### INV-263B-002 — Sampling policy is registration state

Each registration retains whether its Image sampler is Linear or Nearest.
Revalidation MUST rebuild the bind group with that exact admitted sampling
policy. It MUST NOT silently normalize every registration to one sampler.

### INV-263B-003 — Bind groups are derived and replaceable

An Image bind group is derived from the current Image bind-group layout, the
registered texture view, the retained sampling policy, and the current rounded
clip buffer. If pipeline revalidation replaces the layout, the renderer MUST
rebuild every registered Image bind group against the new layout before the
generation is marked validated.

This is Compile-phase GPU resource resolution. It creates no command encoder,
render pass, submission, scene mutation, or caller callback.

**363/364 scoped amendment:** A private historical binding SHALL preserve its exact generation
and the INV-263B-002 admitted sampling policy across same-device revalidation.
Bind groups remain derived Compile resources, not immutable historical content;
each rebuilt binding uses current compatible layout and live coverage inputs.

### INV-263B-004 — Compile validates final executable state

`INV-IMG-005` validation occurs after same-compile pipeline and bind-group
revalidation. Every Image batch that remains in the compiled plan therefore has
a registered bind group available to Execute.

For a genuinely unregistered `texture_id`:

- Debug MUST panic with `INV-IMG-005` during Compile.
- Release MUST remove that Image batch from the executable per-surface batch
  list and log the rejection.

Execute MUST NOT be the first phase to discover a missing Image registration.

**363/364 scoped amendment:** For an Image contribution required by an admitted SPEC-HAL-363/364
composition, the Debug-panic/Release-batch-drop rule above is replaced:
Compile SHALL validate its exact historical/current binding and content
generation after pipeline revalidation. Known unavailability makes the affected
complete surface plan unavailable before Execute in every build mode. The
required batch SHALL NOT be dropped independently, and a debug-only panic SHALL
NOT substitute for the typed complete-group outcome. Unavailable composition
SHALL NOT be reported as missing text.

Ordinary non-group Image batches retain INV-IMG-005 Debug panic and Release
rejection/log behavior. A valid internal historical binding is not an ordinary
unregistered texture_id lookup. Failure first learned after encoding, submission
or a present call follows SPEC-HAL-364 B-9.18; this never permits ignoring known
pre-execution failure.

### INV-263B-005 — Device loss remains distinct

Actual device loss invalidates device-bound textures, views, layouts, and bind
groups together. Recovery may discard registrations as part of complete
renderer/device reconstruction and requires the owning producer to republish
GPU resources. Ordinary same-device pipeline revalidation is not device loss
and MUST NOT use that policy.

---

## 3. Literal parent replacements

**363/364 scoped amendment:** The quoted parent replacements in §§3.1–3.4 retain their ordinary
Image scope. The explicit paired qualifications in INV-263B-001–004 govern only
required occurrence contributions and separately leased historical generations;
they do not change ordinary public registration lookup or device-loss policy.

### 3.1 Replace SPEC-HAL-263 §5.2

```markdown
## **5.2 Compile-Phase Handling (INV-IMG-005)**

After same-compile pipeline and bind-group revalidation, every executable Image
batch must reference a currently registered texture and bind group.

If Compile encounters an unregistered or never-registered `texture_id`:

* Debug build → panic with `INV-IMG-005` message.
* Release build → remove the batch from the executable per-surface batch list.

No placeholder rendering. Execute is not a fallback validation phase.
```

### 3.2 Replace SPEC-HAL-263 §7.1

```markdown
## **7.1 Ownership**

`ImageTextureEntry` owns or retains the registered texture view, its explicit
Linear/Nearest sampling policy, and the current derived bind group.

No separate bind-group cache map. `image_textures.len()` equals the number of
registered Image identities and bind groups.

Registration identity remains alive until explicit unregistration or actual
renderer/device destruction. Same-device pipeline revalidation preserves every
registration and atomically rebuilds its bind group against the current layout.
```

### 3.3 Replace SPEC-HAL-263 §9.1

```markdown
## **9.1 Compile Phase**

Responsibilities:

* Revalidate pipeline layouts and rebuild derived Image bind groups while
  preserving registered texture/view identity and sampling policy.
* Validate final executable texture existence (INV-IMG-005).
* Prepare rounded clip UBO entries for admitted Image batches using the same
  dynamic offset model as Rect pipeline.
* Upload Image vertex/index data to GPU buffers.
```

### 3.4 Replace SPEC-HAL-263 §10

```markdown
# **10. Device Loss and Pipeline Revalidation**

Actual device loss invalidates the complete renderer/device resource set;
recovery may require producer re-registration after renderer reconstruction.

Ordinary same-device pipeline revalidation, including glyph-atlas growth,
target scale-factor change, or surface-format pipeline replacement:

* preserves every registered Image texture/view identity;
* preserves each registration's Linear/Nearest sampling policy; and
* rebuilds every Image bind group against the replacement layout before
  marking the generation validated.

No caller re-registration is required for ordinary pipeline revalidation.
```

---

## 4. Required proof

**Current binding evidence extension:** SPEC-HAL-263 §5.3 additionally names
registration generation and its CPU-only evidence. A same-number replacement
is a new registration generation and revokes prior evidence; same-device
pipeline/bind-group revalidation is not replacement. Unregistration remains
immediate and neither evidence nor retained CPU dependencies extend GPU
lifetime. INV-IMG-005 also rejects an obsolete admitted generation even when
the number currently names a replacement. No new content or device recovery
contract is implied.

1. A scene containing registered Image and Text primitives must survive a
   same-compile pipeline-generation change and encode the Image in that frame.
2. The unchanged retained scene must compile and encode again without
   re-registration.
3. A target scale-factor change must clear/rebuild text-atlas state without
   deleting the Image registration; the next frame must compile and encode.
4. Linear and Nearest registrations must retain their distinct sampling policy
   across revalidation.
5. Existing `INV-IMG-005` debug rejection and release batch-removal behavior
   remain covered.

---

## 5. Non-goals

- device-loss recovery implementation;
- image loading, decoding, streaming, mipmaps, or caching policy;
- changing Image alpha, color, clipping, blending, geometry, or Z ordering;
- changing custom-render publication ownership;
- changing unrelated mesh-cache policy.

---

## 6. Consistency

- **SPEC-HAL-002:** retained state and ID authority are preserved; no widget or
  scene acquires GPU ownership.
- **SPEC-HAL-040:** same-device derived resources are rebuilt at the owning
  Compile boundary; actual device loss remains a full resource reset.
- **SPEC-HAL-084:** Build remains CPU-only, Compile resolves GPU resources, and
  Execute only encodes already-validated intent.
- **SPEC-HAL-150:** compositor output is not changed or reinterpreted.
- **SPEC-HAL-263A:** sampled alpha/color association is unchanged.
