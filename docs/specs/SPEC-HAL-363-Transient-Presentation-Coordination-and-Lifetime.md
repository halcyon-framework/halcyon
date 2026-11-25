# SPEC-HAL-363: Transient Presentation Coordination and Lifetime

**Author:** JC\
**Status:** Proposed — Binding contract upon joint adoption\
**Date:** 2026-10-05\
**Owner:** Core / existing Update and presentation coordination\
**Paired dependency:** [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md); neither contract activates alone.\
**Existing governing dependencies:** [SPEC-HAL-002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),[SPEC-HAL-002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md),[SPEC-HAL-012](SPEC-HAL-012-Theming.md),[SPEC-HAL-021](SPEC-HAL-021-Animations.md),[SPEC-HAL-042](SPEC-HAL-042-Frame-Loop-and-Engine-Ticker.md),[SPEC-HAL-078](SPEC-HAL-078-Layout-Transitions-v1.md),[SPEC-HAL-078A](SPEC-HAL-078A-Layout-Transition-Authorization-and-Policy-Gate.md),[SPEC-HAL-078B](SPEC-HAL-078B-Layout-Transition-Authorization-and-Conflict-Law.md),[SPEC-HAL-142](SPEC-HAL-142-Visual-vs-Interaction-State-Separation.md),[SPEC-HAL-142A](SPEC-HAL-142A-Visual-vs-Interaction-State-Enforcement.md),[SPEC-HAL-145](SPEC-HAL-145-Layout-Transitions-and-Reflow-Authority.md),[SPEC-HAL-146](SPEC-HAL-146-Temporal-Ownership-of-Geometry.md),[SPEC-HAL-147](SPEC-HAL-147-Animation-as-Authorized-State-Interpolation.md),[SPEC-HAL-328](SPEC-HAL-328-Keyboard-Authority-Focus-and-Capture.md),[SPEC-HAL-354](SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md).\
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

## 1. Scope, effect and governing authority

**A-1.1** This contract SHALL govern the engine-owned coordination and lifetime of admitted transient presentation groups. It SHALL NOT own application meaning, canonical widget/member state, layout, input, focus, capture, selection, scroll policy, resource production or GPU execution.

**A-1.2** The mechanism SHALL support finite compositions over current and certified historical presentations. Enter, Exit, Replacement, SharedMove, Reparent and Exchange SHALL be recipe classifications over that mechanism, not separate runtime subsystems or independent authority grants.

**A-1.3** This contract and companion B SHALL become effective only through an adoption record that also activates their required scoped amendments. Until then they remain proposals. Adoption SHALL NOT assert that an operation/pipeline/resource/target execution profile is implemented or verified.

**A-1.4** Constitution SPEC-HAL-002/SPEC-HAL-002A SHALL remain unchanged and superior to every clause here. The implementation SHALL extend existing Update/presentation, retained and renderer/resource owners. It MUST NOT add a global semantic manager, independent scene pipeline, animation manager, clock, scheduler or engine phase.

**A-1.5** Outside an explicit scoped amendment, existing accepted authority SHALL remain unchanged. Neither an experimental consumer nor a successful visual demonstration SHALL create an exception.

## 2. Normative terminology and identity

**A-2.1** Canonical identity means the existing identity and lifetime of a widget, substrate or logical member as defined by its semantic owner. Canonical identity SHALL NOT be created, duplicated, extended or resurrected by this capability.

**A-2.2** A source address identifies an existing producer and its requested paint scope. A widget source SHALL include the existing widget identity and mount lifetime. A member source SHALL include its existing owning substrate/domain identity, stable logical member identity and producer lifetime; a row slot, position or reused projection object MUST NOT substitute for that identity.

**A-2.3** A source publication is an immutable, owner-produced paint-value publication with a source revision and B's provenance. A historical generation is an immutable retained version of certified paint values. Its lifetime belongs to presentation/resource ownership, not to the former widget or member.

**A-2.4** A paint occurrence is one compositional use of a source generation. Multiple occurrences MAY reference one immutable generation, but each SHALL have distinct occurrence identity and explicit placement/order/coverage ownership. Sharing bytes SHALL NOT share semantic or input identity.

**A-2.5** Authored leaf identity means the original stable paint identity and author affinity of a primitive within its source. Occurrence identity SHALL qualify coexistence without replacing that authored identity. Generation revision SHALL NOT be used as stable paint identity.

**A-2.6** A group identifies one coordinated presentation lifetime. A request identifies one causal admission or replacement attempt. A causal epoch identifies the owner-reported semantic/configuration/layout cause. These identities SHALL remain distinct from source address, source generation, paint occurrence, Timeline handle, engine frame/publication identity, AppFrameAttemptId and renderer FrameEpoch.

**A-2.7** Identities introduced by this contract SHALL be opaque, lifetime-qualified and non-wrapping. Exhaustion SHALL refuse the new request before identity reuse. Neither numeric coincidence nor equality of bounds SHALL establish correspondence.

**A-2.8** A participant is a declared compositional role over a source address/generation and occurrence. Roles are Current, Incoming or Historical. The engine SHALL derive and verify a role from actual causal publications. A caller MUST NOT relabel current content as Historical to bypass current clipping or cue law.

**A-2.9** A reservation is Update-owned authorization to consume bounded timing/storage for a causal attempt. It is not visual admission. A frame candidate is an immutable proposed group presentation. Scene admission, complete executable preparation, output production, submission and a present call are distinct observations.

**A-2.10** A continuation endpoint is a presentation-only value derived from an eligible earlier admitted publication. It is not a previous canonical layout rect and MUST NOT be recorded as one.

**A-2.11** A capture certificate, coverage manifest, operation admission and content-generation lease SHALL have the meanings and validation duties specified by B. A user-supplied boolean, string, raw texture ID or arbitrary rectangle SHALL NOT constitute one.

## 3. Ownership laws

**A-3.1** Canonical tree/model state, layout geometry, hit testing, navigation, focus, keyboard authority/capture, pointer capture and scroll capture SHALL remain current and independently authoritative. Presentation progress MUST NOT delay, modify, replay or fabricate a semantic transition or input event.

**A-3.2** Historical occurrences SHALL be paint-only. They MUST NOT publish or replay hit regions, accessibility identities/events, tooltip registrations, input members, focus/capture ownership, layout nodes or other semantic sidecars.

**A-3.3** Interaction state and presentation control records MUST remain physically separated. Presentation MAY consume owner-qualified immutable facts/provenance. It MUST NOT store a second mutable interaction-state mirror, run an interaction reducer or feed inferred facts back to an authority.

