# SPEC-HAL-372: Source-Qualified 3D Object Reveal

**Status:** Adopted / Active / Binding [S1 TECHNICAL CONTRACT ONLY, 2026-11-05]
**Authority:** JC direct mandate; selected and adopted through delegated independent architectural review.
**Implementation classification:** ABSENT for this specific reveal effect; no S2 source, GPU, public S3 or native S4 proof follows from S1.
**Source review:** Independent architectural review verified existing PbrScene3D identity, PBR shader FragmentIn.position, Object112 uniform and dynamic offsets, complete R024/R025/R026/R028 shader composition, R027 depth/front and HAL021 timeline; exact selected closure appears in section 10.
**Historical reading:** Earlier Draft/Inactive assertions and the original drafting scope apply to the original proposal before this dated S1 adoption; all unlisted material and binding parent restrictions remain in force.
**Date:** 2026-11-05
**Layer:** Optional current-source PBR render-only visibility presentation

## Historical drafting scope

- **Objective:** specify only render-only appearance of one qualified object/visual group in an already admitted current PBR scene; preserve semantic source and ordinary working output.
- **Standing objective:** support faithful consumer-authored placement presentation without inferring physical assembly or interfering with the source producer's independently owned original-source work.
- **Review authority:** The delegated architectural reviewer selected the logical requirements; this historical draft records their source concordance and unresolved technical choices. JC retains product/constitutional control. The draft itself grants no implementation or consumer-integration authority.
- **Governing contracts:** HAL002/002A/309, binding HAL021/HAL355/HAL370 and R024/R027/R028; affected scene, snapshot, viewport, raster and Timeline source. HAL358/359 scope and the inactive/rejected statuses listed in section 9 grant no additional effect authority.
- **Owned artifact:** this file only. Existing SPECs, indexes, guides, runtime, tests, manifests, assets, consumer applications and task graph are outside the write scope. Preexisting foreign R024/R028/HAL370/proposal bytes remain protected.
- **Named invariants and forbidden cases:** section 7. Reuse existing Context, Device, Queue, preparation, target/front, Timeline and retained publication; no second renderer, clock, scheduler, source registry or semantic owner.
- **External boundary:** consumer supplies accepted semantic source/model/placement and any genuine physical-part provenance. HAL supplies only qualified render mechanics. No asset or private consumer-source inspection is needed.
- **Verification:** documentary source concordance, full changed-file readback, links/headings/status, exact owned diff and protected beforeimages. Section 8 is future proof, not tests executed here. No Cargo/build/GPU/GUI or runtime workload.

### Architecture boundaries

- Governing law remains HAL002/002A and the exact accepted source/time/render owners; no operative amendment.
- Widget policy/new interaction: none. Existing focus, Capture, navigation, picking and input policy are unchanged.
- No implicit semantic identity, source repair, fractional-alpha substitution, stale-front promotion, hidden runtime/ENV diagnostic switch or capability claim from source presence.
- Existing owners are identified in section 2; a distinct presentation contract is missing, not permission for an application workaround.
- No demo, alternative renderer, import, asset repack or cross-project effect is authorized.

## 1. Purpose and exact scope

The proposed primitive is an explicit optional **visual appearance of already admitted PBR geometry** for one source-qualified object or render group. It changes only that group's qualified rendering visibility/coverage during a bounded visual progression. It does not progressively accept geometry or create a different semantic scene.

Source geometry, original normals/UVs/signed tangent provenance, material factors/roles, placement and source/model identity remain unchanged by the effect. A render group or surface region carries no assertion of physical component boundaries. Ordinary placement is not assembly simulation, a selection effect, material editing, source generation or a generic 2D subtree transition.

Consumer-owned product events choose whether to request the presentation. Product text, prices, manufacturers, manuals, instructions, physical components and connection facts remain consumer evidence. A later explicitly sourced part-by-part instructional Structure Reveal needs its own consumer contract; it is outside this proposal. Outline/Halo, HUD, text reveal and material crossfade are separate capabilities and are not bundled here.

## 2. Current source inventory and missing surfaces

Runtime coordinates below refer to the inspected revision, not a future implementation. Governing documents were read in their current working-tree state; this candidate neither stages their preexisting changes nor declares them committed in the runtime revision.

