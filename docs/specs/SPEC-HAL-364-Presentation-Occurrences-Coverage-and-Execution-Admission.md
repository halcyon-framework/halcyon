# SPEC-HAL-364: Presentation Occurrences, Coverage and Execution Admission

**Author:** JC\
**Status:** Proposed — Binding contract upon joint adoption\
**Date:** 2026-10-05\
**Owner:** Retained scene / renderer and resource owners\
**Paired dependency:** [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md); neither contract activates alone.\
**Existing governing dependencies:** [SPEC-HAL-002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),[SPEC-HAL-002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md),[SPEC-HAL-024](SPEC-HAL-024-Coordinate-Model.md),[SPEC-HAL-040](SPEC-HAL-040-GPU-Resource-Lifetime-Management.md),[SPEC-HAL-083](SPEC-HAL-083-Unified-SceneGraph-Diff-Engine-v2.md),[SPEC-HAL-083A](SPEC-HAL-083A-SceneCache-Dispatch-Mandatory-Wiring.md),[SPEC-HAL-084](SPEC-HAL-084-RenderGraph-v2.md),[SPEC-HAL-085A](SPEC-HAL-085A-Shader-Suite-v2-Binding.md),[SPEC-HAL-150](SPEC-HAL-150-Compositor-Authority-and-Limits.md),[SPEC-HAL-263A](SPEC-HAL-263A-Image-Alpha-Contract-Correction.md),[SPEC-HAL-263B](SPEC-HAL-263B-Image-Registration-Lifetime-Correction.md),[SPEC-HAL-266A](SPEC-HAL-266A-Per-Target-Custom-Render-Preparation.md),[SPEC-HAL-295A](SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md),[SPEC-HAL-354](SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md),[SPEC-HAL-359](SPEC-HAL-359-Cross-Pipeline-Z-Order-Execution.md),[SPEC-HAL-360](SPEC-HAL-360-Complete-Text-Realization.md),[SPEC-HAL-361](SPEC-HAL-361-App-Frame-Realization-Observation.md).\
**Authority:** Subordinate to unchanged SPEC-HAL-002 and SPEC-HAL-002A.\
**Implementation status:** Unadmitted under this pair. Existing primitives and unrelated prior proofs do not establish any new execution profile.\
**Adoption package:** SPEC-HAL-363 §14; required amendments carry the same pending paired-adoption scope.

A denotes SPEC-HAL-363; B denotes SPEC-HAL-364. Clause labels retain these
prefixes for exact cross-document references. MUST, MUST NOT and SHALL state
normative obligations upon adoption, not current implementation claims.

## Publication Pre-Flight

- Objective: publish the complete paired normative proposal and exact scoped
  amendments, then audit actual resulting files before adoption review.
- Mandatory reading: live Work Doctrine/PROJECT-ENTRY, Constitution 002/002A,
  documentation rules and every affected governing document read; exact owner
  clauses used as merge base.
- Authority change: proposed paired adoption package only; no constitutional
  change, runtime permission, status promotion of dependencies or execution proof.
- Scope: SPEC/documentation only; reviewed source fixes and native demo preserved.
- Owners reused: existing Update/presentation, Timeline, LayoutTransition,
  retained scene/diff/dispatch, renderer/resource and canonical semantic owners.
- Prohibitions: new clock/semantic owner/phase, fabricated layout or cue facts,
  stale sidecars, clip escape, unbounded history and inferred renderer admission.
- Verification: documentary clause/reference/status review, read-only structural
  validation, diff whitespace check and source/demo preservation hashes.
  No runtime formatting, tests/builds, GUI, commit, push or release.
- Completeness: full general contract with explicit per-profile admission;
  Exchange and realized Tree/member are acceptance pressure tests.
- Numbering: live corpus had 466 SPEC files and maximum HAL base ID 362;
  363/364 were free with no repository references. Adjacent new base IDs express
  paired general owners rather than lettered amendments to a narrower parent.
- Release files: none; documentary publication is not adoption or release.

## 1. Scope and relationship to existing owners

**B-1.1** This contract SHALL define the immutable source/generation/occurrence values, coverage/order semantics and execution/resource admission required by A. It SHALL extend the existing retained scene, diff/cache, renderer and resource owners.

**B-1.2** This contract SHALL be the explicit new follow-up required by frozen SPEC-HAL-083 §§14–16. It SHALL NOT modify SPEC-HAL-083 by implication, bypass SPEC-HAL-083A dispatch, create another SceneCache or allow the renderer to sample animation time.

**B-1.3** A representable composition SHALL NOT imply an executable profile. Each operation/pipeline/resource/target profile SHALL be admitted by the corresponding owners and verified under this contract.

**B-1.4** Current retained fragment reuse SHALL continue under SPEC-HAL-354. Historical generations SHALL be a separately bounded paint-value lifetime. They MUST NOT retain or replay the current fragment's semantic sidecars.

**B-1.5** A SHALL own requests, timing, group policy and semantic-lifecycle separation. B SHALL own representation/admission evidence and resource-safe publication/retirement. Neither SHALL acquire the other's semantic/time authority.

**B-1.6** Ordinary widgets, PaintContext and SceneNode values MUST NOT gain renderer-private GPU objects or handles into mutable renderer storage. Renderer-owned capture/isolation work SHALL remain under SPEC-HAL-084/SPEC-HAL-040, not SPEC-HAL-295A participant exception.

**B-1.7** No rule here SHALL authorize swapchain reads, arbitrary framebuffer capture, asset loading, a new material/shader language, input transforms or a new canonical coordinate system.

## 2. Sources and capture certificates

**B-2.1** An eligible source publication SHALL be produced by an existing owner during its lawful publication phase. It SHALL identify the source address/lifetime, publication revision, complete paint scope and exact ownership of its values.

**B-2.2** A source SHALL be either an already-realized widget/subtree paint scope or an already-realized logical member paint scope. A caller MUST NOT obtain eligibility merely from a WidgetId, member key, rectangle, scene pointer or texture ID.

**B-2.3** A source publication SHALL contain owned immutable paint descriptors or references admitted for their complete lifetime. It MUST NOT contain widget/model/layout/context borrows, callbacks, semantic sidecars or frame-local shaping/projection objects.

**B-2.4** The capture certificate SHALL enumerate:

(a) source producer/address/lifetime and generation;

(b) historical/current/incoming role evidence;

(c) the closed source cut and affected authored leaves;

(d) source-to-host coordinate provenance, surface/target association and already-applied projections;

(e) scoped clip/scroll/overlay/order topology and each dependency's owner/lifetime;

