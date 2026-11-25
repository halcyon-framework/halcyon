# SPEC-HAL-375: 3D-Anchored Spatial HUD and Information Connectors

**Author:** JC; product direction recorded through delegated technical review on 2026-11-05.
**Status:** Adopted / Active / Binding [scoped §9 first passive PBR HUD and §10 sole-binding companion S1].
**Historical authority of §§1–8:** DRAFT / INACTIVE / NOT BINDING. Product intent and the source candidate are preserved for provenance.
**Implementation:** §10 S2 source, Metal and native frame evidence are recorded in §10.4.
**Owner:** existing HAL 3D projection plus retained 2D text/layout/compositor/input; semantic content and real world anchors supplied by consumer.
**Governing foundation:** SPEC-HAL-002 Constitution and SPEC-HAL-002A amendment law; adopted SPEC-HAL-309. HAL021, HAL048/050/066, HAL081C, HAL150, HAL226, Draft HAL316, HAL343, HAL354/355/357/359/360.
**Constitutional amendment:** none selected.

## Historical contract scope

- Objective: preserve the missing distinct product visual goal and its exact authority boundary before any implementation assignment.
- Standing objective: deliver excellent source-faithful native application presentation through Halcyon, without a disconnected visual demo, parallel runtime or inferred product semantics.
- Review authority: JC owns product intent and final visual acceptance; the delegated architectural reviewer selects the technical contract; implementation requires its separate acceptance.
- Adoption and implementation: this historical scope was Draft. The delegated architectural reviewer must verify the source, close the decisions in section 5 and separately adopt a valid binding S1 before authorizing a complete S2 engineering unit. The running HAL372 Object Reveal S2 continues under its own accepted contract.
- Governing status: Constitution is superior, HAL309 adopted, this document inactive. Every cited Draft/Proposed/Design remains at its original status.
- Authority change: NONE. No production edit, new feature flag, render resource allocation or new user behavior is granted.
- Target owner and verified existing boundary: HAL316 (Draft) proposes widget/screen/Plot annotations, NOT an accepted world-to-screen product anchor system. HAL357 section 12 expressly excludes 3D labels, billboards and world-to-screen text. HAL050/066 and HAL360 supply qualified basic shaping/glyphs and HAL359 real 2D order; none carries source-aware 3D HUD membership, depth visibility and lifecycle.
- Owned mechanism/files: this Markdown SPEC only. No existing authority, code, tests, assets, consumer state or unrelated document changes.
- Reuse and dependencies: existing HAL021 time, HAL355 source/target, R-series source material/depth and appropriate text/overlay owners, only where source compatible. No new runtime mechanism is prescribed by this Draft.
- Named invariants: INV-375-ANCHOR exact source/coordinate/current revision; INV-375-PROJECTION current camera/origin/DPR/viewport; INV-375-TEXT shaping/layout ephemeral and metrics stable; INV-375-OVERLAY actual clip/order/modal/focus/capture; INV-375-CONTENT consumer facts only; INV-375-LIMIT bounded connector routing and labels.
- Forbidden behavior: FORBID-375-INFERRED-PART-ANCHOR; FORBID-375-RENDERER-PRODUCT-FACTS; FORBID-375-CLIP-ESCAPE; FORBID-375-CROSS-PHASE-LAYOUT; FORBID-375-PERSISTENT-GLYPH-BORROW; FORBID-375-SECOND-ANNOTATION-MANAGER; FORBID-375-IMPLIED-HAL316-ADOPTION.
- External boundary: The application remains sole owner of product identity, real physical components, source facts, configuration and domain semantic state. Halcyon owns qualified visual execution and its existing input, text, layout, GPU and target mechanisms.
- Verification here: source/draft-classification, exact file and reference checks; no code, Cargo, backend, native or physical visual proof follows.
- Future test placement: actual nearest engine source tests and existing canonical native owner, as determined during S1 by source inspection.
- Output boundary: product goal, owners, refusal laws and open decisions, not a selected executable ABI, pipeline or complete implementation.
- Release effect: N/A, inactive documentation only.
- External/destructive effects: NONE.
- Out of scope: undefined

## 1. User-visible capability

Show real source-attributed part names, dimensions, product facts and interactive informational cards connected to qualified 3D anchors. Lines and labels track camera/origin/viewport changes, stay readable in compact windows, and report behind-camera, offscreen and occluded states truthfully. Trace-to-Information is the line linking a source anchor to its associated factual card, not a guessed product relationship.

## 2. Existing reality and absence

HAL316 (Draft) proposes widget/screen/Plot annotations, NOT an accepted world-to-screen product anchor system. HAL357 section 12 expressly excludes 3D labels, billboards and world-to-screen text. HAL050/066 and HAL360 supply qualified basic shaping/glyphs and HAL359 real 2D order; none carries source-aware 3D HUD membership, depth visibility and lifecycle.

A working neighboring primitive, an old draft, a rendered mockup, a shader compilation result, and an interactive application are different claims. None implies that this distinct feature is implemented.

## 3. Source and effect ownership

The application supplies explicit current semantic part/product instance identity, real source revision, content and an authorized 3D anchor in an admitted coordinate frame. HAL checks source/view/target identity, projects to screen space, and composes bounded text/cards/connectors using established retained 2D layout, renderer and overlay owners. Interactive cards use ordinary canonical widget input; connector projection itself is noninteractive and not a product-state writer.

**INV:** INV-375-ANCHOR exact source/coordinate/current revision; INV-375-PROJECTION current camera/origin/DPR/viewport; INV-375-TEXT shaping/layout ephemeral and metrics stable; INV-375-OVERLAY actual clip/order/modal/focus/capture; INV-375-CONTENT consumer facts only; INV-375-LIMIT bounded connector routing and labels.

The visual result never modifies canonical product meaning, source geometry, normal/material claims, focus, capture or semantic selection. No timing authority outside HAL021 and no cross-phase mutation or opaque global state.

## 4. Explicit refusals and prohibited shortcuts

FORBID-375-INFERRED-PART-ANCHOR; FORBID-375-RENDERER-PRODUCT-FACTS; FORBID-375-CLIP-ESCAPE; FORBID-375-CROSS-PHASE-LAYOUT; FORBID-375-PERSISTENT-GLYPH-BORROW; FORBID-375-SECOND-ANNOTATION-MANAGER; FORBID-375-IMPLIED-HAL316-ADOPTION.

