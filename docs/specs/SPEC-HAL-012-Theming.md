# SPEC-HAL-012: Theme Configuration and Resolved Component Appearance

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

**Author:** JC
**Status:** Adopted / Binding
**Adopted:** 2026-10-03
**Implementation status:** The complete contract below is not yet implemented. Existing primitives, theme fields and helpers do not establish whole-contract conformance.
**Layer:** Appearance configuration and immutable value provision to existing component, layout, text and paint owners
**Scope:** Component/part appearance requests, independent owner-qualified facts, coherent resolution, finite decorative composition, runtime skin replacement and bounded inspection
**Authority:** Subordinate to SPEC-HAL-002 and its amendment discipline in SPEC-HAL-002A; accepted semantic, layout, text, animation, resource and rendering owners retain their authority.

**Button material contract and implementation admission.**
Contract adoption does not establish source conformance. The existing renderer
and resource owners must verify producer identity, typed request/publication and
commit transport, precise failure causes, generation protection and the actual
Button content/focus/retained integration before execution is claimed.
Unresolved implementation admission remains unresolved and requires the governing contract owner's
resolution, not executor invention. This correction selects no new source, ABI,
target, runtime, material default or consumer behavior and adopts no Proposed
SPEC-HAL-363/364 clauses.
Verification establishes conformance and evidence; it does not admit another
source, ABI, target, runtime, capability or consumer, or lift an unresolved
admission. Such extensions require a separate governing-owner decision.

## 1. Purpose and authority

**Adopted application-policy qualification (2026-10-14):** SPEC-HAL-328's
UiInteractionPolicy supplies framework-owned navigation-cue eligibility
independently of honest focus/capture facts. Appearance consumes that fact;
PointerDriven omits navigation cues from the first admitted frame while actual
Capture cues, value selection, errors, hover and press remain truthful. Skin
configuration cannot select the interaction policy or change keyboard routing.

Halcyon MUST support product-selected appearance for standard widgets and custom-painted components through one semantic contract:

configuration → declared request/address → complete immutable resolved values → existing layout/text and paint consumers.

A product skin MAY substantially change colors, rectangular surface materials, borders, content appearance and admitted decoration while preserving the component's semantic role and behavior. Appearance does not own widget identity, domain meaning, input, focus, capture, navigation, layout algorithms, resource lifetime or rendering execution.

This revision is adopted together with SPEC-HAL-328's visual-policy amendment. The two documents jointly govern appearance configuration and focus/capture projection within their declared ownership boundaries. A reference to a Draft, Proposed, Design or Deferred document does not adopt that document.

The contract specifies observable behavior and ownership, not a required Rust API, registry, parser, cache implementation or object graph. Existing owner-correct mechanisms MUST be reused where they satisfy it.

## 2. Definitions

A theme configuration is an explicitly selected, coherent set of appearance definitions and configured values. It may be product-supplied or the framework default. A configuration generation identifies a coherent selection for dependency validity; it is not a new clock or semantic widget identity.

A component appearance contract declares a component family's appearance inputs and consumed outputs. A component address identifies that contract. A visual part is a declared paint role within it, such as frame, surface, keycap, indicator or legend.

A variant is an explicit appearance choice admitted by the component contract. A qualifier is another declared, typed discriminator whose meaning and supplying owner are identified by that contract. Neither is an input-routing or semantic-policy instruction.

A state fact is a read-only fact supplied by its existing owner. Framework focus and domain-qualified capture, component enabled/selected/expanded state, and product-specific expected/matched/error state remain facts of their respective owners.

Resolved style inputs are immutable values consumed by the existing measurement/layout/text owner before their use. Resolved appearance is the immutable paint-facing value result, including a finite ordered decorative composition where required.

Component content means the text, symbols, values or other meaningful content supplied and drawn by the component's existing owner. A visual part is not that content and is not a content slot.

Decorative coverage is the area touched by admitted paint primitives derived from owner-supplied geometry. It is distinct from canonical layout, hit-test, navigation, scroll/reveal and accessibility geometry.

An admitted capability is behavior both authorized by its owning accepted contract and implemented for the intended consumer/pipeline/target. A field, enum variant, helper, shader or permissive design document alone does not establish admission.

## 3. Requests, visual parts and identity

Each appearance request MUST identify the selected configuration, component contract, declared part, declared variant/qualifiers and the owner-qualified facts consumed by that contract. Defaults for omitted optional request fields MUST be declared by that contract. Resolution MUST NOT guess an unknown component, part, variant or qualifier.

A visual part MUST NOT create a WidgetId, logical member ID, child widget, pseudo-widget, content slot, input target, focus/capture target, layout node, accessibility node, independently retained region, scheduler or lifecycle.

Many instances may use the same part role. One component may draw many keycaps or rows, issuing the same part request with different owner-supplied facts. This does not require a separate appearance definition for every instance.

Logical member identity remains with the component/model owner. It is not automatically a style selector. If one member needs special appearance, its owner MUST explicitly project that choice into an admitted variant or qualifier. An optional scene/debug correlation identity does not change the appearance contract or give the part semantic authority.

The component continues to own part placement and any mapping from its events/model to logical members. Appearance resolution MUST NOT derive a logical identity from paint coordinates, select a member, or infer event meaning.

Existing independently justified child widgets keep their existing ownership. This contract neither requires nor removes them. The need to style a painted part is not justification to introduce a child widget.

**363/364 scoped amendment:** An appearance component/part address does not designate a capturable
source or create a presentation lifetime. SPEC-HAL-363/364 source address,
occurrence and capture generation are supplied only by the existing component/
member paint owner. Styling a repeated part SHALL NOT allocate a WidgetId,
member identity, retained region, input target or scheduler.

## 4. Independent facts and deterministic composition

Facts that can coexist on an intended supported path MUST remain independently representable. Examples include hovered plus focused, selected plus focused, pressed plus focused, validation error plus capture, and disabled plus selected where the component actually admits that combination.

A universal mutually exclusive state enum MUST NOT silently discard another relevant fact. A local adapter may select one value only when the receiving contract loses no required information, or when the complete resolution carries the other facts independently.

The contract MUST identify each fact's owner and domain. A product qualifier named “focused” or “captured” MUST NOT impersonate framework authority. Keyboard capture, pointer capture and scroll capture MUST NOT be collapsed into one undifferentiated fact. Hover/press appearance does not prove any capture.

For simultaneous facts affecting the same property, the selected component definition MUST declare deterministic composition or precedence. Such precedence resolves visual values only; it never changes which facts are true. Required focus/capture visibility cannot disappear through an error, hover, pressed or selected branch.

Resolution MUST be pure. Identical configuration, address, qualifiers, relevant fact snapshot and admitted frame-stable presentation inputs MUST yield identical effective values and local composition order. Diagnostics and incidental allocation/storage order MUST NOT affect that result.

No requirement exists to enumerate impossible states, invent unsupported input families or validate an unbounded Cartesian product. Coverage is bounded by the declared used contract and its supported fact cases.

## 5. Ownership, phase coherence and retained use

Configuration changes enter through the existing authorized application/update path. The selected configuration, fact snapshot and resolved dependencies MUST be coherent for their consuming phase.

Measurement-affecting style values MUST resolve before measurement/layout consumes them. Existing owners compute canonical geometry. Paint consumes that geometry together with coherent resolved appearance; it MUST NOT silently replace the metric inputs used to produce the geometry.

Resolved values may be computed directly or reused while their dependencies remain valid. This SPEC does not require every clean widget to resolve, lay out or paint on every frame. Retained reuse and invalidation follow SPEC-HAL-354.

Resolution, layout and paint MUST NOT load assets, mutate semantic state, start or cancel animations, execute product callbacks, or modify the WidgetTree. Frame/phase exceptions come only from the owning accepted contract; this SPEC adds none.

Resolved values MUST NOT retain references into transient AppState, text-layout results, borrowed widget state or Timeline storage beyond their permitted lifetime. Pure values, opaque IDs and owner-correct immutable sharing may be used. A new hidden global theme service is forbidden.

The appearance system MUST NOT impose a new batching identity. Renderer batching and cross-pipeline order remain renderer-owned. Equivalent style values do not authorize reordering overlapping draws.