(f) capture-safety classification for current-authority cues;

(g) text/font and resource content-generation dependencies;

(h) applicable execution profile and exact retained/work accounting;

(i) publication/output evidence required for the proposed use.

**B-2.5** A certificate SHALL be derived by the source producer and engine traversal/resource owners. Product code MUST NOT forge it by declaring safety, supplying arbitrary bounds, choosing a broader root or inspecting pixels.

**B-2.6** Scope closure SHALL preserve actual paint topology and every external dependency. Capturing a root node SHALL NOT authorize dropping ancestor barriers, child scope boundaries, external order interleaving or descendant author identity.

**B-2.7** The publication SHALL state which coordinate and presentation transforms are already materialized. Replay MUST NOT apply any transform or scroll mapping twice. If the existing mapping cannot be factored/represented exactly for the proposed occurrence, admission SHALL fail.

**B-2.8** Current/incoming values SHALL preserve current canonical content/metric ownership. Historical values MAY preserve a certified previous paint presentation, but MUST NOT claim its geometry is current canonical layout.

**B-2.9** Source certification SHALL precede historical flattening. Unsafe focus/capture or other current-authority cues SHALL cause refusal unless the producer has supplied a lawful explicitly historical-safe paint variant. A raster image without trustworthy cue/coverage provenance SHALL not be certified by pixel inspection.

**B-2.10** Text sources SHALL be re-realizable from owned text and the exact owner-admitted font/style/content generations and logical paint inputs required to reproduce the presentation. A family name or default font assumption SHALL NOT substitute for missing generation provenance. Shaped runs, glyph clusters, atlas UVs and compiled glyph quads MUST NOT become historical source storage.

**B-2.11** Capture of already-realized members SHALL copy/reference owned paint values, not RowProjection or a virtualization slot. No source query SHALL force layout, offscreen realization or a missing member's rect.

**B-2.12** A producer's normal teardown/cache-eviction boundary MAY transfer an already certified immutable publication into bounded presentation custody. This custody SHALL be declared/accounted before the current reference is released. It MUST NOT preserve semantic sidecars or call the deleted producer.

**B-2.13** If readiness/certification did not exist when required, the history SHALL be unavailable. The engine MUST NOT reconstruct an alleged old presentation from a new model/configuration or fabricated geometry.

**B-2.14** Source generations SHALL remain immutable. Changed source values, safety evidence or content resources SHALL require a new generation/admission. Changed external live barriers SHALL instead be re-resolved as live dependencies; they MUST NOT mutate frozen internal coverage.

## 3. Occurrences, authored identity and retained keys

**B-3.1** Every compositional use SHALL have an opaque PaintOccurrence identity distinct from canonical identity, source address/generation, authored leaf identity, request/group identity and Timeline handle.

**B-3.2** Coexisting occurrences of the same source SHALL have distinct execution-key namespaces. Each qualified leaf key SHALL preserve the original authored leaf identity/affinity plus its occurrence qualification. Historical replay MUST NOT overwrite the canonical current cache entry.

**B-3.3** Capturing a subtree SHALL NOT reassign descendant authored leaves to the root widget. A member publication SHALL retain its existing producer/member provenance without inventing a row WidgetId.

**B-3.4** Value/progress/transform changes within a stable occurrence SHALL update retained values/dependencies, not mint new stable authored identity each frame. A source revision SHALL not become a substitute SceneNodeKey.

**B-3.5** An unkeyed/ephemeral leaf MAY be retained only within an otherwise immutable admitted generation, as SPEC-HAL-354 permits. It MAY have a generation-local occurrence address for execution, but that address SHALL NOT be advertised as persistent authored or semantic identity across source revisions.

**B-3.6** New structural occurrences or new anonymous-generation bindings SHALL be explicitly allocated/retired. Layout/progress changes alone SHALL not be disguised as remove/insert churn for stable authored leaves.

**B-3.7** Internal composition/pass nodes MAY have B-owned paint identities. Such identities SHALL carry no WidgetId, input/focus/layout/A11y identity or semantic lifetime. They SHALL not be inserted into WidgetTree or a substrate member model.

**B-3.8** Occurrence/source handles SHALL be lifetime-qualified and non-wrapping. Stale bindings SHALL fail validation before reuse. Removed entries SHALL release generation metadata; validation MUST NOT require a permanent map of all retired keys.

**B-3.9** Sharing a generation SHALL share immutable values only. Each occurrence SHALL retain its own declared host, coverage, order, projection owner and admission result. A shared reference SHALL not authorize a second current cue or competing geometric owner.

**B-3.10** A nested capture SHALL record a bounded immutable expression and generation dependency graph. It MUST NOT capture a live controller/Timeline handle or retain an unbounded chain of prior complete scenes.

**B-3.11** The presented occurrence scene SHALL enter the ordinary SceneGraph diff and mandatory SceneCache dispatch before render preparation. There SHALL be no renderer-private replay path that evades key validation, dependency invalidation or complete scene admission.

**B-3.12** Removal/replacement SHALL evict current mount/member state under its existing owner. Only an independently admitted historical generation MAY remain, and it SHALL never validate a newly mounted/recycled semantic owner.

## 4. Coverage, clipping and coordinate provenance

### C1 — Certified capture boundary

**B-C1.1** Every historical source SHALL identify a producer-certified closed paint scope and its exact ordered source topology. The certificate SHALL identify each applicable clip, scroll mapping, surface/order constraint, owner lifetime, coordinate basis and affected leaf scope.

**B-C1.2** Closure SHALL be proved from owning publications/traversal provenance. Geometric containment, common ancestry, a bounding rectangle, an Arc, a texture handle or the phrase “whole subtree” SHALL NOT prove closure.

**B-C1.3** A caller MUST NOT enlarge/relabel the capture boundary to convert a surviving host, viewport or external constraint into frozen historical decoration. A source cut SHALL not discard dependencies that cross it.

### C2 — Distinct coverage classes

**B-C2.1** InternalCoverage SHALL mean coverage/topology owned wholly within the certified historical source scope, affecting only that captured paint, whose value and applicability belong to the immutable source generation. Only such certified historical coverage MAY move with its occurrence.

**B-C2.2** LiveBarrier SHALL mean an applicable surviving host/surface constraint, ancestor constraint external to the certified cut, or protected external/cross-scope viewport/overlay constraint. Its current owner publication and original applicability scope SHALL remain authoritative.

**B-C2.3** A LiveBarrier MUST NOT be frozen into old geometry, transformed with the occurrence, derived from interpolated bounds, widened, dropped or replaced by a presentation mask. Its coordinate mapping SHALL be the current owner-authorized mapping.