| Current owner/source | Implemented evidence | Boundary for this proposal |
| --- | --- | --- |
| [material_api.rs:1075](../../src/three_d/material_api.rs), `PbrDrawPacket3D` | Geometry/material handles, raster mode and `stable_ordinal`; no consumer semantic group identity or reveal state. | Packet ordinals are technical attribution, not a permanent model/part bridge. A trusted group association and its validation ingress are missing public surfaces. |
| [snapshot.rs:99](../../src/three_d/snapshot.rs), `CommittedMaterialPacket3D`; `MaterialSnapshot3D` | Immutable context-qualified geometry/material leases and structural revision. | Existing snapshot/model identity remains source authority; progress must not rewrite or recreate that source each tick. |
| [viewport.rs:1226](../../src/three_d/viewport.rs), `request_pbr_scene` | Accepts an actual `PbrScene3D`, view revision and view; no reveal argument. [Target preparation:2908](../../src/three_d/viewport.rs) consumes actual target metrics; [Paint:3015](../../src/three_d/viewport.rs) emits the current target-bound WHITE Image. | No public presentation occurrence/group-visibility operation is implemented. Requested, committed and target-local last-good associations must remain truthful. |
| [raster.rs:2855](../../src/three_d/raster.rs), `encode_material_scene` | Private qualified PBR scene/depth draw path. [Pipeline:4806](../../src/three_d/raster.rs) uses `blend=None`, reverse-Z depth writes and no alpha-to-coverage. | A per-group visibility execution contract is missing. These fixed states are not an existing reveal selector. |
| [hdr.rs:633](../../src/three_d/hdr.rs) | Existing scene/depth work feeds the controlled R027 tone pass and the same final-front publisher. | Private scene radiance, depth, toned coverage and registered Image remain distinct. No new publisher or global fade is implied. |
| [animation.rs:842](../../src/core/animation.rs), `AnimationContext` | Public start/start_descriptor/cancel/sample; frame-stable typed observations. [App:17330](../../src/core/app.rs) advances from captured engine time before routing. | Time/value primitives exist. They do not choose source associations, render coverage, presentation endpoints or effect retargeting. |
| [layout_transition.rs:1983](../../src/core/layout_transition.rs) | Existing rect transition replacement cancels a handle and binds `candidate.from` from layout history. | This is not proof of retargeting from an object's currently admitted visual presentation. It must not be relabeled as this capability. |

The missing logical source-association owner is the current typed PBR snapshot/model admission boundary, with actual immutable leases. The missing execution owner is the existing 3D preparation/raster/HDR/target path. Exact public ingress, internal representation and any required technical extension remain decisions in section 10. A producer Boolean or an application's guessed packet number cannot replace HAL's validation.

## 3. Source-qualified association and ownership

The consumer must supply an attributable association between the intended visual group and the exact admitted source/model resources. HAL must check it against the captured source, model/placement, resource generations and complete membership it actually renders. Membership cannot be inferred from labels, spatial resemblance, mesh triangulation, `stable_ordinal` alone or an unrelated retained selection.

An unknown, stale, ambiguous or mismatched group is refused **for effect scope**. This refusal is not permission to alter source or to label an invalid source valid. An already admitted scene retains its ordinary source/front rights under HAL355; failure to admit the extra effect cannot by itself invalidate that source or erase a compatible front.

HAL owns only the qualified rendering association and its finite lifetime. The consumer remains sole owner of source/model semantics, accepted placement, semantic revision, selection and any physical-part meaning. Layout, hit geometry, picking, focus/Capture and actions remain their existing owners. Visual invisibility must never be reported as semantic absence or as removal of an accepted object's picking/action availability.

Any actual source, material, resource or placement change still passes the existing changed-input owner. A presentation request cannot bypass it or mint a new semantic revision merely to drive an animation. Conversely, retaining the same semantic source does not authorize stale GPU or target generations.

## 4. Distinct presentation identity and lifecycle

A presentation has a distinct bounded identity/progress while remaining associated with the exact qualified source/model, current request/view, target and resource generation. These are logical association obligations, not selected struct fields or ABI. View/request serial changes and semantic revision changes are not synonyms.

Repeated equivalent requests, repaint and camera/view changes alone must not restart entry or mint semantic state. A real target/view successor must update its qualified association through existing owners without pretending that an incompatible prior front belongs to the successor. The rules for carrying presentation state across a newly qualified association remain an explicit review block; no identity bridge is invented here.

The following are required **typed outcome distinctions**, not proposed Rust variant names. Exact error precedence/storage and the pending-association policy are unselected in section 10.