**A-3.4** Persistent request/group/Timeline control mutation SHALL occur only in legal Update or an existing owner teardown boundary outside active traversals. Post-layout resolution and paint MUST NOT mutate that control state. Their admission/validity results SHALL be immutable frame outcomes.

**A-3.5** Source values and frame candidates SHALL contain owned immutable data or owner-admitted stable references. They MUST NOT retain widgets, WidgetTree references, borrowed contexts, frame-local layout/projection objects, shaped glyph runs or private renderer objects.

**A-3.6** A group SHALL have exactly one effective progress/time source from the existing Timeline. All coordinated tracks SHALL derive from its immutable Update publication. Timeline SHALL NOT store source snapshots, semantic models or cross-widget mutable state.

**A-3.7** Every participant's coupled geometric write set SHALL have one effective presentation owner. An affine operation that couples axes SHALL claim the complete coupled set. Two independent writers MUST NOT be legalized by labelling their contributions different axes.

**A-3.8** Current authoritative clips and semantic surface/viewport constraints SHALL retain their owners. Historical InternalCoverage and PresentationMasks SHALL have only B's explicitly admitted paint meaning.

**A-3.9** Removing this capability, settling a group or refusing admission SHALL leave canonical semantic and input results unchanged. A visual contradiction MAY cause presentation settlement only; it MUST NOT retarget input.

**A-3.10** Published SceneNodes and admitted historical generations SHALL be immutable. Resource content immutability SHALL be enforced by B's leases, not inferred from an immutable CPU handle.

## 4. Requests, motion policy and preliminary admission

**A-4.1** A request SHALL name its owning producer, causal epoch, group/replacement relationship, finite participant/source set, requested recipe/composition, admitted timing descriptor, required operation profile and explicit settlement policy. Missing owner, cause or required finite policy SHALL refuse the request.

**A-4.2** Event handling MAY emit a request through an existing action/deferred-owner boundary. It MUST NOT install this capability's group control during routing. The presentation owner SHALL accept, replace or refuse it during Update.

**A-4.3** Requests SHALL be ordered by the existing deterministic owner/action order. Repetition of an identical owner/cause/request SHALL be idempotent. A contradictory reuse of the same request identity SHALL be refused. New paint dirt or a continuing animation frame MUST NOT be treated as a new cause.

**A-4.4** A layout-affecting cause MAY reserve timing before its final rects exist, provided the cause is an actual owner-reported layout/configuration change and admission remains conditional on the subsequent authoritative layout result. Ordinary paint-only invalidation MUST NOT manufacture such a cause.

**A-4.5** Product/framework code MAY choose an admitted recipe and finite parameters. It MUST NOT supply fabricated canonical rect history, arbitrary semantic selectors, executable painters, callbacks, private clocks, GPU handles, capture-safety assertions or an unconstrained scene graph as a request.

**A-4.6** A framework motion-policy owner SHALL publish one immutable policy for the frame during Update. The policy SHALL state enablement, treatment of nonessential motion, applicable finite timing limits, permitted recipe/profile classes and finite resource/work limits.

**A-4.7** Product and OS accessibility preferences MAY be inputs to that owner. They MUST NOT directly mutate presentation control or replace framework semantic authority. No proposed visual SPEC or unspecified platform default SHALL supply missing policy.

**A-4.8** When policy disables a requested class, preliminary admission SHALL return Disabled and render the ordinary canonical result. When policy disables an active class, that frame SHALL settle canonically and the owner SHALL cancel its handle in Update. Tree animation's existing explicit enablement/default-off contract SHALL remain unchanged.

**A-4.9** Every group SHALL use one admitted Timeline progress source and finite lifetime. Multiple properties SHALL be deterministic projections of its published value using finite admitted descriptors. A recipe MUST NOT introduce an independent clock, dt accumulator, hidden loop or executable per-frame callback.

**A-4.10** Preliminary admission SHALL validate ownership, request causality, participant bounds, known conflicts, motion policy and timing/storage capacity. Failure SHALL allocate no live animation. Successful reservation SHALL bind its handle to the current captured frame timestamp; its new-handle sample SHALL be exact from.

**A-4.11** Preliminary admission SHALL NOT assert that geometry, historical capture, current cues, resources or rendering are admitted. Those obligations SHALL be completed in stage II and B. Known unsupported capability SHALL return PendingCapability without starting a waiting animation.

**A-4.12** Zero duration, explicitly disabled motion and a proved no-op SHALL select the canonical result without unnecessary historical capture. Equality MUST be established from owner-produced values/dependencies, not solely equal bounding boxes or source identity.

**A-4.13** The existing framework Update/presentation owner SHALL publish one immutable effective motion-policy value for the admitted request scope. Platform preference changes and product requests are advisory inputs to that owner through existing event/update delivery. Neither may start a Timeline, mutate presentation during paint or alter semantic/layout truth.

**A-4.14** The policy SHALL explicitly select disabled/settle or an admitted bounded recipe profile and finite limits. A reduced-motion request SHALL be handled by that declared policy; it SHALL NOT inherit an unadopted duration, assume a universal 75 ms/default curve, or silently convert a recipe to fade/scale. If no conforming permitted motion profile exists, the effect settles to ordinary canonical presentation.

**A-4.15** Policy changes are processed in Update, cancel/replace affected handles there, publish a coherent value and invalidate only changed projection dependencies. An active presentation cannot keep running under a superseded disable policy.

**A-4.16** Existing consumer opt-in/default-off laws, including SPEC-HAL-120, remain stricter requirements. Missing numeric runtime policy is unadmitted capability, not an invented system default. This policy is data and authority within existing framework ownership, not a global motion manager, second scheduler or product semantic state.

## 5. Two-stage phase protocol

**A-5.1** The following protocol SHALL execute within existing SPEC-HAL-042 phases. It SHALL NOT create a new engine phase, reorder custom preparation/paint or permit a second update/layout/scene/presentation sequence.

**A-5.2** Stage I SHALL execute in legal Update. The owner SHALL consume prior immutable outcomes, apply cancellation/replacement, perform preliminary admission, start/cancel its Timeline handles, sample them read-only and publish a complete immutable request/parameter set for the frame.

**A-5.3** The publication SHALL include the request/group/cause identities, exact finite progress and descriptor versions, participant/source references, policy/budget version and permitted settlement outcome. Later phases MUST NOT query Timeline to fill missing values.