**B-C2.4** PresentationMask SHALL mean an additional paint-only restriction at an explicitly declared occurrence/group scope. It SHALL derive from A's immutable parameters and B's admitted value vocabulary. It SHALL neither own nor alter canonical layout, hit testing or clipping.

**B-C2.5** Mere survival/death of a source widget SHALL not by itself classify a clip. The classification SHALL depend on the certified source role, scope and owning constraint. An internal historical copy SHALL not become current canonical authority because its former owner still exists.

**B-C2.6** Ambiguous or unrepresentable classification SHALL refuse historical admission. Classification MUST NOT be inferred from pixels or rectangle intersection.

### C3 — Historical versus current/incoming substitution

**B-C3.1** Frozen InternalCoverage SHALL be available only to an engine-verified superseded/outgoing paint generation. Its use SHALL not change the original canonical clip or layout record.

**B-C3.2** Current/incoming substitution SHALL preserve every applicable current canonical clip, including current descendant scopes, in its actual authoritative geometry and scope. Visual interpolation SHALL not supply its position or extent.

**B-C3.3** A current generation MUST NOT be relabelled Historical to move its canonical clip, bypass a required cue or obtain a different lifetime.

**B-C3.4** If a requested incoming presentation cannot be represented under current canonical constraints, it SHALL be refused/settled. A root crop, altered clip origin, reduced content or hidden materialization SHALL not be an implicit substitute.

### C4 — Scope-preserving composition

**B-C4.1** Composition SHALL preserve the actual paint-scope order: local paint, applicable child/viewport/scroll scopes, ordered children, scope closure and owning overlay paint as defined by SPEC-HAL-354 and the source owner.

**B-C4.2** Occurrence projection SHALL affect draw geometry and only the InternalCoverage explicitly certified to move. LiveBarrier geometry SHALL remain in its current canonical basis at its original applicable scope.

**B-C4.3** Presentation masks SHALL apply at their declared occurrence/group scope. Isolated group opacity SHALL apply once to the complete group result. No operator MAY be hoisted, duplicated, omitted or moved across composition unless the admitted profile proves semantic equivalence.

**B-C4.4** A clip that applies only to children MUST NOT be imposed on local/overlay paint outside that scope. A clip on one branch MUST NOT become a blanket root crop.

**B-C4.5** Source/host/viewport/DPR mappings SHALL each be applied exactly once. Unknown or multiply applied mapping SHALL refuse admission.

### C5 — Coverage algebra

**B-C5.1** Identity projection SHALL preserve source coverage, order and premultiplied compositing semantics. Equality of bounding boxes SHALL not establish equivalence.

**B-C5.2** Canonical clip coverage SHALL retain its defined scope and multiplicity relative to primitive/group composition. A profile MUST NOT commute a clip across alpha-over composition solely because its geometric set is the same.

**B-C5.3** Presentation masks and isolated opacity SHALL have their separately declared once-per-group meaning. They MUST NOT silently substitute for per-primitive canonical coverage.

**B-C5.4** Admission SHALL account for fractional/antialiased coverage. Hard-scissor equivalence SHALL not prove rounded/soft/vector/texture-mask equivalence.

**B-C5.5** If complete certified coverage requires an operation not implemented/admitted for the exact profile, admission SHALL return PendingCapability. Missing, contradictory or unrepresentable provenance/scope SHALL return RefusedCoverage. An unavailable resource for an otherwise admitted operation SHALL return UnavailableResource. No case may be accepted by flattening first and applying the wrong coverage algebra.
Non-normative counterexample: two overlapping opaque primitives individually receiving 0.5 coverage compose to alpha 0.75; flattening their opaque result before one 0.5 mask yields 0.5. The distinction is observable.

### C6 — Raster representation

**B-C6.1** Rasterization MAY be used only when the execution profile certifies which source-internal coverage is baked, which live operators remain external and how replay preserves C4/C5.

**B-C6.2** A raster source that has lost content to a surviving canonical barrier MUST NOT be treated as recoverable complete content by moving/expanding its crop. A baked live barrier MUST NOT be reapplied without proof that this preserves coverage.

**B-C6.3** Pixel analysis, alpha division, root cropping, a full-window screenshot or private framebuffer capture SHALL not establish provenance or remove a barrier.

**B-C6.4** An unrepresentable raster source SHALL be refused. Re-realizable descriptors MAY be used only when they independently satisfy the same certificate and execution laws; descriptor use is not itself proof.

**B-C6.5** Renderer-owned raster/isolation passes SHALL be explicit SPEC-HAL-084 topology with SPEC-HAL-040 lifetime and exact resource provenance. A SPEC-HAL-295A participant MUST NOT capture renderer-owned targets under its offscreen exception.

### C7 — Scroll and viewport

**B-C7.1** A source within a surviving viewport SHALL retain the live viewport/scroll/anchor owner's applicable constraints. A presentation recipe MUST NOT mutate scroll state or override anchor compensation.

**B-C7.2** The source SHALL identify whether geometry/coverage is pre-scroll, already scroll-mapped or otherwise projected. Replay SHALL apply the current admitted mapping exactly once; an old baked offset plus a new offset MUST NOT be guessed into correctness.

**B-C7.3** A historical capture of an entire scroll-owned content scope MAY freeze only the internal coverage/offset explicitly certified as belonging to that source generation. It MUST NOT freeze an outer surviving viewport or another protected live barrier.

**B-C7.4** Such a source SHALL require its own admitted ownership/profile under SPEC-HAL-078A/B amendments. General vocabulary SHALL not waive ordinary LayoutTransition's scroll conflict rule.

**B-C7.5** Missing coordinate factoring, conflicting motion ownership or invalid current anchoring SHALL cause settlement/refusal.

**B-C7.6** Virtualized capture SHALL use already-realized publications only. Recycling SHALL not transfer an old source identity or certificate to the new occupant.

### C8 — Host transfer, reparent and overlays

**B-C8.1** Every occurrence SHALL name an admitted host/surface and source-to-host mapping. A recipe name SHALL not grant permission to change that host or cross a semantic boundary.

**B-C8.2** A host transfer SHALL identify the existing authority that legitimately transfers/disposes every affected live dependency and SHALL certify the resulting current barrier/order chain. Without that authority and evidence, transfer SHALL be refused.

**B-C8.3** Surviving source barriers MUST NOT be dropped merely because a new destination exists. Old/new host origins or canonical rects MUST NOT be guessed from visual overlap.