| Event/outcome distinction | Selected required disposition |
| --- | --- |
| Qualified request admitted; active progression | Bind only the current complete association. Preserve accepted application state/actions and ordinary input availability. No recreated PBR source per tick. |
| Equivalent request or view-only/repaint update | Do not rearm entry, reset progress or create semantic revision. Revalidate only genuinely changed owner inputs. |
| Eligible new presentation target | Retarget from the current actually admitted presentation under section 5; never restart from hidden merely because a target changed. |
| Completed progression | Expose the exact fully readable endpoint at the legal HAL021 completion observation and retain ordinary complete current presentation afterward. No last pre-terminal sample may strand visibility. |
| Explicit no-effect, disabled presentation or reduced-motion request | Select the current fully readable presentation without an animation wait or delayed action. This does not waive source, target, Device or resource admission. |
| Unknown/mismatched group or rejected effect candidate | Report the actual effect refusal. Do not silently infer membership, mutate source, install a partial candidate or call an unqualified request current. Preserve the preexisting valid source/front according to its real association. |
| Source/group replacement or retirement | Terminally retire the invalid old association and its owned progress; repeated cancellation/retirement is idempotent. A successor needs its own genuine eligibility/association, never inherited stale identity. |
| Resize, inactive/unavailable or incompatible target | Apply HAL355 target-local compatibility/withdrawal rules. An obsolete front cannot be kept as current. Continuing, settling or retiring progress for a new valid target requires the explicit target policy in section 10. |
| Failed preparation, finite capacity/OOM or Device failure | Report the true source/effect/target/Device stage and cause. No partial front, silent resource fallback or source repair. Preserve only a compatible actual last-good tuple; terminal Device/target retirement still withdraws it. |

"Actually admitted presentation" means the effective qualified presentation state installed by its lawful owner, not an unadmitted requested endpoint or assumed screen pixels. Admission, encode/submit, Image publication, GPU completion and physical display remain separate evidence stages. HAL355 requested/committed/last-good identities and committed-view queries remain authoritative.

Interaction availability does not relabel a pending requested scene as the committed one. Consumer actions may address their accepted semantic state; mechanical committed-view picking continues to name its actual committed source/view, including a truthful compatible last-good result. The animation may add no readiness delay and may not filter canonical picking to visually revealed portions.

## 5. Engine time, product timing and interruption

Use only HAL021's existing engine-owned time and scoped value operations. One captured frame timestamp controls old-state sampling, cancellation/replacement and the new request's initial observation. No own clock, accumulated animation `dt`, timer/thread, renderer-time advance, polling loop or new scheduler is permitted. Immutable presentation inputs are published through the existing lawful phases; preparation and Paint cannot advance Timeline or acquire semantic mutation authority.

An eligible retarget must begin from the current actually admitted visual presentation sampled at that timestamp and continue toward the new target without returning to an initial hidden state, replaying entry or introducing a value discontinuity/flicker. Timeline identity/state distinctions remain explicit. This does not promise velocity continuity or select a curve, spatial interpolation, GPU sequence or a public retarget method. Their required representation and proof remain open.

Applications explicitly supply a finite admitted duration. The reference indoor-placement regression uses **160ms**; this is a test input, not a framework default or a global timing range. Halcyon prescribes no artistic genre or additional motion-approval gate. No flashy entry, mandatory outro, artificial waiting period or physical assembly claim is part of this primitive.

The new semantic state and all its actions remain available as soon as accepted by their owner, independently of visual progress. Disabled/reduced-motion presentation selects the fully readable current endpoint at the next lawful current-request publication, with no animation-duration wait. This is phase-correct immediacy, not a claim of physical display before a frame is submitted. The preference's input source is not selected here; no ENV/diagnostic gate, alternate runtime or quality fallback follows.

## 6. Coverage, depth, materials, shadows and output

At ordinary covered output the original qualified R024 evaluator, source frame, original factors and all genuine texture roles remain intact. R028's authored sampler/filter/normal association, derivative validity and complete source certificates remain intact. R027's complete ideal radiance envelope, per-sample tone/coverage, current exact physical target, N1/N4 and finite resource/capability rules remain mandatory.

Presentation progress must not be stored in `PbrMaterialDescriptor3D.base_rgba`, `alpha_mode`, another material/source factor or an old reserved ABI word. No source preprocessing, asset repack, geometry bake, normal substitution, role suppression, source deletion/reordering or uncontrolled duplicate translucent mesh is authorized. A 2D Image opacity or room-wide scene fade is not a depth-correct per-object reveal.

R027 alpha remains covered-sample coverage, not arbitrary material translucency. Partial visual appearance must not fabricate covered black as hidden geometry, leave false depth/occlusion, or assert that source geometry has physically disappeared. Existing reverse-Z and depth/Mask semantics cannot be silently weakened. R025 shadow output and any visibility-dependent claims need an explicit source-correct rule; hidden render coverage must not silently delete original casters or claim a shadow/hit correspondence that was not established.