**Scoped §5 qualification — 2026-10-08:** Under the delegated
HAL-GEOMETRY-GESTURE-RETIREMENT-2026-10-08-P2 acceptance, HAL-128's opted-in
geometry-terminal owner may synchronously settle only its cached gesture/capture
presentation from final owner-qualified `FrameworkAppearanceFacts` after normal
Update and complete capture revocation, before Layout. The existing scene owner
then publishes final facts once. This is not another Update or appearance policy;
it performs no timer/animation, layout, product action or other-widget mutation.
Unchanged owners keep coherent normal Update caches. Layout/paint resolution
restrictions, all other §5 rules and §10 focus/capture authority remain unchanged.
Implemented in 0.208.0-beta; independently source-accepted on 2026-10-08.
HAL-128's R4/RV1 subsection records the completed owner-target proof; native and
product integration remain separate.

## 6. Finite composition around component-owned content

Every visual part MAY resolve to a finite ordered composition, rather than exactly one fill and one border. The composition may include admitted underlays, surfaces, borders, decorative overlays, state projections and focus/capture projections around component-owned content.

The component contract MUST declare the available roles, their ordering relative to its content and any permitted optional absence. The actual resolved composition MUST be finite, bounded and deterministic. It contains typed values for known consumers, not arbitrary executable painters, recursive effect graphs, a free-form SceneGraph or a second retained hierarchy.

Content remains supplied and drawn by its existing component owner. Appearance may provide admitted content colors and typography inputs and order decoration relative to those draws; it MUST NOT replace content meaning, add content callbacks or gain text/editing ownership.

The paint/composition owner maps declared local ordering to actual explicit Z and existing traversal scopes. Required front/back order MUST use ordering supported by SPEC-HAL-359. Equal Z or incidental emission order across pipelines MUST NOT stand in for required ordering. Appearance owns neither global Z allocation nor a surface/compositor hierarchy.

Not every component needs every role. An admitted minimal component contract may expose fewer roles when that is its correct bounded design. A complete general skinning implementation, however, MUST NOT advertise the full contract while silently ignoring declared material, content or decorative outputs.

**363/364 scoped amendment:** A paint producer MAY explicitly declare a finite historical-safe
output purpose under its own component contract. It SHALL derive owned values
from honest owner-qualified inputs and identify which current-authority cues
are excluded and why the retained result remains non-authoritative. This purpose
is not a fabricated framework fact snapshot, new visual-state owner or permission
to omit a cue from current/incoming appearance. If safe capture cannot be
certified before flattening, capture refuses or settles canonically.

The appearance result remains a finite component-owned composition.
SPEC-HAL-364 receives certified paint values from the paint owner; this
appearance contract SHALL NOT grow a recursive occurrence graph, general
transform/mask language or renderer registry to carry it.

## 7. Typed surfaces, borders, content and typography

Rectangular surfaces are typed selections consumed within a destination rect already supplied by the component/layout owner.

- Solid supplies the color value required by the existing solid surface path.
- Image supplies an admitted resource reference/current binding, source selection such as UV coordinates, and tint supported by the existing Image path.
- Gradient may be admitted only for the actual supported gradient representation and its clipping/color/target limits. Current linear/radial and bounded-stop implementation is not a promise of arbitrary gradient families or universal rounded-ancestor clipping.
- Stretch NinePatch is an adopted material family under [SPEC-R-019](SPEC-R-019-Nine-Patch-and-Border-Image-Rendering.md) (Adopted / Binding, 2026-10-06; Implemented). Its validated core material, Image lowering and initial square Button surface/underlay profile are implemented, with retained-resource and production Image shader visibility proof.

There is no universal Brush, shader program, material graph, filter graph or generic renderer capability implied by this vocabulary.

Border/stroke/radius/shadow and other decorative values MUST correspond to admitted consumers. A two-opaque-rectangle border technique MUST NOT be treated as a transparent border over Image content where it would overwrite that content. Composition must preserve the chosen material's actual semantics.

Content colors, icon/text tint and typography values may be supplied only to the consumers that support them. Measurement/shaping attributes are style inputs under §8; paint-only attributes remain paint-facing values. A skin MUST NOT change a label's text, an icon's semantic role, a training result, a selected value or an editing model.

SPEC-HAL-208 remains binding for labeled controls: indicator and label are one semantic control; no synthetic label widget or separate event sink is introduced; indicator-before-label order and existing text-engine/font restrictions remain. A visual label role does not authorize typography effects that the owning control contract prohibits.

Coordinates and device conversion remain governed by SPEC-HAL-024. Pipeline-specific color/alpha contracts, including SPEC-HAL-085A and the Image amendment SPEC-HAL-263A, remain authoritative. This SPEC neither selects one universal straight/premultiplied convention nor performs an extra sRGB, alpha or DPR conversion.

### 7.1 SurfaceFill value and initial execution profile

**Procedural Button material ownership and publication.**

A procedural Button material is a typed paint projection supplied through the
existing appearance and renderer/resource owners. Existing prepared product
appearance and selection precedence remain controlling. Button owns no GPU
resource or second application consumer API.

The Button publishes immutable material input in its single ordinary Update.
Equivalent input retains its checked request revision; changed input increments
it, and overflow rejects without wrapping. The engine qualifies the request with
the current owner/mount lifetime, SurfaceId and the immutable SPEC-HAL-266A target
basis. Previously admitted private static sources and their material ABI remain
under their existing renderer ownership and capability limits. This contract
does not admit arbitrary shaders, additional targets or a new SurfaceFill family.

Material projection may consume real Button-owned press state, never focus as
a substitute for press. Immediate endpoint changes use the existing pointer and
keyboard press/release/cancellation lifecycle. No new input owner, activation,
capture semantics, Timeline handle or continuous animation follows. Hover and
required focus remain independently truthful. Equivalent input must not churn
resources or request revisions merely because another frame occurs.

**Rounded gold Button material — Adopted / Binding, 2026-11-18.**
The public selector `ButtonSdfRestingProfile::ChampagneV4` requests this profile
through the existing explicit Button request, subordinate to prepared appearance.
The renderer owns one private fixed gold source with linear-light interpolation
of the sRGB stops `#F1D1A0` at 0.00, `#EBC597` at 0.22, `#D3A776` at 0.66
and `#C69662` at 1.00.
The face has a localized upper-left sheen and a warm inset edge. The Button
draws its caption in `#0A1117` over the admitted face. The existing focus cue
remains independent of material readiness and above content. Its outline uses
the same 8-logical-unit radius as the face.

**Scoped enabled continuity amendment, 2026-11-18.** Update selects one
immutable Enabled content basis or a separate Disabled basis from actual
availability. The Enabled generation contains four complete cells for Rest,
Hover, Rest with Pressed, and Hover with Pressed. Hover brightens the existing
sheen and rim. Both pressed cells retain the darker lower valley and shallower
rim. The entire Enabled generation is admitted atomically.

Ordinary Update publishes actual hover and press in the separately checked
interaction revision. Pre-Paint selects their current UV cell under the same
certified K. Hover, press, release, and cancellation change only that projection.
The same Ready face binding and dark caption continue through each transition.
Disabled uses one softer static face and preserves HAL-112 inertness.
Availability changes use ordinary staged admission. When the rounded gold material
is pending or unavailable, ordinary legible content and the rounded focus cue remain.
Animation samples stay outside the material key and producer inputs.

This scope supersedes the earlier two-cell description of the rounded gold
profile in HAL-295A §3.3. The same private producer and registrar certify the
whole texture and retain its exact writer, CPU and GPU sampling custody.
Other admitted endpoint representations follow their existing contracts.

**Prepared material endpoints.**

These endpoint-pair requirements apply only where the existing mechanism is
already admitted. They neither require nor admit endpoint pairs on other
material paths.

For the existing preadmitted endpoint mechanism, the stable resource-material
basis revision is distinct from the checked interaction request revision.
Ordinary Update publishes each actual press/release/cancel change. Each revision
retains its existing independent validation and purpose; the interaction revision
alone changes neither resource basis nor K. The resource owner separately records immutable content basis C and original generation
admission as defined by SPEC-HAL-266A's scoped procedural-content qualification.
C cannot replace K or establish target eligibility.