**B-C8.4** Local overlay paint and semantic overlay surfaces SHALL remain distinct. Capture SHALL preserve each actual scope and order. Menu/modal/popover lifetime SHALL not become a paint-only lifetime by renaming it.

**B-C8.5** Cross-surface composition SHALL require explicit surface, target and resource execution admission. It SHALL not read a swapchain, share private surface resources implicitly or invent an overlay semantic owner.

### C9 — Nested presentation

**B-C9.1** Capture of an existing presentation SHALL freeze an immutable admitted expression with fixed parameters and exact generation dependencies. It MUST NOT retain a live nested controller, Timeline handle or callback.

**B-C9.2** Every surviving live barrier, current-cue obligation and resource/content lease SHALL remain explicit in the dependency closure. Flattening MUST NOT erase it.

**B-C9.3** Nested generation references SHALL form a bounded acyclic graph. The effective geometric ownership plan SHALL remain unambiguous over every affected leaf.

**B-C9.4** Admission SHALL include the full reachable storage/work/resource cost and inherited retention origin. Repeated interruption MUST NOT hide unbounded history behind a single root reference.

**B-C9.5** Nesting that cannot preserve scope, algebra, current truth or ownership SHALL be refused/settled.

### C10 — Live invalidation and refusal

**B-C10.1** Frozen InternalCoverage SHALL not mutate when the current source changes. Changed source paint SHALL produce a new certified generation.

**B-C10.2** A changed LiveBarrier SHALL be resolved from its current owner before publication. The affected candidate SHALL be revalidated without substituting interpolated geometry.

**B-C10.3** Owner/host removal, incompatible coordinate lineage, invalid order closure, unsafe current cue, unavailable content generation or unrepresentable coverage SHALL invalidate admission. A SHALL apply complete settlement/cancellation; B SHALL retain only safety-required resource references.

**B-C10.4** No fallback SHALL replace an authoritative clip with visual/interpolated bounds. Visual positioning does not supply authoritative clipping, including for a current Label whose content width changes.

**B-C10.5** Refusal SHALL identify the failed provenance/scope/profile condition. It MUST NOT be disguised as successful partial rendering or an inferred root crop.

## 5. Projection vocabulary and mathematical meaning

**B-5.1** A composition SHALL be a finite immutable value expression over admitted occurrences and the operators defined here. It MUST NOT contain executable painters, arbitrary shaders, callbacks, a parser/DSL, semantic actions or a live animation evaluator.

**B-5.2** The general vocabulary SHALL include identity, rigid translation, finite two-dimensional affine projection, presentation masks, isolated group opacity and scoped occurrence ordering. Availability of a vocabulary term SHALL not imply renderer execution admission.

**B-5.3** Every affine value SHALL specify its source/host coordinate basis, pivot convention, matrix composition order and complete finite coefficients. The implementation SHALL reject non-finite coefficients or non-finite/out-of-profile derived geometry before execution.

**B-5.4** Scale, rotation, shear and reflection SHALL be representable in the general affine contract. Each SHALL require explicit operation/pipeline/coverage admission. Perspective, 3D projection or a new canonical coordinate system SHALL not be inferred.

**B-5.5** Degenerate/rank-reducing affine values SHALL be admitted only when the profile defines their finite coverage and required computations. If an operation requires an inverse and no valid inverse exists, admission SHALL fail. No inverse transform SHALL ever be used to move canonical input with visuals.

**B-5.6** Rigid translation SHALL preserve the complete source's logical shape, intrinsic content metrics and relative surface/border/content geometry. An Exchange rigid profile MUST NOT realize translation by stretching a texture, changing glyph metrics or interpolating content dimensions.

**B-5.7** A mask SHALL be a typed finite coverage descriptor with an explicit scope/basis and an owner-admitted coverage source. Coverage families MAY include bounded analytic/vector primitives or immutable alpha-resource coverage only when their exact owner/profile admits them. A new coverage kind SHALL declare its semantics and execution profile; an arbitrary string or shader pointer SHALL not define it.

**B-5.8** A mask SHALL only restrict the composition at its declared scope. It MUST NOT enlarge/drop a LiveBarrier, rewrite current canonical clips or change hit/layout geometry.

**B-5.9** Mask combinations SHALL be finite and explicitly ordered/scoped. Their coverage rule SHALL be declared by the profile; rectangle set intersection SHALL not be assumed to model fractional alpha composition.

**B-5.10** Group opacity SHALL be a finite scalar in [0, 1] applied once to the fully composed isolated premultiplied group result. Applying opacity independently to overlapping children SHALL not implement group opacity.

**B-5.11** For an isolated premultiplied group value G, admitted presentation coverage m and group opacity o, the additional modulation SHALL be (G.rgb × m × o, G.a × m × o). This equation SHALL not relocate or duplicate canonical coverage already applied at its C4/C5 scope. Mask resource sampling SHALL name its coverage channel/association; scene tint rules SHALL remain SPEC-HAL-263A-owned.

**B-5.12** Scoped ordering SHALL name a complete finite occurrence order within an admitted closed scope. Internal authored source order and external SPEC-HAL-359 order SHALL remain preserved. Missing/duplicate references, unowned external leaves or ambiguous order closure SHALL refuse admission.

**B-5.13** Each executable profile SHALL declare its numerical/coverage precision and conservative bounds under existing coordinate/pipeline owners. Approximate transformed bounding boxes MUST NOT silently replace required rotated/rounded/soft coverage or permit escape from a live barrier.

**B-5.14** Empty visual coverage SHALL remain distinct from missing/incomplete content. A fully occluded but complete admitted source MAY remain a group participant; its preparation/resource obligations SHALL remain those of the declared profile. Visibility shall not be used to manufacture successful realization of an unavailable required source.

**B-5.15** Source content extent and visible composition coverage SHALL remain distinct. A mask/clip change MUST NOT resize a custom-render resource or change its UV interpretation solely to fit the visible crop.

**B-5.16** Future admitted material/effect output MAY be composed as a certified source. This vocabulary SHALL not define the effect's shader, material semantics or resources.

## 6. Execution admission, order and isolation

**B-6.1** An execution profile SHALL bind the exact source/primitive/pipeline families, operations, coverage algebra, color/alpha rules, resource publication path, target/surface/metrics scope, limits and failure semantics it supports.

**B-6.2** Profiles SHALL be declared by existing renderer/producer owners. This contract MUST NOT introduce a global discovery registry, dynamic semantic service or user-supplied executable profile.

**B-6.3** Admission SHALL validate the entire required participant/operation set. Support for translated solid rectangles SHALL NOT imply support for rotated text, mesh, Image, custom rendering, soft masks or group isolation.