Semantic picking and actions remain available on accepted source throughout; their contract must not be reinterpreted as "only visibly revealed pixels can be picked." Any exposed visible-fragment/occlusion result must accurately name its different stage and association. The precise reconciliation of render visibility, source-complete shadows and visibility-qualified queries is an architectural blocker, not a choice delegated to an executor.

The GPU technique is **NOT SELECTED**. Coverage/discard and private resolved composition are unresolved examples, not admitted alternatives or fallback branches. The architect must select and justify a complete technique, source/evaluator interpretation and bounded resources before engineering. No new sample count, alpha-to-coverage rule, depth export, intermediate format, program variant, output registration or shadow technique is implicitly chosen.

Only the owning current complete target/front may be published through the existing Context/Device/Queue and Image owner. Charge all unique actual geometry, material/program, target, temporary, upload and in-flight/last-good/retiring leases required by any selected mechanism. Existing limits are not raised or bypassed. Exact new charges and capacities remain open; no cost estimate is asserted.

## 7. Proposed mechanically testable invariants

| Invariant | Forbidden counterpart |
| --- | --- |
| **INV-372-SOURCE:** progress leaves original source/model/material identity and bytes unchanged. | **FORBID-372-SOURCE:** source reconstruction or factor/geometry rewriting to animate visibility. |
| **INV-372-GROUP:** every affected region has the exact checked live source/group association. | **FORBID-372-INFERENCE:** ordinals, geometry proximity or visual resemblance as semantic/physical identity. |
| **INV-372-INPUT:** accepted actions, canonical hit/picking and focus/Capture do not await progression. | **FORBID-372-WAIT:** input lockout, delayed acceptance or animation-controlled selection. |
| **INV-372-TIME:** only HAL021's captured time and legal phase sampling control progress. | **FORBID-372-CLOCK:** private clock, timer, polling, renderer advance or recreated per-tick source. |
| **INV-372-RETARGET:** an eligible successor's initial value equals the current admitted visual value at the shared timestamp. | **FORBID-372-RESTART:** hidden reset/replayed entry from repaint, view-only change or retarget. |
| **INV-372-NO-EFFECT:** explicit no-effect/disabled/reduced-motion yields ordinary fully readable current presentation without a duration wait. | **FORBID-372-BLANK:** delayed blankness or waiving a genuine source/target refusal. |
| **INV-372-COVERAGE:** current source/shading/tone/coverage/depth/shadow claims remain truthful at their actual stages. | **FORBID-372-ALPHA:** invented material translucency, false hidden depth or Image-opacity substitution. |
| **INV-372-FRONT:** every visible current/last-good result names its true target/source/view/resource association. | **FORBID-372-STALE:** incompatible or retired front relabeled current. |
| **INV-372-BOUNDS:** all selected storage, uploads, overlap and GPU work have finite owner-accounted admission. | **FORBID-372-LAUNDER:** hidden resource owners, unlimited history or per-frame policy bypass. |
| **INV-372-RETIRE:** removal, replacement and cancellation retire only their owned association, idempotently and queue-safely. | **FORBID-372-REVIVE:** a late callback/sample reviving retired source or canceling an unrelated successor. |

## 8. Future causal proof obligations — not executed

The implementation task, fixtures, observables and acceptance criteria require independent selection after section 10 closes. Existing Timeline or legacy GPU tests do not establish the new primitive. Nearest source test owners are `tests/animation_timeline_contract.rs`, `src/tests/three_d_context.rs`, `three_d_snapshot.rs`, `three_d_raster.rs`, `three_d_viewport.rs`, `custom_render_preparation.rs` and the existing retained/input proof owners.

| Required future distinction | Source-grounded proof required |
| --- | --- |
| Complete source preserved; scoped group only | Same original source/model/material identities through progression; unaffected room/groups unchanged; no semantic revision per tick. Unknown/stale group effect refusal is attributable. |
| Application-supplied timing and frame-stable completion | Explicit 160ms fixture at 60/120 cadence, shared-timestamp values, exact terminal observation and ordinary full presentation afterward. No global HAL timing policy inferred. |
| Interruption/retarget | Retarget at a non-boundary time from current admitted presentation; no hidden reset, replay or discontinuity. Same request/repaint/camera-only update does not restart. |
| Semantic/input independence and no-effect | Operations and canonical picking remain available from source acceptance. Exact normal complete presentation for explicit no-effect/disabled/reduced-motion; no artificial wait. |
| Source/lifetime changes | One genuine removed/replaced source, late observations and repeated cancellation/retirement; no stale revival or unrelated successor cancellation. |
| Current target and visible output | N1 and N4, active resize/incompatible withdrawal, depth/occlusion, R025 shadows, original R024 roles/frame/material, R028 sampling, R027 alpha/tone and current Image association at the claimed stage. |
| Finite refusal and last-good | Finite capacity/OOM/Device/target errors before unauthorized effects; correct compatible last-good status and incompatible-front withdrawal; no partial candidate or resource leak. |