The resource owner prepares complete Rest and Pressed endpoints in two fixed,
non-overlapping cells of one finite offscreen texture. Each cell preserves the
canonical face F, derived finite effect domain, AA/shadow support and exact
target-scale mapping. The existing authenticated admission and generation
custody protect the whole texture's one current ImageBinding identity/generation.
For the rounded gold profile's Enabled basis, the same whole-texture requirements
cover all four fixed cells described above.

At the ordinary pre-Paint cutoff, the current Update-published pressed value
selects exactly the corresponding endpoint UV. This selection creates neither a
new resource generation nor a new GPU admission. A warm current pair projects
that endpoint in the first affected retained Paint without an intermediate old
endpoint or a preparation/completion wait on the press transition.

Withdrawal, revocation or a changed profile/theme, geometry, target or mount
basis ceases stale Image projection and requires fresh evidence for the complete
current K. Fresh resource production and admission remain required unless the expressly scoped unchanged-content qualification in SPEC-HAL-266A certifies current eligibility for an already admitted, still-valid immutable generation. That qualification does not alter original admission or authorize cross-basis last-good fallback or reassigned credentials. Exact writer, CPU and GPU sampling custody and certified retirement apply to the whole pair, including old compiled and cancelled plans.
Initial optional activation and all out-of-scope replacements retain their staged frame order. Only SPEC-HAL-266A section 4's already-admitted explicit-request F-size replacement exception may prepare and genuinely admit a fresh complete pair in the same attempt before Paint; changed content still receives new C/G and complete current-K evidence. Content, focus, canonical layout/hits, typography, on-release activation and strict late-failure checks remain with their existing owners.

These are requirements on the existing admitted mechanism, not a material
selection, default-style decision, generalized shader/cache API or new consumer
admission. Appearance configuration, execution capability and actual resource
availability remain distinct under §§1, 11 and 12.


`SurfaceFill<Source>` is the shared, geometry-independent rectangular fill value:
Solid, the existing bounded Gradient, Image, or validated Stretch NinePatch. The
Image source parameter preserves the source owner's typed address/binding
boundary; it does not provide
lookup, loading, callbacks or a new resource registry. Appearance uses this same
value with `AppearanceImageSource`, retaining the distinction between an authored
resource reference and a current registration-number shape. Neither shape alone
proves renderer/resource admission.

SPEC-R-019's NinePatch extension to this same `SurfaceFill` is implemented as the
immutable validated `SurfaceNinePatch<Source>`, carrying relative normalized
source splits, logical destination border insets and existing Image tint/binding
semantics. Both PaintSink source paths lower it to at
most nine ordinary Image primitives. The initial square Button profile admits
current ImageBinding evidence with zero own radius; no new material graph or
resource owner is implied.

`SurfaceGradient` SHALL freeze the existing linear/radial `GradientDef` only
after checking 2–8 ordered stops, finite normalized offsets/endpoints, and finite
unit-range straight linear color channels. Its immutable storage may be shared;
cloning a Solid value MUST NOT allocate or acquire Gradient/Image dependencies.
Existing `Color` and Image tint semantics remain unchanged.

The rectangular PaintSink helper SHALL dispatch the selected fill directly to
the existing Rect, GradientRect or registered Image primitive. It SHALL NOT
resolve policy, register/upload a resource, add a pass, synthesize a fallback,
or change the caller's canonical clips or geometry. Image use requires a current
binding already admitted by its existing resource owner. Own rounding, borders
and finite decoration order remain the component/composition owner's inputs;
this helper does not imply rounded Image execution or a universal clip profile.

The initial Button consumer admits Solid and the linear/radial Gradient profile
of SPEC-HAL-085A §6.8, including its existing own radius and current effective
parent clipping. Ordinary Image and Stretch NinePatch use the square admitted
ImageBinding profiles in §12 and SPEC-R-019. Semantic resolution alone still
makes no execution-conformance claim.

Solid-only consumers MUST retain their existing primitive path without new
material allocation, traversal, resource preparation, GPU passes or dependency
tracking. Optional material storage and lowering are paid only by selected
material use. This amendment creates no Theme, ControlChromeSpec,
DecorationStack, interaction or animation authority. NinePatch authority is
supplied separately by adopted SPEC-R-019; its core lowering and initial square
Button profile are implemented without generalized consumer admission.

**Button material status.**

The procedural Button material is a typed Button-owned paint projection,
separate from its canonical allocation and explicit prepared product appearance,
whose precedence remains unchanged. Let K be the complete current eligibility
key consisting of material-owner/mount identity, SurfaceId, material revision
and the immutable current SPEC-HAL-266A target-metrics basis. Every existing
field retains its meaning and equality requirements. A changed target extent
changes K; no field may be omitted or normalized to equate it with an earlier K.
The engine commits K atomically with exactly one outcome: Ready or Unavailable.
Immutable content basis C and original generation admission remain distinct
from K. These are contract meanings, not declarations of a Rust API.

Ready carries the actual current ImageBinding identity/generation, verified
availability and a resource-owner guarantee that its published front remains
valid and unmodified for sampling. A queued texture ID or superseded generation
is not Ready. Unavailable carries an explicit resource-owned cause for K and,
for revocation, the identical withdrawn image identity/generation. Existing
Unregistered, Replaced, RendererDestroyed and DeviceLost meanings are preserved;
a failure before binding issuance is not mislabeled as a binding revocation.
Intentional optional absence lies outside this requested-material status and
is not a third outcome.

Ready for a changed K requires fresh current eligibility evidence. Only
SPEC-HAL-266A's scoped unchanged-content qualification may make a still-admitted
immutable generation current for that K without another resource generation.
The original producer admission remains unchanged and is never itself evidence
for the new K. A superseded, replaced or revoked generation cannot be revived this way.

Initial activation without required current evidence SHALL reject atomically
without replacing the previous valid requested selection. A current request
committed as Unavailable SHALL omit its stale material Image and expose
MATERIAL_UNAVAILABLE and its genuine resource-owned cause separately from
intentional absence, while retaining Button content, independent FocusCue,
input semantics and canonical layout/hit geometry. It SHALL NOT synthesize
Solid/default/NinePatch, reuse a last-good Image, hide the Button or report
material success. Preserving an old requested selection does not authorize its
stale projection. Missing or incompatible current eligibility is rejection of
evidence; it SHALL NOT fabricate producer failure or binding revocation.
Only the scoped active-size replacement Pending case in SPEC-HAL-266A section 4 defers dependent target commitment/Paint/output before a new material outcome is frozen. Pending output is not a third material status, producer failure or binding revocation. Genuine Unavailable continues to expose MATERIAL_UNAVAILABLE and its actual cause and retains the optional omission behavior above. Initial activation and intentional withdrawal are unchanged.
Recovery from actual resource unavailability requires a new successful producer
publication and newly admitted current generation matching K under existing
failure rules. This does not grant recovery for any failure class whose owner
does not admit it; baseline device/queue loss remains subject to
SPEC-HAL-295A INV-295A-009. A change covered by SPEC-HAL-266A's scoped
unchanged-content qualification with a still-valid generation instead establishes
fresh current eligibility; it is not resource recovery.
Obsolete results, certificates and failures SHALL NOT replace newer commitment.

## 8. Layout-affecting configured values

**Editable value typography — Adopted / Binding scoped default decision, JC,
2026-10-25:** the existing theme typography owner provides a separate
`input_font_size`, default 14 logical units, for TextField values, placeholders
and password masks, TextArea/TextEdit document text, and the editable numeric
value regions of SpinBox and Slider. Embedded editors and their host's value
projection/cache SHALL consume the same token. ComboBox editable input and ghost
completion use it; popup options and non-editable selection text retain the
ordinary font. The general UI `font_size` remains 13: surrounding labels, section
headings, Button captions, tooltips and other UI text are not enlarged.

This is a small value-text default refinement, not a global scale or adoption of
Draft 092B's role taxonomy. Existing density recipes, line height 20, baseline
correction, padding and control-height policy remain unchanged; Intermediate30
single-line inputs remain 30 logical units at 1x/2x. Layout/cache, shaping,
selection, caret, password slots and Paint SHALL use coherent input metrics under
their existing owners. A font change requires normal text/layout invalidation,
never a paint-only font substitution or a per-widget optical correction. Existing
explicit product configuration remains controlling.