**A-5.4** Layout SHALL consume only current semantic/style inputs through existing owners. It MUST NOT consume presentation progress, historical geometry, presentation masks or framework focus/capture facts through LayoutContext.

**A-5.5** After layout, stage II SHALL resolve actual geometry, pure eligibility, effective projection ownership and presentation endpoints from the immutable publication. Eligibility/arbitration SHALL remain independent of easing/progress and widget internals. Pure projection MAY then consume the published progress.

**A-5.6** Stage II SHALL complete source, coverage, cue and operation admission when the existing producer/retained-paint boundaries make the complete values available. “Post-layout” MUST NOT be interpreted as permission to paint a widget early or move custom preparation out of its established phase.

**A-5.7** Source production and descriptor resolution SHALL be side-effect-free with respect to widget, model, interaction, layout and presentation-control state. Retained-cache publication and GPU resource preparation MAY perform only the mutations separately authorized to those existing owners. Such resource work MUST NOT be described as a pure resolver.

**A-5.8** A stage-II frame result SHALL be one complete group candidate or one explicit whole-group settlement/refusal result. Required participants MUST NOT publish independently. A temporary incomplete source, missing capture or unadmitted operation SHALL NOT become a placeholder. Canonical settlement is permitted only before the complete presented scene is selected for SceneGraph diff, the final scene-choice cutoff in B-9.8; after that cutoff B-9.9 and B-9.18 govern.

**A-5.9** The first scene-admitted group candidate SHALL use the reservation's exact same-frame start sample. If that causal frame cannot admit the complete candidate, the attempt SHALL expire. It MUST NOT first appear in a later frame at advanced progress, retain a hidden running interval or revive merely because a resource later becomes ready.

**A-5.10** A later attempt SHALL require a fresh explicit owner request with valid current causality, sources and timing. It MUST NOT claim the old first-publication opportunity or fabricate a delayed layout history.

**A-5.11** The complete selected scene SHALL pass through ordinary presented-scene construction, diff, SceneCache dispatch and renderer preparation. No parallel appearance/presentation scene path SHALL bypass those owners.

**A-5.12** Post-layout admission and execution outcomes SHALL be immutable records associated with the exact request/publication/attempt/surface. They MUST NOT start/cancel/retarget Timeline, mutate an Active flag, invoke widgets or trigger semantic actions. Their persistent lifecycle effects SHALL be reconciled at the next legal Update.

**A-5.13** An outcome for an obsolete request SHALL NOT alter a newer request. Its unneeded references SHALL retire through their owners. A late or missing outcome MUST NOT resurrect an occurrence.

**A-5.14** The renderer's prepared/output/submission/present-call observations SHALL retain B's exact meanings. A successful admission or present call MUST NOT be reported as physical display completion.

**A-5.15** Existing engine demand SHALL service active intervals and the bounded next-Update reconciliation they require. PendingCapability/refused requests and pure resource-retirement records SHALL NOT create anonymous animation demand, a polling loop or a private scheduler.

## 6. Lifecycle and outcomes

**A-6.1** The Update-owned control lifecycle SHALL be Requested, Reserved, Active, Completing, Retiring, Retired. Refused, Disabled, NoOp, PendingCapability, Cancelled, SettledCanonical and Unavailable SHALL be distinct terminal/reconciliation outcomes, not interchangeable absence. Refused and Unavailable name outcome families only: every actual result MUST carry its specific A §12/B §9 reason. They are not standalone ambiguous absence values.

**A-6.2** Requested SHALL become Reserved only after successful preliminary admission and exact-time reservation. Otherwise it SHALL produce its precise outcome and release any request-local reservations.

**A-6.3** Reserved SHALL remain nonauthoritative as to visual activation. This frame's immutable candidate/outcome SHALL report actual admission and execution. At the next Update, a successful matching publication/execution outcome SHALL permit Active; a failed, expired, disabled, no-op or settled outcome SHALL cancel/release the reservation and enter Retiring as needed.

**A-6.4** A known execution failure SHALL NOT be treated as successful visual activation. An unchanged retained scene MAY carry forward earlier successful output evidence only when B proves identical admitted presentation and unchanged relevant dependencies; a skipped or failed attempt alone SHALL NOT provide that proof.

**A-6.5** Active SHALL publish exactly one complete admitted group result per affected frame. A current-cue, coverage, ownership, policy or resource failure SHALL select canonical settlement for the entire group only before the complete presented scene is selected for SceneGraph diff, the final scene-choice cutoff in B-9.8. Its immutable result SHALL cause cancellation/Retiring at the next Update. After that cutoff—including Build and Compile—B-9.9 applies; failure learned after execution begins follows B-9.18.

**A-6.6** An explicit cancellation/replacement in Update SHALL cancel the old Timeline handle before a replacement handle is sampled. Cancellation SHALL NOT synthesize completion or dispatch a semantic action.

**A-6.7** On Timeline Completed, the owner SHALL publish its exact terminal value for that frame and enter Completing. The terminal scene SHALL contain the canonical final presentation and no unnecessary outgoing occurrence. Failure to execute that terminal scene SHALL be reported honestly; it MUST NOT be hidden by replaying a preterminal value.

**A-6.8** Completing SHALL enter Retiring after its matching outcome is reconciled. Retiring SHALL remove presentation relevance and release logical references exactly once; already submitted work MAY retain physical resources only under B's resource owner.

**A-6.9** Retired SHALL retain no live Timeline handle, source capture, occurrence or semantic sidecar. Inspection MAY retain only bounded latest terminal evidence; it MUST NOT accumulate removed-key/request tombstones.

**A-6.10** Cancellation, refusal, settlement and completion SHALL leave canonical current content to its normal owners. If that canonical content is itself unavailable, the existing renderer/surface failure SHALL remain visible as failure; this capability MUST NOT manufacture a successful fallback.

**A-6.11** Source deletion SHALL NOT be delayed for animation. A historical generation MAY continue only if previously certified, independently leased and still valid under its host/barrier/cue dependencies. Missing preexisting capture SHALL refuse Exit rather than call a deleted producer or recreate its widget.

**A-6.12** Parent/host removal, incompatible reparent, window/surface teardown, device invalidation or irrecoverable source/resource failure SHALL prevent further invalid publication and retire the affected records through existing owners. Teardown outside active traversals SHALL release abandoned control without waiting for a future Update that cannot occur.