Predeclare predictions and observations separately for source/host admission, actual supported backend output, registered Image and physical/native observation. Finite GPU observations are not universal pointwise arithmetic proofs; R024 section 8.2.4 remains unchanged. This draft reports no current shader/API/GPU/native PASS, performance number or memory estimate.

## 9. Normative consistency matrix

| Authority and current status | Preserved boundary |
| --- | --- |
| [HAL002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md), supreme; [002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md), permanent | Retained ownership, phase separation, ID associations and source/semantic sovereignty; no constitutional amendment or second owner. |
| [HAL021](SPEC-HAL-021-Animations.md), Binding | Sole generic engine time/value owner. Effect requests must be idempotent at their own owner; Timeline is not a semantic request registry. No second playback/scheduler. |
| [HAL355](SPEC-HAL-355-Native-3D-Rendering-Substrate.md), binding substrate | Exact source/model/target generations, finite leases, true requested/committed/last-good state and incompatible-front withdrawal. |
| [HAL370](SPEC-HAL-370-Opt-In-3D-Visual-Profile-Admission.md), adopted profile authority | Any required effect/profile extension needs its actual separate adoption and qualification. This candidate does not silently widen an existing profile. |
| [R024](SPEC-R-024-Typed-PBR-Metallic-Roughness-Materials-and-Textures.md), Adopted / Binding | Original source/frame/material identity and complete ideal source law; actual host/source validation versus separately qualified backend output under section 8.2.4. |
| [R027](SPEC-R-027-HDR-Scene-Color-and-Tone-Mapping.md), Adopted / Binding | Coverage alpha, controlled per-sample tone, full ideal RGB ceiling60000, private scene/depth/final separation and current output publication. |
| [R028](SPEC-R-028-Texture-Sampling-Quality-and-Rendering-Conformance.md), Adopted / Binding | Original roles, sampler/filter/normal proofs and derivative-valid controlled sampling; no guessed visual substitute. |
| [HAL328](SPEC-HAL-328-Keyboard-Authority-Focus-and-Capture.md), Binding | Existing actual focus/Capture and application keyboard policy; visual progress gains no input authority. |
| [HAL358](SPEC-HAL-358-Luminous-Contour-Rendering.md), Adopted / Binding | Screen-space contour material only; no 3D reveal, physical topology, clock or semantic association inferred. |
| [HAL359](SPEC-HAL-359-Cross-Pipeline-Z-Order-Execution.md), Adopted / Binding | Existing 2D per-surface draw order. It does not provide PBR depth or authorize a 2D global fade as this effect. |
| [HAL316](SPEC-HAL-316-Annotation-Layer.md), Draft | Plot/widget annotation proposal is not adopted or bundled. HUD/label effects remain separate. |
| [HAL332](SPEC-HAL-332-Professional-Visual-Charter.md), REJECTED / RETIRED / INACTIVE / NOT BINDING | No universal genre, duration/easing prohibition or replacement aesthetic-approval gate is revived. |
| [HAL363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md) / [HAL364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md), Proposed/unadmitted | Neither pair nor its proposed amendments are adopted here. No occurrence/composition machinery is borrowed as operative law. |
| [HAL309](SPEC-HAL-309-Documentation-System.md), adopted documentation authority | Draft intent, owner selection, adoption, source implementation, executed tests and native observation remain distinct. |

## 10. Primary owner technical S1 adoption - eight CLOSED decisions, 2026-11-05

JC's direct execution mandate authorizes this narrow technical choice. Delegated independent architectural review adopts ONLY the fully specified, optional, whole-source PBR Object Reveal v1 below. This is technical S1 law, not a claim of a built effect, working GPU code, backend acceptance, physical appearance, product semantic part knowledge or new framework scheduling.

### 10.1 BLOCK-372-GROUP-ASSOCIATION - CLOSED