An incomplete, stale or unsupported extra effect must be refused at its actual owner without corrupting current source/target, semantic revision or existing compatible last-good front. The source, GPU, presentation and consumer-fact authorities remain separate.

## 5. Missing technical S1 choices (all OPEN)

Select exact typed anchor/occurrence and source association; actual world-to-screen transform and invalidation; depth/occlusion plus behind-camera/offscreen policy; label overlap, leader-line placement, clipping and bounded routing; text measurement/caching, accessibility, interactive-card hit/focus; compositing and finite resources; whether HAL316 is amended/adopted in separate owner unit or left Draft.

These decisions require delegated architectural review subject to live source, constitutional constraints and JC's retained authority. This file does not authorize selection or implementation of alternatives. No general engine feature is activated simply by publishing this Draft.

## 6. Required independent conformance after future adoption

Real 3D source with two anchors through orbit/zoom/resize, depth occlusion and clipping, 800x600 layout, long labels and real glyph output; target/current source replacement, no stale fact, input/modal correctness, GPU + canonical native output verified separately.

Backend output, native App behavior, physical user input and JC visual acceptance are separate proof obligations. A text-only S1 or passing headless test is not a completed visual effect.

## 7. Boundary with adjacent work

No new semantic graph, text shaping engine or generalized automatic annotation layout system. HAL316 remains Draft; HAL357 exclusions remain until a new accepted scoped contract.

**Adoption:** DRAFT / INACTIVE / NOT BINDING as of 2026-11-05. Future S1 must explicitly select and verify ownership, source representation, exact temporal semantics, supported GPU/text composition, finite resources, error priority and source/backend/native proof without weakening HAL002/002A.

## 8. First-profile technical candidate — 2026-11-07

**DRAFT / INACTIVE / NOT BINDING.** This section records the delegated architectural reviewer's first-profile selection and its source review at Halcyon 0.232.0-beta, `2e6f979a6aa3febe75458c1dbc0d9da4ada0e832`. The selected product scope is passive, static, source-associated labels and leaders on a current PBR viewport. The broader historical product intent above is outside this profile. **BLOCK** rows require a decision by the delegated architectural reviewer before adoption and S2. Proposed API names describe the review boundary.

- Governing authority is HAL002/002A, HAL354, HAL355, R027/HAL370, HAL359/360 and the existing HAL048/050/066 text path. HAL357 §12 allows a separately accepted extension while keeping 3D labels outside domain composition. HAL316 remains Draft.
- Display-only widget policy applies. There is no new hit region, focus, capture, modal, accessibility action, semantic selection or event writer. Reuse the existing viewport participant, retained scene, text atlas and compositor. The §4 prohibitions, HAL354's paint and invalidation rules, and PM006/007/008/011/018 apply. PM009/016 describe virtualized content. Their single-measurement and phase rules identify a related failure to avoid here.
- This is documentary source qualification. There is no change to the Constitution, parent SPECs, source code, consumer, release version, demo or test.

### 8.1 Anchor ingress and occurrence identity