**A-6.13** Logical retirement SHALL be bounded independently of GPU completion. Physical reclamation SHALL wait for the resource owner's proof of safety or device destruction. A stalled GPU SHALL cause bounded retained accounting/backpressure, not unsafe release, unbounded new admissions or a false finite-time reclamation promise.

**A-6.14** Every state transition SHALL be deterministic from owner publications, explicit requests, the single frame time and matching outcomes. Unspecified/stale external observations SHALL not constitute an implicit retry or state transition.

## 7. Interruption, retargeting, reversal and rebase

**A-7.1** Interruption/retargeting SHALL be an explicit Update-owned replacement attempt. The owner SHALL identify the old request and its eligible immutable presentation publication, cancel the old handle and reserve the replacement at the current frame timestamp.

**A-7.2** A continuation source SHALL have valid source/coverage/resource provenance and matching execution-backed evidence under B. A known failed/unexecuted candidate MUST NOT be claimed as the previously visible presentation. An unchanged retained presentation MAY carry forward equivalent prior evidence under B. A producer-certified historical-safe alternative is a valid paint-purpose source only under its own contract. Its certificate alone does not prove that those alternative values were previously executed or that continuation from them is visually continuous.

**A-7.3** If an eligible continuation source is unavailable, the replacement SHALL settle canonically. It MUST NOT guess the current visual value, backdate a start, synthesize a prior layout visit or keep a hidden interval waiting for evidence.

**A-7.4** Rebase MAY derive a new presentation-only endpoint from an eligible prior publication and the actual current canonical endpoint. The old and new canonical layout records SHALL remain unchanged. The derived value SHALL be labelled as presentation continuation, never from_rect or previous layout state.

**A-7.5** The new source-to-host mapping SHALL be revalidated against current coordinate, clip, surface, cue and resource dependencies. Incompatible or unknown mappings SHALL settle/refuse the replacement. A recipe name MUST NOT authorize an inferred offset or a cross-surface transfer.

**A-7.6** A replacement MAY retain a bounded immutable expression of a prior presentation, including fixed projection parameters and referenced generations. It MUST NOT retain a live child controller, Timeline handle or callback whose future values can mutate that expression.

**A-7.7** Reversal SHALL be explicit. It MAY reverse roles/path parameters only when the required endpoint generations, current source roles, host constraints and execution profile remain valid. Otherwise it SHALL be a newly admitted replacement or canonical settlement; it MUST NOT silently substitute fade, scale, delay or another recipe.

**A-7.8** An admitted reverse operation SHALL begin at its verified continuation value and terminate at its declared destination. A request for a perceptual inverse SHALL preserve the declared path and coverage relation under reversed progress where those dependencies permit it.

**A-7.9** This contract SHALL guarantee only the continuity specifically proved by the admitted value/path relation. It SHALL NOT imply velocity, momentum, friction, physical simulation or exact inverse easing. Any such stronger requirement requires its owning authority; Timeline's existing no-physics boundary remains.

**A-7.10** Repeated replacement SHALL obey the same bounds as first admission. Recapturing or wrapping an existing historical lineage MUST NOT reset its retention age or conceal its resource dependencies. No-op replacements SHALL avoid new history and handles.

**A-7.11** A canonical reflow SHALL reset canonical truth immediately. It MAY be followed by a newly admitted presentation-only continuation under the scoped SPEC-HAL-145/SPEC-HAL-146/SPEC-HAL-147 amendments. It MUST NOT preserve old canonical ownership or feed visual geometry back into layout.

## 8. LayoutTransition and effective presentation ownership

**A-8.1** Ordinary LayoutTransition SHALL continue to derive geometry from actual previous/current layout states and its existing identity/eligibility rules. This capability SHALL NOT redefine that history or make every source eligible for ordinary LayoutTransition.

**A-8.2** A group MAY consume an eligible reflow result only through an explicit engine-owned ownership plan naming the affected participants and coupled geometric channels. That plan SHALL select one effective projection. The ordinary LayoutTransition projection for consumed channels MUST NOT also be applied.

**A-8.3** If consumption/replacement of an existing visual owner is not explicitly authorized, the group SHALL be refused on those conflicting participants. It MUST NOT cancel another owner merely because product code requested an effect.

**A-8.4** A retained admitted owner SHALL keep its channels until explicit replacement/cancellation or a higher-priority authority invalidates it. If new candidates conflict without an already authorized replacement relationship, the conflicting new candidates SHALL be denied as complete groups. Source order, width, area, ID magnitude or swept-envelope intersection MUST NOT invent a winner.

**A-8.5** A coupled affine write set SHALL include every position/size/axis dependency affected by its pivot, rotation, shear or matrix composition. Independent scalar channel claims MUST NOT legalize noncommuting writers.

**A-8.6** Ancestor/descendant ownership SHALL be evaluated over the actual affected paint scope. Two active projections MUST NOT both transform the same effective leaf geometry. Independent scopes MAY proceed; a frozen historical inner expression MAY be consumed by one outer owner; otherwise an explicit single coordinated ownership plan is required or the conflict SHALL be denied.

**A-8.7** Current layout correctness, canonical input, scroll anchoring and semantic overlay/surface constraints SHALL take precedence over presentation. New source kinds SHALL use their explicitly admitted rules; they SHALL NOT inherit a blanket exception to SPEC-HAL-078A/B's scroll, virtualization, overlay, reparent or anchoring denials.

**A-8.8** Compatible independent sibling reflow SHALL NOT be denied solely because static from/to union envelopes overlap. Such intersection is not evidence of competing influences on the same participant.

**A-8.9** Eligibility/arbitration SHALL consume only owner-published identity, canonical geometry and admitted scope/constraint facts. It MUST NOT infer policy from easing/progress, widget implementation details, pixels or user callbacks.

**A-8.10** After eligibility, pure complete-candidate validation MAY reject a projected result that violates coverage, cue or execution law. Such rejection SHALL not create a second geometry owner or silently blend conflicting projections.

**A-8.11** A pointer/input contradiction detected under existing SPEC-HAL-078 law SHALL request presentation settlement while canonical routing continues unchanged. It MUST NOT redirect, replay, suppress or delay the input event, grant capture, or create activation.

**A-8.12** If a group that consumed reflow fails complete source/scene admission before B-9.8's scene-choice cutoff, the affected group SHALL settle canonically for that frame. Failure after that cutoff SHALL follow B-9.9 and B-9.18. A different animation MUST NOT be substituted as an implicit fallback. Obsolete handles/control SHALL reconcile at the next lawful owner boundary.