**B-6.4** A profile SHALL preserve C1–C10 and A's current-cue/ownership laws. A pipeline's ability to draw pixels SHALL not override source or semantic constraints.

**B-6.5** Unsupported capability SHALL produce PendingCapability; invalid provenance/ownership/coverage SHALL produce the corresponding refusal; unavailable admitted resources SHALL produce UnavailableResource. No category SHALL silently degrade to another operator, material or renderer path.

**B-6.6** A complete group SHALL be admitted atomically. Every required participant, text source, resource generation, scope and ordering relation SHALL be valid before the group can become an executable result.

**B-6.7** An isolated group SHALL occupy a certified closed external order scope. Its internal leaves MUST NOT be interleaved with external leaves in a way that flattening/reordering changes the image. Equal Z, shared parent, equal clip hashes or contiguous allocation SHALL not prove closure.

**B-6.8** Inside a group, authored source order and explicit occurrence order SHALL govern. Outside it, SPEC-HAL-359's strict per-surface cross-pipeline order SHALL govern. Text backgrounds/glyphs SHALL retain their required contiguous treatment.

**B-6.9** No recipe SHALL reorder across a semantic surface/modal boundary, select another surface, invent a global Z policy or reinterpret equal-Z ties. The compositor SHALL execute the resolved order, not choose choreography.

**B-6.10** RenderGraph Build SHALL declare complete composition/isolation topology, dependencies and logical resources after mandatory SceneCache dispatch. Renderer-owned offscreen/copy/mask passes SHALL be explicit graph nodes.

**B-6.11** Compile SHALL resolve resources, pipeline bindings and complete executable validity without changing frozen topology/descriptors. Execute SHALL encode the frozen admitted plan without new allocation, scene reads, source realization or policy decisions.

**B-6.12** A changed pipeline, resource binding, target metric, surface format, device lifetime or live coverage dependency SHALL invalidate exactly the admission/prepared value that depends on it. A descriptor MAY be re-realized when valid; an obsolete compiled plan SHALL not be treated as immutable reusable history.

**B-6.13** A profile that requires a new shader ABI, bind-group layout, coordinate convention or public resource-admission boundary SHALL include the exact owning amendment before execution admission. B's general vocabulary SHALL not waive SPEC-HAL-085A, SPEC-HAL-263A or another closed boundary.

**B-6.14** Cross-frame physical resources SHALL have explicit SPEC-HAL-040 lifetime/pool ownership and B's leases. Frame-scoped attachments MUST NOT be cached across frames by convention. Swapchains SHALL remain sinks and MUST NOT become sources.

**B-6.15** Admission and rendering SHALL remain deterministic from the same immutable source/profile/target inputs. Resource allocation order, hash iteration, wall time and widget type-specific heuristics MUST NOT choose occurrence order or fallback.

**B-6.16** Operation profiles SHALL distinguish descriptor/CPU proof, pipeline execution proof and native surface proof. A headless complete plan SHALL not claim output/submission/present-call or physical display completion.

## 7. Resource/content-generation leases and text

**B-7.1** A historical resource reference SHALL identify both an exact resource object/binding lifetime and an immutable content generation. Object liveness, a numeric texture ID or an Arc/view reference alone SHALL not prove content immutability.

**B-7.2** Before admission, the resource owner SHALL grant a bounded read lease protecting the exact sampled content, sampling policy, alpha/color association, full content extent and target/device validity required by the occurrence.

**B-7.3** While a generation is leased for possible sampling, its producer MUST NOT overwrite, recycle or repurpose that content. A current-front replacement or unregistration SHALL not itself release this obligation.

**B-7.4** Current Image registration and historical binding SHALL remain distinct. Unregistering a current public ID SHALL retain its existing meaning for ordinary lookup. A separately admitted historical generation MAY remain privately bindable through its lease, but MUST NOT make ordinary lookup of the unregistered ID succeed or silently bind it to another generation.

**B-7.5** A historical occurrence SHALL resolve its exact leased generation, never whichever current front happens to share an ID/address. Reuse of the producer's former front as a writable back buffer SHALL be denied while the historical generation can still be sampled.

**B-7.6** A producer unable to grant immutability SHALL not be captured by retaining its handle. An independent snapshot MAY be used only through an explicitly admitted renderer/resource-owner operation with complete source provenance and SPEC-HAL-084 topology. The source SHALL remain read-protected until that operation's consumption is safe; the resulting independent generation SHALL acquire its own lease.

**B-7.7** Queue ordering/publication SHALL follow SPEC-HAL-295A and the owning resource contract. A generation MUST NOT be published before its production is correctly ordered ahead of sampling. No hidden encoder, producer callback into DrawPlan or renderer-private target access SHALL implement capture.

**B-7.8** Lease acquisition SHALL be atomic for the required group. Failure SHALL release partial candidate acquisitions without publishing partial content. Shared leases SHALL retain exact reference/accounting ownership.

**B-7.9** Logical retirement SHALL release future-use rights. Physical resources SHALL remain owned/accounted until all submitted consumers are safe to retire. Unregister-before-final-drop, device lifetime and existing pool rules SHALL remain explicit.

**B-7.10** Same-device pipeline/bind-group revalidation SHALL preserve the admitted content identity and sampling policy. Device loss SHALL invalidate device-bound admission separately. This contract SHALL not claim an implemented device-recovery path or introduce a public device epoch by inference.

**B-7.11** Target metrics SHALL be the existing immutable per-target snapshot. Full resource extent/UV content and visible composition coverage SHALL remain separate. Clipping, masks or partial visibility MUST NOT shrink a producer's render extent as a substitute for composition.

**B-7.12** A historical target-bound binding SHALL be a separately leased generation, not an additional mutable current-front entry in SPEC-HAL-266A's current presentation table. Current publication SHALL still have its existing exact target isolation.

**B-7.13** Text SHALL be re-realized from owned text and exact font/style/source generations plus the lawful logical paint inputs needed to reproduce it. Missing old font/content provenance SHALL fail admission; substitution of a newer/default font SHALL not be silent fallback.

**B-7.14** Shaping results, glyph clusters, glyph-run borrows, atlas UV quads and compiled text batches SHALL remain frame/Compile-local. An immutable historical text descriptor SHALL not confer persistent validity on those execution products.

**B-7.15** Text backgrounds, caret and selection/highlight paint SHALL satisfy the same capture-safety classification as other current-authority cues. They MUST NOT be retained merely because they are attached to a TextNode.