012 owns provision and coherence of configured values consumed by layout, not their interpretation or canonical geometry. Fonts and shaping attributes, padding, gaps, control dimensions and other metrics retain the meaning, constraints and ownership assigned by the component's accepted layout/text contract.

A changed configured value MUST be classified by its actual effects under SPEC-HAL-354. Color or Image binding changes at a stable allocated extent can be PaintOnly; changed measurement/shaping inputs require the corresponding text/layout invalidation. Unchanged effective dependencies require no new work.

A field called style, border width, radius, shadow, density or image is not automatically paint-only or layout-affecting. The classification follows how the intended consumer uses it. PaintOnly is valid only while constraints, measurement, child geometry, scroll extents, structure and other relevant dependencies remain valid.

Focus/capture projection MUST NOT change measurement, padding, fonts used for measurement, canonical bounds, topology or layout participation. With unrelated layout inputs unchanged, focus/capture appearance changes remain paint/composition changes. Other state-dependent metric changes are permitted only where the existing owner contract explicitly admits their input and invalidation path; appearance does not create that authority.

An image's source dimensions MUST NOT silently resize a widget. An intended destination-size change is an explicit input to the existing layout owner. Bitmap pixels, configured logical extents and target device scale are separate quantities.

Instance properties retain their existing owner and precedence contract. Theme configuration MUST NOT silently replace an explicitly owned widget property. No second numeric geometry schema, universal token grid, late paint metric override or new layout solver is introduced.

## 9. Decorative overflow, clipping, scrolling and ordering

Admitted decoration MAY cover pixels outside the component's allocated rect. This includes supported stroke expansion, halo, shadow spread or offset. Such coverage is derived paint geometry, not another canonical widget rect.

Decorative coverage MUST NOT enlarge or move layout bounds, child placement, hit regions, spatial-navigation candidates, scroll extents, reveal targets or accessibility geometry. Decorations add no tooltip triggers, routing scopes, event interception, focusability or input overlays.

All decoration remains within the existing paint/composition owner's admitted clip, scroll, surface and main/overlay scope. A skin MUST NOT change OverflowMode, pop an ancestor clip, escape a viewport, borrow another surface or choose an unlimited global Z to expose an effect. A decorative overlay is not permission to use an interactive popup/overlay path.

Local part ordering does not determine parent/child traversal authority. An effect may use an already admitted owner decoration placement only when that placement actually supports its clipping and ordering requirements. A hidden, clipped or nonpaintable component does not acquire rendering eligibility through decoration.

Emission, retained replacement, and any culling or damage calculation already performed by an owner MUST account for the actual admitted effect coverage, including removal of old decoration. This requirement does not mandate a new bounds service, OS partial-damage implementation or region runtime.

Generic subtree transforms, rotation/pivot, group opacity, masks and blur are not inferred from decorative overflow. Any such operation needs its exact accepted owner and implemented consumer. Geometry transitions remain under SPEC-HAL-078/145/147; the compositor under SPEC-HAL-150 receives authorized values and does not invent positions.

**363/364 scoped amendment:** SPEC-HAL-363/364 projection needs its own complete source, coverage
and execution admission. Successful appearance resolution does not admit
historical capture, group opacity, affine transformation, mask execution or
resource lifetime. Canonical clips, layout metrics and required current cues
remain governed by their existing owners.

**Button material coverage.**

The material face SHALL coincide exactly with allocated F without changing
metrics, content baselines, child placement, hits, navigation or focus geometry.
Its finite Image domain E=(x-L,y-T,w+L+R,h+T+B) is derived from F and the material's
validated supported outsets. The renderer/resource owner supplies the admitted
target mapping and validates execution limits; fixture sizes and experimental
calibration values are not layout or appearance policy.

Source UV spans SHALL preserve the exact face mapping. Stretching the padded
texture into F or cropping away required effect coverage is forbidden. The
ordinary Image quad has zero own radius; material pixels do not imply a new
rounded-Image execution capability.

All E coverage remains under existing ancestor clip, scroll, surface and viewport
scopes, with strict shadow < face < content < focus order, including internal
shadow/face order in a baked material Image. Replacement/removal SHALL account
for old and new E in existing retained/culling/damage paths. No global dirtiness,
clip escape, new layout extent or OS partial-damage guarantee is inferred.

The rounded gold profile uses an 8-logical-unit rounded distance field on F for face coverage
and its inset edge. Its finite E carries one physical pixel of antialias support
around F. The profile admits every finite effective target scale in the inclusive
range 0.5 through 4.0 and a face large enough for radius 8. The exact target ratio
governs the SDF, physical storage and UV crop. Whole-pixel storage padding and
cell alignment leave F and E at their original logical positions.
Unsupported scale or geometry is an explicit material refusal before GPU
allocation. V1 and V2 retain 1x/2x admission; V3 uses the scoped 2026-11-23
scale extension below without changing the 1x/2x source or output.
No decorative shadow, layout change or new clip capability is required.

## 10. Focus/capture appearance under the paired 328 amendment

Under the paired SPEC-HAL-328 amendment, a selected skin MAY modulate existing primitives and add a finite admitted composition of halo, glow, ring, image treatment or decorative overlay for focus/capture. Primitive count, type, resting visibility and relative Z need not match resting appearance.

SPEC-HAL-328 remains the sole policy boundary for truthful focus/capture projection. The framework supplies authoritative domain-qualified ownership/mode facts and lifecycle transitions; the component supplies only its already-authorized internal target and geometry. The selected skin supplies visual values and composition, not ownership.

`Button::with_focus_cue_modality(FocusCueModality::KeyboardOnly)` applies the
per-Button navigation cue rule from HAL-328 §12.4. Resolution uses the current
framework keyboard qualification with the same admitted stroke and geometry.
The default Button continues to show the cue whenever navigation focus is eligible.

The complete effect is one coordinated projection through the component's declared paint path, optionally using shared helpers. The framework MUST NOT append an independent default indicator over that complete projection. Multiple primitives are not multiple semantic owners.

The current focused owner MUST remain visibly identifiable where 328 requires indication, and current capture MUST remain distinguishable from focus-only indication. Simultaneous error, hover, press or selection cannot suppress those guarantees. Required labeled-control enclosure covers indicator, label and gap subject to 208 INV-D3 and 328, including JC's 2026-10-24 default-selector exception.

Presentation may use the existing Timeline under §14 and 328. It MUST NOT delay the only current-owner cue, display an old owner as still current, mutate semantic state or drive layout. Outgoing decorative tails are allowed only under 328's explicit truthfulness and lifetime rule.

Theme::control_border remains a useful default bordered-control adapter. It is not the engine's universal visual ceiling or focus owner. A default recipe that masks required focus/capture under its error branch needs a complementary truthful cue; retaining the helper's signature does not excuse invisible ownership.

## 11. Completeness, capability admission and fallback

For each declared used request and supported supplied fact case, resolution MUST produce either:

1. complete values and composition whose required capabilities are admitted; or
2. an explicit deterministic unresolved result identifying the unmet contract.

Declared defaults and fallbacks may produce the complete result. Their selection order, applicability and outcome MUST be deterministic and inspectable. An intentionally absent optional decoration is complete only where the contract permits absence. Absence cannot silently erase required component content or focus/capture indication.

Unknown addresses, missing required values, unsupported required primitives, invalid ordering/clip requirements and unavailable required bindings MUST NOT be accepted by guessing another part, ignoring the field, silently substituting a different material or returning partial appearance as success.

A product may explicitly approve a bounded fallback, such as an admitted Solid treatment instead of a requested Image or an intrinsic cue instead of a halo. The fallback must still meet semantic visibility and composition requirements and must be reported as the selected fallback. A product may reject fallbacks it has not approved. An implicit “best effort” skin is not a complete result.

Implementation support MUST be stated for the actual pipeline, target and composition scope. In particular, this contract MUST NOT claim group opacity from a Layer field that is not executed, masks from a clip-shaped value, backdrop blur from a shadow, generic transforms from coordinates, or universal rounded clipping from a primitive's own corner radius.

A complete declaration does not by itself establish resource availability. Style-resolution failure and renderer resource/binding failure MUST remain distinguishable under their respective owners. Missing style cannot be disguised as an image placeholder.