The only v1 group is all PBR draws belonging to ONE currently admitted whole PbrScene3D source Arc. Exact source identity uses PbrScene3D::same_identity / Arc::ptr_eq, with the real source snapshot/model/context/revisions, not stable_ordinal, mesh labels, spatial proximity, material similarity or inferred physical parts. Verified current source: material_api.rs:1106-1153,1293-1365; snapshot.rs:99-118,192-211. An effect request carries a strongly owned PbrScene3D source token; it must match the exact viewport's currently requested/current source and its valid Context/Device family. The optional effect rejects the complete request if the source contains ANY LegacyAppearance packet or no qualified PBR drawable triangle; it does not show a partial effect as if complete. Ordinary PBR/legacy render remains unchanged and available. Later independently sourced per-component grouping is NOT selected by v1.

### 10.2 BLOCK-372-PRESENTATION-STATE - CLOSED

Keep existing Viewport3D::new_pbr and request_pbr_scene APIs and no-effect behavior unchanged. Add one typed optional PbrObjectRevealRequest3D holding source:PbrScene3D, checked request_revision:u64 (nonzero monotonic within the viewport), duration:Duration (finite nonzero opt-in) and enabled:bool. Add exactly Viewport3D::request_pbr_object_reveal(&mut self, request:PbrObjectRevealRequest3D)->Result<bool,PbrSourceError3D>. A viewport owns at most one source-qualified visual occurrence with current source Arc, revision, one HAL021 animation handle and current admitted progress. It is never a domain Scene writer or secondary renderer. Requested, active, complete and retired effect state are separate from HAL355's actual source/target/current-front ownership. Validate before mutation, accept one current occurrence or refuse. Update publishes progress before legal per-target preparation cutoff; Paint, shader and Queue never advance time. Equivalent request/repaint/camera-only view changes do not rearm, change source identity or reset progress.

### 10.3 BLOCK-372-RETARGET-EVALUATOR - CLOSED

Use solely HAL021's captured engine frame time and AnimationContext start/cancel/sample. Selected easing is LINEAR, not unqualified cubic-Bezier x-inversion (animation.rs:219-230). Progress p is a finite valid f32 in [0,1], 0 hidden at entry and 1 fully ordinary-visible at completion. The consumer explicitly supplies duration; the 160ms regression does not establish a global mandatory HAL duration. An admissible same-source retarget starts from the LAST ACTUALLY ADMITTED p sampled at the shared current frame timestamp; never from an uninstalled requested endpoint. Original source replacement retires old occurrence and starts a new one; never transfer progress between Arcs. p=1 terminal must be observed and stop requesting new frames. No-effect/disabled/reduced-motion selects p=1 without waiting. Input/focus/capture/selection/semantic action are never delayed by reveal.

### 10.4 BLOCK-372-COVERAGE-DEPTH-SHADOW - CLOSED

The sole chosen technique is a SOURCE-WORLD-ALIGNED VERTICAL PLANAR REVEAL executed by the actual PBR fragment pipeline as BINARY COVERAGE DISCARD, with no material alpha or 2D Image opacity. Compute ONCE for a newly admitted source the scene-wide finite bounds of all referenced PBR triangle vertices transformed through their actual associated ModelPlacement3 into one common source-world frame. Use exact real geometry source from PbrGeometryLease3D.data.source (pbr.rs:729-744; geometry.rs:273-275,337-352) and exact snapshot placements (snapshot.rs:145-149; raster.rs:1718-1747), not stable ordinals or unrelated CAD assumptions. Axis preference: Y if maxY>minY, otherwise X if maxX>minX, otherwise Z if maxZ>minZ. All degenerate/nonfinite/unrepresentable source geometry refuses this optional effect with an attributed typed cause; no fabricated epsilon or box.

Shader FragmentIn.position at pbr.rs:899-911 is ALREADY object.model_relative * local vertex position in render-relative coordinates. No new varying or WGSL interstage location. Convert checked source-world lower/width into precisely the CURRENT render origin and metric units: for R025 use the same meters_per_scene_unit scaling as direct_shadow::metric_object in raster.rs:1699-1734. On camera/view-origin movement adjust only the per-frame O(1) render-relative interval, never rescan vertices. All f64 -> f32 bounds must be finite, ordered and representable; refusal never invalidates the otherwise qualified source.