## 9. Current cues and semantic lifecycle

**A-9.1** Current appearance SHALL consume the existing owner-qualified framework/component/product facts. Focus domain absence, active window/domain gating and independent keyboard, pointer and scroll capture SHALL remain honest. Presentation SHALL NOT fabricate a focus-domain ID or derive authority from local animation flags.

**A-9.2** Each admitted frame SHALL contain exactly the complete current focus/capture treatment required by SPEC-HAL-012/SPEC-HAL-328 and the component owner. Historical content MUST NOT duplicate or contradict that treatment. An appended default ring MUST NOT compensate for unsafe historical pixels.

**A-9.3** A historical source SHALL be admitted only with producer-certified paint that does not assert stale current authority. Certification SHALL occur before flattening. Pixel inference, color removal, image analysis or fabricated “focused=false” facts SHALL NOT establish safety.

**A-9.4** A producer MAY publish an explicitly historical-safe value variant under its own accepted appearance/semantic contract. This is a paint-purpose publication, not mutation of authoritative facts. If such a variant cannot be produced lawfully, capture SHALL be refused.

**A-9.5** Selection, error, disabled, caret, hover/press and other meaningful visual output SHALL be classified by its owning contract. This capability SHALL NOT presume that all such output is either harmless history or universally removable decoration. Missing required classification SHALL refuse historical capture.

**A-9.6** A group's masks, opacity, transforms or ordering MUST NOT suppress, duplicate or falsify a required current cue within the current authoritatively visible scope. Where preservation cannot be certified, the entire affected group SHALL settle/refuse. No parallel cue owner SHALL be introduced.

**A-9.7** Existing SPEC-HAL-328 neutral outgoing tails SHALL retain their exact owner/geometry/scope/lifecycle limits. They SHALL NOT authorize a focused/captured ghost after semantic unmount, scope loss or invalid configuration replacement.

**A-9.8** Fact/configuration changes SHALL invalidate the narrowest affected current candidate/capture-safety dependency. They MUST NOT mutate an immutable historical generation in place or dirty layout for focus/capture alone.

**A-9.9** Widget/member removal, focus recovery, capture release, accessibility removal and parent/window teardown SHALL occur under their existing owners without waiting for presentation. A historical source address SHALL be provenance only, not evidence that the semantic owner still exists.

**A-9.10** A scene-build or inspection query SHALL be read-only with respect to those authorities. No refusal, execution receipt or retirement callback SHALL dispatch a semantic action.

## 10. Tree/member sources and realization

**A-10.1** Realized member sources SHALL be part of the general source contract, not an Exchange-specific extension. Their producer SHALL publish owned immutable paint values, stable logical source address and B's certificate from already realized content.

**A-10.2** The capability MUST NOT create WidgetIds, member/input/focus/layout identities, synthetic rows or offscreen realization to obtain a source. It MUST NOT query an unrealized member for a rect or retain a recyclable row slot as identity.

**A-10.3** A retained historical member generation SHALL contain paint values only. It MUST NOT retain RowProjection, layout caches, borrowed model members or other frame-local derived objects.

**A-10.4** Current Tree expansion/collapse, ordering, selection, focus, hit testing and scroll/anchor semantics SHALL take effect immediately according to their existing owners. Presentation progress MUST NOT alter realization policy or keep removed/collapsed members interactable.

**A-10.5** Member recycling or lifetime mismatch SHALL invalidate current source admission. Already certified historical values MAY survive only under their independent generation/host/resource lifetime; they MUST NOT bind to the recycled member.

**A-10.6** Capture readiness SHALL be established while the producer can lawfully publish the source. An existing teardown boundary MAY transfer an already certified owned value before current-cache eviction under B's bounded custody rules. It MUST NOT repaint or resurrect a deleted producer.

**A-10.7** A SPEC-HAL-012 visual part SHALL NOT become a semantic member or independent layout/input owner. A part address alone SHALL NOT confer capture eligibility or a retained lifetime; only its producer's explicitly certified paint scope may be used under B.

**A-10.8** Tree/member acceptance SHALL preserve SPEC-HAL-120's explicit enablement/default-off, realized-only behavior and scroll stability. The scoped member-source amendments SHALL not reopen SPEC-HAL-164/SPEC-HAL-167's closed semantic ownership.

## 11. Bounded accounting, demand and retirement

**A-11.1** Every admitted execution policy SHALL provide finite limits for active/reserved groups, queued requests, participants, retained generations, occurrences, nesting/dependency edges, CPU retained bytes, text-source bytes, resource/intermediate-target bytes, in-flight/retiring holdings, per-frame capture/preparation work and historical retention age.

**A-11.2** A missing limit SHALL make that policy unadmitted. This contract SHALL NOT invent numerical defaults. A concrete policy SHALL be published with measured rationale and its acceptance evidence before runtime admission.

**A-11.3** Accounting SHALL include candidates, current group holdings, continuation lineages, failed-attempt cleanup and resources retained for submitted work. Replacing a record SHALL not reset counters while its old holdings remain live.

**A-11.4** Shared immutable storage MAY be charged once per physical allocation in an accounting domain, but every live reference/lease SHALL be accounted for. Strongly reachable retained storage SHALL be charged even when only a small subregion is painted. A pointer, alias, crop or resource ID SHALL NOT conceal retained bytes.

**A-11.5** Admission SHALL test peak coexistence of the old, new and retiring generations before acquiring holdings that exceed the limit. A partially acquired candidate SHALL roll back its presentation/resource reservations through their owners without changing canonical state.

**A-11.6** A new request exceeding a limit SHALL return RefusedBudget. An active group that can no longer satisfy its admitted limit/validity SHALL settle canonically and retire. The engine MUST NOT evict required canonical resources, drop one required participant or silently downgrade the recipe to stay within budget.

**A-11.7** A historical generation's age SHALL be measured from its first capture in the still-live continuation lineage using existing engine time. Aliasing, nesting, interruption or recapture of that history MUST NOT reset the origin. A genuinely new capture from a current canonical publication MAY begin a new lifetime after prior lineages retire; this SHALL require no unbounded tombstone history.

**A-11.8** At the retention deadline, the affected group SHALL settle/retire. Resource references required by already submitted work SHALL remain separately charged until their owner permits reclamation. That physical wait SHALL not preserve active presentation or semantic identity.

**A-11.9** Group completion/cancellation SHALL release Timeline demand at the lawful observation boundary. GPU retirement alone MUST NOT keep an animation handle or generate a new frame loop. Resource progress SHALL use its existing owner mechanisms.