A configuration activation boundary MUST validate the declared used contract and reject unresolved required appearance, or apply an explicitly approved complete fallback configuration. It MUST NOT install a knowingly incomplete skin as successful. This does not invent a global error UI or asset recovery system. A later resource failure follows the owning resource contract and the skin's declared applicable fallback, if any; it does not authorize fabricated resource success.

Strict validation MUST distinguish explicit coverage, permitted declared fallback, valid optional absence and unresolved required appearance for the declared used component/part/variant/qualifier and supported state cases. The supported set is bounded by the intended integration. This is not a requirement for a stylesheet parser, exhaustive unsupported-state matrix or independent validation service.

**363/364 scoped amendment:** Appearance resolution, historical-source certification and renderer/
resource execution are distinct admissions. A successful result here does not
replace the SPEC-HAL-363/364 complete-source and operation outcome.

## 12. Images: authored identity and current binding

An authored skin may name an opaque resource reference supplied by its application/asset owner. A current renderer registration number MUST NOT be required to act as a portable or persistent authored asset identity.

Pure appearance/layout/paint consumers receive the current admitted binding from the existing resource owner. They MUST NOT open files, fetch, decode, upload, register, reconstruct GPU resources or run resource discovery during traversal.

Already-bound programmatic Image values are valid on an explicitly declared path. They do not establish a generalized durable asset namespace, automatic density selection, hot reload or device recovery.

The ordinary Button Image profile admits `SurfaceFill::Image` with a current
SPEC-HAL-263 §5.3 `ImageBinding`, explicit normalized UVs, straight-linear tint,
and zero own surface radius. The existing Image pipeline and ancestor clipping
execute it; no new rounded Image surface capability is implied. A raw
registration-number-shaped value or opaque authored reference remains pending
resource admission. A nonzero own Image surface radius remains pending execution
capability. Activation SHALL reject unavailable required evidence atomically;
pure resolution SHALL NOT poll its mutable availability. Subsequent revocation
uses the resource owner's INV-IMG-005 law and narrow SPEC-HAL-354 dependency
invalidation, never another generation or an implicit Solid fallback. This
profile does not admit historical Image content under SPEC-HAL-364. Adopted
SPEC-R-019's initial square Button NinePatch surface/underlay profile is implemented
and admitted with current ImageBinding evidence and zero own radius on the same
Image path. NinePatch border insets are paint-only, never Button padding or
measurement inputs.

The producing application/asset contract owns the mapping from authored reference to current image representation. SPEC-HAL-263/263A/263B and the existing renderer own admitted binding, sampling, alpha/color association and registration lifetime; SPEC-HAL-040 retains device/resource execution authority. SPEC-HAL-266A remains the owner for its existing target-bound custom presentation path. No such identity becomes skin-owned GPU state.

Source density and representation metadata belong to the asset owner. Source pixels, logical destination size and target DPR MUST remain distinct. Selecting a higher-density source MUST NOT silently alter layout or apply device scale twice.

SPEC-HAL-063 remains a Design/Not Implemented candidate for future generalized image-loading/asset authority. This revision does not adopt, revise or implement it. SPEC-HAL-263B's same-device registration persistence is not a substitute for that missing asset contract.

**363/364 scoped amendment:** Authored Image identity, current appearance binding and retained
historical content-generation binding are distinct. The resource owner supplies
the latter under SPEC-HAL-364/040/263A/263B/295A; appearance SHALL NOT preserve a
stale current registration number as a lease.

**Button material binding.**

The procedural Button material consumer follows §7.1's committed status and §9's
derived coverage. It does not weaken ordinary Image/NinePatch admission profiles,
current-generation checks or the pure-resolution ban on mutable availability
polling.

## 13. Runtime skin replacement

A supported runtime replacement MUST preserve stable widget identity, component semantics, focus/capture ownership, selection, navigation and content ownership. It changes configuration through the existing authorized update path.

The owner MUST publish a coherent new configuration and invalidate all actually affected dependencies. Paint-only changes invalidate appearance/local fragments and necessary composition reuse. Metric changes invalidate the relevant measurement/text/layout dependencies and paint. No-op effective changes require no new work.

Paint MUST NOT combine old layout-sensitive values with a newly selected incompatible appearance generation. Relevant retained fragments cannot remain valid merely because a WidgetId did not change.

Skin replacement MUST NOT require reconstructing the WidgetTree as a styling mechanism, and MUST NOT promise that every replacement is paint-only. Image selection, control spacing or typography are classified by actual effects, not by being inside a theme object.

Replacement during a live presentation effect MUST cancel or retarget owner-bound handles under 021, preserve truthful current ownership under 328, invalidate obsolete decorative coverage and release resources only through their existing lifetime owner. It cannot leave stale fragments or anonymous frame demand.

This section does not introduce a ThemeManager singleton, inter-theme animation runtime, authoring hot reload or persistence format.

**363/364 scoped amendment:** Historical values may coexist during replacement only as separately
certified SPEC-HAL-364 paint generations. They SHALL NOT provide old metrics or
facts to current layout/paint. Current configuration still changes coherently
through Update; §8 ownership and explicit instance precedence are unchanged.

## 14. Presentation timing and lifecycle

012 may supply admitted presentation endpoints and transition parameters. SPEC-HAL-021 owns generic interpolation, payload admission, handles, exact same-frame/terminal sampling and Once/Loop/Alternate playback. SPEC-HAL-042 owns the frame order and captured engine time; the existing scheduler owns frame admission. SPEC-HAL-147 remains the separate post-layout geometry-transition authority.

A skin MUST NOT provide its own clock, elapsed-time accumulator, timer restart loop, Timeline instance, renderer sampler or autonomous animation embedded in SceneGraph nodes. A new property graph, spring system, keyframe engine, pause/resume system or general animation DSL is not implied.

Event/update processing may start, replace and cancel admitted owner-bound handles through existing capabilities. Update publishes immutable presentation values where later phases need them. A paint-owned finite read may use 021's read-only sampling allowance; paint cannot write cached values, clear handles, cancel playback or advance time. Lifecycle bookkeeping stays in an authorized mutation phase.

The implementation MUST distinguish active, completed, cancelled, missing and type-mismatch outcomes according to 021, including exact terminal observation. It cannot retain an accidental preterminal value or silently treat an invalid handle as successful completion.

Once is the default. Loop or Alternate requires an explicit admitted descriptor and finite count or explicit infinite lifetime. Infinite playback needs an identifiable owner and cancellation when the effect ceases, the owner unmounts or configuration replaces it. Actual Timeline support is not permission for indefinite focus tails or another scheduler.

Owner initialization is explicit; there is no implicit mount animation or assumption that every neutral value is zero. Retargeting uses the current authorized value and 021 cancellation/replacement semantics. No handle survives anonymously after its owner or target lifetime ends.

Invalidation follows actual effects under 354. Settled values create no animation demand. A live admitted effect may report demand to the existing scheduler; it does not wake the platform itself. Focus/capture timing additionally obeys 328's immediate truthful-cue and outgoing-tail rules.

**363/364 scoped amendment:** Coordinated occurrence timing remains SPEC-HAL-021-owned and
SPEC-HAL-363-controlled. This does not grant post-layout Timeline mutation,
move framework facts into layout or expand Button typography ownership.

## 15. Standard controls and custom-painted components

Standard widgets and custom-painted components use the same request/value boundary while preserving their own existing behavior and geometry contracts.

ControlChromeSpec may remain a specific resolved representation for control chrome. It is not the universal appearance container for all components, and ControlChromeState is not the universal model of independent facts. A complete consumer must preserve relevant facts and consume the applicable surface, border, content and decorative outputs.

DecorationStack may remain a rendering helper where its supported sequence and values match the resolved composition. It owns no lookup, state precedence, focus authority, resource lifecycle or general composition semantics. Its current color-oriented/background-hover-focus shape does not define the full 012 contract.

A custom component may expose repeated frame/keycap/legend or analogous roles without adding semantic nodes. The component supplies geometry, meaningful content and owner-derived facts. Product domain meaning remains outside Halcyon appearance.

Non-normative examples:

- A Button can use a default solid surface or a registered-image product surface in the same allocated rect, keeping activation and focus eligibility unchanged. Its complete focus recipe may add an admitted halo under the adopted SPEC-HAL-328 amendment.
- A hypothetical KeyboardView can draw many keycaps and legends inside one widget. FTL supplies expected/held/matched facts and key meaning; the component supplies placement. A keycap part request does not create a framework focus target, training state or a runtime region. This example is not a claim that KeyboardView exists in the current source.