At progress p exactly 0, discard all reveal-group fragments. At p exactly 1, never discard on reveal grounds. For 0<p<1, KEEP iff the relevant coordinate q[axis] <= lower_axis + span_axis*p; otherwise DISCARD. Source Mask alpha discard is still independent and unchanged. ALL original implicit-derivative R024/R028 texture sampling (surface(i) or sample_surface(i)) and Mask evaluation must run BEFORE the potentially divergent reveal discard, which happens before covered HDR/color/depth writes. Direct/indirect selected fragments preserve their original BRDF and source roles; later explicit-LOD environment lookups are allowed after discard only when valid. R027 FP16 tone/coverage, reverse-Z, N1/N4, no-alpha-to-coverage, original R028 UV gradients and current target publishing all remain unchanged for kept samples. Critically R025 shadow caster pipeline ALWAYS uses the original complete source/Mask and is NOT affected by reveal; visually hidden surfaces can still cast their source-complete shadows and are never presented as physically removed. Canonical picking/actions remain source-complete.

### 10.5 BLOCK-372-TYPED-ABI - CLOSED

The new program is opt-in, versioned and separate from original R024 Object112. Choose Object128Reveal as EXACT 128-byte shader/host uniform with bytes 0..112 IDENTICAL to old Object112 (model_relative mat4x4<f32> and normal_to_world mat3x3<f32>), then offset112 progress:f32, offset116 lower_axis:f32, offset120 span_axis:f32, offset124 axis:u32 (0/1/2). Assert Rust/WGSL byte sizes/offsets/alignments and actual Device acceptance; no old reserved-byte reuse. Keep group(1)/binding(0) but use an opt-in layout with min_binding_size 128; existing old 112-byte path, other group bindings, existing source material64, textures, samplers and source shader remain byte-unchanged. New exact dynamic object stride is round_up(128, actual device min_uniform_buffer_offset_alignment), checked against per-target arenas, u32 dynamic offset and actual hard max buffer sizes. No assumption that it equals old stride on every backend.

Build controlled opt-in shader variant from the SAME existing pbr::PBR_WGSL / mip_sampling::shader_source and exact R025 direct_shadow / R026 indirect compositor generation (raster.rs:4423-4532; pbr.rs:878-978; direct_shadow.rs:2134-2148; indirect.rs:990-1042). Transform ONLY the known Object112 declaration/var into Object128Reveal and augment the actual PBR color entries (pbr_fragment, direct_fragment, indirect_fragment, with R028 V2) with the post-sampling/post-Mask reveal predicate; preserve shadow_fragment and shadow source. Exact known shader anchors must each be validated and rejected on drift instead of performing blind string replacements. Reuse existing FragmentIn.position; no extra vertex attributes, bind group or interstage data. Module/pipeline cache key includes the reveal version/bit along with existing sample count, cull, observe, profile and sampler family; once created, progress changes only uploaded uniform bytes, not shader/geometry/source/texture allocation. Check max_uniform_buffer_binding_size>=128 on reveal variants only and preserve existing full feature/capability checks and pipeline limits.

### 10.6 BLOCK-372-FINITE-RESOURCES - CLOSED

One current occurrence/Timeline handle per viewport and one source-bound finite geometry-extent proof per exact immutable source, retired with its legal leases. Source extent traversal is bounded by existing admitted packet/triangle counts and performed ONCE per new qualified source, never once per animation tick. Potentially expensive source scan must be prepared through an already admitted CPU-owned source preparation path without blocking native UI, or the effect is refused with an exact owner/capacity cause: no invented scheduler, uncharged Arc or long UI scan. Charge the real Object128 per-packet arena incl. actual dynamic alignment (even if unchanged physical stride in one device), real shader/module/pipeline cache variants, target bind groups, uploads and overlap with pending/current/last-good/in-flight/retirement through existing Context3D/Device/Queue budget/lease owners. Old no-effect route allocates NONE of these reveal-specific resources. No increased quotas, global cache, second device/compositor, open-ended histories or hidden feature gate.

### 10.6A CPU extent handoff selection - binding S1 amendment (2026-11-05)

A current PbrScene3D has NO precomputed complete source-wide extent and NO existing generic extent worker. This explicit decision supersedes section 10.6's assumption of an already admitted CPU path without weakening no-long-UI-scan. HAL372 owns one CPU-only job interface but starts NO worker, scheduler, timer, or event loop. The existing consumer CPU executor dispatches the job.

Source: geometry.rs:245-276 stores immutable Arc vertex arrays and GeometryIndices::Unindexed/U16(Arc)/U32(Arc) from content.rs:333+, while snapshot.rs:99-118,145-149 holds live geometry leases and ModelPlacement3. On UI, Context3D::begin_pbr_object_reveal_extent(&PbrScene3D, maximum_cpu_bytes:usize) -> Result<(PbrObjectRevealExtentCpuJob3D,PbrObjectRevealExtentPending3D),PbrSourceError3D> checks exact live whole-source Arc, snapshot, model, context, packet closure and bounded custody; reserves source-retention and descriptor/result capacity BEFORE O(packet count) captures of immutable vertex/index Arcs and copied placements. It never walks vertices or indices. Pending is !Send and holds UI-only source/Context association.