**A-11.10** Idle capability state SHALL perform no per-frame capture allocation, provenance construction, full-tree scan or speculative source realization. Work SHALL be proportional to admitted/changed sources and declared dependencies in addition to the existing normal passes.

**A-11.11** A projection-only update SHALL invalidate affected occurrence paint/composition and necessary ancestors, not canonical layout or unrelated siblings. A genuine style metric change SHALL invalidate layout through its existing style owner. Framework focus/capture changes SHALL remain paint-only.

**A-11.12** Cleanup SHALL remain bounded under repeated toggles, source deletion, cancelled preparation and delayed GPU completion. No record without a live request, occurrence, retained outcome bound or resource-safety obligation SHALL remain retained.

## 12. Outcomes, authoring diagnostics and inspection

**A-12.1** Admission SHALL expose distinct outcomes for Admitted, NoOp, Disabled, PendingCapability, RefusedOwnership, RefusedSource, RefusedCoverage, RefusedCue, RefusedBudget, UnavailableResource, ExpiredFirstPublication, Cancelled, SettledCanonical and Completed. The implementation MAY refine reasons, but MUST NOT merge absence, pending support and successful rendering.

**A-12.2** NoOp SHALL mean that the complete declared trajectory and relevant values/dependencies are equivalent to the canonical presentation. Equal first samples, equal endpoint rects or equal IDs alone SHALL NOT establish NoOp.

**A-12.3** PendingCapability SHALL identify the unadmitted operation/pipeline/resource/target combination. It SHALL NOT be a hidden running request or automatic retry queue.

**A-12.4** Refusal before activation SHALL select ordinary canonical presentation. Failure after activation SHALL settle the entire group when lawful before B-9.8's final scene-choice cutoff, or follow B-9.9/B-9.18's complete failure and truthful late-failure rules. Canonical presentation failure SHALL retain its owning failure outcome.

**A-12.5** Fallback SHALL be explicit and complete. No request SHALL silently become fade, scale, delayed appearance, root crop, raster snapshot, missing participant, Image-to-solid substitution or a different recipe.

**A-12.6** Inspection SHALL be on-demand and shall correlate owner/cause/request/group identities; source addresses/generations; occurrences/authored leaves; Update progress; actual canonical endpoints and separately labelled continuation; effective ownership; motion/budget policy; coverage/resource/cue dependencies; operation admission; exact execution outcome; and retirement obligations.

**A-12.7** Inspection SHALL distinguish current control state from the current immutable frame result. It MUST NOT report a Reserved request as displayed merely because it has a handle.

**A-12.8** Inspection SHALL neither force layout/realization nor sample/mutate Timeline outside its allowed phase. It MUST NOT expose renderer-private handles, return dangling widget/model references or create unbounded hot-path provenance/history.

**A-12.9** A terminal request MAY retain bounded latest diagnostic evidence under explicit policy. Evicted evidence SHALL be reported as absent; it MUST NOT be reconstructed from reused IDs.

**A-12.10** Public authoring SHALL expose only finite request values and typed results appropriate to the requesting owner. Capture certification, occurrence namespace allocation, canonical geometry extraction, content leases and renderer execution selection SHALL remain engine/producer-owned capabilities.

**A-12.11** Initial public exposure MAY be restricted while the full mechanism remains specified. Internal-only status SHALL NOT permit violating this contract, and a later public API SHALL NOT reinterpret an already admitted source or outcome.

## 13. Acceptance obligations, recipes and non-goals

**A-13.1** Adoption SHALL require review of the complete A/B authority chain and every scoped amendment. Implementation admission SHALL additionally require the proofs for its exact profile; adoption alone SHALL not establish runtime conformance.

**A-13.2** Canonical authority proofs SHALL compare presentation-enabled/disabled executions and establish unchanged semantic actions, activation count, stable WidgetId/member identity, layout/hit truth, independent focus/capture channels and accessibility/input teardown.

**A-13.3** Phase/timing proofs SHALL establish one time source, one coherent group sample, exact start/terminal values, no late Timeline/control mutation, no half-visible first activation, and no extra scene sequence.

**A-13.4** Lifecycle proofs SHALL cover intended replacement/interruption/reversal, no-op/zero duration, policy disable, source/parent/window removal, reparent incompatibility, resource failure and bounded physical retirement. Known-first-frame failure SHALL not later join an advanced interval.

**A-13.5** Retained proofs SHALL establish narrow paint-only invalidation, correct metric-owned layout invalidation, unchanged sibling reuse, distinct occurrence identity, immutable generations and bounded repeated replacement.

**A-13.6** Exchange SHALL be an acceptance recipe. Surrounding content moving in direction D and complete incoming content moving from that side in direction -D SHALL derive their motion and presentation-only occlusion boundary from one group progress. The incoming surface/border/content SHALL be coherent from its first visible pixel.

**A-13.7** The Exchange acceptance profile SHALL use rigid translation and admitted presentation masks only. It MUST NOT stretch content, use opacity dissolve, scale/zoom, bounce, elastic overshoot, placeholders, artificial delay or late materialization. Its reverse SHALL satisfy the admitted inverse-path law or explicitly settle when dependencies make that inverse invalid.

**A-13.8** The Exchange native proof SHALL use ordinary current widgets and real routing, retain fixed comparison controls and demonstrate stable semantic identity. Static state labels or synthetic input facts SHALL not substitute for routed hover/press/focus behavior.

**A-13.9** Tree/member SHALL be a separate acceptance pressure test. It SHALL demonstrate already-realized member capture, current expansion/selection/focus/hit/scroll semantics, recycling-safe stable source addresses and no invented WidgetIds/offscreen geometry.

**A-13.10** CPU/scene/retained proofs SHALL establish mechanical laws. Native visual acceptance SHALL establish the intended perceptual recipe. Neither SHALL be used as evidence for an untested affine, text, resource or target profile.

**A-13.11** Negative proofs SHALL cover reachable confusable violations of admitted paths. This contract SHALL not require an unbounded rejection catalogue for unrelated unsupported features.

**A-13.12** This capability SHALL NOT define product domain meaning, widget input/focus algorithms, a new layout system, asset loading, NinePatch, CSS/cascade/DSL, a material/effect/shader language, private GPU capture, typography expansion or a second animation framework.

**A-13.13** Blur, glow and other admitted material output MAY be hosted as paint values only when their own rendering/resource authority admits them. This capability SHALL NOT infer their implementation from its general composition vocabulary.