## 16. Default visual policy and optional authoring frontends

Framework defaults MUST be expressed as default configuration or a specifically scoped default policy. A professional default may choose restrained colors, solid fills, small radii and intrinsic borders. Such choices MUST NOT be misrepresented as a universal limit on lawful product skins.

SPEC-HAL-332 remains Proposed. This revision does not adopt its charter, radius caps or no-glow policy as engine law. A later adopted default/product charter must identify its own scope. Accepted widget geometry and semantic constraints still bind every skin.

SPEC-HAL-072, 073 and 074 remain optional, unadopted authoring/composition/optimization directions. Revised 012 MUST be usable independently of them. A future frontend may author the values, variants, qualifiers, facts and composition admitted here, but cannot override this contract or its owning authorities. No inheritance/cascade rule or syntax choice is made here.

SPEC-HAL-096A remains Draft. Reusing its implemented helpers does not adopt its policy, fixed stack as a universal model, independent state assumptions or conflicting focus/render rules. Helper changes must consume the adopted 012/328 result under the actual rendering owners.

Product semantic colors and meanings remain product-owned. A skin may present a training outcome or financial value; it cannot compute, infer or change that meaning.

## 17. On-demand inspection

An implementation claiming this contract MUST support an on-demand coherent description of the actual inspected appearance result. It MUST expose:

- selected configuration identity/generation and component/part address;
- variant/qualifiers and the relevant owner-qualified fact snapshot;
- effective layout-sensitive and paint-facing values as applicable;
- actual ordered composition and admitted capability/scope outcome;
- explicit coverage, declared fallback, optional absence or unresolved result; and
- relevant resource-reference/current-binding outcome without claiming asset lifecycle ownership.

The description MUST correspond to one coherent snapshot, not merge facts from different frames or configuration generations. It must describe the result used or the failed request, not merely list authored defaults.

Existing diagnostic/agent mechanisms may carry this description. This requirement does not create a new inspector runtime, transport, UI, accessibility tree or mandatory per-frame trace.

Source locations, explanatory strings and optional resolution provenance MUST NOT enter semantic address identity, hot-path cache keys or batching identity. Enabling diagnostics MUST NOT change rendering or require per-frame provenance allocation. Actual configuration/fact/value dependencies remain necessary for correct retained validity.

## 18. Adoption and implementation conformance

Authority adoption and implementation completion are separate decisions. This revision and the corresponding SPEC-HAL-328 visual amendment are adopted as one authority unit, including the consequential 328 paragraph and index corrections. Adoption does not establish implementation conformance; existing source, a demo or a green test cannot substitute for the proof below.

A first implementation claim MUST declare its intended component/part/material/target coverage. It may claim only the behavior actually completed. It MUST provide proportional evidence for:

1. a framework default and materially different product skin using the same component contract with preserved identity and semantic behavior;
2. deterministic complete results and simultaneous owner-qualified facts on intended reachable cases, including required focus/capture plus another state;
3. the custom one-widget/many-repeated-parts contract without additional widget/input/focus/layout identities;
4. both intrinsic and additional-primitive focus/capture recipes under the adopted 328 amendment, with visible/distinct truthful ownership, required enclosure subject to 208/328's scoped default-selector exception, admitted clipping/order and no semantic or layout mutation;
5. paint-only replacement and metric-affecting replacement producing correct invalidation and retained reuse;
6. the intended Image binding/color/alpha/order path and honest reporting of any unsupported material/clip requirement;
7. intended animation retarget, exact completion, cancellation, theme replacement and unmount without stale ownership decoration or orphaned frame demand; and
8. bounded strict validation and coherent on-demand inspection of coverage, fallbacks and capability/resource outcomes.

Use existing test, diagnostic and rendering proof mechanisms. Pixel/visibility proof is required where that is the claimed behavior; a structural test alone cannot establish perceptibility. Tests remain bounded by the implemented intended paths and confusable ingress that protects them. This contract does not authorize an unsupported-space audit stream or exhaustive hypothetical-state matrix.

No new backend, generalized image loader, NinePatch implementation or full widget migration is implied by adoption. An implementation cannot claim omitted parts of the contract as supported.

## 19. Explicit exclusions

This revision does not authorize a new SPEC number, generic semantic runtime, second scene/region hierarchy, universal material/effect graph, clip/Z scheduler, renderer registration owner, asset manager, 063/R-019 implementation, parser/DSL/cascade, authoring hot reload or new animation/frame system.

It does not redesign RadioGroup, choose internal navigation targets, add mobile/touch/gamepad support, implement accessibility, alter product data models, turn display-only widgets into controls, or adopt lower-status specification families.

The contract provides coherent appearance values to existing owners. Any capability beyond their accepted and implemented boundaries requires its exact owning authority and proof before it can be advertised.

## Scoped contract adoption - 2026-10-28

**Authority status:** Adopted / Binding scoped amendment.
**Owner decision:** JC approved exact coordinated proposal SHA-256
`1e06d45b3ae2f21c576acf2fea8557287ae58e1d2ef6ff5727cba452d72ed169`
(38,130 bytes). The literal amendments incorporated in SPEC-HAL-012,
SPEC-HAL-266A and SPEC-HAL-295A are adopted together for the same-owner,
same-SurfaceId, window-extent-only immutable-content C/current-full-K
eligibility extension.

**Implementation status:** Contract adoption only. This records no source
conformance, implementation admission or runtime acceptance. Implementation
requires separate explicit governing-owner authorization and the scoped proof
obligations before implementation acceptance or execution claims.

**Preserved history and status:** Existing adoption dates, historical records
and all unrelated rules retain their status. Proposed SPEC-HAL-363/364 clauses
remain inactive. Later proposal revisions and the proposal's narrative or
experimental evidence are not adopted by this record.

## Final scoped contract adoption - 2026-10-28

**Authority status:** Adopted / Binding final scoped amendment.
**Final owner decision:** JC approved exact current coordinated proposal SHA-256
`76c044622fe37d7c9e6222e177342907268ddf5947bb77a35201357b31921248`
(38,084 bytes). Its literal SPEC-HAL-012, SPEC-HAL-266A and SPEC-HAL-295A
amendments define the current adopted same-owner, same-SurfaceId,
window-extent-only immutable-content C/current-full-K eligibility extension.

**Supersession and history:** This final adoption supersedes all earlier
proposal-hash instructions and the earlier
`1e06d45b3ae2f21c576acf2fea8557287ae58e1d2ef6ff5727cba452d72ed169`
scoped selection and wording. The earlier record and historical proof remain
preserved as history; they are not alternate current authority for this scoped
extension. No other proposal revision or experimental narrative is adopted.

**Implementation status:** Normative contract adoption only. This does not grant
implementation admission or establish source conformance, runtime acceptance
or a resize-fix claim. Implementation remains subject to separate explicit
governing-owner authorization and all applicable proof obligations.

**Unaffected authority:** Original dates and unrelated binding requirements,
existing proof conditions and historical records retain their status. Proposed
SPEC-HAL-363/364 provisions remain inactive.


## Current-placement eligibility contract adoption - 2026-10-28

**Authority status:** Adopted / Binding scoped coordinated owner amendment.
**Owner decision:** JC's owner direction on 2026-10-28, authorizes adoption
after independent consistency validation of the exact proposal
`Button SDF current-placement eligibility amendment`,
SHA-256 `5d92d6d33288f0e4e01414076dc6803d21f7aef5fe50cdbaffc6ccdea3749878`
(15,585 bytes). That validation passed against the current Constitution and
SPEC-HAL-012, SPEC-HAL-024, SPEC-HAL-266A and SPEC-HAL-295A.
Only the packet's eight literal coordinated replacement blocks are adopted
into SPEC-HAL-012, SPEC-HAL-266A and SPEC-HAL-295A. Its narrative, pre-flight,
measurements and draft status do not become independent framework authority.

**Scope:** The existing same-owner, same-SurfaceId, active-target procedural
Button generation may receive fresh complete-current-K eligibility across
window-extent changes, fixed-size F translation and consuming-viewport changes
only under the unchanged-content and current-placement qualification above.
Current destination, viewport, physical coverage, clipping and attempt
freshness remain distinct from immutable C, original admission and exact G.
Original generation custody, failure, cancellation and retirement remain strict.