**B-7.16** Every required text realization SHALL obey SPEC-HAL-360: complete requested text or the owning whole-surface Unavailable result. Ghost text MUST NOT be dropped to preserve the rest of a required composition.

**B-7.17** Actual resource/shader/pipeline support SHALL remain independently admitted. Image, custom content or font lease representation SHALL not claim that loading, copying, recovery or a particular transformed pipeline is implemented.

## 8. Retained publication and dependencies

**B-8.1** The canonical current retained cache and historical generation holdings SHALL be distinct lifetime classes under the existing retained owner. Current mount/member eviction SHALL not be delayed; historical survival SHALL require an independently admitted value/lease.

**B-8.2** A historical record SHALL retain paint values and required provenance only. Reusing a current SPEC-HAL-354 fragment SHALL continue replaying its current semantic sidecars; replaying a historical generation SHALL replay none.

**B-8.3** Publication SHALL preserve SPEC-HAL-354's local/child/viewport/scroll/overlay order and C1–C10. A shared Arc to a larger object SHALL not authorize retaining forbidden sidecars or ignoring its full reachable memory cost.

**B-8.4** Dependency records SHALL identify actual source/configuration revisions, cue-safety evidence, live host/barrier lineage, surface/target metrics, font/resource content generations, operation profile and occurrence projection/order.

**B-8.5** A changed dependency SHALL invalidate exactly the source/candidate/prepared value that consumes it and the necessary composed ancestors. Unchanged siblings SHALL retain their fragments. Projection-only changes SHALL not dirty canonical layout.

**B-8.6** Historical source values SHALL not be patched when current facts/configuration changes. Admission SHALL either keep the independently safe generation, derive a new certified generation or settle/refuse; it MUST NOT mutate old paint into current truth.

**B-8.7** Current member recycling, mount replacement, host removal and incompatible source reparent SHALL invalidate their current bindings. Old generations SHALL not validate the new semantic lifetime.

**B-8.8** Scene materialization SHALL use one selected complete presentation scene per admitted surface sequence, followed by diff and mandatory dispatch. Capturing/admitting a source SHALL not introduce a second widget paint, scene build or renderer-private replay.

**B-8.9** Prepared plans SHALL have exact attempt/target/pipeline/resource validity. A subsequent Compile or invalidating owner change SHALL supersede them as the existing owner specifies. Keeping the plan object alive SHALL not make it reusable.

**B-8.10** Source-readiness custody and candidate capture SHALL be bounded and explicit. Clean unrelated widgets SHALL not acquire permanent history, a second current-state mirror or speculative offscreen preparation.

**B-8.11** A presentationally unchanged retained scene MAY reuse valid source/execution association only when all relevant values and dependencies are equal under the admitted profile. Equal IDs, rectangles or scene pointers alone SHALL not prove equivalence.

## 9. Failure, execution observations and retirement

**B-9.1** The implementation SHALL distinguish at least: scene candidate admitted; preparation not attempted/skipped with reason; complete executable preparation; unavailable preparation with exact reason; output produced; submitted; present-called; and resource-retirement safety.

**B-9.2** Those observations SHALL be bound to the exact group/request/publication and surface/target. Canonical App attempts SHALL use the actual AppFrameAttemptId when the App owner supplies it. Noncanonical/headless execution MUST NOT fabricate that identity or native output flags.

**B-9.3** Internal group outcomes SHALL be bounded immutable owner publications sufficient for A's next-Update reconciliation. They MUST NOT depend on an application enabling or observing SPEC-HAL-361's opt-in/latest-only public observer.

**B-9.4** Outcome construction/publication SHALL not invoke widgets, re-enter layout/paint, dispatch semantic actions or schedule an independent frame loop. The public SPEC-HAL-361 receipt SHALL retain its existing exact meanings and bounded latest-only semantics.

**B-9.5** An admission/preparation result SHALL not prove output. Submitted SHALL mean a real queue submission for that output; present-called SHALL mean the existing present call occurred. Neither SHALL prove GPU or physical display completion.

**B-9.6** An execution-backed continuation source SHALL match its exact source/publication, group/request where applicable, target/surface and owning execution attempt. For a native presented surface the association requires actual output production, successful submission and the owning present call, with no known failure invalidating that result. For an explicitly admitted non-presenting output target it requires the exact complete output and successful submission required by that target's owner; absence of a native present call is then stated explicitly. A CPU-only prepared plan is not such an association. An explicitly verified retained-equivalent publication MAY carry forward the earlier association only while all relevant values/dependencies remain equal. Skip, failure, matching IDs or matching rects alone do not establish equivalence. None of these observations proves GPU completion, physical display or that a clipped leaf was seen.

**B-9.7** Fully clipped/empty contributions MAY be part of an otherwise valid output association, but SHALL not be described as pixels known to have been seen. Missing required realization SHALL not be inferred successful from invisibility.

**B-9.8** Before the complete presented scene is selected for SceneGraph diff, an inadmissible group SHALL select complete ordinary canonical settlement from lawful owner-produced values in that same scene construction. No widget is repainted for fallback. If canonical content is unavailable, its owning failure remains. Selecting the scene for diff is the final scene-choice cutoff.

**B-9.9** After that scene-choice cutoff—including during diff, dispatch, RenderGraph Build and Compile—a required group failure known before Execute SHALL make the affected complete surface plan unavailable before partial visible output. The engine MUST NOT select/splice an alternative scene, repeat widget paint, rerun the sequence, remove a required occurrence or execute surviving group fragments. Build/Compile may report unavailability through their existing owners; no frozen topology/descriptors may be changed.

**B-9.10** The base contract SHALL not provide a late canonical alternative branch. Such branching requires an explicit separately admitted execution contract with predeclared complete alternatives; it MUST NOT be inferred from a generic fallback field.

**B-9.11** Within a required group, missing Image binding/generation SHALL produce complete group/surface unavailability in every build mode before Execute. Release-time removal of only that Image batch SHALL not satisfy admission. Ordinary non-group SPEC-HAL-263B behavior SHALL remain unchanged.

**B-9.12** Composition failure and SurfaceTextRealization SHALL remain distinct observations. A surface MAY have Complete text preparation and still have no output because composition/resources failed. The implementation MUST NOT falsify a text result to report an unrelated failure.

**B-9.13** Atomicity SHALL name its engine publication/execution scope, not physical simultaneous display. Existing independent surface output remains independent. A profile spanning surfaces SHALL be unadmitted unless its surface owner explicitly supplies the required all-participant preparation/failure protocol; a recipe name SHALL not create that authority.