**A-13.14** Compatible sibling reflow and authoritative current clipping SHALL remain correct under this capability. No consumer-specific spacing, widget type, width or ID exception SHALL satisfy the general acceptance obligations.

## 14. Paired adoption and scoped amendment package

**ADOPT-1** SPEC-HAL-363 and SPEC-HAL-364 SHALL carry the same documentary status and explicit reciprocal dependencies. JC remains author and final adoption authority under PROJECT-ENTRY. Repository publication is authorized for these verified paths; it is not final adoption. The pair and every required scoped amendment SHALL be reviewed and adopted together.

**ADOPT-2** Constitution SPEC-HAL-002/SPEC-HAL-002A SHALL remain unchanged and supreme. The adoption review SHALL prove that SPEC-HAL-363/SPEC-HAL-364 use existing owners/phases and introduce no global semantic service, canonical tree, input authority or clock.

**ADOPT-3** The adoption change set SHALL contain A/B and the exact scoped N replacements listed in §14.1 below. A broad “newer spec wins” statement is insufficient. Each affected document or explicit frozen-document follow-up SHALL name the replaced clause, the new scope and what remains unchanged.

**ADOPT-4** Normative source edits required by this proposal are SPEC-HAL-021, SPEC-HAL-042, SPEC-HAL-078, SPEC-HAL-078A, SPEC-HAL-078B, SPEC-HAL-145, SPEC-HAL-146, SPEC-HAL-147, SPEC-HAL-024, SPEC-HAL-150, SPEC-HAL-354, SPEC-HAL-084, SPEC-HAL-359, SPEC-HAL-040, SPEC-HAL-263B, SPEC-HAL-266A, SPEC-HAL-295A, SPEC-HAL-012, SPEC-HAL-328 and SPEC-HAL-120, plus the new A/B documents. SPEC-HAL-083's changes live in new B as its required follow-up; SPEC-HAL-164/SPEC-HAL-167 source-handoff permission lives in A/B, preserving their closed semantics.

**ADOPT-5** Cross-reference/status-only work is limited to the actual relationship/status notes needed for SPEC-HAL-263A, SPEC-HAL-085A, SPEC-HAL-360, SPEC-HAL-361, SPEC-HAL-083A and closed SPEC-HAL-164/SPEC-HAL-167, and the new documents' dependency tables. Halcyon PROJECT-ENTRY explicitly states there is no separate accepted-authority index; this package SHALL NOT invent one. Non-binding guide/catalog maintenance, if separately authorized, is not adoption and cannot weaken clauses.

**ADOPT-6** Profile-specific SPEC-HAL-085A ABI amendments, a new public SPEC-HAL-263A registration boundary, changed surface/window protocols or a device-recovery contract are conditional execution gates. This general package does not invent those implementations. A profile that requires one remains PendingCapability until that exact owning contract and proof are added; it cannot be implemented through an inferred exception.

**ADOPT-7** General vocabulary and lifecycle are complete now: widget/subtree/member sources, multiple immutable generations/occurrences, continuation/reversal, coverage classes, affine/mask/opacity/order, leases and typed refusal. Runtime verticals MAY admit measured profiles incrementally, but SHALL NOT narrow the general identity/lifetime model into a Button-only or WidgetId-only API.

**ADOPT-8** First execution admission SHALL name concrete source/pipeline/operation/target scope, finite measured budgets, lawful source producer certification, exact phase integration, complete failure behavior and proof ownership. It SHALL record CPU/retained/resource execution proof separately from native visual acceptance.

**ADOPT-9** Exchange and Tree/member are acceptance recipes/pressure tests, not new subsystem names. No recipe carries implicit renderer, virtualization, reparent, overlay, resource or current-cue permission.

**ADOPT-10** The late failure cut SHALL be explicit: after selecting the complete presented scene for diff, no alternate scene or second sequence is permitted. A required failure discovered in Build/Compile invalidates complete surface execution. No generic late canonical branch is adopted.

**ADOPT-11** All adopted text SHALL preserve ordinary legacy consumers unless a listed clause expressly changes that path. Ordinary SPEC-HAL-078 matching, unmigrated appearance hooks, non-group SPEC-HAL-263B failure, current SPEC-HAL-354 sidecars and current SPEC-HAL-266A target bindings remain distinguishable. No source/API presence becomes proof of new conformance.

**ADOPT-12** Before documentary adoption, the editor SHALL perform a final cross-reference and conflicting-MUST review against the live files, not this mailbox alone. Documentary adoption is not an implementation, release, benchmark or native acceptance claim.

### 14.1 Exact owner amendment surface

N denotes a scoped normative amendment; X denotes relationship/status annotation only. This table records this pair's change surface; it is not an authority index. In-document paired status guards apply until joint adoption. Unlisted clauses remain unchanged.