**Historical qualification:** The earlier `76c044622fe37d7c9e6222e177342907268ddf5947bb77a35201357b31921248`
and `1e06d45b3ae2f21c576acf2fea8557287ae58e1d2ef6ff5727cba452d72ed169`
adoption records remain verbatim as history. This later coordinated adoption
extends their window-extent-only limitation only through the eight literal
replacements named above. All unrelated binding clauses, existing proof
conditions, original dates and historical evidence retain their status.

**Implementation status:** Normative contract adoption only. This record
grants no Rust implementation admission and establishes no source conformance,
native executable provenance, rendered-continuity acceptance or resize-fix
claim. Production implementation requires separate explicit governing-owner
authorization and the scoped post-adoption implementation proofs. Existing
baseline fixture evidence is not proof of implementation of this extension.

**Unaffected authority:** SPEC-HAL-002 and SPEC-HAL-024 are unchanged. No new
source, shader, scale, state, runtime, consumer, public API, default material,
fallback, shared-resource cache or inactive-target retention policy is adopted.
Proposed SPEC-HAL-363/364 provisions remain inactive.

## Active Button SDF size-replacement contract adoption - 2026-10-28

**Status: Adopted / Binding, scoped owner amendment.** JC's explicit owner decision, adopts the coordinated literal changes in `Active explicit Button SDF size-replacement amendment`, exact proposal SHA-256 `f6bc9795425cdb8a92e1a1c7eebc88a3d78cbb08fa7a881b242db30b82781e5c`.

This scope is only the existing canonical already-admitted Rest/Pressed pair's active F-size replacement after its initial resource and first-target-output lifecycle: fresh current-C/G/full-K admission before Paint, temporary target-local replacement Pending, and independently authenticated producer-write/CPU/sampling custody. Initial optional A/B activation, HAL-018 section 16 startup, existing source/profile/state/scale and public API/caller limits, genuine failure and every unrelated rule remain unchanged. Universal default replacement remains on hold; no inactive universal-default or HAL-363/364 provision is adopted.

Contract adoption does not establish source conformance or native visual/performance acceptance. The new post-adoption obligations gate implementation acceptance and execution claims; prior proof/history retains its original status. Source-bound before/after hashes, exact literal preview and independent review belong to this adoption's receipt. Implementation requires separate governing-owner authorization.

## Adopted scoped appearance capability: HAL-388 S1 Local Ambient Glow (2026-11-13)

JC ordered execution of independently defined optical materials. HAL-388 §7 admits precisely one opt-in finite decorative underlay for an owner-supplied real rect, 0..=2 validated lobes and strict Z under existing PaintSink/SceneBuilder. This is not a new SurfaceFill family, default Button style, semantic focus/capture source, product-selected global light or widget-owned GPU resource. Existing §6 finite composition, §9 clip/overflow, §10 truthful cue and §11 unresolved-capability rules are binding. Unsupported PaintSink or clip/target requirements fail distinctly, not via silent shadow, border, image or omitted glow. Existing Solid/Gradient/Image/NinePatch paths and standard control defaults do not change. HAL-385/386/387/389 remain Candidate/Inactive; HAL-384 remains PROPOSED/INACTIVE.

## Adopted HAL-388 S1A delayed clip admission (2026-11-13)

**Binding addition to scoped HAL-388 opt-in decoration.** The local `PaintSink` Result validates only the locally authorable glow request. A parent may attach or change a rounded clip after the local paint was emitted or reused. Its real inherited clipping eligibility is therefore checked by the existing composed-scene CPU `DrawPlanBuilder` at Build, as defined in HAL-388 §8 and HAL-084's scoped addition. An unsupported positive-radius rounded ancestor yields a typed `DrawPlanBuildError::UnsupportedLocalAmbientGlowClip` before Compile. This is an explicit unresolved required appearance, not a retroactive failure from PaintSink, a surface loss, automatic omission, fallback, or success. An all-zero-radius or rectangular ancestor remains supported, and successful no-glow paths are unchanged. Existing §9 overflow/clip scope and §11 complete-failure semantics are preserved.

## Adopted Smoked Glass and independent reflective rim S1 (2026-11-13)

Under JC's implementation mandate, HAL-386 section 8 and HAL-389 section 7 supply the single binding first analytic face profile. The geometry-independent checked SurfaceAnalytic value has a Smoked Glass constructor and optional checked SurfaceRimLight. Use the fallible PaintSink::push_analytic_surface rect/corners/material operation and one ordinary SceneNode/renderer path. The existing infallible SurfaceFill vocabulary and its four variants do NOT change; an unsupported sink returns AnalyticSurfaceError::UnsupportedPaintSink. This explicitly reconciles, rather than activates wholesale, the inactive HAL-384 proposal.

Authored opacity controls ordinary transparency; no backdrop sampling is admitted. Rim strength zero preserves the fully lit base face, its alpha, text and input geometry. No default theme/control or product is restyled. Appearance declaration does not certify a clip attached later: the existing final composed-scene Build owner admits rectangular clips plus at most one actual rounded ancestor using the original source geometry, or returns the exact HAL-386 section 8.3 typed failure before Compile. Sections 6, 9, 10 and 11 remain binding. Existing LuminousContour, Local Ambient Glow and private Button materials retain their own mechanics and authority.

## Adopted HAL387 S1 independent surface micrograin (2026-11-13)

HAL387 section7 and HAL386 section10 admit SurfaceMicrograin as an ordinary immutable optional appearance value on the already admitted SurfaceAnalytic face. It is independent of the material selection, rim and glow; absence or zero amplitude preserves prior output and retained identity. Only the existing fallible analytic paint path consumes it. No SurfaceFill variant, global style, default Button change, theme resolver, monitor-profile discovery or product skin is introduced. Its explicit reference-luminance budget converts into ordinary bounded linear material RGB, with existing target composition unchanged. Grain does not alter semantic state, hit/layout/scroll geometry, text, own AA, ancestor clip or mandatory cues.

## Adopted true Frosted Glass appearance (HAL-385 section 8, 2026-11-13)

HAL-385 section 8 admits a checked FrostedGlassAppearance and fallible PaintSink/SceneBuilder::push_frosted_glass for actual same-surface background filtering. This is distinct from SurfaceAnalytic Smoked Glass. Existing SurfaceFill variants and standard-control defaults remain unchanged. The authored owner supplies only rect, corners, optical values, Z and key; no framebuffer, SurfaceId or GPU resource. Local acceptance is not final inherited-clip or target admission. Final Build and Compile return the exact typed refusals selected in HAL-385, never an omitted blur or substituted material.

HAL-387 grain remains independent and may modify only the explicitly selected tint face, not the sampled backdrop. Reflective rim, emitted contour, ambient halo and foreground content retain their separate owners. Existing clip/overflow, interaction and complete-appearance guarantees remain binding. This adopts no cross-window/cross-surface source, product palette, generic filter graph or physical display claim.

## Adopted configurable optical surfaces after JC visual review (2026-11-14)

JC rejected the S1 visual demonstrations as synthetic and explicitly required configurable transparency, frosting, tint, surface light and edge appearance. HAL386 section11 / HAL385 section11 / HAL388 section10 / HAL389 section8 / HAL387 section9 select the complete S2 optical correction. This records technical authority, not a global skin or visual acceptance. Existing bounded technical proof is preserved.

Use immutable checked GlassSurfaceOptics and independent SurfaceEmission on the existing analytic/frosted materials. Body opacity, tint/transmission, reflection, directed rim, grain and connected glow remain independent authored controls; optional edge-lens displacement belongs ONLY to actual Frosted Glass. No default control automatically becomes glass or luminous. The existing four SurfaceFill variants, old paint entry points and standard interaction policies remain unchanged.

Appearance may select finite material parameters; it cannot select source SurfaceId, GPU resources, hidden state, new frame phases, clip escape, layout/hit/scroll geometry or an arbitrary shader/effect graph. Configuration changes follow normal app value binding and owned Update publication; retained clean values stay clean. All required declared properties must execute or return a typed failure, not be silently replaced by a preset. The lab uses actual existing controls and displays validation limits; it is not a new settings subsystem or an exception to the Constitution.
## 2026-11-18 coordinated scoped appearance adoption: HAL-393/394/395 [S1 Binding]