Use the exact immutable admitted scene. [PbrScene3D](../../src/three_d/material_api.rs#L1342) owns the complete source Arc, exposes snapshot/model and tests Arc identity at line 1370. Its scalar HDR identity alone is insufficient. [HAL374 admission](../../src/three_d/outline.rs#L90) establishes how an explicit occurrence slot binds to the exact ordered snapshot and corresponding model placement. This mechanical association is independent of HAL374's selection type, opaque replay restriction and rendering resources.

The candidate typed ingress for review is:

~~~rust
pub struct PbrSpatialHudAnchor3D {
    pub label_id: std::num::NonZeroU64,
    pub packet_slot: u32,
    pub world: WorldPoint3,
    pub text: std::sync::Arc<str>,
}

// Context3D
pub fn admit_pbr_spatial_hud(
    &self,
    scene: &PbrScene3D,
    anchors: &[PbrSpatialHudAnchor3D],
) -> PbrSourceResult3D<PbrSpatialHud3D>;

// Viewport3D
pub fn request_pbr_spatial_hud(
    &mut self,
    hud: Option<PbrSpatialHud3D>,
) -> PbrSourceResult3D<bool>;
~~~

`PbrSpatialHud3D` is an opaque immutable, quota-backed scene association. `label_id` is unique in the supplied ordered batch. `packet_slot` names an actual PBR occurrence in that scene. Two occurrences sharing a geometry lease remain distinct. Check finite world points, slot range and kind, live resource/Context/Device association and selected capacity before retaining a batch. The consumer owns the semantic relationship among occurrence, text and point. HAL cannot infer a part or move the point to a guessed surface. An application-supplied center-point anchor does not by itself establish visibility on the source surface. Occurrence association and geometric visibility require separate checks.

`Some` must match the exact requested complete source. `None` clears the HUD. An unchanged accepted immutable HUD on an unchanged tuple is clean. An invalid request cannot rebind an older HUD to a new source. PBR source and front keep their own refusal and publication state. [Existing viewport request pattern](../../src/three_d/viewport.rs#L1318)

### 8.2 Projection and coordinates — source path established

Use the target's actual render origin, physical render extent `W,H` and View80 packed for that scene. Subtract the origin in checked f64. For Direct present apply the same positive metric scale before checked f32 packing. Direct-absent uses the ordinary render-relative conversion. The current packer selects `ViewDescriptor3::prepared_matrices` or `metric_view`. Both support perspective and orthographic projection, including the existing finite or infinite perspective far choice. [raster.rs:1736](../../src/three_d/raster.rs#L1736), [math.rs:704](../../src/three_d/math.rs#L704), [direct_shadow.rs:760](../../src/three_d/direct_shadow.rs#L760)

For finite clip point `q = View80.view_projection * (p_relative,1)`, require `q.w > 0`, `-q.w <= q.x,q.y <= q.w`, `0 <= q.z <= q.w` and a point inside the half-open local framebuffer. Top-left UV is `(0.5 + 0.5*q.x/q.w, 0.5 - 0.5*q.y/q.w)`, physical position is `(u*W,v*H)` and reverse-Z depth is `q.z/q.w`. Suppress behind-camera, near/far-outside and offscreen points without clamping them to an edge. Invalid packed inputs refuse the affected HUD. CPU projection establishes placement. The selected GPU comparison must establish sample visibility.

Map the UV into the full `CustomRenderTargetMetrics.final_logical_rect`. The usable crop intersects it with `final_logical_viewport` and surface bounds, then applies the inherited paint clips. Cropping cannot change camera aspect or UV origin. Consume the supplied physical render extent and fractional scale without substituting composition coverage or truncating logical dimensions. [custom_render_prepare.rs:231](../../src/renderer/custom_render_prepare.rs#L231), [HAL355 §10.2](SPEC-HAL-355-Native-3D-Rendering-Substrate.md#102-target-sizing-and-activity)

### 8.3 BLOCK — same-frame depth and visibility delivery

The actual PBR pass stores reverse-Z depth at N1/N4. Its canonical `Depth32Float[N]` has `RENDER_ATTACHMENT` usage only. The fourth R027 plane is binary coverage, and the registered FP16 image contains toned RGB and coverage. Neither identifies depth or a winning occurrence. The existing final-color pin does not pin depth. [hdr.rs:651](../../src/three_d/hdr.rs#L651), [raster.rs:2917](../../src/three_d/raster.rs#L2917), [HdrOutput3D::pin_front](../../src/three_d/hdr.rs#L771)

Both branches of the existing viewport encoder receive the exact canonical depth view. The HDR owner calls them before tone, submit, swap and registration. The delegated architectural reviewer must select one use of that stored depth and an exact GPU result ABI. [viewport.rs:3764](../../src/three_d/viewport.rs#L3764), [hdr.rs:930](../../src/three_d/hdr.rs#L930)

1. Permit an opt-in sampled view of canonical depth with `TEXTURE_BINDING` and actual Device-qualified N1/N4 loads. A bounded GPU evaluation compares against the matching physical pixels and samples and supplies the anchor and stroke visibility result. This needs a named attachment-usage and binding extension.
2. Keep attachment-only depth and encode a private depth-tested annotation pass against the stored canonical view. Disable depth writes and produce a bounded GPU result for each anchor and visible stroke. HAL374 establishes the read-only depth attachment pattern on its own replay depth. HAL375 still needs the canonical association, result capture and text delivery. HAL374's opaque-only replay cannot establish canonical Mask/discard visibility. [outline.rs:1145](../../src/three_d/outline.rs#L1145)

`CustomRenderPresentationBinding` carries one image ID and a presentation mode. `TextNode` and `LineNode` have no depth or visibility input. Select a narrow GPU gate on the existing per-surface text/line schedule, or one private HUD output produced through the same renderer with a proved closed order, text, alpha and publication contract. Bind the result to the same depth generation and registered front. Visibility remains on the GPU. No per-label CPU fence/readback during orbit, fifth canonical MRT, color-derived visibility or cross-generation comparison. [custom_render_pass.rs:163](../../src/renderer/custom_render_pass.rs#L163), [TextNode](../../src/core/scene/mod.rs#L667), [LineNode](../../src/core/scene/mod.rs#L1025)

Select the sample and occluder rules explicitly. Strict reverse-Z `Greater` can suppress a depth tie. Allowing a surface-coincident tie for the owning occurrence requires an actual same-sample winner or equivalent proof. Decide partial N4 and uncovered-sample treatment and the depth of the offset leader and label. One candidate is a straight leader at the anchor's projected depth, tested along its full stroke against actual target samples. A foreground wall or unrelated nearer triangle must block the stroke regardless of color. The anchor's visibility does not admit its whole leader or label footprint. Select whole-pair suppression on a blocked footprint or visible-interval clipping with no stroke across a blocked gap. Excluding farther nonmember surfaces as well requires occurrence attribution. Depth alone orders surfaces.

### 8.4 BLOCK — finite placement, text admission and ordering

Use layout's resolved typography and scalar measurements. Existing Label layout obtains shaped width and line metrics. Retain stable keys, immutable text, resolved bindings and owned numeric measurements. Key them by label/content and typography/scale generation. On orbit with unchanged typography, only the bounded projection and placement change. Glyph runs, atlas UVs and temporary layout borrows stay ephemeral. Paint consumes frozen placements and uses the existing text and line owners. [Label::layout](../../src/widgets/label.rs#L142), [GlyphRun](../../src/core/text_engine.rs#L267), [TextEngine](../../src/core/text_engine.rs#L485)

The bounded placement candidate processes the supplied batch in order and tries measured single-line rectangles above, right, below and left. Accept the first rectangle fully in the usable crop, disjoint from accepted rectangles, and within the approved leader length. Join the anchor to the nearest point of its accepted box boundary. Constrain glyph ink and stroke to their authorized boxes and inherited clip. Omit the pair if no position fits. For `A` anchors this evaluates at most four candidates per anchor and `2*A*(A-1)` rectangle comparisons. Reserving boxes before GPU occlusion allows a suppressed anchor to keep its slot for that view. The delegated architectural reviewer must select this rule or a fixed alternative and approve overflow handling, clip behavior, typography/gap/padding, and explicit caps on anchors, bytes, glyphs, box area, leader length/width and raster work. Existing Label and Context caps do not define these choices.

The layout/text and GPU result phases need one selected admission seam. `CustomRenderParticipant` receives final metrics before Paint without a text engine. `UpdateContext` also has no layout text measurement. Use layout-owned immutable metrics and the existing viewport preparation for derived placement. HAL360 §4 rejects the surface when admitted text demand is unavailable. The optional HUD therefore needs an owner-qualified complete text working set before it can enter ordinary surface demand. The delegated architectural reviewer must select this scoped admission boundary with actual preparation order. Silent glyph loss or raw diagnostic scene text cannot supply it. [custom_render_prepare.rs:170](../../src/renderer/custom_render_prepare.rs#L170), [UpdateContext](../../src/core/widget/mod.rs#L2644), [wgpu_renderer.rs:7768](../../src/renderer/wgpu_renderer.rs#L7768), [HAL360 §4](SPEC-HAL-360-Complete-Text-Realization.md#4-render-admission-and-compatibility)

Within the existing viewport clip and surface, keep the image below the leader and the leader below its label with strict Z for overlapping classes. Keep text-native backgrounds contiguous with their glyph run. Preserve control and modal order and normal input routing. The viewport's layout/projection owner supplies immutable geometry for the compositor to execute. [HAL359 §§2.3–2.4,3](SPEC-HAL-359-Cross-Pipeline-Z-Order-Execution.md#23-surface-and-compositor-boundary), [HAL150 §D](SPEC-HAL-150-Compositor-Authority-and-Limits.md#section-d--authority-boundaries)

### 8.5 Currentness, phases, refusal and finite custody

Bind each prepared HUD to participant/mount, HUD request epoch, full scene Arc and live Context, PBR request serial, view revision and exact view, target key, physical extent/N, final rect/clip/scale, depth generation and corresponding registered front generation. Requested and committed associations must match. Inactive view, source/view/target mismatch, cancellation, unmount, retired resource or Device loss revokes the HUD. A compatible last-good PBR front can remain as its original publication without new labels. Same-source orbit/zoom/resize keeps the anchor facts. A re-tone may reuse visibility only with the exact live depth and unchanged placement dependencies and must bind to the resulting new front. A loading proxy or absent current canonical front is ineligible. [PbrViewportTuple3D](../../src/three_d/material_api.rs#L1708), [Viewport3D target key](../../src/three_d/viewport.rs#L103)

Update owns request replacement and narrow retained invalidation. Layout supplies completed geometry. The engine runs viewport preparation after final layout before Paint. Paint consumes the frozen result, and the renderer resolves the target binding per surface. Invalidate on the actual changed dependency. No layout/semantic mutation during Paint, new clock or ongoing demand. `None`, inactive targets and unchanged complete tuples perform no HUD shaping, placement, allocation or encode. [app.rs:3278](../../src/core/app.rs#L3278), [widget_tree.rs:1393](../../src/core/widget_tree.rs#L1393), [viewport.rs:4563](../../src/three_d/viewport.rs#L4563)

Check association, work limits, Device capabilities and all reservations before HUD writes. Typed input, text, GPU or capacity refusal affects only the optional HUD. Use checked `P=W*H`. The canonical R027 minimum is `P*(20*N+16)`. Add every HUD attachment as `width*height*samples*bytes_per_texel` and account actual aligned buffers, CPU text/placement capacity, texture/view and pipeline slots, atlas work, uploads and all current/pending/retiring overlap. Share the canonical depth charge while holding its exact generation for dependent reads. Any separate mask, stroke plane or resolved output is additional. Predeclare the command/texture-read and glyph/stroke work ceiling for the selected §8.3–8.4 ABI. Use the actual configured single-view and aggregate caps, currently defaulting to 536870912 and 1073741824 attachment bytes, plus existing dynamic, upload and glyph limits. An image ID is not a content lease. Queue ordering must prevent reuse of a live depth, mask, glyph or front generation. [PolicyLimits3D](../../src/three_d/context.rs#L75), [target and storage reservations](../../src/three_d/context.rs#L2156)

### 8.6 Adoption gate and one S2 vertical

**Delegated architectural-review decision required.** Select the depth and GPU visibility delivery ABI, comparison and label/leader policy, scoped text admission and numeric placement/work/capacity limits. Adopt a named HAL375 extension for target-local depth access and ordered passive text/leader output under HAL355 §§9–11 and R027 §§6–8/HAL370. Any chosen presentation, geometry or text-admission exception must name its exact HAL150/359/360 boundary. Canonical MRT4 and FP16 registration keep their existing rules. HAL357 §12 supplies no HUD implementation authority. The source seams above are available for review. The added ABI has no Device/backend proof.

After adoption, put typed ingress next to `src/three_d/material_api.rs` and `viewport.rs` and the selected depth operation next to `raster.rs`/`hdr.rs`. Extend the current `custom_render_prepare.rs`/`draw_plan_builder.rs` and text realization/render execution owners only as required by the selected GPU delivery. Keep one viewport, one retained tree and the existing text atlas.

1. In `src/tests/three_d_viewport.rs` and nearest existing math/raster/text owners, use the real PBR occurrence fixture pattern at `three_d_viewport.rs:54–266`. Include two visible labels on distinct placements sharing geometry and a third anchor behind an independently admitted foreground occluder. With the same scene/model/material, compare two views and orbit/zoom. Prove actual target-matched depth, label and leader visibility at N1/N4 and final pixel attribution. Include a Mask occluder or a typed, adopted refusal of that profile. Exercise near/far/behind/offscreen and both projections with the actual render origin.
2. At 800×600 and desktop, fractional and Retina scales, measure boxes and actual glyphs. Include coincident projections, a long label, partly blocked leader, rounded ancestor clip and unavailable text/capacity. Assert nonoverlap, continuous visible stroke intervals, no bridge through occlusion, stable anchor IDs, reuse of unchanged metrics, and ordinary keyboard, focus, capture and modal routing.
3. Prove source/target/mount switch, resize, cancellation and Device loss with correct last-good provenance and Queue retirement. Compare disabled, suppressed and refused cases with the canonical N1/N4 front and ordinary draw plan. No HUD allocations or frame demand on the disabled/no-op path.
4. Add one bounded real `App::run` fixture in `tests/app_frame_realization_native.rs` following the current HAL374 native owner. Verify actual realized text, the registered matching front, submitted and presented_called AppFrame, and quiet static reuse. Physical input, scanout and JC visual review are separate evidence. This S1 adds no demo or consumer integration.

The inspected application's private projection and current-front guard in `src/application/editor_ui.rs:441–626` establish the existing caller boundary. The screenshot symptom is the product input. This candidate records source qualification only.

## 9. Adopted first passive PBR HUD profile — 2026-11-07 [S1]

Independent architectural review adopts this profile under JC's express technical delegation and retained veto in `HAL375-SPATIAL-HUD-FIRST-PROFILE-S1-ADOPTION-AND-S2-EXECUTION-02`. The reviewed Draft beforeimage is SHA256 `dee6e5a177093505b1897220dc9563c5bb687bab9953c77f909a419649dc45ea`. This resolves §8's decision gates for this profile alone. HAL002/002A, HAL357 §12 and HAL316's Draft status retain their authority. The grant covers one coherent S2. Backend execution, native frame evidence and JC's visual acceptance remain separate.

**Source and phase.** Adopt the exact types and signatures of §8.1 and the projection in §8.2. One immutable ordered batch of at most eight anchors is scoped to the live Context and exact admitted PbrScene3D Arc. IDs are unique within the batch. A checked PBR packet slot names its occurrence and model placement, including repeated geometry. The finite WorldPoint3 and one-line text are consumer facts. The slot has no visibility meaning. The caller supplies exterior points with clearance. None removes the HUD, and repeating the same request is clean. Update owns replacement and narrow invalidation. The existing Viewport3D layout may retain resolved text bindings and scalar measurements without changing fill size. Derive the placements from final layout and the same checked View80, render origin and optional metric scaling used by canonical PBR. Perspective and orthographic share this path. Suppress out-of-range, nonfinite, behind-camera and clipped anchors. Apply inherited clipping without crossing ordinary control, modal or surface order.

**Depth and GPU result.** Only an admitted HUD target generation may give the actual canonical `Depth32Float[N]` both `RENDER_ATTACHMENT | TEXTURE_BINDING`. Qualify actual Features::empty Device support before use. The existing PBR pass writes the depth, including actual Mask/discard. N1 loads `texture_depth_2d` and N4 loads `texture_depth_multisampled_2d` at the actual target pixel and every sample. There is no replay, fifth MRT, depth approximation or color/shadow visibility proxy. Same-Queue GPU evaluation after the depth producer and before the surface's HUD commands writes one binary u32 per original ordered anchor in a nonaliasing generation-bound buffer. The renderer receives an opaque private lease for the matching depth generation, with no CPU visibility readback. Keep the ordinary target-bound Image and its canonical publication.

**Whole-pair visibility.** Anchor, entire label box and full leader stroke use the anchor's constant projected reverse-Z depth. Test the projected anchor pixel and every physical pixel center covered by box or stroke. A stored zero is clear. Against nonzero stored depth the anchor must be strictly greater. All samples at every tested pixel must pass, including all four in N4. Any blocked sample or tie suppresses the entire pair. This is a visual plane, with no depth-owner or physical geometry claim. Across the batch, at most 1,048,576 physical pixel centers may be tested in a target frame, with all N loads accounted at each center. At most eight flags are produced. Overflow, missing association and insufficient resources refuse the optional group.

**Layout and text.** Each label is one logical line of at most 96 original UTF-8 bytes and 64 shaped glyphs, at most 512 glyphs in the batch. Use ordinary theme-resolved text at 13 logical px. Ink width at most 240 and line height at most 28, with horizontal padding 8 and vertical padding 4 on each side, give a box at most 256×36 logical px. Admit the first disjoint measured box in Above, Right, Below, Left order, gap 6 and crop margin 6 logical px. Use batch order and keep earlier reservations even when GPU-occluded. Draw one straight leader to the nearest box boundary, length at most 160 and width 1.5 logical px. Use existing high-contrast theme text/connector color. Constrain ink and stroke to the viewport crop and inherited clip. When no candidate fits omit the pair. There is no ellipsis or truncation, and invalid text or a limit refusal does not degrade ordinary PBR or UI. There are at most four positions per anchor and 2*A*(A-1) box comparisons. Camera changes reuse matching text measurements and recompute only projected placement.

**Optional atomic Compile.** Reserve and realize ordinary HAL360 text first in the existing surface compiler. Only after that attempt the complete HUD working set through the same shaping, rasterization, atlas and lowering owners with ordinary glyphs protected. A failed HUD candidate contributes no text, backgrounds, leaders or visibility use to the admitted schedule and preserves the ordinary surface outcome. Admitted HUD is in the same immutable HAL359 schedule. Strict local order is image, leaders, then labels with contiguous text-native backgrounds. Use the existing retained clip, target and surface. Paint does not modify layout, atlas or the tree. There is no additional text engine, surface, input target, compositor owner or time demand.

**Currentness and limits.** Use §8.5's source/request/mount/Context/view/origin/target/N/extent/crop/scale/depth/front/compiled-generation association. Only a complete matching registered current front permits HUD. Stale or failed presentation withdraws HUD, with original current or compatible last-good PBR carrying its own provenance. Reserve actual aligned CPU/GPU data, depth resources, glyph/atlas demand and in-flight overlap against the existing view and aggregate policies. Share one canonical depth payload for the current scene and charge any older retained physical allocation until release. None and unchanged/inactive states have no extra font, depth, visibility or frame demand. Unsupported combinations and capacity refusal affect only HUD. Queue ordering and actual leases protect all reads and writes.

**Verification.** Execute §8.6's one coherent source, GPU and native vertical, with actual N1/N4 sample tests, Mask/discard, source-distinct occurrences, strict ties, blocked anchor/box/leader, ordinary text parity, finite costs and a registered submitted/presented_called AppFrame. Physical scanout and JC review require separate observation. No broader cards, HAL316, input, animation, source semantics or consumer integration is admitted.

## 10. Sole-binding presentation companion for the passive PBR HUD

**Status:** Adopted / Active / Binding [scoped §10 S1]. Independent architectural review examined and adopted this exact first-companion profile under JC's technical delegation in `HAL375-ZERO-VIEW-SOLE-BINDING-COMPANION-S1-ADOPTION-AND-S2-EXECUTION-03`. The reviewed `HAL375-ZERO-VIEW-SOLE-BINDING-COMPANION-S1-NORMATIVE-CLOSURE-02` beforeimage is SHA256 `1fce0c4ce477d655888b9078e7b34954506eba41cb5ae6590854b8847f567a01` on source basis `909c10850b5b131992fb77f257d1b2b824319915`. Section 9 retains its binding status. The baseline HUD transport conflicts with HAL355 §§5.3 and 15. This adoption authorizes one scoped S2 correction. GPU and native S2 conformance require actual execution.

### 10.1 Adopted clauses and compatibility

**HAL266A §3.4, sole publication.**

> An admitted HAL375 viewport may include one opaque immutable frame-scoped presentation companion in its sole existing per-target Image publication. The outcome associates it with the same `CustomRenderAdmission` identity, `SurfaceId`, texture and current front generation, and exact renderer-owned target snapshot. The companion carries qualified data and resource custody only. Its ingress and handler are sealed to the engine for HAL375 §9. The participant receives no renderer, atlas, DrawPlan, RenderGraph, compositor, execution, or semantic authority. No additional Image, participant, surface, public extension API, or binding key is admitted.

**HAL266A §6, INV-266A-006, two-phase resolution.**

> Paint continues to emit the ordinary target-bound Image. The binding and prepared payload are immutable during Paint. After Paint, the renderer-owned DrawPlan materializer may record one occurrence receipt in that same binding record when the ordinary Image is actually inserted. The receipt identifies the exact inserted primitive in the current surface and build, including its final effective order, inherited clip and scale. The materializer writes occurrence evidence only. It never changes the binding, resource, placement, or target authority. An absent Image supplies no receipt. A second inserted occurrence for the same binding makes it ambiguous for the rest of the build. Freeze all receipts before Compile. Compile may use a unique receipt only while its exact Image survives ordinary Compile and registration checks for the current target.

**HAL266A §6, INV-266A-007A, isolation and expiry.**

> The engine authenticates the existing `(presentation identity, SurfaceId)` key and keeps the optional companion and occurrence receipt in the same frame-local record as the one Image binding. Invalid, inactive, absent, ambiguous, or stale companion evidence is ineligible for optional output. Ordinary Image eligibility retains its own contract. Compile receives read-only records. Release the frame records when Compile consumes them or the frame is abandoned. Only admitted queue-safe execution custody may outlive that frame's Compile. Ordinary non-companion publication and the existing `Copy` Image binding retain their meaning.

**HAL295A INV-004/006, renderer compatibility clarification.**

> A HAL266A companion for HAL375 is private data within the sole existing binding. The producer's sole scene contribution remains the registered, target-bound Image. Only the renderer may admit the complete HAL375 §9 text, backgrounds, leaders, and visibility into the normal surface schedule. INV-004/006 retain their prohibitions on producer access to placement, lowering, DrawPlan, and graph authority.

**HAL355 §17.4, cross-reference.**

> HAL375 §9's private visibility and ordered optional presentation use the sole-binding companion and frozen occurrence receipt under HAL266A §3.4 and INV-006/007A. Sections 5.3 and 15 apply unchanged, including through generic custom-render preparation. Inactive viewports and applications without 3D do no HUD work. The normal Image, canonical N1/N4 depth and front, and compatible last-good retain their existing owners.

**HAL375 §9, technical concordance.**

> The qualified viewport delivers its source, front and depth lease only through the sole HAL266A binding. The renderer must prove the exact current surviving Image before optional glyph demand. Protect all ordinary HAL360 text first, then admit the complete optional group through the same atlas. Compile owns the final HAL359 schedule on the same surface. Missing or refused HUD leaves the ordinary Image, text, and surface outcome under their existing laws. Section 9's source, occlusion, clipping, order, budget, and retirement rules apply unchanged.

### 10.2 Types and owner stages

`CustomRenderParticipant::prepare_custom_render` keeps its exact signature. The public `CustomRenderPresentationBinding` keeps its current fields, getters, and `Copy` semantics. The declarations below specify the interface and record shapes.

```text
pub struct CustomRenderPreparationOutcome {
    pub front_publication_changed: bool,
    publication: Option<CustomRenderPublication>,
}

pub trait CustomRenderParticipant {
    fn prepare_custom_render(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
    ) -> CustomRenderPreparationOutcome;
}

pub(crate) struct CustomRenderPublication {
    binding: CustomRenderPresentationBinding,
    companion: Option<Box<CompanionRecord>>,
}

type PresentationKey = (CustomRenderPresentationId, SurfaceId);

struct CompanionRecord {
    prepared: Arc<PreparedHud3D>,
    metrics: CustomRenderTargetMetrics,
    occurrence: ImageOccurrence,
    next_recorded: Option<PresentationKey>,
}

enum ImageOccurrence {
    Unseen,
    Unique(ImageOccurrenceReceipt),
    Ambiguous,
}

struct ImageOccurrenceReceipt {
    order_key: u32,
    batch_key: DrawBatchKey,
    clip: ClipState,
    rect: RectF,
    scale_factor: f32,
}

pub(crate) struct CustomRenderPresentationBindings {
    entries: HashMap<PresentationKey, CustomRenderPublication>,
    recorded_head: Option<PresentationKey>,
}
```

Keep the public `new`, `front_publication_changed`, and `with_presentation_binding` constructors. The ordinary getter becomes `pub const fn presentation_binding(&self) -> Option<CustomRenderPresentationBinding>` so it can return the copied binding without consuming the owned outcome. The outcome can no longer implement `Copy`. Its ordinary constructors, method-call form, and flag retain their existing meaning. Keep its ordinary `Default`, `Debug`, `Clone`, and equality behavior. The engine uses `pub(crate) fn into_publication(self) -> Option<CustomRenderPublication>` to move the single record. Ordinary callers have no companion constructor. The engine-only constructor below accepts only the viewport-qualified payload and fresh target metrics. The companion keeps the current preparation attempt in those metrics even if the viewport reuses an immutable `PreparedHud3D` from an earlier attempt with matching content. An old cached payload is not evidence of current frame publication.

```text
CustomRenderPreparationOutcome::with_spatial_hud_companion(
    self, prepared: Arc<PreparedHud3D>, metrics: CustomRenderTargetMetrics,
) -> CustomRenderPreparationOutcome
```

**Prepare.** Move `prepare_spatial_hud_publication` into the end of `Viewport3D::prepare_custom_render`, after it has chosen its actual Image binding and before it returns. Keep the existing request/source/view/Context/mount/target/depth/front qualification and publication-change bit. Only the matching canonical Image may carry the payload. An ineligible, inactive, or absent request carries no companion. WidgetTree takes the generic outcome, verifies its admission ID and publishes its one record. It does not downcast to `Viewport3D`. Plot keeps its own callback and registered Image unchanged. [Outcome and table](../../src/renderer/custom_render_prepare.rs) at lines 37–206. [Copy binding and UI-local pin](../../src/renderer/custom_render_pass.rs) at lines 158–184 and 336–365. [Tree preparation](../../src/core/widget_tree.rs) at lines 1393–1454. [Viewport preparation](../../src/three_d/viewport.rs) at lines 1409–1538 and 4593–4805. [Plot](../../src/plot/mod.rs) at lines 2393–2435 and 4448–4465.

**Materialize and freeze.** A `PresentationFrame<'frame>` owns the existing table's exclusive borrow from preparation through teardown. Paint has no recording access. The materializer takes the guard into an `ImageOccurrenceRecorder<'frame>`, which can temporarily lend `ResolvedTargetImage<'_>` for one existing target-bound lookup. Freezing moves the guard into `FrozenPresentationBindings<'frame>`, which exposes only shared reads. The guard clears the frame records once on teardown, including paths that skip or fail Compile. Capacity of the existing table can be reused. No frame-local payload survives because its Image was omitted or ambiguous. No interior mutability or second table is needed.

```text
CustomRenderPresentationBindings::begin_frame(&mut self) -> PresentationFrame<'_>

PresentationFrame::publish_for_target(
    &mut self, surface_id: SurfaceId, expected: CustomRenderPresentationId,
    publication: CustomRenderPublication,
) -> Result<(), CustomRenderPreparationError>

PresentationFrame<'frame>::begin_materialization(
    self, build_frame: u64, attempt: Option<AppFrameAttemptId>,
) -> ImageOccurrenceRecorder<'frame>

ImageOccurrenceRecorder::resolve_target_bound(
    &mut self, id: CustomRenderPresentationId, surface_id: SurfaceId,
) -> Option<ResolvedTargetImage<'_>>

ResolvedTargetImage::binding(&self) -> CustomRenderPresentationBinding
ResolvedTargetImage::record_inserted(self, scale_factor: f32, primitive: &FlatPrimitive)
ImageOccurrenceRecorder<'frame>::freeze(self) -> FrozenPresentationBindings<'frame>

Batcher::flatten_scene_for_surface(
    &mut self, nodes: &[SceneNode], mesh_cache: &mut MeshCache, scale_factor: f32,
    current_frame: u64, surface_id: SurfaceId, recorder: &mut ImageOccurrenceRecorder<'_>,
)

DrawPlanBuilder<MultiSurfaceContract>::build_multi_with_presentation_bindings<F>(
    &mut self, surface_scenes: &HashMap<SurfaceId, SceneGraph>,
    surface_stack: SurfaceStackSnapshot<MultiSurfaceContract>, mesh_cache: &mut MeshCache,
    recorder: &mut ImageOccurrenceRecorder<'_>, scale_factor_for_surface: F, current_frame: u64,
) -> DrawPlan<MultiSurfaceContract> where F: Fn(SurfaceId) -> f32
```

Only `ImageTextureSource::TargetBound(id)` takes the existing target-binding lookup. The one scoped lookup lends the record and copies its ordinary binding for normal insertion. Call `record_inserted` only with the actual inserted Image and after the existing control-subtree order stamp. Batcher passes the borrowed recorder to its existing traversal and stores no companion or receipt. The first receipt prepends that same record to the table's active receipt chain. A second receipt changes the state to `Ambiguous` without another link. Freeze reverses only those links once to recover materialization order. Without a companion there is no receipt, link, heap allocation, or follow-up traversal. Registered or admitted non-target-bound Images take their ordinary paths. [Image insertion and order stamp](../../src/renderer/batcher.rs) at lines 1421–1444, 1981–2024, 2080–2128 and 2300–2312. [DrawPlan builder](../../src/renderer/draw_plan_builder.rs) at lines 103–195.

**Compile ingress.** App passes the frozen view after its existing registration drain and materialization. It has no HAL375 predicate or type. The existing two-argument `compile_sdf_frame` remains the ordinary adapter without companion inputs. The native binding path uses the following crate-private adapter. An empty receipt chain enters the same ordinary compiler with no HUD pass, per-surface probe or allocation.

```text
WgpuRenderer::compile_sdf_frame(
    &mut self, frame: SdfFramePlan, surfaces: Option<&HashSet<SurfaceId>>,
) -> Result<Option<SdfCompiledFrame>, wgpu::SurfaceError>

WgpuRenderer::compile_sdf_frame_with_presentation_bindings(
    &mut self, frame: SdfFramePlan, surfaces: Option<&HashSet<SurfaceId>>,
    bindings: FrozenPresentationBindings<'_>,
) -> Result<Option<SdfCompiledFrame>, wgpu::SurfaceError>
```

The frozen borrow belongs to that build and renderer preparation. The canonical App attempt must also match when present. A noncanonical test without an App attempt still requires the same single build and renderer preparation. The current `HdrFrontPin` carries `PhantomData<Rc<()>>`. The `Arc<PreparedHud3D>` therefore remains on the UI phase path and is not `Send` or `Sync`. Compile borrows it there. A successful compiled group carries `SpatialHudWork` and its queue-safe `HdrFrontReadLease`, depth, program and storage. Neither the UI pin nor the frame table enters a graph executor, queue callback, or worker. [App and Compile adapters](../../src/core/app.rs) at lines 19113–19161 and [renderer](../../src/renderer/wgpu_renderer.rs) at lines 6694–6733. [Work and retirement](../../src/renderer/wgpu_renderer/spatial_hud.rs) at lines 443–567 and 643–647.

### 10.3 Image survival, Compile and Execute

1. Admit and protect all ordinary surface working sets first. Then visit only unique frozen companion receipts in materialization order, restricted to current output targets with complete ordinary demand. Locate the exact `FlatPrimitive.order_key` in its surface. Require one match and the recorded Image key, effective order, full inherited `ClipState`, rect and scale. Recheck frame, source, current target, and `HdrFrontPin::registered()` against the actual registry identity and front generation. Compare the actual sampled view to the pin. An absent, ambiguous, filtered, or mismatched occurrence refuses HUD before optional glyph demand.
2. Prove the ordinary batch containing that occurrence. `build_batches` and Image vertex/index lowering use the same sorted primitives and count each Image as four vertices and six indices. Recover the occurrence's surface-local ranges from that order with checked arithmetic. Require containment in the exact ordinary Image batch with the matching key and clip. Apply the same registration and custody predicate to this batch before HUD glyph admission and in the final ordinary Image filter. Extract the existing predicate once as `WgpuRenderer::image_batch_is_executable(&self, batch: &DrawBatch, surface_id: SurfaceId, sdf_custody: Option<&SdfSceneCustody>) -> bool`. Preserve the ordinary diagnostics and failure branch. Texture ID alone cannot identify the primitive or batch. The final retained batch and its rebased ranges must still match after ordinary revalidation. An intervening revocation or failure withdraws the whole group before visibility work or schedule admission. Atlas-driven bind-group rebuild preserves the existing Image registration identity. [Batch ranges](../../src/renderer/batcher.rs) at lines 475–491 and 2394–2517. [Image rebind, lowering and final filter](../../src/renderer/wgpu_renderer.rs) at lines 3040–3062, 3857–3875 and 7459–7527.
3. The private renderer handler uses the existing HUD text admission and final-atlas lowering. Protect ordinary glyphs before any HUD attempt. Lower both ordinary and admitted optional glyphs only after working-set admission ends. A failed optional group supplies no leader, background, text, visibility dispatch or draw. For a successful group Compile merges its leader and label commands at the Image's effective order with Z+1 and Z+2. Preserve contiguous text-native backgrounds and inherited clipping. The exact §9 group remains below the overlay boundary. The private active surface schedule can represent unchanged ranges of ordinary `SurfaceDrawCommand`s between the sealed HUD draws. Compile fixes that single order and the ranges. Execute consumes it without a second sort or plan discovery. [HAL359 §5](SPEC-HAL-359-Cross-Pipeline-Z-Order-Execution.md) and [HAL360 §§3–4](SPEC-HAL-360-Complete-Text-Realization.md). [Current HUD admission and lowering](../../src/renderer/wgpu_renderer/spatial_hud.rs) at lines 44–152.
4. Use existing renderer-owned graph nodes for the active target. The sealed visibility executor uses the current charged depth and writes the existing indirect visibility results before the active main pass. Represent that dependency with the existing `RenderPassNode` and `PassExecutor` seam. The main pass uses the merged order on the same output attachment. The overlay path remains ordinary. Keep the active schedule and work in the renderer-private `SdfCompiledFrame` and `SdfRenderOutput` custody path. Populate custody directly from admitted groups and carry only the encoded target's work through actual submit and existing Queue retirement. The ordinary plan, graph and command dispatcher have no HUD variant, pass, or discovery scan. No producer sees an encoder or executor. [Pass interface](../../src/renderer/render_graph/pass.rs) at lines 17–58 and 107–129. [Current submit boundary](../../src/renderer/wgpu_renderer.rs) at lines 3308–3350. HAL295A §5's prohibition on unrepresented passes remains in force.
5. Remove the separate HUD binding table, common Batcher and DrawPlan vectors, App HUD compile route, ordinary per-surface HUD maps and scans, and the two common command-list scans for visibility and output custody. Qualify HUD compute/indirect and depth support only on an active qualifying viewport. The eager HUD-specific predicate at `wgpu_renderer.rs:2237` and its copied flags must leave the ordinary renderer and resource path. Use the same already selected adapter handle, retained with the existing renderer-owned GPU handles, for the active query. This needs no new adapter or Device request, feature, or service. Budget companion records and active-only execution storage under existing limits. No admitted HUD means no HUD atlas or execution work. An otherwise valid viewport can already have charged canonical sampled depth at Prepare before a later optional Compile refusal. Such a refusal admits no visibility or draws and cannot erase a live charge. On inactive retirement the viewport releases HUD-only retained program and depth ownership, with admitted in-flight resources protected through Queue completion.

### 10.4 S2 conformance gate

- Check the ordinary library with default features disabled and check existing targets for API compatibility. Run native Label/overlay with generic Plot, first without a Context and then with a Context and never-active viewport. Verify ordinary Image/clip/order, Complete text, bit outputs, and unchanged submissions. Count companion allocations and extra preparation, Compile, glyph, and GPU work. Require zero HUD activity in both controls. Also retire a formerly active viewport and verify release after Queue completion. Preserve prior HAL372, HAL374 and HAL381 paths.
- Extend the existing `hal375_registered_mask_depth_gates_real_surface_text_n1_n4` owner with two source-distinct occurrences sharing a mesh, actual Mask/discard, N1 and all four N4 samples, box and leader occlusion and exact depth ties. Prove the actual surviving primitive, registered generation, target, order and clip. Refuse missing or filtered Image, two occurrences of one binding, stale generation, mismatch of source/target/window/resize, and unavailable optional text or storage with ordinary text and Image parity. Verify the same-surface immutable order, no orphan work, and submit and retirement custody. [GPU test owner](../../src/tests/three_d_viewport.rs) at line 766.
- Execute the existing native `App::run` owner in both sample modes. Require the `hal375_native` receipt, exact current registered front, actual surface output, `submitted` and `presented_called`. Physical scanout remains a separate observation. [Native harness](../../tests/app_frame_realization_native.rs) at lines 2745–2775 and 3355–3385.

```sh
cargo check --offline --lib --no-default-features
cargo check --offline --all-targets
cargo test --offline --lib hal375_
cargo test --offline -q --features test-harness --test app_frame_realization_native -- --run-spatial-hud-native --samples-one
cargo test --offline -q --features test-harness --test app_frame_realization_native -- --run-spatial-hud-native
cargo test --offline -q --features test-harness --test app_frame_realization_native -- --run-spatial-hud-zero-view-native
cargo test --offline -q --features test-harness --test app_frame_realization_native -- --run-spatial-hud-zero-view-native --inactive-context
cargo test --offline -q --release --features test-harness --test app_frame_realization_native -- --run-spatial-hud-native --samples-one
cargo test --offline -q --release --features test-harness --test app_frame_realization_native -- --run-spatial-hud-native
```

**S2 source and execution record.** The owned outcome shares the immutable prepared payload and its charge through an Arc. The renderer boxes the mutable occurrence receipt only in the single authenticated table row. The fallible private `attach_spatial_hud_companion` preserves the Image when a companion charge is refused. The frame guard lends the table during preparation and owns its teardown. One `validate_image_batch` predicate is shared by the active HUD admission and the ordinary final Image filter. Its typed rejection preserves the existing HAL381-specific withdrawal and the ordinary Image diagnostic.

The no-default-features library and all-target checks passed. On Metal with the existing Device feature set, `hal375_` passed 6/6. N1 and N4 verified Mask/discard, depth ties and whole-pair occlusion with the canonical FP16 pixels unchanged. Missing, duplicate, clipped and stale Image evidence, and optional atlas exhaustion, admitted no HUD commands. The 2D GPU control preserved the rounded-clip pixels, ordinary text, draw order and base topology with an inactive viewport. The native Plot/Label/overlay controls each produced one registered Image batch, one overlay command and two base graph nodes with Complete ordinary text and no HUD work.

The actual native N1 and N4 runs passed in Debug and Release. They observed the same registered PBR Image with two visible HUD pairs and one occluded pair, then the same Image after HUD removal. Both frames were submitted and called present. Focused HAL372, HAL374, HAL381, HAL359 strict order and HAL360 complete-text checks passed. The separate `control_subtree_order` filter reproduced its two known `TextRealization::Unavailable` failures. Physical scanout and JC visual acceptance remain separate.