The Send CPU-only job carries no Context, Device, Queue, controlled geometry lease, GPU resource, widget or semantic authority. job.verify(cancel:Arc<AtomicBool>) traverses exactly referenced triangle corners off UI, computes f64 checked source-world min/max with actual copied placements, and returns one non-Clone typed result. The existing consumer worker executes it; HAL does not spawn. UI Context3D::finish_pbr_object_reveal_extent(pending,result,&current_scene) validates exact live source Arc/revisions/Context/ticket and consumes the result, returning opaque PbrObjectRevealExtent3D without source scan.

PbrObjectRevealRequest3D additionally carries extent:Option<PbrObjectRevealExtent3D>; enabled=true requires the matching qualified proof, no-effect/disabled does not. The Viewport request stays O(1); existing request_pbr_scene unchanged. Camera changes adjust the already sealed bound to render-relative coordinates in O(1), no source rescan.

**Retained cost law:** original registered source CPU arrays remain accounted for while ANY cloned Arc exists. Precharge all additional CPU descriptors, result/capture and actual concurrent retention. An owner-issued Send-safe retention escrow shared by CPU job and pending must remain live until the worker-held last Arc and UI pending are retired, including cancellation/stale results; never move Context or GPU lease to worker and never release a source charge early. Cooperatively cancel without UI join/recv/block; typed capacity failure before capture. No unbounded allocation, duplicate accounting of physically shared Arc, hidden cache or new scheduler.

Must test actual indexed/unindexed geometry, multiple rotated placements, referenced-only extrema, no UI vertex reads, Send job/!Send pending, exact source/Context mismatch/stale/cancel/retire, retained budget until last Arc, real original PBR N1/N4 and no-effect regression. This is S1 technical contract selected through delegated independent architectural review, NOT a claim of implemented or GPU-accepted source; the same active S2 engineering task continues.

### 10.7 BLOCK-372-TARGET-FAILURE-POLICY - CLOSED

The effect request/ready/current/reject/retire stages are not HAL355 source or front state. A refusal of optional reveal never becomes a successful altered source; ordinary complete source remains eligible under its own unchanged agreement. Stale source, different Arc, changing Context/Device generation, obsolete request or incompatible target retire that effect and cannot be revived by a late result. Existing HAL355 source/target/current/compatible-last-good and incompatible-front withdrawal remain authoritative. On resize or hidden/unavailable target with SAME live source, update progress only under actual engine Timeline semantics; on next legitimate target use its current sample without replaying entry. No false ready image when unavailable, no new per-target clock, no fake last-good/current relabel. Terminal Device failure withdraws invalid fronts; cancellations and drop release actual GPU work queue-safely. Completed p=1 stops effect-driven frames and leaves ordinary source presentation.

### 10.8 BLOCK-372-CONFORMANCE - CLOSED

Separate S2 implementation must show exact source/identity/mixed packet refusal; Y/X/Z extent fallback, source transform/metric origin and f32 bound refusal; Object128 size/offset/device/arena lifetime; original no-effect and p=1 visual/color/depth parity; p=0/0.25/0.5 real planar mask; R028 implicit derivatives before discard and original color/normal roles; N1/N4, R025 original complete source caster shadows, R027 covered FP16/image, camera/origin changes and partial occlusion; 60/120 frame timing with 160ms terminal and same-source interruption; no-effect/reduced-motion, old-source retirement, resize/inactive/Device/capacity refusal, and complete native App::run input/presentation proof. Compare exact source/host reference outcomes and supported actual backend/Image observations with predeclared fixtures; no invented epsilon, universal f32 proof or native success from a screenshot alone. JC owns final visual judgment.

### 10.9 Disposition

These are **ADOPTED, ACTIVE, BINDING S1 technical selections** made through delegated independent architectural review, replacing the prior unselected eight-block register ONLY in the v1 whole-source case. The capability itself is still NOT IMPLEMENTED. JC's 2026-11-05 mandate authorizes active progression: after independent verification of this source-bound S1, a distinct exact S2 implementation authorization must include real source/GPU/native effect. Neither implementation nor a test may invent a new semantic source grouping, solver, rendering shortcut or missing detail. Source contradictions found during S2 require a decision by the delegated architectural reviewer without silently reducing the product outcome.