| Owner | Kind | Exact scope |
| --- | --- | --- |
| [SPEC-HAL-021](SPEC-HAL-021-Animations.md) | N | §2.3; ANIM-INV-11; §6 — bounded Update parameter, pure later projection, no late Timeline control |
| [SPEC-HAL-042](SPEC-HAL-042-Frame-Loop-and-Engine-Ticker.md) | N | §3.2; §§6.6–6.7 — existing phase order, one complete scene before diff |
| [SPEC-HAL-078](SPEC-HAL-078-Layout-Transitions-v1.md) | N | §3; §5; LT-INV-3/7/8/9; §§8–10 — truthful endpoints, certified ghosts, live coverage |
| [SPEC-HAL-078A](SPEC-HAL-078A-Layout-Transition-Authorization-and-Policy-Gate.md) | N | §2; §6; §§7–8 applicability — timing reservation versus pure authorization; explicit member-source route |
| [SPEC-HAL-078B](SPEC-HAL-078B-Layout-Transition-Authorization-and-Conflict-Law.md) | N | §§3–5; §7; §8 — effective channel owner, sibling non-conflict, nested scope and clipping |
| [SPEC-HAL-145](SPEC-HAL-145-Layout-Transitions-and-Reflow-Authority.md) | N | C/F/H/I — canonical domains, consumed reflow, separately admitted continuation |
| [SPEC-HAL-146](SPEC-HAL-146-Temporal-Ownership-of-Geometry.md) | N | C/D/E/F/G; H/K/L matching restatements — canonical reset and new visual interval |
| [SPEC-HAL-147](SPEC-HAL-147-Animation-as-Authorized-State-Interpolation.md) | N | C/D/F/H; Scope/I/K matching restatements — endpoint classes and reflow exception |
| [SPEC-HAL-024](SPEC-HAL-024-Coordinate-Model.md) | N | Layer A §§3/5–7/10/12; Layer B §§4.2/5/9–10 and §2/6/11 matching restatements — resolved presentation mapping, canonical clips/input |
| [SPEC-HAL-150](SPEC-HAL-150-Compositor-Authority-and-Limits.md) | N | D/E/F/G/H; J/M matching failure and acceptance qualifications — authorized projection, complete failure |
| [SPEC-HAL-354](SPEC-HAL-354-Retained-Value-Updates-and-Scene-Fragments.md) | N | §3/3.2/3.3; FRAGMENT-3/4/5; §§4.1/4.3–4.4/5 — distinct history, sidecars, dependencies and pre-diff cutoff |
| [SPEC-HAL-084](SPEC-HAL-084-RenderGraph-v2.md) | N | INV-RG-007/008/011/016; D.1–D.3 — named leases, topology, complete failure and honest late outcomes |
| [SPEC-HAL-359](SPEC-HAL-359-Cross-Pipeline-Z-Order-Execution.md) | N | §§2.3/3/5/8 qualifying §§2.1–2.2/4/6 — closed occurrence order, complete schedule |
| [SPEC-HAL-040](SPEC-HAL-040-GPU-Resource-Lifetime-Management.md) | N | §3/§8, §2 matching category reference — explicit Retained Presentation Generation; §§4–6 boundaries preserved |
| [SPEC-HAL-263B](SPEC-HAL-263B-Image-Registration-Lifetime-Correction.md) | N | INV-001–004, §3 literal-parent applicability — distinct historical binding and required-group complete failure; INV-005 unchanged |
| [SPEC-HAL-266A](SPEC-HAL-266A-Per-Target-Custom-Render-Preparation.md) | N | §3.3; INV-005/007A/013, INV-006/010 required-group applicability — immutable fronts, full extent, separate leases |
| [SPEC-HAL-295A](SPEC-HAL-295A-Custom-Render-Pass-Authority-Reconciliation.md) | N | INV-006/008 qualifying INV-004 — cooperative content immutability, no producer composition authority |
| [SPEC-HAL-012](SPEC-HAL-012-Theming.md) | N | §§3/6/9/11–14 — source distinction, historical-safe purpose, independent renderer admission |
| [SPEC-HAL-328](SPEC-HAL-328-Keyboard-Authority-Focus-and-Capture.md) | N | §§12.2/12.4/12.6/15 — truthful current cues, capture safety, narrow invalidation |
| [SPEC-HAL-120](SPEC-HAL-120-Tree-Expand-Collapse-Animation-Semantics.md) | N | §§7/10/13 — realized member source and explicit motion policy; Design — Approved preserved |
| [SPEC-HAL-263A](SPEC-HAL-263A-Image-Alpha-Contract-Correction.md) | X | §8 relationship only — alpha and closed public Image admission unchanged |
| [SPEC-HAL-085A](SPEC-HAL-085A-Shader-Suite-v2-Binding.md) | X | §9 relationship; §2.1 obsolete Image-deferral status correction only — ABI execution gate unchanged |
| [SPEC-HAL-360](SPEC-HAL-360-Complete-Text-Realization.md) | X | §1 relationship only — complete text and prepared-plan validity unchanged |
| [SPEC-HAL-361](SPEC-HAL-361-App-Frame-Realization-Observation.md) | X | §3 relationship only — optional latest observation distinct from internal outcomes |
| [SPEC-HAL-083A](SPEC-HAL-083A-SceneCache-Dispatch-Mandatory-Wiring.md) | X | Relationship section only — dispatch remains mandatory |
| [SPEC-HAL-164](SPEC-HAL-164-TabularSubstrate-Root-Abstraction-and-Invariants.md) | X | §16 relationship only — closed semantics and frame-local projections unchanged |
| [SPEC-HAL-167](SPEC-HAL-167-TabularSubstrate-Body-and-Row-Projection.md) | X | §14 relationship only — closed semantics and frame-local projections unchanged |

### 14.2 Preserved authority and separate existing contradictions

Constitution SPEC-HAL-002/SPEC-HAL-002A and state separation SPEC-HAL-142/SPEC-HAL-142A remain unchanged. SPEC-HAL-081A/SPEC-HAL-081B/SPEC-HAL-081C/SPEC-HAL-163 retain surface, stack and routing authority; SPEC-HAL-148/SPEC-HAL-149 and existing scroll/anchor, input, accessibility and member owners retain their own semantics. No cross-surface protocol is admitted by vocabulary alone.

SPEC-HAL-046 §§5.2–5.3/6 fixed-step VirtualTime language remains a separate pre-existing reconciliation item; this pair uses SPEC-HAL-021/SPEC-HAL-042/SPEC-HAL-354 and permits no SPEC-HAL-046-derived second clock or catch-up loop. Frozen SPEC-HAL-083 §9 is resolved only for this pair's presented-value path by SPEC-HAL-364 §12; the frozen source is unchanged. SPEC-HAL-271 §§4.1/5–7 paint-time mutable timing cannot override Constitution phase law and is not an admissible control/capture path here; a dependent producer correction requires separate owner authorization. SPEC-HAL-093A/SPEC-HAL-097/SPEC-HAL-097A remain Proposed and supply no adopted default duration or reduced-motion policy. No contradiction-ledger document is amended.

### 14.3 Constitutional compliance obligations

| Constitutional authority | Required conformance |
| --- | --- |
| SPEC-HAL-002 Pillars A/C; §§3.1/3.4/3.8 | One canonical tree/identity, immediate semantic teardown; historical paint is noninteractive. |
| Pillar B; §§3.2/4.5/4.10/5 | Update control; immutable later values; existing phases, no post-layout Timeline mutation. |
| Pillar D and SPEC-HAL-142/SPEC-HAL-142A | Finite explicit data, physically separate state, one-way facts and no input feedback. |
| Pillar E; §§4.7/8 | Deterministic immutable scene; existing diff, dispatch and RenderGraph. |
| §§3.9/4.8/7 | Owned re-realizable text; no retained shaping/glyph/atlas borrows. |
| Pillar F | Platform motion preferences are advisory inputs to the existing framework owner. |
| SPEC-HAL-002A | Explicit subordinate scoped amendments; no constitutional alteration. |

These are obligations for documentary review and later execution admission, not implementation testimony.