**B-9.14** At the first known failed output, A SHALL receive an honest outcome and settle/cancel at its next legal boundary. The failed attempt MUST NOT later begin at advanced progress. Unchanged successful reuse SHALL be distinguished from skip/failure by B-8.11.

**B-9.15** Retiring occurrences SHALL stop future source use and release logical leases exactly once. Physical release SHALL wait for resource-owner safety; already submitted work SHALL remain accounted. Late/stale outcomes SHALL not resurrect retired groups or new mounts.

**B-9.16** Resource retirement records SHALL contain only bounded identity/accounting/safety data needed by their owner. They MUST NOT retain semantic sidecars, a live Timeline interval, complete obsolete scene history or unbounded receipt tombstones.

**B-9.17** Window/device teardown SHALL invalidate further publication and release control/custody through existing owners. Cleanup SHALL not depend on callbacks from deleted widgets or an Update that can no longer run.

**B-9.18** B's complete-group rule requires validation of all known source/pipeline/resource conditions before executable surface output. A failure first learned after command encoding, submission or the present call SHALL be recorded with the exact operations that actually occurred. The engine SHALL stop further invalid use at its existing lawful owner boundary, reconcile/settle the group and retire holdings safely. It SHALL NOT claim rollback of already submitted GPU work, falsify a submission/present flag, retroactively report physical atomicity or add an independent recovery/polling loop. Device loss and resource-safety handling remain with SPEC-HAL-040/SPEC-HAL-295A and their explicitly admitted contracts.

## 10. Bounds, inspection and execution-policy evidence

**B-10.1** Every execution profile SHALL publish the finite accounting policy required by A §11. Its admission calculation SHALL include all captured payload allocations, immutable expression nodes, occurrence and dependency records, owned text/font references, content-generation leases, intermediate targets, preparation scratch, pending outcomes and resources awaiting safe retirement.

**B-10.2** A shared allocation SHALL have one accountable physical owner and an explicit bounded set of leases. Sharing SHALL not erase the cost of retaining the allocation, its dependency closure or the per-occurrence execution work. A cropped view SHALL be charged for the allocation it keeps alive.

**B-10.3** Peak admission SHALL include the old active publication, the complete candidate, rollback holdings and already submitted work. A candidate SHALL not acquire an unbounded resource set and only then ask whether the budget permits it.

**B-10.4** A profile SHALL state finite bounds for source count, leaves, occurrences, expression depth, dependency edges, mask complexity, isolation targets, retained bytes, per-frame admission/realization work and historical age. Each bound SHALL identify its accounting owner and complete refusal/settlement behavior. An absent bound SHALL make the profile unadmitted; numerical values SHALL be justified by measurements rather than invented by this contract.

**B-10.5** One admission SHALL reserve sufficient budget for its declared bounded work or fail completely. Incremental preparation MAY be an implementation technique only within an already admitted complete publication protocol; it MUST NOT expose partially realized content, delay the request's first activation or turn PendingCapability into a waiting animation.

**B-10.6** Exhaustion while admitting a candidate SHALL return the corresponding complete refusal and release partial acquisitions. Loss of validity or budget during activity SHALL follow A's settlement and B §9's publication/execution cut. The implementation MUST NOT keep a stale presentation, remove an arbitrary leaf, reduce mask quality or substitute another material without an explicitly admitted complete alternative.

**B-10.7** Logical retirement SHALL remove future-use references at the legal owner boundary. Resources still needed by submitted GPU work SHALL remain charged until safe reclamation. A stalled device SHALL cause bounded refusal/backpressure through its owner, not unbounded accumulation, premature reuse or a claim that wall-clock retirement is guaranteed.

**B-10.8** Repeated interruption, reversal, nesting and aliasing SHALL preserve the historical lineage age and full retention cost. Rewrapping an old generation or flattening its descriptors SHALL not renew its lifetime.

**B-10.9** Inactive capability state SHALL have no per-frame capture, history traversal or provenance allocation. Active work SHALL be bounded by the admitted participant/dependency closure and changed values; unrelated clean subtrees SHALL retain their ordinary fragment reuse.

**B-10.10** On-demand inspection SHALL identify the selected profile and exact operation/pipeline/resource/target admission; canonical source and mount/member lifetime; capture/publication and content generations; occurrence and authored-leaf identity; source and destination bases; captured internal coverage; every live barrier and presentation mask; scoped order; cue-safety certification; dependency validity; charged holdings; execution observations; and outstanding retirement obligations.

**B-10.11** Inspection SHALL distinguish source certification, scene admission, Compile success, output production, submission, present call and resource reclamation. It SHALL not convert a retained object, successful CPU preparation or receipt delivery into proof that pixels were displayed.

**B-10.12** Inspection SHALL be read-only and demand-driven. It MUST NOT force offscreen realization, allocate a new snapshot, sample/mutate Timeline, infer authority from pixels, expose renderer-private objects or retain unbounded historical receipts. Unavailable/evicted evidence SHALL be reported as such.

**B-10.13** Private implementation diagnostics MAY use bounded counters and reason codes. Such diagnostics SHALL not become a second semantic registry, event reducer, clock or source of focus/capture/layout truth.

**B-10.14** Performance evidence SHALL separately report steady-state idle cost, normal active-frame cost, first admission, interruption peak, text/resource preparation, retirement lag and budget exhaustion. Measurements SHALL state their source/pipeline/target profile; a cheap solid-only profile SHALL not establish the cost of general affine, isolated opacity or resource-backed composition.

## 11. Acceptance, compatibility and non-goals

**B-11.1** Adoption SHALL require a clause-level review of A, B and the amendment package, including the exact exceptions to older endpoint, clipping, cache-lifetime and failure laws. Runtime execution SHALL remain unadmitted for each profile until its implementation proofs pass.

**B-11.2** Identity/retained proofs SHALL show that one canonical widget or member can produce multiple paint occurrences without duplicate semantic identity, duplicate sidecar replay, key collision or accidental remove/insert churn for a value-only projection update. Removal/recycling SHALL invalidate current bindings while retaining only independently certified historical values.

**B-11.3** Coverage proofs SHALL exercise the C1–C10 distinctions: internal historical clips, live external barriers, current/incoming canonical clips, presentation masks, nested clip topology, scroll bases, child overlays and an ancestor surviving source deletion. A static root crop or a clip replaced with an interpolated rect SHALL fail these proofs.

**B-11.4** Coverage algebra proofs SHALL include overlapping translucent/antialiased children so that per-leaf canonical clipping cannot be mistaken for a post-composition mask. Identity projection SHALL preserve the authoritative painter's clip/order/alpha result. Any raster profile SHALL prove the exact baked/live coverage split and required source support rather than compare only bounding boxes.