Under JC's explicit technical-decision delegation recorded in PROJECT-ENTRY, adopt only the checked per-instance, immutable first optical profile selections from HAL-393 sections 7-8, HAL-394 sections 8-9 and HAL-395 sections 8-10. Existing GlassSurfaceOptics remains the single selected coating value; it may carry separately checked optional ThinFilmParams and SurfaceBloom, each absent/zero exactly preserving existing results. The existing SurfaceAnalytic and FrostedGlassAppearance producers carry the same coating without new widget, state source, input owner, appearance service, renderer clock, or dynamic registration. A material selection is ordinary authored appearance, NOT a feature-gated second runtime path. HAL-021/354 govern motion and retained invalidation; no default control or skin changes. Shader/graph/physical output authority remains with its accepted owners, and implementation/capability is not inferred from this S1 design admission.


## Explicit theme-selected Button material (2026-11-22)

**Status: Adopted / Binding, scoped authoring extension.** JC requires Button
material selection to be independent of a particular Showcase control and of
`DensityMode`. This section extends only the existing optional material input
in sections 7.1 and 16; it does not activate the held universal-default proposal.

**INV-012-BUTTON-MATERIAL-SELECTION:** `ButtonMaterialPolicy` is a pure appearance
configuration value with `Standard` and `SdfV3` choices. `Theme` carries
`button_material_policy`; all framework Theme constructors default to `Standard`.
An explicitly selected `SdfV3` policy applies to ordinary Button instances through
their existing Update: light Theme resolves `LightV3`, dark Theme resolves
`DarkV3`. A caption, callback, example, NodeId, construction order or density
preset MUST NOT select or exclude this family.

Selection precedence is fixed: existing prepared appearance, then an explicit
Button material override, then the selected Theme policy. An unset override
means Theme selection. `request_sdf_material(Some(profile))` retains its explicit
profile meaning; `request_sdf_material(None)` explicitly withdraws and excludes
Theme material until `use_theme_material()` restores Theme selection. Prepared
appearance and `use_default_appearance()` retain their existing independent
semantics. Existing disabled-material suppression is unchanged. This is a bounded
Button configuration rule, not a widget-tree cascade or global appearance service.

The existing ordinary Button Update copies the Theme choice before publishing
the effective request through its current checked request state. Missing Theme
input MUST NOT invent or withdraw an inherited selection. Failed checked input
preserves the previous valid selection and reports the real cause. Equivalent
effective input does not advance the material revision. Selection changes must
reach ordinary Update even when the current effective profile is equal; changed
publication uses the existing owner-local retained invalidation. No work moves
into Layout or Paint and no extra Update, scanner or material registry is added.

`Theme::set_density` preserves material policy. `Theme::toggle` preserves it
while replacing light/dark values. Product theme replacement explicitly carries
the desired policy through the existing theme-change owner. Density continues
to determine ordinary metrics; final allocation and the complete current target
key determine material geometry. The four density recipes, instance metrics,
caption placement, input, Focus/Capture and mount identity remain unchanged.

Theme-derived requests use exactly the existing HAL-266A/HAL-295A request,
qualification, preparation, admission, publication and custody path. Existing
canonical App/base-target scope, V3 effective-scale admission as amended
on 2026-11-23, valid geometry, initial staged admission, active-size Pending,
and Ready/Unavailable truth remain binding.
Selection alone is neither resource readiness nor proof of unsupported execution.
No SurfaceFill variant, shader, producer, resource owner or fallback is added.

Showcase selects `SdfV3` once in its application Theme for its Button widgets
at every density. It preserves that choice on theme replacement and removes
the entire Stage-only request/helper/retained-ID/retheme path. Visible command
labels and callbacks remain content. Dialog/Menu/child-window action primitives
are not reclassified as Button consumers. Other applications keep the framework
`Standard` default; the universal-default hold and its wider capability gates
remain unchanged.

Verification must establish selection independence on multiple mounted Buttons
at all four densities, light/dark replacement on the same mounts, explicit
override/withdrawal and prepared-style precedence, and actual current admitted
material with correct geometry at the supported target scales. A submitted
request or an ordinary cold-frame projection is not SDF output evidence.

### V3 fractional target-scale admission (2026-11-23)

**Status: Adopted / Binding, scoped Windows/HiDPI correction.** The existing
SdfV3 Button material accepts finite effective target scale in the inclusive
range 0.5 through 4.0, including fractional DPI. Earlier V3 1x/2x statements
remain exact reference profiles, not an exclusion of intermediate scales.

**INV-012-V3-SCALE:** V3 coverage outsets (left, right, top, bottom) are
`[3.9, 3.9, 2.8, 5.0] + 0.5 / effective_scale` logical units. At 1x and 2x
these are exactly the previously admitted outsets, respectively
`[4.4, 4.4, 3.3, 5.5]` and `[4.15, 4.15, 3.05, 5.25]`.
The source must use the exact current target ratio for physical storage, UV,
shader evaluation, and generation qualification; no quantization to the nearest
integer scale is authorized. Invalid or out-of-range scales are reported as
typed material unavailability; the existing Button pending/ordinary appearance
policy remains unchanged. V1/V2 keep their original exactly 1x/2x admission.

This extension preserves the existing Button theme policy, the V3 shader and
material ABI, two resting/pressed cells, retained image and Focus/Capture
ownership, target-qualified resource admission, and every current lifetime and
publication requirement. Headless GPU tests exercise V3 at 1x, 1.25x, 1.5x,
1.75x, and 2x; mapping tests cover the larger finite range. They do not
establish physical Windows display acceptance.

## Text-selection palette and LightV3 shadow correction (2026-11-22)

**Status: Adopted / Binding, scoped appearance correction.** JC's direct request is to use the existing TextField/TextArea selection appearance throughout existing selectable text and refine the LightV3 exterior shadow while preserving Button press feedback.

**INV-012-TEXT-SELECTION-PALETTE:** Existing editable and read-only character selection consumes one pure Theme-resolved RGBA value. Preserve the existing editor precedence: a colors.accents.muted value unequal to the current light/dark default is the explicit selection tint; otherwise use focus_ring.capture_color. The final alpha is typography.selection_alpha. This color resolution does not depend on current Focus or Capture. Existing owners still determine whether selection is available and visible, its range, copying, editing and focus-loss behavior.

This rule includes TextField, TextArea, selectable TextBlock and Label, existing selectable readout text, MarkdownView user selection and Console output in both overflow modes. EventLog detail consumes it through its existing mounted TextBlock. It does not change row/item selection, search-match decoration, Markdown code/strikethrough decoration, caret color, selection eligibility or an input policy. Existing theme replacement and retained invalidation owners remain controlling.

**INV-012-LIGHTV3-SHADOW-REFINEMENT:** The existing private LightV3 reference profile uses shadow opacity 0.16 in place of 0.34. DarkV3 remains at 0.30. The existing one-pass shadow shape, logical offset (0.0, 1.1), sigma 1.3, directional weighting, finite tail, and the 1x/2x output and F/E mappings remain unchanged. The material face, perimeter, grain and independent hover/focus cues remain unchanged. The existing Pressed endpoint retains its 0.45 shadow multiplier and concave face response; its shadow therefore scales from the revised light base. Input ownership, endpoint selection, current admission and resource lifetime remain unchanged. The V3 fractional-scale admission is governed by the 2026-11-23 extension above.

This correction neither changes the framework's Standard material default nor adds a public material/shadow API or a density/caption-specific recipe. Visual acceptance requires actual output at the relevant size, theme, scale and interaction state; implementation and passing tests alone do not establish that acceptance.


## Adopted Frosted Glass Edge Feathering (2026-11-25)

HAL-385 section 12 admits the checked per-instance immutable edge_feather_logical width through FrostedGlassAppearance::try_with_edge_feather and its read-only getter. Zero preserves all existing appearances. A positive width attenuates the complete backdrop-dependent glass body inward from its own SDF boundary without changing layout, hit testing, ordinary interaction or independently authored emission/Bloom. This is an authored material value in the existing PaintSink and retained execution path, not a new widget, global policy, opacity overlay or feature gate.