**B-11.5** Projection admission proofs SHALL establish finite numeric behavior, coordinate-basis and matrix order, text/content rigidity for rigid translation, isolated group opacity where admitted, mask scope and closed occurrence ordering. Unadmitted affine/pipeline/target combinations SHALL remain explicit PendingCapability, not silently become translation or an axis-aligned crop.

**B-11.6** Resource proofs SHALL distinguish resource-object lifetime from immutable content lifetime. They SHALL cover current-front replacement/unregistration while history is retained, prevention of producer writes/recycling into leased content, exact generation selection, sampling-policy preservation, queue ordering, partial acquisition rollback and delayed GPU retirement.

**B-11.7** Text proofs SHALL retain owned text/style/font identity and re-realize through the current text owner after atlas/prepared-plan invalidation. They SHALL prove complete text-or-surface-unavailable behavior, no persistent shaped-run/UV assumptions, no font substitution and no background-only or suffix-only success.

**B-11.8** Phase/failure proofs SHALL demonstrate one ordinary scene→diff→dispatch→prepare path; no private replay, second widget paint, Timeline mutation/sampling after its legal boundary, post-freeze topology mutation or partial required-group execution. A missing Image in a required group SHALL fail completely in every build mode without falsifying SurfaceTextRealization.

**B-11.9** Observation proofs SHALL separate scene admission, execution-backed continuation, skipped/unchanged reuse, known failure, output/submission/present-called and physical retirement. Public observer disablement SHALL not disable internal lifecycle correctness. Late outcomes SHALL not revive a retired request or a recycled source.

**B-11.10** Dependency proofs SHALL cover focus/capture/configuration changes, parent clip/order changes, resize/DPR changes, scrolling, virtualization, reparenting, window loss and nested composition where those paths are admitted. A pure presentation change SHALL not invalidate layout; unchanged siblings SHALL retain fragments. Unsupported source classes SHALL not be realized merely to test rejection.

**B-11.11** Lifetime stress SHALL exercise repeated toggles, reversal and replacement before retirement, source/parent deletion, resource failure and policy disable. It SHALL demonstrate finite peak holdings, no age reset by aliasing, no retained semantic sidecars and eventual safe reclamation when the resource owner supplies completion. A stalled-GPU case SHALL prove bounded refusal rather than impossible immediate reclamation.

**B-11.12** Exchange SHALL be the first native acceptance recipe over admitted rigid translation and presentation masks. Its complete incoming surface/border/content and counter-moving surroundings SHALL share one progress source, while canonical routing remains current. Tree/member capture SHALL be an independent generality proof over already realized paint values without new WidgetIds or offscreen geometry.

**B-11.13** CPU invariants, renderer execution tests, resource-lifetime tests and native perceptual acceptance SHALL be recorded separately. A specification adoption, successful compile or synthetic scene test SHALL not establish native visual acceptance or renderer support for another profile.

**B-11.14** This binding SHALL not own input/focus algorithms, canonical layout, semantic tree/member lifetime, product recipes, asset loading, NinePatch, typography policy, CSS/DSL/cascade, material/effect definitions, arbitrary GPU programs, device recovery, a second scene cache or another animation clock.

**B-11.15** Existing renderer/material owners MAY supply future admitted output. Hosting that output SHALL preserve their coverage, alpha, ordering, resource and failure contracts; B SHALL not infer blur/glow/gradient/shader admission or extend public GPU access.

**B-11.16** Public API exposure and implementation verticals MAY be staged. The general source, lifetime, coverage and outcome laws SHALL remain complete; implementation staging SHALL be represented by explicit admission, not by an intentionally incomplete identity or lifetime model.

## 12. Explicit frozen SceneGraph follow-up and owner compatibility

**B-12.1** This binding is the new follow-up required by SPEC-HAL-083 §§15–16. It extends paint occurrence identity and bounded historical presentation without modifying SPEC-HAL-083's frozen text. Ordinary scene keys and cache lifecycle remain SPEC-HAL-083-owned. A repeated source occurrence receives a distinct engine occurrence namespace qualified with its authored leaf identity. Canonical WidgetId/member identity, source generation, occurrence identity and SceneCache handle generation SHALL not substitute for one another.

**B-12.2** Projection/progress updates SHALL retain an occurrence's stable key when its authored leaf identity is stable; generation/revision changes SHALL not force key churn by convention. Generation-local anonymous leaves have no cross-generation identity guarantee. Removed occurrence entries SHALL release their live-entry metadata; monotonic non-wrapping allocation SHALL not become a tombstone history.

**B-12.3** For A/B-presented scenes, SPEC-HAL-354's immutable presented snapshot is the input to the normal diff→mandatory dispatch→render-preparation chain. SPEC-HAL-083 §9's target-only/no-animation-rediff/renderer-clock language SHALL NOT govern this path. Presentation value changes are diffed under existing value-update rules; the renderer receives resolved values and never runs a second animation clock.

**B-12.4** B does not waive SPEC-HAL-083A dispatch, cache Build/Dispatch/Frozen mutation rules, typed generation validity or the separation of SceneCache from GPU/resource ownership.

**B-12.5** This follow-up leaves SPEC-HAL-083's frozen source unchanged. SPEC-HAL-083A remains binding: the scene-choice cutoff precedes diff, mandatory dispatch and renderer preparation. No authority index is created.

**B-12.6** Under SPEC-HAL-084 INV-RG-016, this contract is the explicit extension for occurrence composition and named retained resource leases. INV-RG-001/002/003/004/009 remain unchanged: no semantic or geometry inference, no SceneCache reads beyond Build, no new phase, deterministic topology and swapchains as sinks. All renderer-owned composition, isolation, mask and copy work SHALL be declared graph topology; the participant-owned exception in SPEC-HAL-295A SHALL NOT be used for renderer capture.

**B-12.7** SPEC-HAL-263A's straight-linear tint, premultiplied-linear sampled content and closed public content-admission boundaries remain unchanged. An internal historical lease derived from a previously admitted exact generation is not a new public upload/registration operation and SHALL NOT normalize or rewrite content. Additional admitted coverage and opacity follow §4 and §5 exactly once. A profile needing a new public boundary or shader ABI MUST satisfy B-6.13 first.

**B-12.8** SPEC-HAL-164/SPEC-HAL-167 remain Implemented / Closed in their semantic and frame-local projection scope. A certified owned paint closure is a new output under this pair, not an extension of RowProjection, layout-borrow or row-model lifetime. Column geometry, structural domains, scrolling, realization and member semantics remain with their existing owners.
