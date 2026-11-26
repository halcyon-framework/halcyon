# Changelog

All notable changes to the Halcyon UI framework.

## [0.272.0-beta] — 2026-11-26

Local release candidate.

- **Switch activation** — Commit a captured mouse press once on release inside the control. Pointer exit, outside release, capture loss, and disabling cancel the pending activation.
- **Switch appearance** — Add checked per-instance `SwitchAppearance`, `SwitchAppearanceError`, and `Switch::with_switch_appearance`. Opt in to a rounded track and moving rounded thumb with configurable OFF and ON colors.

## [0.271.4-beta] — 2026-11-26

- **GPU startup refusal** — Add `RendererInitError` and fallible renderer and window-surface constructors. `App::run` reports a refusal and ends startup before Screen entry. `App::run_with_gpu_init_error_handler` delivers the original typed failure to the caller.
- **Web Showcase startup errors** — Show a short message when the browser lacks WebGPU or initialization is refused. Keep the underlying diagnosis in the console, and retire the failed App's bootstrap and startup nodes.
- **Button cursor affordance** — Add opt-in `Button::with_pointer_cursor(true)` for ordinary in-bounds mouse interaction. Existing Buttons retain their default cursor behavior.
- **Browser context menus** — Add WASM-only `AppConfig::with_web_context_menu_enabled(true)` to allow the browser context menu without changing other default-event policies. The local WASM helper is `halcyon-winit` 0.30.13; native upstream Winit remains 0.30.12.
- **Text editor motion** — Unify qualified TextField character reveal/conceal and discrete caret navigation, including TextArea same-line navigation, at 40 ms. Preserve EaseOut and adaptive range motion (100–220 ms). The maintainer accepted the visual trial; all 13 selected timing tests passed. The separate pre-existing rejected-input failure remains open.

## [0.271.3-beta] — 2026-11-25

- **Cargo distribution** — Prepare the `halcyon-rs` package with Rust library and executable name `halcyon`. The registry beta will use the explicit `cargo add halcyon-rs@0.271.3-beta` invocation after publication.
- **Winit package identity** — Use registry `winit` 0.30.12 on native targets and the Apache-2.0 `halcyon-winit` 0.30.12 helper on WebAssembly. Import interoperable types through `halcyon::winit`. On WebAssembly, upstream Winit's types and the helper's types are distinct.
- **Package contents** — Drop obsolete application-root Cargo patches and omit duplicate Onest formats from the Halcyon archive. The bundled variable font, notices, renderer resources, tests, and examples remain included.

## [0.271.2-beta] — 2026-11-25

- **macOS App startup** — Show a theme-matched native “Loading…” view in the App window while required content is pending. Remove it after GPU completion and the first ready primary frame’s Present call.
- **macOS drawable availability** — An initial primary acquisition that reports `Occluded` leaves startup pending. Native unocclusion wakes ordinary rendering for primary and child windows.
- **Dependency setup** — Remove the `wgpu-hal` override and use the published 30.0.2 release. Retain the `winit` patch and pin `windows` to 0.62.0 for DX12 dependency alignment.
- **Verification** — macOS windowed and fullscreen startup, App child and low-level unocclusion recovery, focused startup tests, and a WASM library check passed. The canonical primary `Occluded` deferral and Windows native runtime remain unverified. Fullscreen capture showed the ready scene after the next ordinary frame. Attribution to the first texture and physical scanout remain unmeasured.

## [0.271.1-beta] — 2026-11-25

- **Window focus loss** — Deliver hover leave to the current widget before clearing the losing window's interaction state. Returning focus restores keyboard focus through the existing lifecycle, and a fresh pointer event establishes hover.

## [0.271.0-beta] — 2026-11-25

- **Compound Path fills** — Preserve contour boundaries and tessellate the combined region under NonZero or EvenOdd. Disconnected shapes, holes, overlaps, and self-intersections use the same cached draw path. The registered CHECK renders both arms, keeps the gap clear, and blends once at the overlap.
- **Path fill API** — Add `Path::try_to_mesh_with_fill_rule` and explicit fill-rule emission on `SceneBuilder` and `PaintSink`. Existing path emission and `Path::to_mesh` default to NonZero. The `core::icon::FillRule` import continues to work. Low-level `PathNode` struct literals now require `fill_rule`. `MeshCacheKey` struct literals now require `fill_rule` and `dpr_bits`.
- **Path diagnostics and triangulation** — Generated-path edge overlays follow the projected contours. The simple-polygon index helper returns counter-clockwise triangles for either input orientation and an empty result for invalid input.

## [0.270.3-beta] — First public beta

This section records the changes since the private development milestone
`0.196.2-beta` (2026-08-24), through the final private source cutoff on
2026-11-25. It establishes the baseline for the first public source release.
Earlier sections record private development milestones, not previous public
releases. See [HISTORY.md](HISTORY.md) for the complete development history.

- **Initial beta source distribution** — Prepare the complete 0.270.3-beta source tree under the MIT license, including redistribution notices for both patched dependencies and the bundled fonts. Document the consuming-root Cargo patches, standalone application setup, architectural rationale, known platform limits, participation rules, security reporting, and roadmap. The 0.0.0 registry notice remains distinct from a functional framework release.

- **Developer shortcuts (HAL354 §10.5)** — Remove the built-in F12 outline toggle and key reservation. Applications can bind F12 through ordinary shortcut routing; programmatic App-owned diagnostic outlines and the existing F9 blueprint shortcut remain available.

- **Text quality example** — Use theme-derived text color and enable vertical and horizontal scrolling for all 50 original samples (font sizes 6–55). Preserve natural unwrapped text widths so large samples can be inspected in narrow windows.

- **Example collection** — Retire eleven older demonstrations, including `screen_navigation` and `compositor_single_surface`, leaving 109 Cargo examples. Preserve their relevant regression assertions in the surviving compositor example and existing test modules, including the two- and three-rectangle Z-order cases and fixed-height GroupBox items during FlowGrid wrapping.

- **macOS App initialization** — Create the ordinary hidden primary window during the active event loop's Init dispatch, so window-creation callbacks reach the installed handler. Reuse the same App event-handler initialization and preserve the existing non-macOS and browser startup routes.

- **Native SDF material shutdown** — Cancel suspended App compilation and release retained scene custody on LoopExiting before retiring staged material generations through the existing renderer ownership and completion checks. Preserve ordinary Quit and window-close behavior without introducing another render frame.

- **Retained presentation regression coverage** — Update internal transient-presentation fixtures to use current source and retained-scene identities. Preserve solid-paint Exchange coverage and explicitly verify refusal of an unsupported Standard-gradient source without changing production behavior.

- **ListView lifted-column live updates** — Invalidate the lifted column's retained paint when a realized, visible cell's presented text or Choice proxy changes, including when the pointer is stationary. Share the same presentation comparison with Paint; unchanged or offscreen values do not require this additional invalidation.

- **Build and demonstration cleanup** — Retire unused internal helpers and unused Showcase demo state, keep test-specific layout and renderer diagnostics available to internal Release tests, and move Stage, Apply, and Rollback callback messages to debug tracing.

- **Windows 10 runtime observation** — The maintainer subsequently exercised native Showcase in both Debug and Release successfully. This supplements the earlier Windows DX12 compilation evidence; broader platform and fractional-scale display qualification remain separate.

- **Introductory examples** — Add `hello_world`, `listview_basic`, and `treeview_basic` as small, independently runnable starting points for the ordinary App/Screen lifecycle and data-backed views.

- **Dependency security maintenance** — Update the selected registry dependencies, including pulldown-cmark 0.13.6 and libz-sys 1.1.33, without changing the Markdown syntax profile or the selected renderer stack. The 2026-11-25 RustSec audit of the resolved source lockfile reports zero vulnerability findings and no suppressions. Four warnings remain: the restricted Glyphon use of lru 0.16.3 (RUSTSEC-2026-0253) and the maintenance-status notices for chrono, rustybuzz and ttf-parser. This is a dependency-audit result, not a general security guarantee.

- **SDF Button resize continuity** - Renew current-attempt eligibility for an admitted size replacement when a superseded Compile retries with the same resource key. Preserve the original immutable generation and exact resource custody instead of leaving dependent output pending on stale evidence.

- **Frosted Glass edge feathering** — Set a checked inward feather width with `try_with_edge_feather` to dissolve the glass body into its current backdrop while keeping foreground text sharp.

- **Bundled Inter loading** — Use the existing embedded Inter provider for the legacy Web font loader and the real-shaper editing tests.

- **Windows HiDPI Button material** — Keep Showcase's selected SdfV3 image material at fractional effective target scales, preserving exact 1x/2x profiles and V1/V2 limits. Headless GPU validation covers 125%, 150%, and 175%; physical Windows display verification remains open.
- **SpinBox borders** — Give the embedded numeric field sole ownership of its two interior seams, retaining the side buttons' exterior borders and all numeric editing behavior.

- **Windows DX12 build** — Align `gpu-allocator` 0.28.0 and the vendored `wgpu-hal` 30.0.2 on `windows` 0.62.0, eliminating the conflicting `ID3D12Device` types. The Windows-target Showcase `cargo check` passed; native GUI execution remains separately unverified.

- **Web App navigation** — Project public routes from the existing navigation host to native browser history. Trusted Back and Forward traverse the committed route trail under the application's guard.

- **Browser TextField clipboard** — Copy and Cut export the focused editor's selection through the trusted browser clipboard event. Cut deletes the selection only after the transfer succeeds and records one undo step.

- **Grouped ComboBox scrolling (HAL-401)** — Keep the current group name and real option count visible beneath the scrollable rows. Strengthen heading contrast and dividers.

- **Grouped ComboBox popup (HAL-401)** — Give headings neutral bands, smaller titles and one-pixel section dividers. Indent grouped option labels and fit whole rows within the viewport.

- **ComboBox selection restoration (HAL-400/401)** — Use `set_display_selection` to project a model's selected option and caption before mounting. Browser and UIKit pickers receive the current option without a commit.

- **Grouped ComboBox projections (HAL-401)** — Add `set_grouped_projection` for nonselectable popup headings, browser SELECT optgroups, and UIKit picker headings. Real options keep their original IDs and Resolver selection.

- **Showcase Session profile** — Cap the TextArea at 220 logical pixels so added lines scroll inside the editor.

- **Text selection and LightV3 shadow** — Use the editor selection palette for existing selectable text and reduce the LightV3 Button's exterior shadow opacity from 0.34 to 0.16.

- **Screen theme toggle** — `ScreenContext::toggle_theme` preserves density and the selected Button material policy in both directions.

- **Button material and divider policies** — Themes can select SdfV3 for Buttons that inherit their material policy. Showcase uses this policy at every density, shows outlined tooltip targets with no tooltip on Disabled, and keeps the Workbench SplitPane dividers visible at rest.

- **Native first-frame startup** — Backport the upstream macOS hidden drawable fix to wgpu-hal 30.0.2. A terminal failure of fully ready native startup reports the attempt, expected primary surface and available cause. External application roots must select the patched wgpu-hal source to receive the Metal repair.

- **Upstream wgpu 30.0.2** — Use the upstream wgpu/Naga 30.0.2 family with glyphon 0.12.0 and remove the old wgpu vendor override. Public GPU handles now use wgpu 30. Surface acquisition reports `RendererSurfaceError`, raw surface textures present through the original queue, and `DeviceLimits3D` reports a `u64` uniform binding limit and inter-stage shader variables. `HardLimit3D::InterStageShaderVariables` replaces the components variant. Browser editor exit restores canvas focus after the active App callback releases its borrow.

- **App implementation layout** — Split the existing application runner, root container implementation and dedicated UI loop into private modules while retaining the public App API and runtime behavior.

- **Native touch choice picker (HAL-400)** — Present ComboBox choices through the browser's native SELECT or UIKit's picker on an admitted finger tap. Apply the returned option through the existing Resolver and preserve the original Mouse, Pen and keyboard popup.

- **Touch Input example verification** — Check that pans from the mounted TextField and Button scroll, and that only Button taps increment the count.

- **TextField example history verification** — Verify the mounted demo counts successful Undo and Redo notifications and preserves the new document after redo is invalidated.

- **TextArea example history verification** — Verify mounted Undo and Redo restore the document, caret, and selection, and a new edit clears redo.

- **TextArea example verification** — Route Enter, Unicode input, selection replacement, and wheel scrolling through the mounted multiline example.

- **TextField example** — Demonstrate single-line editing and selection, with a visible count of each text-change callback.

- **Browser Touch Input example** — Run the native touch controls through the local WebAssembly development tool, with a counter that records every Test tap activation.

- **Single-surface compositor example** — Show blue and red rectangle ordering by scene Z and identify the standalone compositor snapshot as console-only.

- **Compositor surface order example** — Describe the three overlapping rectangles as a same-surface Scene Z demonstration and label the standalone compositor snapshot separately.

- **Icon gallery example** — Show all 20 registered built-in icons at S, L, and XL in a 760×460 window, for 60 visible samples.

- **Browser text input order (HAL-398)** — Apply hardware caret and selection changes before the next browser text edit, and preserve input order when the framework is waiting for geometry or widget availability.

- **Mobile touch input (HAL-397–399)** — Tap to edit or activate controls, pan through them without activating them, and flick a ScrollArea to continue after release. WebAssembly editors use the browser's software keyboard and IME through the current Halcyon editor session.

- **Headless shaping example** — Ship the four Noto font fixtures and upstream notices. The example exits with status 1 when any sample produces empty output or missing glyphs.

- **ListView Choice residency** — Preserve focused or captured Choice editors offscreen and keep the same resident editor through temporary commit-policy denial. Withhold that editor's input and popup until synchronization succeeds.

- **Touch content scrolling (HAL-396)** — Pan an overflowing ScrollArea with one finger. An accepted pan takes over the unfinished Button press, while ordinary taps still activate Buttons. Halcyon reserves touch input on its WebAssembly canvas.

- **Breaking — legacy Agentic UI API retired** — Removed the agent channels, WebSocket server, Tower, UI MCP bridge, declarative widget factory, assembly protocol and associated tools. The local F9 snapshot reads widget annotations through `WidgetTree::set_diagnostic_hint`.

- **ScrollArea scrollbar reveal** — Opt into fully hidden idle scrollbars that reappear on scrolling or rail proximity and fade out after inactivity.

- **ComboBox popup scrollbar tuning** — Set per-instance AutoHide hold and fade durations and independent theme-relative track, normal thumb, and hovered or pressed thumb opacities.

- **Selective glass Bloom** — Opt into a local reflection halo on Smoked and Frosted glass. The two-light comparison in Glass Material Lab shows when the reflections cross the Bloom threshold.

- **Thin-film glass coating** — Opt into direction-dependent reflection and rim colors on Smoked and Frosted glass. Glass Material Lab compares 5° and 80° light elevations on the same coating.

- **Crossing reflections in Glass Material Lab** — Watch two surface reflections approach, merge, cross, and settle over one 2.4-second Timeline playback.

- **ComboBox scrollbar opening hint** — AutoHide briefly shows an overflowing popup's scrollbar on open, then fades it away after inactivity.

- **Keyboard-only Button focus cue** — Let a Button keep normal pointer focus and activation while showing its navigation outline after keyboard navigation or activation.

- **ChampagneV4 hover continuity** — Keep one admitted four-state gold face and its dark caption through hover and press changes. Match the independent focus outline to the face's radius of 8 logical pixels.

- **Mounted text after scale changes** — Refresh paragraph layout, source ink and retained paint when the shaping scale changes at equal logical bounds. Keep auxiliary output scales independent of the primary tree's shaping input.

- **Procedural Button visibility after scroll** — Prepare the SDF face when its scrolled position enters the viewport and certify current placement when reusing an already-admitted material.

- **Paragraph shaping scale** — Read the live shaping scale through `LayoutContext::text_engine` to reproduce bound paragraph runs for source-range effects at fractional and integer scales.

- **Fractional ChampagneV4 Button scale** — Keep the rounded gold face, inset edge and prepared press cell at the actual target scale from 0.5× through 4×.

- **Source-range text ink** — Recover the exact bound face runs of wrapped or fallback-bearing paragraphs, qualify UTF-8 ranges for per-cluster ink rectangles and paint a selected foreground and 0.90–1.15 ink scale around the selection's geometric center. Preserve whole-run shaping, line metrics, neighboring glyphs and existing atlas keys.

- **ChampagneV4 Button material** — Opt into a radius-8 gold SDF Button face through `request_sdf_material`. The renderer produces distinct Rest, Hover, Pressed and Disabled appearances. A warm press selects the prepared endpoint immediately and the Button keeps its own caption and focus cue.

- **ComboBox search preview** — Move prefix highlighting and ghost suffix matching into the model. Keep the existing option order, editing, and explicit resolver commits.

- **ComboBox popup hit geometry** — Publish scrollbar hit geometry with the retained overlay. During reveal, pressing the visible track pages and pressing the visible thumb starts a drag. Preserve the full legal row area.

- **Input icon and chevron alignment** — Offer per-instance leading icon paint offsets on TextField and ComboBox, and per-instance ComboBox chevron dimensions within the current affordance. Default paint and input geometry stay fixed.

- **ComboBox Hover with Focus** — Optionally prefer the Hover face on a settled closed, focused ComboBox. Focus cues and capture materials remain independently resolved.

- **ComboBox outside press** — Dismiss the non-modal popup and deliver the original click to the exposed control. The popup keeps its own rows and scrollbar, and modal overlays retain their input priority.

- **ComboBox type-to-select** — Let non-editing popups delegate typed prefix matching to a per-instance adapter. Typing moves the highlight within the current projection. Explicit selection still commits through the resolver.

- **ComboBox popup scrollbar** — Add an opt-in AutoHide policy. The native scrollbar appears for gutter hover, scrolling, or dragging. Hidden gutter space stays available to the rows.

- **ComboBox leading icon size** — Allow a per-instance logical size for the registered icon. The existing label and editor use the fitted leading slot, and the default size follows the input font.

- **TextField hover surface** — Honor the existing input appearance state surfaces on standalone TextField, with Active ahead of Hover and the base surface when idle or disabled.

- **Native input icon parity** — Let a TextField choose its leading icon size within the existing allocation. Refine the globe's rim and grid and add the built-in briefcase icon.

- **ComboBox availability** — Honor the existing mounted enabled request. Disabling closes the popup and retires interaction without committing, while preserving the value, icon, layout and selected appearance.

- **Browser-safe Frosted Glass preparation (HAL385)** — Return explicit Ready, NoTargets or Pending Compile status. Resolve browser GPU allocation scopes asynchronously and resume the same qualified preparation through the App wake; never encode a pending or stale plan. Preserve actual OOM errors, native compilation and original submission custody.

- **Frosted ComboBox display** — Attach the existing checked same-surface backdrop material to a standalone display-mode MatchTrigger ComboBox. One joined glass body precedes its crisp parent-owned foreground; aligned row tints share its outline without a hard Path underlay or doubled base opacity. Preserve ordinary defaults and resolver/input ownership.

- **Joined ComboBox material coverage** — Use the actual common outer rounded clip for an aligned selected trigger/popup, retaining full coverage at their internal junction. Preserve allocated geometry, gradient mapping, popup clips, default controls and input behavior.

- **Independent ComboBox open surface** — Add `InputSurfaceAppearance::with_open_surface` for the genuine trigger while the popup opens/closes, independently of closed Hover/Focus/Capture materials. Preserve the previous Active fallback, popup palette, defaults, resolver and input authority.

- **Mail outline geometry** — Draw the native envelope with supported simple contours, keeping its interior clear at 1×/2× and the real TextField caret separate. Preserve IconId::MAIL and the existing leading-icon API.

- **Selected ComboBox perimeter and popup corners** — Admit explicitly transparent Focus/Capture borders per instance, retaining distinct truthful cues on the existing caret. Clip row and match fills through native rounded primitives instead of rectangular text backgrounds; preserve default chrome, popup width and resolver behavior.

- **Local browser live reload (HAL392; internal development tooling)** — Type `build` and press Enter in the foreground session terminal to start a serial offline/locked WASM build. Startup and saves leave compilation idle. Successful publication reloads Safari. Failed, superseded and cancelled attempts keep the working page. This utility is maintained outside the public framework distribution.

- **Mail input icon** — Register the reusable native `IconId::MAIL` envelope for the genuine in-field TextField adornment; retain every existing icon identity.

- **Input adornments and state surfaces** — Add a genuine leading icon to TextField with shared text/caret/hit geometry, smooth the built-in globe meridians, and let ComboBox instances select Idle/Hover/Active materials through InputSurfaceAppearance. Preserve default controls, real focus/capture, popup commits and registered font styles.

- **Browser caption font styles** — Admit real bundled Inter Bold, Italic and Bold Italic faces through the ordinary embedded bootstrap and preserve the complete catalog in renderer registration. Keep Regular bytes/identity and native SystemUI selection unchanged; no synthetic weight or slant.

- **Input appearance and popup policy** — Allow product-selected Focus/Capture colors for TextField and ComboBox, with a shared per-instance popup width policy. Keep standard defaults and resolver behavior; remove the rounded-clip tangent coverage seam in the existing Rect, Image and Gradient paths.

- **Transparent selected ComboBox** — Omit explicitly transparent surface, border, inner edge and affordance paints per instance. Keep real content, popup, selection and authoritative Focus/Capture cues; preserve ordinary defaults.

- **Selected ComboBox surface and leading icon (HAL012/HAL201)** — Consume the checked input surface for the real gradient shell, affordance and popup; reserve a normal leading icon inside the same control, including the built-in globe. Preserve default chrome, resolver commits, real editor delegation and framework Focus/Capture.

- **Selected standalone TextField appearance (HAL012)** — Add a checked per-instance gradient surface, border, inner edge and radius through the ordinary TextField paint path. Preserve authored alpha, authoritative Focus/Capture cues, default control chrome, editing and the shared SpinBox/Slider surface.

- **Cell-integrated editor chrome (HAL391)** — Reuse one qualified presentation role across ListView resident controls and PropertyEditor/TreeView edit sessions. Fill the exact cell allocation, retain normal editor content and popup behavior, and draw one cell-owned edge from current mount-qualified Focus/Capture and availability. Preserve standalone controls, factory signatures and editing/history semantics.

- **TextField Unicode end-edit retargeting** — Keep P6 motion when complete shaped runs preserve the same bound prefix glyphs and positions while terminal kerning changes their advances. Preserve exact source verification and canonical snapping for an incompatible contextual retarget.

- **TextField no-op navigation** — Preserve an active caret-navigation transition on ordinary Delete at document end. Keep accepted deletions and verified repeats as snap boundaries; qualify repeated end-Space against the existing 55 ms P6 contract.

- **Source-qualified TextField motion** — Remove the prototype ASCII, 32-byte and half-viewport presentation limits. Use complete shaped sources and actual visual cluster regions for contextual Unicode and RTL edits, preserving immediate text/history, the existing 55/65 ms and range timings, plain mode and native/browser ownership.

- **Local Web Showcase** — Reuse the canonical Showcase screen and tabs in a separate 780×900 browser example, with loopback-only hosting and explicit native-worker/metrics limitations.

- **Release target validation** — Use the ordinary render-target registration query for Frosted Glass admission, preserving `TargetUnavailable` refusal without calling a debug-only registry helper in release builds.

- **Trusted browser TextField Paste** — Let the original Meta/Ctrl+V produce Safari's trusted paste event and deliver plain text once to the same captured, mounted TextField. Apply its explicit byte limit after normalization and admit intake through checked, fallible allocation without a transport quota. Revalidate document, selection and editing policy before the existing atomic Paste/Undo operation; preserve native clipboard behavior and leave uncaptured events to the browser.

- **App-owned F12 outlines (HAL354 §10)** — Keep diagnostic outlines within their AppContext and invalidate retained paint only when the value changes. Preserve clean ON/OFF fragments, original outline geometry and F12 routing. Replace the unscoped module toggle/query with `AppContext::set_debug_outlines_enabled` and `AppContext::debug_outlines_enabled`.

- **Captured modified-key ordering (HAL328 §20)** — Deliver normal captured Ctrl/Alt/Meta keys to the actual widget first. Offer a genuinely unhandled key to Screen/App once after revalidating its concrete owner, and keep modified Tab out of plain focus navigation. Preserve existing reservations, physical capture, Escape and public routing signatures.

- **Glass material lab side reflection (HAL386 §15)** — Author the editable face with a soft side reflection and localized edge glint using existing coating math; expose independent rotation and linear RGB controls for both sources. Preserve the five fixed comparisons, original controls, retained publication and library defaults.

- **Markdown reading-state handoff (HAL390 §16/HAL383 §11)** — Capture exact ordered selection in the same immutable document, opt into logical blur/outside-click retention, and stage a one-shot restore on a fresh reader. The existing mount-qualified deferred owner cut applies it before first layout. A defaulted Widget hook distinguishes implicit outside-down from explicit force-clear; default selection reset, routing and TextField behavior remain unchanged.

- **Canonical child clips and stable material lab (HAL088 §17/HAL386 §13)** — Derive child boundaries from their declaring allocation before scroll projection, preserving retained fragments and existing viewport metadata. Give all six lab cells real clipping ancestors and separate background/material children, with explicit blue/pink layering, unchanged controls and synchronized finite motion.

- **Native Quit returns to the caller (HAL333 §7)** — Initialize standard macOS application roles without a product menu, buffer Quit through the existing UI wake, and stop the ordinary App loop through its original exit preparation. Hidden/idle windows can quit without rendering; `App::run` returns for caller-owned cleanup. Add compatible defaulted MenuHost lifecycle methods.

- **Configurable glass optics and connected emission (HAL385–389 S2)** — Add checked, independent coating/reflection/rim and silhouette-connected face/edge/halo emission to the existing material paths; add Frosted Glass coverage opacity and signed local refraction. Extend micrograin's reference-luminance amplitude to 8/255 and provide a live native material lab. Preserve unextended materials, ordinary geometry/input and sealed command custody.

- **Registered editable-text coverage (HAL049 §18)** — Preserve registered face runs from TextField/TextArea layout through caret, selection, handoff validation and painting. Keep primary-supported text on its original face, render admitted covering glyphs, and retain source-faithful Degraded output for genuinely uncovered clusters.

- **Current-backdrop Frosted Glass and sealed command custody (HAL385)** — Add checked same-surface Gaussian filtering/replacement, explicit graph barriers and typed Build/Compile refusal, with a finite native comparison. Seal the original command together with its transient backing until exact disposal or authenticated queue completion. Replace mutable `MultiTargetRenderOutput.command_buffer` access with consuming `into_raw_parts`; exported backing stays quarantined. Preserve normal no-frost rendering and independent text/SDF/HUD custody.

- **Native Markdown reader (HAL390)** — Add bounded, cancelable immutable source-bearing documents and one read-only native view with registered styled text, exact reading/source/code copy, native tables and genuine code/table overflow through the existing ScrollArea. Keep links, images, HTML and task markers passive; provide a public native reader example.

- **Registered text coverage and visible missing-cluster containment (HAL048/049/360/361)** — Resolve complete source clusters against exact registered Unicode coverage and preserve their face bindings through TextBlock layout, selection and paint. Render verified uncovered clusters as explicit geometric markers with a typed Degraded result, preserving same-surface progress and the required-text fault guard; structural, raster and resource failures remain refused.

- **Filtered material micrograin (HAL387)** — Add checked stationary scale, reference-luminance amplitude and seed on Smoked Glass, with exact pixel-footprint filtering, independent rim/opacity and retained invalidation. Extend the existing analytic uniform within its current slot and provide an ordinary native OFF / RESTRAINED / MAXIMUM comparison.

- **Smoked Glass and independent reflective rim (HAL386/389)** — Add checked opt-in analytic face paint with bounded two-light reflections and ordinary opacity, plus an independently disabled directional rim in the same draw. Qualify final inherited clips and bounded per-surface parameters before Compile; preserve existing materials, shaders and input semantics. Add a native comparison example.

- **Local Ambient Glow (HAL388)** — Add checked opt-in zero/one/two-lobe decoration through the existing ShapePipeline, with strict Z, rectangular ancestor clipping and typed CPU Build refusal for unsupported rounded ancestors, including retained clip-only changes. Standard Button appearance and Shape0–4 remain unchanged.

- **Localized luminous-contour opacity (HAL358)** — Add validated opt-in front-opacity boost to the existing ShapePipeline LuminousContour material. Admit zero-source-alpha contours only when the authored boost is positive, with matched Batcher and GPU-lowering guards. Preserve all legacy zero-boost output, ordinary stroke behavior, clipping and draw order. Verify the original Metal zero-alpha RED-to-GREEN at 1×/2× without a second draw or a new shader pipeline.

- **TextField P6 end-edit timing (HAL101)** — Set qualified single-character end-append/end-Backspace reveal/conceal and its mask-only caret-boundary sibling to 55 ms Once/EaseOut. Preserve immediate logical edits, 65 ms navigation and adaptive range timing.

- **Quiet dark gradients (HAL085A/PM026)** — Bound deterministic gradient dither in display-space units before converting the perturbation to premultiplied linear RGB. Remove the amplified repeating near-black grain while preserving gradient stops, alpha and rounded clipping.

- **Sealed ScrollArea content removal (HAL154/PM025)** — Reject direct removal of wired content before tree mutation when its sealed ScrollArea survives.

- **Resolver-qualified anchor guard (HAL131A/PM024)** — Validate the live ScrollArea-to-content relationship before post-layout scroll requests. Apply the existing positive-geometry debug guard only to content exposing a real anchor resolver; valid nonresolver content at zero or positive extent retains direct scrolling. Missing content and wrong-parent wiring fail structurally. Preserve HAL352 and ordinary layout/input phases; a native consumer's 21-resize replay returned successfully while the historical failing pair and physical scanout remain unqualified.

- **Spatial HUD frame custody (HAL375)** — Admit optional labels and leaders from the exact surviving Image in one per-target publication. Ordinary 2D keeps its existing render schedule. `CustomRenderPreparationOutcome` is now owned and non-`Copy`. Its Image binding remains `Copy`.

- **Passive PBR spatial HUD (HAL375)** — Admit source-bound labels and leaders in a viewport and gate each pair against its matching canonical N1/N4 depth. Keep ordinary text admission and surface draw order authoritative when the optional HUD is unavailable.

- **Spectral loading presence and duration (HAL381)** — Derive a luminous contour and mottled translucent fill from qualified proxy coverage. The native demo exposes an explicit 80–3000 ms materialization duration with a 160 ms default and the existing registered-front readiness gate.

- **Loading presence resize budget (HAL381)** — Preserve the canonical PBR frame at the requested extent when only the optional presence transition exceeds the shared single-target attachment limit. Keep the retiring preview charged until renderer reads finish.

- **Native wake admission** — Retain the existing Forced ticker reason when a native unit UserEvent requests a redraw, so the idle33ms throttle cannot consume and lose that one-shot request. Admit retained paint settled by Screen flush in the current redraw without requiring a second wake or scheduling another frame. Preserve coalescing, hidden-window deferral and ordinary idle policy for clean Tick frames.

- **Source-qualified visible selection outline (HAL374, 0.232.0-beta)** — Add explicit scene-bound packet occurrence selection and a static two-physical-pixel outer stroke. Replay the complete certified opaque scene into private exact float winner tags and reverse-Z depth, reject hidden/internal samples, and compose MAX analytic stroke coverage over the registered, immutable-pinned original PBR front. Preserve canonical MRT4, original no-selection output, finite shared target/upload budgets and queue-safe retirement. Add a stable native `pbr_selection_outline` example with A/B/None, orbit and zoom controls; physical display/input and JC appearance acceptance remain separate.

- **Source-qualified loading presence (HAL381, 0.231.0-beta)** — Add CPU-prepared, Context-admitted proxy geometry and scoped viewport source association. Present a real self-occluding matte ghost while full PBR is unavailable, then materialize over160ms only after the matching canonical front is registered and content-pinned. Preserve canonical MRT4/PBR coverage, use one rounded clip at every stage, retain actual queued-reader charges through re-tone/cancellation, and respect explicit static motion. Add a stable native `pbr_loading_presence` example and N1/N4 GPU/native proofs. External consumer proxy production, physical display/input observation and maintainer visual acceptance remain separate.

- **Source-linear HDR scene precision (R027/HAL370)** — Store private PBR scene radiance and per-sample coverage in four R32Float planes in the existing depth pass. Preserve low positive radiance until the original exposure and per-sample tone operation, with unchanged final FP16 Image, N1/N4 coverage, source shading and Device features. Account the larger complete scene and retiring fronts under the existing finite target caps; residual source-zero shading and physical visual acceptance remain separate.

- **Control subtree phase guards (HAL359)** — Permit lawful construction and deferred subtree-order changes in Release as well as Debug. Track actual Paint activity independently of relaxed release authority checks, retain Layout/Paint mutation rejection, and preserve nested scope and unwind cleanup.

- **Independent PBR scene replacement and reveal replay (HAL355/HAL372)** — Add explicit `Viewport3D::replace_pbr_scene` for a separately admitted, live complete source with a different HDR identity. Preserve the original successor API and view-revision checks, retire the prior reveal, and retain truthful compatible last-good fronts. Add the native `pbr_object_reveal` example with two immutable certified 3D sources and source-qualified 900ms replay; native and physical evidence remain separately reported.

- **CPU PBR geometry admission (HAL380)** — Add CPU-only geometry preparation and successor certification with short Context begin/finish registration. Expose HAL-calculated input, additional-peak and additional-retained CPU cost preflight/receipts for one request-wide producer budget. Reuse the original source converter and R024 certificate/successor kernels, preserve synchronous admission, and retain real CPU charges and predecessor custody through cancellation and stale results. Consumer integration and native selection latency remain separately qualified.

- **Explicit focus-request lifecycle** — Route ordinary widget `request_focus()` through the existing focus transition owner so old keyboard Capture is released before Blur and the new owner receives Focus. Restore native SplitPane divider keyboard resizing after pointer focus without changing panel-content focus, keyboard policy or widget APIs.

- **TreeView leaf activation (HAL371)** — Add `set_activation_callback(FnMut(u64))` for qualified same-leaf pointer release and framework-focused routed Enter. Keep stable node identity, headerless composition and existing selection/disclosure behavior; retire an armed body's pointer capture on parent Blur through an engine-checked descendant effect. Preserve PointerDriven keyboard restrictions and no-callback behavior. Consumer integration and native visual acceptance remain separate.

- **Canonical native resize/render path** — Remove the retired gated resize diagnostic subsystem and its specialized renderer entrypoints under JC's 2026-11-04 owner order. Keep one ordinary resize/render path, existing surface custody and error handling, and the canonical HAL361 frame observer unchanged. No replacement diagnostic mode is introduced.

- **Regular28 framework default (0.230.2-beta)** — Select the existing Regular density in `DensityMode::default()`, `Theme::light()` and `Theme::dark()` under JC's 2026-11-04 scoped supersession of the Intermediate30 default trial. Preserve all four explicit preset recipes, typography, UI scale, colors, interaction and density-preserving theme changes; Intermediate30 remains independently selectable at 30 logical pixels. Native visual acceptance is separate.

- **Source guards and backend UV/gradient uncertainty (0.230.1-beta)** — Separate deterministic source/host representability checks from optional modeled screen-determinant, reciprocal-w and helper/footprint enclosures. Admit source-valid triangles when only those backend-dependent sufficient bounds are indeterminate, while preserving typed source overflow refusals, complete homogeneous exclusions and exact original attribution. Report positive modeled summary bounds only when established for every admitted triangle; preserve original geometry, sampler, shader and publication behavior. Original external-consumer Metal N1/N4 observation and S3/S4 acceptance remain separate.

- **Attributed original-source UV/gradient numeric qualification (0.230.0-beta)** — Preserve the PBR packet and stable ordinal, original certified primitive/triangle, main-scene or shadow-face pass, failed inequality and exact outward numeric bounds through public `pbr_failure`. When the original screen-determinant enclosure is indeterminate, evaluate the same determinant and Cramer numerators using outward row differences; this qualifies the supplied first original external-consumer packed-input counterexample without an epsilon, triangle skip, source change or shader change. Retain the original sufficient route, FMA/flush and covered/helper bounds, UV/footprint guards, pre-write refusal and compatible original last-good behavior. Full unchanged external-consumer Metal execution, S3 acceptance and native S4 readiness remain separate.

- **External PBR test-harness GPU observations (0.229.0-beta)** — Expose the captured headless adapter, a bounded current committed PBR FP16-front pixel with its exact source/target/serial and Image ID, and the existing production Image pipeline's GPU-sampled RGBA8 solely under the existing `test-harness` feature. Keep ordinary builds free of these observations and raw GPU handles; return typed missing, stale, format and readback failures. This enables external source-to-Device tests without adding production readback or claiming original external-consumer GPU qualification, S3 acceptance or native S4 readiness.

- **Typed unlinked PBR consumer ingress (0.228.0-beta)** — Add checked, strongly typed public source and resource inputs for original/certified A/B/C geometry, all five V1/V2 material roles, and complete V2 normal mip chains bound to their authored sampler and exact normal scale. Capture independent source generations, D/E/G and HDR in immutable complete scenes, expose truthful requested/committed status, and publish through the existing unlinked Viewport3D FP16/Image writer. Reject linked-G1 and domain-owned requests before mutation; preserve compatible original last-good fronts, incompatible-target withdrawal, legacy rendering and ordinary 2D behavior. This is the verified S2 consumer API candidate; original external-consumer asset-to-Metal qualification, S3 acceptance and native/visual S4 remain separate.

- **Compact TagInput composition (0.227.18-beta)** — Refine TagInput-local chip heights, padding and rounding, lighter tonal surfaces and smaller remove/action marks. Use existing themed checkbox size/gap and shared optical baselines for balanced clipped popup rows, retaining row hit geometry, tag commands, host Capture, validation and the real inline TextField. Native visual review remains pending.

- **Showcase initial tab (0.227.17-beta)** — Start on Inputs and Controls through the existing initial section value. Tab identity, selection, navigation and rebuild behavior are unchanged.

- **Keyboard Capture accent hierarchy (0.227.16-beta)** — Use subdued neutral resting Slider/RangeSlider materials and reserve reference azure for the current keyboard-Capture owner. Keep navigation cues, modest hover and independent pointer press/drag treatment; identify the RangeSlider's keyboard handle and keep numeric-editor Capture on its host frame. Shared editor chrome now honors the configured Capture edge, replacing the default orange with theme-adapted azure while preserving explicit configurations and validation. Paint predicates use framework ownership; input semantics, geometry, clocks and TextArea content/motion remain unchanged. Native visual acceptance remains pending.

- **Slider rail highlight placement (0.227.15-beta)** — Move the shared fill's lighting ridge one physical pixel inside its rounded coverage and derive every stop from the current accent, keeping the highlight distinct at full hover. Retain the blue core with a shaded lower edge, existing geometry and input behavior. Native visual acceptance remains pending.

- **Lit Slider rail fill (0.227.14-beta)** — Light the shared Slider/RangeSlider fill across its thickness with a narrow highlight, blue core and shaded lower edge through the existing four-stop GPU gradient. Preserve the current hue, rounded coverage, geometry, input, disabled treatment and idle behavior. Native reference matching remains pending.

- **Reference-blue Slider accents (0.227.13-beta)** — Give default Slider and RangeSlider tracks and thumbs the supplied light reference's azure color family, with a lighter blue sheen through the existing three-stop GPU gradient. Keep explicit accent pairs, neutral input chrome, geometry, value editing, disabled state, Capture and focus cues. Adapt the same color family to dark surfaces; add no animation or idle repaint demand. Native visual matching remains pending.

- **Password TextField presentation safety (0.227.12-beta)** — Immediately discard ordinary reveal/conceal visual sources when password masking is enabled, then cancel their animation at the lawful Update. Render the static masked caret through the existing text pipeline above mask glyphs, sharing ordinary input chrome without retaining or animating password text. Preserve logical editing, mask geometry, Capture, IME and clipboard policy. Native visual review remains pending.

- **TagInput chips and typing geometry (0.227.11-beta)** — Add restrained chip depth through the existing GPU gradient path. Reserve bounded space for the real inline TextField, resolve visible chips and the clipped overflow badge during Layout, and clip content inside the host surface. Honor stretched allocation height so chips, text, popup and input regions share current geometry. Preserve tag creation, selection, callbacks, clipboard and host Capture. Focused routed/retained geometry checks pass; native visual review remains pending.

- **Slider and RangeSlider material (0.227.10-beta)** — Share the existing GPU gradient path for recessed track wells, controlled blue active fills and raised thumb lighting with a pressed face. Preserve value/drag/keyboard semantics, alignment, numeric editing, Capture enlargement and focus cues; add no clock, shader, generated texture or continuing idle effect. Native visual review remains pending.

- **TextArea frame visibility (0.227.9-beta)** — Keep the complete TextArea chrome above opaque section-container fills, with selection and glyphs strictly above the input surface. Restore the visible frame in composed layouts without changing colors, geometry, clipping, scrolling, overrides or input behavior.

- **Refined default TextArea material (0.227.8-beta)** — Reuse the neutral input surface, restrained layered frame, caret and selection treatment in both themes and all densities while preserving public theme overrides, viewport, glyph metrics, clipping, scrollbars and input policy. Defer post-edit caret visibility until Layout publishes current scroll dimensions, fixing the final-line Enter panic. Correct the existing stretched TagInput test to verify actual glyph baselines and containment instead of equating different-sized text-box tops.

- **Refined default TagInput material (0.227.7-beta)** — Use the shared neutral input material, layered edges, tonal chips and a flush affordance compartment in both themes and all densities. Preserve public theme overrides, current focus/Capture and validation cues, embedded editing, immediate commits, policies, chip hit areas and popup geometry; retain semantic creation/action colors and enforce strict paint order.

- **Dimensional ordinary Button default (0.227.6-beta)** — Expose a lit bevel around an inset face through the existing Gradient and Border primitives in rest, hover, press and disabled states. Preserve all density/layout/hit/caption geometry, focus/input, prepared product precedence and explicit SDF materials; no Showcase exception or appearance flag.

- **Refined default TimePicker material (0.227.5-beta)** — Use coherent neutral input surfaces, fine layered rectangular edges and a restrained Capture ring in both themes and every density. Project current framework focus/Capture and navigation-cue policy; preserve public theme overrides, intrinsic column geometry, text, editing, commit/revert and read-only behavior without flags or new renderer paths.

- **Coherent default Button material (0.227.4-beta)** — Use a unified light/dark gradient family for ordinary rest, hover, press and disabled states, with a restrained edge, small corners, neutral focus and readable disabled captions. Preserve density metrics, activation/capture, prepared-product priority and existing explicit SDF appearances; no new appearance flags or renderer paths.

- **V3 hover outline rounding (0.227.3-beta)** — Keep the Stage trial's existing hover/pressed border rounded to the same radius-3 face and focus cue. Colours, shadow, geometry, input and authenticated endpoint-pair custody are unchanged; other Button appearances retain their existing radii.

- **Stage E finishing (0.227.2-beta)** — Shorten the Intermediate30 opt-in Stage trial to 84×30 with unchanged 13px caption metrics, round its existing V3 focus cue to the radius-3 face, and strengthen only LightV3's finite external contact shadow. Face tones, grain, material edge, DarkV3 material, V1/V2, other Buttons, endpoint-pair admission and input/custody behavior remain unchanged. Native visual acceptance stays open.

- **LightV3 reference-E tone (0.227.1-beta)** — Lighten only the opt-in LightV3 face toward the owner-selected pale satin reference, retaining the darker concave pressed endpoint. DarkV3, V1/V2, shared shader, perimeter, grain, shadow, paired admission and input/custody behavior remain unchanged. Native visual acceptance remains open.

- **Preadmitted V3 Button endpoints (0.227.0-beta)** — Prepare the existing Stage-only Rest and Pressed materials in two cells of one authenticated texture. Ordinary Update keeps a separate checked press revision; the first affected retained Paint selects its endpoint UV without another GPU preparation/admission cycle. Cold or changed-basis admission, exact-generation custody, certified retirement, colours, typography, focus and on-release activation remain unchanged.

- **Atomic widget availability (0.226.0-beta)** — Add mount-qualified deferred enable/disable requests for Button and the three ordinary editors, plus independent editor read-only requests. The engine settles owner-only input cancellation and scoped focus transfer before Update; read-only retains selection/copy/navigation while rejecting document/history/IME mutation. Construction defaults, typography, geometry and prepared appearance precedence remain unchanged.

- **Showcase idle telemetry (0.225.1-beta)** — Keep status telemetry on update-only demand so unchanged readouts reuse retained paint and skip rendering. Changed readings still invalidate their owner; display-paced updates, input, caret and real animations keep their existing cadence. Optional render-audit output separates updates from actual surface submissions/present calls, and the existing profiler counts local paints.

- **Editable value typography (0.225.0-beta)** — Add the theme's separate 14px input-value token, increasing single-line, password, multiline and editable numeric values from 13px while captions, buttons, popup options and general UI remain 13px. Existing density heights, line height, baseline correction and editing/selection ownership are preserved.

- **Optional reference-E Button V3 (0.224.0-beta)** — Add a separate Stage-only light/dark trial with a thin material perimeter, deterministic filtered satin grain, gentle raised face and finite contact shadow, plus a concave pressed endpoint. V1/V2 sources, default controls, geometry, typography, focus and exact-generation transition custody remain unchanged; JC visual acceptance stays open.

- **Optional 4E Button V2 continuity (0.223.1-beta)** — Keep the authentic committed V2 state while its next generation prepares, then switch Image and custody atomically in retained Paint. Two generations per owner bound overlap; withdrawal and resource failure remain fail-closed. Static material endpoints, default styling and interaction semantics are unchanged.

- **Optional 4E Button V2 (0.223.0-beta)** — Add separately selected light/dark sculpted resting and concave pressed material endpoints driven by the existing pointer/keyboard lifecycle, with checked revisions and exact-generation custody. Showcase Stage uses V2; V1, universal Button styling, 30px geometry, 13px text and independent focus remain unchanged. Native visual acceptance remains open.

- **Button satin resting face (0.222.26-beta)** — Default light and dark Buttons use the approved four-stop vertical gradient without an ordinary stroked border at rest. Focus styling, other state recipes, Intermediate30 metrics and 13px typography are unchanged; shadows and gradual hover/press lighting remain outside this slice.

- **Intermediate30 density trial (0.222.25-beta)** — Add JC's scoped fourth density recipe under binding SPEC-HAL-091A and make it the framework/Showcase default, with nominal 30 logical px controls. Showcase accepts explicit Compact, Regular, Comfortable and Intermediate flags in that order of priority. The original three recipes, typography, UI scale and interaction policy are unchanged.

- **Comfortable default density (0.222.24-beta)** — Halcyon and Showcase now default to the existing Comfortable preset with nominal 32 logical px controls for JC's visual trial. Explicit `--regular-density` selects Regular 28 and `--compact-density` selects Compact 24, with Compact taking precedence when both are supplied. Preset recipes and UI scale are unchanged.

- **Regular default density (0.222.23-beta)** — Halcyon now defaults to the existing Regular preset with nominal 28 logical px controls. Showcase accepts `--compact-density` for the unchanged Compact preset; it takes precedence over the still-supported `--regular-density` flag. Density recipes and UI scale are unchanged.

- **TextArea idle and scroll demand (0.222.22-beta)** — Caret animation demand follows current keyboard Capture during ordinary Update, while independent scrolling remains active through its final displayed position. Changed visual scroll offsets invalidate only the owner's paint; retained focus, editor behavior and the legacy no-publication fallback are preserved.

- **TextField caret demand (0.222.21-beta)** — Standalone editors derive caret animation demand from current keyboard Capture during ordinary Update, so retained focus after release settles without unnecessary repaint. Captured blinking, editor behavior and the legacy focused fallback without appearance publication remain unchanged.

- **Resting Button contrast and density preview (0.222.20-beta)** — Default Buttons retain the existing RaisedClickable resting surface and border without Compact canvas attenuation. Showcase accepts an optional `--regular-density` startup flag using the existing Regular recipe; its default density and control interactions remain unchanged.

- **JC selector outlines (0.222.19-beta)** — Remove the additional enclosing focus rectangles from Checkbox, RadioButton/RadioGroup options and Switch under JC's scoped default-selector ruling. Existing indicators, labels, focus ownership, interactions and animation custody remain unchanged.

- **DropdownMenu static hover (0.222.18-beta)** — Enabled triggers use the existing row-hover background immediately through owner-local hover callbacks. Focus styling, disabled content, geometry and menu actions remain unchanged, with no animation or continuing frame demand.

- **DropdownMenu disabled content (0.222.17-beta)** — Disabled triggers use the existing muted typography tint for both label and caret. Enabled text, ordinary chrome, focus styling, geometry and action behavior remain unchanged.

- **DropdownMenu configured focus style (0.222.16-beta)** — The existing navigation-focus border consumes the theme's focus tint and stroke width through the current border adapter, preserving focus eligibility, ordinary trigger chrome and menu behavior.

- **DropdownMenu framework focus (0.222.15-beta)** — The existing trigger focus border follows current framework ownership and construction-time cue policy, suppressing false PointerDriven and stale-owner cues while preserving trigger appearance and menu behavior.

- **Checkbox press handle custody (0.222.14-beta)** — Existing press replacements and keyboard clears cancel admitted playback when capability is available, otherwise retaining one displaced handle until Update can cancel it. Press visuals, legacy Paint sampling, timing and input semantics are unchanged.

- **Checkbox check publication (0.222.13-beta)** — Checked visuals publish and settle in Update before retained Paint. Pointer preview, release/cancel behavior and silent programmatic sync preserve their semantics, while unavailable animation capability retains admitted check handles until actual cancellation. Press presentation is unchanged.

- **SegmentedControl hover publication (0.222.12-beta)** — Hover progresses and settles through ordinary Update and retained Paint while the pointer is stationary. Paint reads published progress; replacement and unavailable-capability paths retain admitted handle custody without changing selection, focus, geometry or existing hover timing.

- **SegmentedControl framework focus (0.222.11-beta)** — The existing segment ring follows current framework ownership and navigation-cue eligibility. PointerDriven and owner loss suppress false keyboard cues while segment geometry, selection, callbacks and input behavior remain unchanged.

- **RadioButton hover custody** — Unavailable animation capability keeps admitted hover handles pending until Event or Update actually cancels them. Static fallback, normal hover timing, read-only Paint and projected semantics remain unchanged.

- **TabBar keyboard demand (0.222.10-beta)** — Keyboard activation requests a frame only when it emits a selection intent. Active-tab and unavailable-target no-ops stay idle, while external selection ownership and pointer handling remain unchanged.

- **Checkbox hover settlement (0.222.9-beta)** — Hover publishes and settles during Update, with owner-local invalidation and retained cancellation custody when animation capability is unavailable. Existing press/check presentation, active-press hover pin, activation, labels and focus enclosure remain unchanged.

- **TabBar disabled navigation boundaries (0.222.8-beta)** — Left/Right skips disabled tabs without wrapping and retains the enabled target when no candidate exists in that direction. Existing reveal scrolling, external selection, input consumption and focus appearance remain unchanged.

- **TabBar framework focus (0.222.7-beta)** — The current enabled tab gets one immediate theme-driven focus border from framework ownership and cue eligibility, including when selected. External content selection keeps its separate stripe; PointerDriven suppresses the navigation cue, with unchanged tab geometry and input behavior.

- **TabBar retained hover (0.222.6-beta)** — Moving between tabs immediately invalidates the bar's changed hover paint without requesting continuous frames. Same-tab motion stays clean; tab selection, keyboard handling, geometry and scrolling remain unchanged.

- **Switch presentation settlement (0.222.5-beta)** — Hover and thumb transitions publish and settle during Update, leaving Paint read-only. Retargeting cancels owned effects, missed completion and unavailable capability use explicit endpoint policies, and settled controls release local update demand without changing binary activation, label geometry or focus appearance.

- **Selector focus enclosures (0.222.4-beta)** — Checkbox, Switch and the current RadioGroup member immediately show one framework-qualified focus perimeter around the complete indicator, gap and owned label. Owner loss and PointerDriven policy remove the cue without stale fades or duplicate radio rings; selection, activation, hit bounds and ordinary appearance remain unchanged.

- **Switch inset thumb (0.222.3-beta)** — Switch draws a circular movable thumb using the theme's diameter and inset, with a thin neutral border and restrained track corners. Existing track placement, labels, hit bounds, state colors and activation behavior are preserved.

- **ScrollArea resize allocation (0.222.2-beta)** — Bounded Flex allocations use the engine's full-constraint-aware child dispatcher, so nested ScrollAreas receive final tight constraints even when their dimensions match a loose measurement. Retained two-axis resize reversals, collapse and restore keep viewport, scrollbar and fitting-axis scroll geometry current; unchanged settled frames reuse completed geometry.

- **TextField narrow resize (0.222.1-beta)** — Horizontal scrolling uses the existing caret-inclusive extent and clamps before visibility shortcuts. Tight aligned allocations, including widths smaller than the caret and zero, preserve editor state without asserting impossible containment; restoring width settles visibility through normal Layout. The existing mixed-editor regression covers shrink, restore, idempotence and callback preservation.

- **Opt-in full-width SegmentedControl (0.222.0-beta)** — Added `with_fill_width` to distribute finite offered width across equal segments while preserving measured label minima and reporting narrow logical overflow. Intrinsic defaults and `with_equal_width` retain their behavior. Showcase Execution Flags opts in for Live/Replay/Canary/Locked; SPEC-HAL-274 records the scoped JC-approved layout extension.

- **RangeSlider active thumbs (0.221.1-beta)** — Both round thumbs enlarge during either handle's drag and remain enlarged while the range holds keyboard Capture. Per-handle color, keyboard targeting, values and hit geometry retain their existing behavior.

- **Native contact input (0.221.0-beta)** — Added identity-preserving iOS touch transport through InputFusion and the canonical pointer router, with per-contact capture and terminal cancellation. Button explicitly admits touch press–release activation and keeps the initiating pointer until its terminal event; other widgets retain a static opt-in boundary. Existing Mouse/Touchpad input stays on its original route. The additive public event variants advance the zero-major minor version; long-press, pan and pinch recognition remain separate.

- **Gesture cancellation (0.220.1-beta)** — Routed gesture commitment now notifies every registered losing candidate, including candidates still waiting to request commitment. A selectable Label ends its drag when pointer capture is cancelled, preserving its selected range and independent keyboard focus. Nested candidates receive their cancellation and geometry-watch cleanup exactly once; desktop priority and capture ownership are unchanged.

- **Native iOS host support (0.220.0-beta)** — iOS reuses CoreText font discovery/rasterization and UIKit clipboard transport. App can bind its exact primary native window to an application-owned scene host; suspend stops frame admission and releases the primary render target, and resume restores it through the canonical frame path. Desktop platform branches retain their existing behavior.

- **ScrollArea overlay domains** — Opposing overlay scrollbars no longer manufacture movement on a fitting axis. Ordinary final layout and retained initialization determine eligibility from current logical content, including after shrink; genuine overflow retains its edge-reveal allowance. Child constraints and full content extents are unchanged.

- **Restrained desktop chrome** — Built-in themes use achromatic surfaces and quieter slate accents; Compact editable fields use minimal two-unit corners. GroupBox keeps its effective legend metrics while integrating a fine outlined caption with the rounded section frame; explicit colors and square custom borders remain supported. Segmented hover fill stays inside its existing outer corners.

- **Native drag lifecycle** — Native App retires completed layout-transition bookkeeping after Timeline advance. Geometry-dependent Absolute drags ignore terminal transition entries while genuinely active ancestor transitions still cancel safely; no product-side capture bypass.

- **GroupBox readability** — Standard section frames use the neutral theme border; legend sizes now mean their effective font size without hidden scaling. Inputs and Controls inherits the standard frame and surface instead of faint local overrides; explicit colors, fills and padding remain supported.

- **Explicit control composition** — Local alignment groups can opt into value-before-label trailing slots; horizontal Sliders can place their owned caption on the left independently of numeric-field side. Calibration shows both real independent compositions without changing defaults or editor ownership.

- **ComboBox input separation** — Expanded ComboBox keeps its joined owner-driven outer enclosure while a neutral theme border separates the actual input from the popup, above or below; closed and invisible popups add no separator.

- **Local control alignment** — Added opt-in `ControlAlignmentGroup` and value-only Layout parts requests for horizontal Slider/RangeSlider and real TextField/ComboBox rows, including tree-owned `ControlCaption` and transparent TooltipHost forwarding. Local measured label/value reserves align both visible track edges without moving fields, changing values or sharing input ownership. Overflow retains full logical content with traversal clipping; Showcase Envelope demonstrates mixed editable rows in one group; Calibration consistently places both numeric fields on the right in its independent group. Additive capability increments Halcyon to 0.219.0-beta.

- **Slider active appearance** — Default Slider and RangeSlider emphasize the current active thumb or existing handle shape in the accent instead of a separate Capture ring. Navigation remains distinct; decorative emphasis preserves value centers, allocation, pointer mapping and numeric-editor ownership.

- **Showcase standard Commands buttons** — Stage, Apply and Rollback inherit the current theme’s default Button padding and appearance. They remain naturally sized, with labels, callbacks, tooltips, wrapping and row spacing preserved.

- **Allocated slider width** — Horizontal Slider and RangeSlider honor explicit width while preserving natural measurement and label/value sizing. Range painting and pointer mapping share the allocated track; Stretch propagates finite cross width through transparent hosts without constraining an unbounded main axis. Showcase Envelope and Calibration declare stretched rows.

- **Steel-blue built-in accents** — Light and Dark themes use calmer steel-blue accent, hover, tint, navigation-focus and accent-icon roles. Capture, semantic status and neutral colors, control geometry and explicit custom appearances retain their existing owners.

- **Compact Button resting weight** — The default Compact Button softens its resting surface and outline contrast against the configured canvas. Typography, measured rows, natural width, hit bounds, square corners and active/focus cues stay unchanged; explicit Button appearances and other densities retain their existing paths.

- **Default control row metrics** — Standalone and child-backed default Buttons share the configured text-row reserve with TextField, ComboBox and SpinBox. Layout grows natural rows for actual font metrics and the existing optical baseline while preserving child measurement, explicit Button padding/appearance and allocation constraints. Theme font, spacing, padding, radii and colors are unchanged.

- **Joined ComboBox enclosure** — An expanded ComboBox paints one continuous input/popup perimeter from the current geometry, including upward placement, wider or shifted popups and exposed ledges. Both clip scopes use actual framework Focus/Capture appearance; programmatic opening alone does not claim Capture. The shared edge has no border seam, while popup scrolling, overlay routing and selection retain their existing owners.

- **GroupBox legend alignment** — Layout publishes caption metrics through the existing text placement helper. The legend interrupts the snapped top frame on its center with equal side gaps; long captions stay inside the frame. Existing colors, border styles, content ownership and interaction remain unchanged.

- **Slider first-click editing and endpoints** — A primary click in the displayed numeric field enters the existing editor and selects the full value, matching SpinBox; subsequent clicks place the caret normally. Track dragging and value emission stay separate. Embedded alignment geometry is prepared during Layout before the first editor Paint. The blue Track thumb touches the visible track edge at both extrema, with the focus/capture ring outside the track and matching pointer mapping.

- **Editable text cursors** — Enabled writable text fields and compound controls’ actual text-entry regions show the standard I-beam without changing focus, Capture or editing. Cursor output settles in admitted input order across mixed event batches, preserving resize requests, Hidden restoration and relative-pointer lease priority. Dedicated native hosts consume ordered cursor output separately from SceneFrame and restore it on owner shutdown. Added `spawn_ui_thread_with_cursor_output` and `UiThreadCursorOutput`; existing spawn APIs remain compatible. This additive public capability increments Halcyon to 0.218.0-beta. TagInput shares Layout-owned inline geometry with Paint and refreshes it through local Update invalidation.

- **RadioGroup row geometry and mouse-over** — Each row measures its owned label and font metrics during Layout, and reuses those row bounds for Paint and both pointer paths. Stretched blank space no longer selects a shorter option. Mouse-over immediately uses the keyboard-target border highlight on the eligible row without changing semantic focus or Capture, with local paint invalidation only when hover changes. Selection, keyboard focus, row-gap ownership, primary mouse-Down selection and release cancellation are preserved.

- **Radio ownership and presentation phases** — RadioGroup stores member IDs and owned label values instead of embedded RadioButton widgets. Radio focus and standalone hover transitions publish frame-stable values during Update, retire exact terminal/cancelled handles, and invalidate only changed local paint. Existing public APIs, input semantics, geometry and paint primitives remain unchanged.

- **Current tooltip projection** — App-managed tooltips resolve content and placement from the completed frame's current trigger declarations before retained scene reuse and overlay emission. Moved or removed regions stop emitting old tooltip output; empty declarations retire the last trigger. Hover timing stays at the pre-layout owner, with existing frame demand for pending hover settlement. Explicit tooltip anchors and widget consumers are unchanged.

- **Live Dialog body** — Added `Dialog::with_body_update` for an opt-in body producer driven by the mounted widget's existing Update phase and engine delta. Changed text refreshes wrapping, panel and button geometry through ordinary layout without rebuilding the modal or replacing its buttons, focus or capture. Unchanged results retain the current scene; static dialogs keep their existing behavior. The additive public method increments Halcyon to 0.217.0-beta.

- **App-local UI scale** — Canonical `App::run` accepts a typed initial scale and deferred App-owned runtime requests, defaulting to 1.0 independently of platform DPI. Per-target effective density feeds the existing geometry, text, Image/NinePatch and clip owners once; scale settlement preserves editing and keyboard Capture while retiring affected pointer gestures. Readback separates request admission, effective target metrics and output submission. Inactive or deferred primary output does not block prepared child targets. The dedicated UI-thread transport remains at 1.0 and explicitly rejects non-default requests. Native IME support and product element sizing are unchanged. The 0.216.0-beta zero-major minor adds `AppConfig.ui_scale`. The runnable `app_ui_scale` example demonstrates initial and runtime scale, public readback, text, Image/NinePatch and clipped scrolling.

- **Button caption alignment** — Added explicit caption placement using the existing TextAlign enum, with Center unchanged by default. Left and Right use the admitted content padding; update-time changes invalidate only local paint and preserve layout, hit bounds and input behavior.

- **Explicit Button caption placement** — Added an opt-in signed logical-pixel text offset for standalone and child-backed Buttons. The default is zero; update-time replacement invalidates only local retained paint and preserves layout, surface and input geometry. Product placement remains separate from the shared platform optical correction.

- **Opt-in pointer-driven UI** — Added construction-time `UiInteractionPolicy` to App and UI-thread configuration. The default Navigation/Capture contract is unchanged. PointerDriven retains real pointer interaction, explicit Screen/App commands and editor/physical Capture while disabling focus-only keyboard UI delivery and navigation cues. Unhandled Capture Tab releases without traversing or capturing a neighbor; Escape retains its existing priority. Framework appearance publishes cue eligibility separately from honest focus/capture facts.

- **First-frame startup visibility** — Canonical App startup keeps the primary native window hidden while the existing frame loop settles required Screen resources. The ready primary output is submitted and completed on the GPU before revealing the window and immediately presenting its retained texture. Windows suppresses DWM transitions only for that first reveal and restores them after composition. Added the read-only `Screen::startup_ready` declaration and hidden-startup settlement of lower modal producers; startup failure reports its cause instead of leaving a hidden process waiting. Resource admission, fullscreen sizing, ordinary public Window constructors and later frame scheduling retain their owners.

- **Escape routing priority** — The shared native/UI-thread input path releases keyboard Capture and unwinds structural scopes before offering the original unhandled Escape to Screen/App handlers. Removed the obsolete synthetic focused-widget Escape consultation; configured modal dismissal keeps its existing owner. Root-scope Escape preserves focus, and physical Capture interruptions retain their existing settlement path.

- **Physical keyboard identity and response capture** — Added explicit physical key positions alongside logical keys/text, discriminated logical/physical bindings, snapshot-based physical KeybindEditor recording and typed interruption of opted-in response capture at observed reservations and focus loss. Existing logical UI defaults and fullscreen/clipboard actions retain their behavior. The 0.211.0-beta zero-major minor adds physical/reservation fields to public key-event records and changes Shortcut/InputPattern/KeyChord key literals to an explicit BindingKey; legacy synthetic literals use Unknown and no reservation. Native-input and external-consumer acceptance remain separate.

- **Explicit UI typography** — Added immutable admitted face bindings and nonnegative shaped-cluster spacing for Label and standalone/child-backed Button text, carried through measurement, logical scenes and Compile rasterization. Source-cluster selection and wrapped-line terminal spacing follow the same shaping operation; typography replacement invalidates layout and retained paint. The 0.210.0-beta zero-major minor adds fields to public TextStyle, GlyphRun, TextNode, PrimitiveData::Text and AppConfig records; existing struct literals require their explicit default fields. Default SystemUI bootstrap and ordinary input policies remain unchanged. Native/product font acceptance remains separate.

- **Native startup fullscreen** — Added `AppConfig::with_fullscreen` with a windowed default and a creation-time native Borderless fullscreen request. Initial compositor, base-surface and navigation bounds now use the obtained window extent normalized once by its actual scale; requested windowed size and existing resize/fullscreen controls are preserved.

- **Retained geometry and Visible overflow** — Executed layout and changed inherited geometry invalidate their actual paint/composition dependencies while clean skipped siblings retain their fragments. Overflow changes publish viewport and clip state before the first successor; canonical and accelerated hit traversal preserve Visible descendants outside non-clipping ancestor allocations while respecting root, explicit clip and scrollport boundaries.

- **Geometry-dependent Absolute drag retirement** — Added explicit geometry dependencies and sealed physical-press routing. Affecting allocation, reflow and admitted scroll effects retire the exact gesture before stale input or the first affected scene, with complete cancellation and final capture appearance settlement. Ordinary unqualified controls and existing frame/layout ownership remain unchanged.

- **Workspace preview composition** — Added an explicit decorative main channel for current handle overlay output above closed control subtrees, preserving source ownership, clipping, retained reuse and ordinary input/capture.

- **Public frame-begin accounting** — EngineTicker now initializes layout-call accounting for each genuinely separate frame, including deterministic public CPU scene consumers. Scene helpers preserve the same-frame single-layout guard; native App uses the same frame-begin owner.

- **Explicit control-subtree order** — Added closed ordinary-control composition with matched paint/hit member order and retained child-local reuse on reorder. Existing primitive Z, overlay placement, keys and Image generations remain unchanged. Hit-test acceleration covers every intersected viewport cell and validates current ancestor clips/scroll coordinates. This zero-major minor adds optional composition fields to publicly constructible retained-fragment, batch-key and path-draw records; their existing struct literals require the ordinary `None` field. `PathSortKey` now carries the complete effective order and is Clone rather than Copy; construct it through `from_path_draw`. Ordinary Button layout, rendering and activation now use the same constrained final size.

- **Retained horizontal scroll continuity** — Added explicit sequential-X attachment at ScrollArea while keeping ordinary horizontal links live-only. Reconstructed areas restore presented X/Y once against final bounds before paint, and newer scroll intent wins. Supporting ListView bodies supply current logical metrics so the scroll owner settles retained coordinates before the first visible-window realization.

- **Absolute drag cancellation** — Cancelled absolute terminals discard pending gesture commitments and deliver the existing candidate/capture-owner cleanup in PostProcess. Clarified the public external Widget/DragStart contract and proved positioned handle routing, lossless preview cleanup and narrow paint invalidation.

- **Native NinePatch Button acceptance** — Added a durable native Button example showing asymmetric Stretch NinePatch borders and proportional small-destination collapse through the admitted Image path. Fixed the native scene-cache handoff to retire an old primitive-kind generation before inserting its replacement under the same widget-owned visual-root key.

- **Button Stretch NinePatch profile completed** — Ordinary Button surface and underlay admit square Stretch NinePatch materials with current ImageBinding evidence and expose their existing Image-cell lowering through appearance inspection. Exact binding lifetime, narrow retained invalidation and production Image shader GPU readback are proved; SPEC-R-019 is Implemented.

- **Core Stretch NinePatch material** — Added typed source/destination insets and immutable validated `SurfaceFill::NinePatch`, with non-allocating row-major partitioning into at most nine existing Image primitives on legacy and admitted-binding PaintSink paths. Small destinations compress borders proportionally; only the first emitted cell carries the caller's visual-root key. Button admission and production visibility proof remain pending.

- **Stretch NinePatch architecture adopted** — SPEC-R-019 is now the binding rectangular SurfaceFill material contract: normalized source splits, logical border compression and at most nine existing Image primitives, with an initial square Button appearance profile. Implementation remains pending.

- **Deferred decoded Image registration** — `GpuResources::queue_image_rgba8_srgb` accepts owned decoded straight-sRGB RGBA8 pixels with synchronous typed validation, reuses the renderer's existing normalization/upload path, and resolves generation-specific `ImageBinding` evidence through the existing admission wake and retained-scene servicing.

- **Retained paint after direct activation** — Direct keyboard activation now dirties changed retained paint in RadioGroup, Checkbox, SegmentedControl, DropdownMenu, Breadcrumbs, and static Switch/DisclosureSectionHeader fallbacks. PM-018 records the failure class, and startup doctrine requires matching PM lookup for bug/regression work.

- **RadioGroup mouse and keyboard selection** — Primary mouse Down selects immediately; mouse Up and mouse capture cancellation clear armed state without another selection change. Keyboard navigation moves focus only, while `i` / `Enter` / `Space` activate the focused member. The contract distinguishes normalized input mapping from the device-agnostic semantic reducer.

- **Slider label pointer targeting** — Actual label clicks focus the Slider without changing its value or starting drag. Hit testing uses layout-owned label geometry shared with paint, preserving track/body interaction and value-field editing instead of swallowing every point beyond the track end.

- **Capture-specific control focus visuals** — Restored `capture_color` for captured control borders and chrome. Slider keeps its cue local to the indicator, separates the Track outline from the thumb fill, and paints the cue above the control.

- **Slider indicator-local focus and capture** — Navigation and keyboard Capture outline only the Track thumb, Bar/Pill edge handle or Radial dial. The paint-only outset stays within the existing control paint rect, remains visible at endpoints and excludes owned label/value regions. Equal configured widths still add one device pixel only in Capture; distinct widths are preserved. Shared Theme defaults, input, value, layout and hit geometry remain unchanged.

- **Radial Slider value-field side** — A Left-side value field now uses the width already reserved by layout: the dial and label shift right, and radial pointer angles use the same existing cached offset. Right-side placement, value editing and Track/Bar/Pill behavior remain unchanged; no public API or new state is added.

- **RangeSlider focus and capture visibility** — All RangeSlider display styles paint an outline around the current handle, bounded by the existing control paint rect so endpoint cues remain inside an unchanged parent clip. When configured focus/capture widths are equal, only RangeSlider adds one device pixel in Capture; distinct configured widths are preserved. Shared Theme defaults, handle ownership, value, input, layout and hit geometry remain unchanged.

- **Captured pointer focus redirect context** — Widget-initiated pointer focus redirection retains the current routing text engine while releasing the old captured owner. New-owner capture entry and paired release use the same engine; existing focus, mode and redirect behavior remain intact.

- **Modal capture teardown context** — App's polling input boundary supplies its current text engine when clearing a captured focus outside the modal root. Existing focus clearing, capture-leave ordering, pointer ownership and public helper compatibility remain unchanged.

- **Capture release action lifecycle** — `RELEASE_CAPTURE` delivers capture-leave with the current text engine after publishing Navigation, once per actual capture transition. App Escape reuses that action; focus, independent pointer capture, widget values and paired text suppression remain unchanged.

- **Pointer keyboard-capture entry** — A normal primary click on the focused capturable target now enters Capture through the existing action owner after target directives. Slider arrow editing works immediately after the click; capture preemption, explicit release, existing capture requests and ordinary Button activation remain intact.

- **Deferred capture lifecycle context** — Layout-produced focus/capture requests retain the current App text engine through old-owner capture-leave and newly mounted editor capture-enter. Both input loops use the same bridge; next-frame appearance publication, localized repaint and unchanged layout/sibling retention remain intact.

- **Pointer focus-transfer context** — A primary click transferring focus from a keyboard-captured widget delivers capture-leave with the current App routing text engine. Capturable peers still enter capture and ordinary Buttons still activate once through the same click; focus, routing and retained invalidation behavior remain unchanged.

- **Window pointer cancellation** — Window focus loss notifies normal pointer capture owners before clearing domain records, independently of keyboard focus. The existing Fused fallback also delivers lossless cancellation; duplicate loss stays inert and pure cancellation keeps layout and unaffected retained siblings unchanged.

- **Window capture teardown context** — Window focus loss now delivers capture-leave with the current App text engine before clearing focus and domain authority. Duplicate loss still notifies once; remembered focus, local repaint, sibling retention and layout remain unchanged.

- **Keyboard capture lifecycle context** — Widget-requested capture entry and focus-redirection capture entry retain the current routing text engine, so capture callbacks can rebuild text immediately. Paired release reborrows the same engine; focus, mode, and routing authority remain unchanged.

- **Pointer capture cancellation** — Actual captured owners now receive a terminal callback after capture revocation, independently of gesture candidates. Button clears cancelled pointer activation and its pressed appearance, preserves an independent keyboard press, and rejects stale releases without changing focus or layout. Cancellation repaints only the affected owner and its composition ancestors.

- **Button pointer activation** — A private interaction record now arms primary presses and consumes each valid release once. Leaving the canonical bounds or losing focus cancels activation and its current pressed appearance; re-entry and stale releases cannot click. Visual state changes do not grant or cancel input authority, and fresh pointer cycles still activate normally.

- **Window focus-loss lifecycle** — Both input loops deliver the existing focus/capture leave callbacks and remember the old focus target before interaction-domain teardown clears it. Held Button keyboard presses now cancel on deactivation, restored focus does not activate a cancelled press, and focus-only changes retain sibling fragments and layout.

- **Button appearance no-op scheduling** — Reapplying identical appearance inputs or the already-selected Framework default does not renew settled update demand. Pending publication, explicit padding changes, configuration generations, selection and interaction work remain live; an equivalent Image request still validates its current resource generation.

- **Button appearance inspection** — `Button::inspect_appearance` describes the coherent last Update request/result, padding, ordered layers and supported inherited paint profile on demand. Image generation availability is reported separately from semantic resolution and does not claim execution or select a fallback. Failed resolution retains its complete request; ordinary frame processing adds no inspection storage or allocation.

- **Image admission mailbox retirement** — Renderer teardown cancels outstanding admission requests and closes its CPU mailbox to later requests from surviving `GpuResources` clones. Concurrent enqueue cannot leave work behind a retired renderer; unused mailboxes allocate no pending queue during cleanup.

- **Image execution generation custody** — Replacing or unregistering an Image registration retires the current compiled preparation before target acquisition or encoding. An admitted generation cannot silently execute with a same-number replacement; a missing-registration unregistration remains a no-op.

- **Gradient stop interpolation** — Non-endpoint stops preserve their endpoint color and alpha outside the stop interval; every positive stop interval interpolates without an arbitrary minimum width. Equal-offset stop selection, linear premultiplication and clipping remain unchanged.

- **Multi-surface Gradient parameters** — Compile prepares one bounded immutable Gradient parameter table for all targets, preserving each surface's colors, gradient coordinates and rounded clips. Capacity exhaustion refuses compilation before staging work instead of substituting offset zero.

- **Transient source retirement** — Update releases current certified sources when their widget or configured owning scope disappears, without waiting for paint. Surviving sources keep their existing certificates; presentation group retirement leaves input and focus authority unchanged.

- **Transient source font accounting** — non-text certified sources retain no font generation and remain reusable across unrelated font changes. Text sources continue to retain, charge and validate their exact immutable font backing.

- **Generation-specific Image admission** — the existing renderer issues CPU-only evidence for actual Image registrations, with explicit pending requests, replacement/revocation and exact generation validation. Ordinary Button admits square bound Image surfaces without owning GPU resources; retained resource changes repaint only consumers and ancestor composition. Legacy Image sampling and premultiplied-texture/straight-linear-tint behavior remain unchanged. `AppearanceImageSource` now uses `Clone` instead of `Copy`; `GpuResources` adds the evidence mailbox.

- **Typed rectangular SurfaceFill** — share Solid, immutable bounded Gradient and owner-typed Image values between paint and appearance. The rectangular helper reuses existing primitives; ordinary Button also accepts Gradient while Image/resource admission stays explicit and pending there. Solid emission adds no material allocation. Appearance values containing Gradient use `Clone` instead of `Copy`; configuration, metrics, input and focus ownership are unchanged.

- **Gradient material correctness** — preserve the already-linear `Color` input when premultiplying gradient stops, and apply the current rounded parent clip in main and overlay rendering. Clip data stays in existing Gradient uniform slots without extra Solid work or shared Rect/Image clip capacity.

- **Production macOS relative locked pointer** (`SPEC-HAL-362`, `0.200.0-beta`) — after pinned AppKit point-unit audit, two-display native lock/cleanup captures, real App lifecycle capture, deterministic denial-path proof, and operator cursor observation, the bound macOS window advertises `RelativeLocked` with `DownAnchor` and normalization epoch 1. Acquisition remains fallible; the input-fusion debug example logs canonical relative motion, and conformance reports retain their evidence-only UNPROVEN aggregate row.
- **macOS cursor visibility after focus loss** (`SPEC-HAL-362`) — on focus regain, revalidate the pinned AppKit window's cursor rectangles after relative-lock cancellation so a cursor shown while the app was inactive is not left invisible inside its window. Native and engine diagnostics record whether that AppKit refresh ran; visual confirmation remains an operator check.
- **Offline SPEC-HAL-362 report replay** — the macOS conformance binary can recalculate native or engine verdicts from saved JSONL with the current analyzer, validating capture identity and contiguous records without opening a window. Partial captures retain their missing obligations as UNPROVEN. Duplicate-lock denial no longer gates native capture completion because it cannot prove native OS denial; `Q` can record active-lock shutdown without repeating motion coverage from an earlier run. The diagnostic never enables production support.
- **macOS point-unit relative-motion proof** (`SPEC-HAL-362`) — the audited AppKit `NSEvent.deltaX/Y` identity path now requires real locked/unlocked capture at one backing scale; two-scale capture remains required for physical-pixel or scale-dependent conversions. The guided verdict retains failure on any observed mismatch and keeps multi-display, scale-change cleanup, and human cursor evidence separate from normalization.
- **macOS relative-pointer denial proof** (`SPEC-HAL-362`) — when the pinned host API has no safe way to force a genuine lock refusal, the platform proof may use an audited unforceability record and deterministic adapter-to-engine denial tests for that stimulus only. Real lock acquisition/release, every other native requirement, and typed fail-closed cleanup remain mandatory. Conformance reports identify observed native denial and keep independent proof under review.
- **Real-engine macOS relative-pointer diagnostic** (`SPEC-HAL-362`) — the debug-only `--engine` mode of the existing conformance binary mounts a relative-gesture owner in actual `App::run`, routes native motion through the real fusion/router/lease path, and records ordered cleanup for Up, real owner unmount, semantic cancellation, focus loss and shutdown. Its support decorator is isolated from ordinary `AppConfig`; reports retain UNPROVEN for genuine native OS denial and human cursor visibility that JSONL cannot establish.
- **macOS relative-pointer conformance capture** (`SPEC-HAL-362`) — guided evidence now derives window-edge continuation numerically from the bounded anchor, captured window extent, and canonical deltas; requires real locked absolute callbacks at the captured backing scale; combines the pinned AppKit source/unit proof with runtime identity checks; and reports monitor/scale invalidation without unsafe restore separately. Native OS denial, real engine lifecycle, and human cursor visibility require separate review outside the native capture.
- **Same-frame relative-pointer semantic cancellation** (`SPEC-HAL-362`) — `Screen::update` can queue a non-structural owner-and-pointer cancellation intent on `WidgetTree`. Both App frame loops drain it after destroyed-widget cleanup; EventRouter validates the active binding and uses the existing typed terminator with current window facts. Safe cursor restoration, unlock, visibility, capture, session, and pending-motion cleanup finish before layout without waiting for another input event. Owner unmount retains forced-cancel precedence and never warps the cursor.
- **Relative locked pointer engine routing** (`SPEC-HAL-362`) — committed relative gesture winners now bind an owner-only lease to matching hard capture and the original pointer-down anchor. Canonical deltas route only to that owner; normal PointerUp completes once and restores a safe anchor before unlock/show, while focus, capture, overlay, owner, normalization, and shutdown exits cancel with typed outcomes and clear pending motion. Without a platform authority, admission fails explicitly; native backend activation remains separate.
- **Visible EventLog expanded detail** (`SPEC-HAL-313`, `src/widgets/list_view/body.rs`) — the detail background now renders below its mounted child content in strict Z order, restoring the expanded TextBlock text while preserving the row frame and separators.
- **Application frame realization observation** (SPEC-HAL-361, `0.199.0-beta`) — Canonical `App::run` associates custom preparation with an opaque application attempt ID and exposes an opt-in, cloneable latest-snapshot observer through AppConfig and AppContext. Deterministic per-surface receipts preserve exact HAL-360 text/FrameEpoch evidence separately from actual output, queue submission and present-call facts. Incomplete text and missing output cannot claim successful submission; startup and skipped/failed Compile remain explicit. Latest-only retention and exact attempt/surface matching prevent older receipts from certifying newer candidates. No GPU or display-completion guarantee is implied.
- **Complete-required text realization** (SPEC-HAL-360, `0.198.0-beta`) — Compile exposes immutable text preparation outcomes for each surface and frame. Missing glyphs, raster/storage failures and atlas capacity refusal prevent that surface from being encoded or acquired for presentation; valid no-ink and empty surfaces remain complete. Frame working-set protection and final UV lowering preserve admitted glyphs across atlas growth, eviction and later-surface pressure. Superseded/foreign compiled plans are refused. Native backend signatures remain unchanged; no-ink uses explicit empty bitmap storage. The default ListView settings button uses the built-in vector SETTINGS icon, preserving its geometry and interaction without a U+2699 font dependency. Preparation evidence does not claim GPU or presentation completion.
- **Overlay pointer-route isolation** (`src/core/event_router.rs`, `src/widgets/list_view/`) — EventRouter now preserves whether a pointer hit came from overlay or ordinary content while retaining the normal logical capture/target/bubble path. Descendant popup motion can no longer be reinterpreted as covered ListView row/header/scroll-surface interaction, and overlay PointerDown bypasses underlying virtualized projected selection. A routed ListView-cell ComboBox regression proves popup hover/commit stays with the option list while the covered row remains unhovered and unselected.
- **Legible ComboBox popup perimeter** (`src/widgets/combo_box.rs`) — elevated ComboBox popups now use the existing strong-border token at one logical pixel, matching `MenuSurface` instead of collapsing to a low-contrast single device pixel on Retina displays. A 2× WGPU readback regression recreates the horizontally scrolled ListView cell popup and proves a closed, two-device-pixel, contrast-bearing border on all four sides.
- **Logical paragraph line order** (`src/core/paragraph.rs`, `src/widgets/text_block.rs`) — recursive soft wrapping now emits completed lines in source order while retaining only the final partial line for a following run. Three-line paragraph and retained `TextBlock` regressions prevent the former `1 / 3 / 2` paint sequence.
- **Visible ComboBox popup chrome** (`src/widgets/combo_box.rs`) — the popup now uses one strict owner-local ladder for its opaque surface, option text/backgrounds, scrollbar chrome, and outer border. This restores the ListView inline-choice dropdown border after strict cross-pipeline Z execution and prevents row backgrounds from hiding the scrollbar.
- **Explicit text Z ownership** (`SPEC-HAL-048`, `SPEC-HAL-359`, `SPEC-PM-007`, `src/core/text_engine.rs`, `src/platform/`, `src/widgets/`) — every `TextEngine` draw entry point now requires and preserves a caller-owned `ZIndex` instead of silently emitting visible text at zero and relying on the renderer's former text-last pipeline order. Composite widgets and examples assign strict local layers for text, fills, borders, popups, and overlays; behavioral paint-graph regressions cover Property Editor, Order Book, and Console while engine-level proofs cover ordinary and background-bearing text submission.
- **Cross-pipeline strict-Z execution** (`SPEC-HAL-359`, `SPEC-PM-006`, `src/renderer/compiled_draw_plan.rs`, `src/renderer/render_graph/passes/`) — Compile now produces one immutable per-surface schedule spanning ordinary batches, images, executable paths, and text runs; main and overlay passes consume each item according to its own `ZIndex`. Strictly higher Z now composes later across pipeline families, including Text, while equal-Z ties remain deliberately non-semantic under SPEC-PM-007. Actual GPU-readback regressions cover text below/above opaque rectangles in both pass scopes, unchanged rectangle/rectangle and two-sided image/rectangle controls, and executable Path ordering without empty-path index drift.
- **Image registration lifetime across pipeline revalidation** (`SPEC-HAL-263B`, `src/renderer/wgpu_renderer.rs`) — same-device glyph-atlas, target-scale, and surface-format pipeline generation changes now preserve every registered Image texture/view and its Linear/Nearest sampling policy while rebuilding only layout-derived bind groups. Final `INV-IMG-005` validation now runs after revalidation, so Execute cannot receive a batch whose registration Compile deleted; release builds remove genuinely invalid batches from the executable surface plan.
- **Widget-safe closed stroked paths** (`SPEC-R-004`, `src/core/paint.rs`, `src/core/scene/mod.rs`) — added an object-safe `PaintSink::push_stroked_path` operation that accepts `Path` plus `StrokeParams` while keeping tessellation and mesh ownership inside Halcyon. `Path::close` now survives the widget paint boundary as a true closed contour with joins rather than a repeated-start open polyline with endpoint caps; the resulting stroke node retains ordinary clip, scroll-transform, z-index, and scene-key behavior without copying the generated mesh vectors.
- **Zoned Luminous Contour material** (`SPEC-HAL-358`, `SPEC-HAL-085A`, `src/core/paint.rs`, `src/renderer/shaders/shape.wgsl`, `examples/luminous_contours.rs`) — extended the one-pass analytic contour with a bounded thin core luminance lift, narrow saturated inner aura, and wider weak outer halo. The zones share one coverage owner and combine without extra geometry, blur, bloom, or additive join hotspots; moving fronts continuously strengthen only the crisp core and near-field aura. The compatible `try_new` single-zone result remains available, while `try_new_zoned` and live preset controls expose the explicit material. The calibrated `BALANCED` proof reads as a pale source with rich cyan/amber near-field and faint far-field separation at normal screenshot scale while retaining 1:1 update/submission/presentation cadence.
- **Luminous Contour live-tuning lifecycle** (`SPEC-HAL-358`, `examples/luminous_contours.rs`) — tuning keys now mutate retained paint state without requesting a structural screen rebuild, so presets and direct controls no longer trip the one-root rebuild invariant or restart moving fronts. Legitimate later rebuilds remount exactly one root child.
- **Luminous Contour rendering** (`SPEC-HAL-358`, `src/core/paint.rs`, `src/core/scene/mod.rs`, `src/renderer/wgpu_renderer.rs`, `examples/luminous_contours.rs`) — added a checked screen-space line/polyline primitive with immutable shared point storage, an allocation-free bounded set of consumer-positioned contour fronts, analytic core/halo falloff in the existing one-pass Shape pipeline, and `u32` multi-surface shape indices. Complementary joint ownership now prevents straight, corner, and closed-path halo accumulation; fronts modulate the same-hue halo continuously without white overlay geometry. Existing ordinary strokes retain their path route. The dedicated resize-aware black example compares ordinary and luminous line, curve, square, and sampled-circle contours, exposes live `SUBTLE`, `BALANCED`, and `STRONG` appearance controls with displayed values, exercises moving fronts without importing temporal semantics into Halcyon, and reports renderer cadence under `render-audit`.
- **Stable spatial-domain basis cue** (`examples/spatial_domains_3d.rs`) — replaced separately overlapping planar shaft/head pieces with one checked example-local arrow builder that emits a connected, non-overlapping CCW silhouette for every direction, eliminating the moving bright patch caused by equal-depth subpixel coverage and front/back appearance disagreement.
- **High-refresh swapchain cadence** (`SPEC-HAL-030A`, `src/renderer/wgpu_renderer.rs`) — restored the canonical WGPU window-surface queue-depth hint from `1` to `2` and made registration/resize share one renderer-owned configuration constructor. On the measured 120 Hz Metal path, the exact `spatial_domains_3d` release example rose from approximately 60–66 to sustained 117–118 completed frames/s without changing engine pacing, Viewport3D's separate two-texture offscreen publication, animation arithmetic, materials, or shared spatial-domain authority.
- **Shared 3D spatial-domain composition** (`SPEC-HAL-357`, `src/three_d/domain.rs`, `src/three_d/viewport.rs`, `examples/spatial_domains_3d.rs`) — added context-admitted one-level `Planar2` and `Spatial3` domains with checked positive-uniform-scale similarity embeddings, immutable structural/state/frame identities, deterministic lowering into the existing flat 3D renderer, domain-specific aggregate retained-metadata limits/usage/errors, pure local/world point-vector-ray conversion, and exact committed-domain associations. Domain-aware `Viewport3D` requests preserve shared depth, independent placement revisions, compatible last-good interaction evidence, boolean committed-association currentness, and narrow invalidation. Legacy flat requests remain source-compatible and perform zero domain admission, lowering, association, or accounting work; the pre-domain exhaustive `PolicyLimits3D`, `PolicyBudget3D`, and `Context3DError` shapes remain unchanged. The domain-neutral example makes that contract visually explicit with a delicate dark technical presentation: a large hairline amber Planar2 Cartesian grid has subordinate minor/major lines, a thin finite-edge treatment, and a restrained RGB local-basis cue, moves through a low-mass opaque cool-blue Spatial3 cube with crisp cyan edges, and retains a fixed camera.
- **Last-good committed 3D rays** (`SPEC-HAL-355`, `src/three_d/viewport.rs`) — an active compatible `FailedWithLastGood` target now continues to expose its old committed-view ray while newer requested camera/model revisions remain distinct. Inactive or incompatible targets still expose no committed ray.
- **Showcase unavailable-presentation wheel teardown** (`SPEC-HAL-356`, `examples/showcase/tab_viewport_3d.rs`) — the 3D consumer now derives wheel-Timeline availability from the actual custom-render preparation outcome. Inactive, terminal, first-commit-missing, and active failure paths without a published front retain the handle only until the next owner update cancels it and drops its anchor before sampling; a failed preparation with a valid last-good presentation remains available.
- **Showcase geometric-anchor traversal fix** (`SPEC-HAL-356`, `examples/showcase/tab_viewport_3d.rs`) — the scoped nearest-geometric-hit prototype now continues across individual triangle misses before selecting the nearest declared hit, so a cursor over a later triangle or the closed-volume cube no longer falls back to centre-only FOV zoom merely because an earlier triangle missed. Added direct outward-FOV anchor composition and outward cube-normal conformance proofs.
- **Showcase closed-volume cube** (`SPEC-HAL-356`, `examples/showcase/tab_viewport_3d.rs`) — added a named `ClosedVolumeCube` to the visible Fly-LMB target selector. It is Showcase-local indexed closed geometry with six outward-normalled faces and twelve CCW exterior triangles, a dedicated normal/vertex-colour appearance, and an oblique placement for legible depth; it remains an explicit consumer figure rather than generic primitive or picking machinery.
- **Perspective wheel return limit** (`SPEC-HAL-356`, `src/three_d/navigation.rs`, `examples/showcase/tab_viewport_3d.rs`) — Perspective optical FOV now treats its admitted initial/default FOV as the exact wide endpoint: inward wheel input narrows to the existing finite minimum, while outward input restores only to default and cannot produce over-wide edge stretch. The 80 ms consumer transition still composes from its latest target and rebases from its displayed state; orthographic two-sided span zoom is unchanged.
- **Showcase Fly entry and bounded velocity response** (`SPEC-HAL-356`, `examples/showcase/tab_viewport_3d.rs`, `src/three_d/navigation.rs`, `src/core/app.rs`) — fixed Fly startup/toggle keyboard activation to retain a Showcase-local pending capture entry until EventRouter actually enters capture on the mounted viewport, rather than treating deferred-focus scheduling as success. Routed W/A/S/D now drive the normal update path before any free-look gesture. Fly target speed rises from 2.5 to 3.0 world units/s (12.0 with Shift); controller-owned, engine-`dt` bounded velocity response accelerates toward held input and decelerates after release, then is cancelled by reset/mode/focus lifecycle without a second scheduler.
- **Fly tether-orbit and vertical-binding removal** (`SPEC-HAL-356`, `src/three_d/navigation.rs`, `examples/showcase/tab_viewport_3d.rs`) — removed the default world-vertical `Space`/`X` Fly translation bindings. Space is now a captured Fly-only modifier: ordinary RMB remains free look, while Space+RMB selects the existing bounded orbit rotation around the controller's explicit current `orbit_target` and retained radius. The tether branch does not hide the cursor, select a scene target, or use the Showcase selector/wheel anchor; Fly LMB remains the separately owned named-figure local-Z transform.
- **Showcase 3D selector and Fly activation repair** (`SPEC-HAL-356`, `src/widgets/radio_group_widget.rs`, `examples/showcase/tab_viewport_3d.rs`, `src/core/app.rs`) — fixed labeled RadioGroup measurement under a `Flex::row`: controls now retain a finite natural layout and full row hit target instead of painting labels beyond a zero-width hit rect. Routed named-figure clicks now flow through the ordinary binding state into Fly-primary local transforms. Default Fly and a routed Fly toggle both enter viewport keyboard Capture through the deferred EventRouter lifecycle, so W/A/S/D motion works before any free-look drag.
- **Showcase 3D navigation lifecycle repair** (`SPEC-HAL-356`, `examples/showcase/main.rs`, `examples/showcase/tab_viewport_3d.rs`, `src/core/app.rs`, `src/core/widget_tree.rs`) — custom-render inactivity and reset/control changes now retain, then owner-correctly cancel the pending wheel Timeline before any later sample, dropping the complete anchor record. Fly/Orbit changes use the EventRouter’s ordered deferred focus lifecycle to release active look/transform pointer capture and restore the cursor before Fly immediately enters keyboard Capture. The normal UI-thread loop now flushes form bindings like the native loop, so every visible 3D control applies; Fly is the Showcase default.
- **Explicit Showcase figure transform** (`SPEC-HAL-355`, `SPEC-HAL-356`, `examples/showcase/main.rs`, `examples/showcase/tab_viewport_3d.rs`, `src/three_d/math.rs`) — added a visible four-figure source-owned selector and Fly-primary local-`+Z` `TransformRotate` gesture. It updates only the selected figure's complete placement while preserving translation/positive scale and composing the named backface-probe spin separately; camera input, wheel-anchor geometry, selection/picking, hierarchy/root transforms, translate/scale, gizmos, cursor hiding, and keyboard capture remain outside this vertical.
- **Explicit 3D camera operation bindings** (`SPEC-HAL-355`, `SPEC-HAL-356`, `src/three_d/navigation.rs`, `examples/showcase/tab_viewport_3d.rs`) — named each admitted Showcase camera binding as orbit rotation, Fly free look, camera pan, Fly translation, Perspective optical-FOV zoom, or orthographic-span zoom, each targeting checked camera state. Model transform operations, semantic transform targets, and selection/picking remain distinct application-owned policy; no scene/root rotation or nearest-geometry wheel anchor is promoted into a reusable transform or picking contract.
- **Smooth Showcase 3D wheel zoom** (`SPEC-HAL-356`, `src/three_d/navigation.rs`, `src/three_d/viewport.rs`, `examples/showcase/tab_viewport_3d.rs`) — replaced target-relative perspective wheel dolly with bounded optical FOV zoom, scaling checked `tan(vertical_fov / 2)` while preserving camera pose, orbit metadata, and near/far planes; orthographic wheel remains pose-preserving span zoom. Showcase now prototypes cursor anchoring only from its active committed view/model/snapshot/target and four explicitly declared local triangle lists: the controller couples FOV to checked camera-plane compensation, retains one accepted anchor across 80 ms Timeline rebase/composition, and falls back deterministically to centre-only FOV zoom on every stale/invalid/miss path. Alpha masks and culling are intentionally not mirrored, so it is not rendered-visible or generic picking.

## [0.196.2-beta] — 2026-08-24

### Fixed
- **Capture-owned 3D drag cursor lifecycle** (`SPEC-HAL-356`) — bound cursor hiding to the active pointer-capture owner, added idempotent visibility restoration across normal completion, capture denial/cancellation, Escape/Tab, focus loss, invalid targets, destruction, rebuild, and shutdown, and made Showcase Fly look hide only for admitted secondary-button drag while leaving primary scene input and target-relative wheel/dolly behavior unchanged.

## [0.196.1-beta] — 2026-08-24

### Fixed
- **Concurrent Fly interaction authority** (`SPEC-HAL-356`) — changed the configurable default pointer policy to secondary-button Fly look and auxiliary-button pan while leaving primary input on the ordinary scene path, preserved held keyboard movement independently of the look-capture lifetime, and changed vertical Fly defaults to Space/X with optional Shift speed modification.
- **Bounded logical-pixel 3D dolly/zoom** — replaced abstract-unit wheel response with checked sensitivity per canonical logical scroll pixel, a finite fused-event admission cap, scale-relative derived perspective/orthographic bounds, deterministic saturation at both limits, and target-relative Fly dolly that cannot cross the target or translate without bound.

## [0.196.0-beta] — 2026-08-24

### Added
- **First-class 3D camera navigation authority** (`SPEC-HAL-356`) — adopted the binding owner split for checked camera presentation state, optional application/widget Orbit and Fly policy, explicit EventRouter capture, engine-`dt` held motion, pose-preserving mode switches, camera-only pan/strafe, and honest absolute-pointer portability limits.
- **Reusable 3D camera controller** (`CameraNavigationController3`) — added checked perspective dolly, orthographic zoom, orbit rotation, pointer pan, fly look, normalized forward/back/strafe/vertical motion, configurable logical key bindings, bounded speed modification, repeat-free held-key lifecycle, and transactional cancellation/reset operations without `Viewport3D`, model, renderer, Timeline, or private-clock ownership.

### Changed
- **Showcase 3D navigation** — replaced demo-local orbit arithmetic with the public controller, added a visible Orbit/Fly switch, primary rotate/look, middle-button true camera pan, explicit keyboard capture with Escape/focus-loss release, WASD/QE/Shift Fly controls, and decimated mode/pose/held-state diagnostics while preserving model spin and renderer controls.

## [0.195.0-beta] — 2026-08-23

### Added
- **Generic engine-owned Timeline** (`SPEC-HAL-021`) — added bounded owner-scoped handles, explicit `Once`, `Loop`, and `Alternate` playback, frame-stable typed lifecycle samples, exact finite terminal observation, owner cancellation, and full-precision monotonic frame-time evaluation without a second scheduler or clock.
- **Feature-gated render cadence audit** (`render-audit`) — added owner-local counters for admitted 3D updates, model revisions, custom preparation, encoding, submission, swap, publication, surface presentation, and renderer advance, with a decimated Showcase readout that stays absent from normal builds.
- **Aggregate dynamic 3D budgets** (`SPEC-HAL-355`) — added finite context-wide limits and high-water accounting for GPU dynamic-storage bytes, object capacity, live and in-flight storage slots, retained CPU staging, and simultaneously retained model-state identities.

### Changed
- **Retained dynamic 3D model updates** — split structural snapshot identity from model placement state, retained compatible raster resources across revisions, wrote only changed uniform blocks, and kept requested, committed, and last-good tuples transactional across target-local and aggregate budget failures.
- **Showcase 3D motion and controls** — moved continuous scene rotation onto the shared Timeline, kept unmodified-wheel zoom and direct pointer orbit controls, corrected vertical orbit direction, and expanded the scene so winding, back-face culling, double-sided policy, depth, alpha mask, texture replacement, and dynamic model updates remain visibly distinguishable.

### Fixed
- **Bounded layout-transition admission** — propagated Timeline capacity exhaustion as a typed rejection, cancelled the superseded visual transition, and fell back to authoritative layout instead of panicking or retaining a stale interpolation endpoint.
- **Display-phase continuous pacing** — replaced the hard-coded drifting 120 Hz deadline with the current monitor's reported refresh interval, retained the scheduled phase across ordinary event-loop wake latency, skipped whole missed periods without catch-up redraw bursts, and kept 120 Hz as the unknown-display fallback.
- **3D submission and retirement truth** — preserved same-Queue write/encode/submit ordering, retired replaced dynamic storage only after submitted work completes, retried aggregate-budget failures only after a real release, and prevented failed preparation from changing the published front or last-good tuple.

## [0.194.0-beta] — 2026-08-22

### Added
- **Complete typed 3D texture and sampler vertical** (`Texture3D`, `Sampler3D`) — added immutable CPU values, context-bound generational handles and leases, lazy exact-generation GPU materialization, replacement-safe cache lifetime, and finite source, resident, cache, request, and publication budgets.
- **Explicit 3D device-terminal truth** (`Viewport3D`) — added lazy generic device-lifetime observation armed only by the first admitted consumer and shared across contexts on the renderer-owned device, per-target terminal status, exact requested/committed tuple retention, and one-time withdrawal of every published surface front after device loss.
- **Native 3D proof surface** (`SPEC-HAL-355`) — added owner-bound test-only structural counters, an actual Metal `Features::empty()` / `Limits::default()` gate, production Panel/Image composition coverage at 1x and 4x, and a release Showcase viewport with textured geometry, replacement controls, per-target status, zoom, and pan.

### Fixed
- **Canonical retained 3D composition** — placed viewport Image output on the renderer-owned content layer, corrected ordering documentation, and proved rounded-Panel clipping and production Image alpha composition with target-bound published fronts.
- **Transactional 3D work admission** — made pending request/publication reservation finite and retry-safe so rejection cannot split last-good image, tuple, front, or lease truth.

## [0.193.0-beta] — 2026-08-21

### Added
- **Native 3D rendering substrate authority** (`SPEC-HAL-355`) — adopted the binding architecture for explicit context ownership, immutable render descriptions, bounded programmable appearances, per-view offscreen production, retained Image composition, lifecycle, budgets, and backend-gated conformance. Architectural adoption alone does not claim a rendering implementation or backend support.
- **Checked 3D math and precision kernel** (`three_d`) — added Halcyon-owned f64 world and ray values, f32 local/orientation/positive-scale values, checked TRS and inverse-transpose normal transforms, render-origin conversion, finite/infinite perspective and orthographic reverse-Z projection, committed-extent analytic rays, checked world-to-local ray conversion, and pure plane/sphere/AABB/triangle intersections. `glam` 0.30.10 is a private computation backend only; no third-party math type crosses the public API.
- **Finite 3D context and immutable CPU resources** (`three_d`) — added explicit `GpuResources`-bound context identity, queryable conservative/custom hard-device and policy limits, checked canonical geometry source descriptors with typed straight-linear UNORM8 vertex colors, retained unadmitted appearance-source metadata, typed context/kind/slot/non-wrapping-generation handles, immutable replacement, explicit lease-owner accounting, and lease-gated retirement/reuse. Context construction and CPU registration create and submit no 3D GPU resources.
- **Lazy canonical 3D geometry cache** (`three_d`) — added exact 36-byte interleaved vertex packing with deterministic normal, UV, and straight-linear color defaults; exact unindexed, `u16`, and `u32` index materialization; context/device/generation-bound immutable GPU buffers; preparation-scoped upload and actual GPU-residency accounting; unleased eviction; and retry-safe typed pre-wgpu validation and budget rejection. Repeated demand reuses a generation's buffers, while replacement and retirement retain old buffers only through their leases; allocation and device failure remain at the fatal device-lifetime boundary.
- **Bounded 3D appearance admission** (`three_d`) — added a version-locked Naga 25 compiler adapter with exact source/signature/semantic and budget checks, source-span-preserving direct IR composition, validated WGSL roundtrips, fixed 80/112/16-byte view/object/appearance ABI layouts with device-derived dynamic strides, opaque-white fallback binding metadata, opaque and alpha-mask modes, and generation-keyed immutable admitted-module caching with transactional shader-module accounting. Admission creates no pipelines, textures, samplers, command encoders, render passes, or submissions.
- **Immutable 3D render snapshots** (`three_d`) — added context-bound versioned snapshot descriptors with ordered draw packets, checked monotonic model-revision identity, per-draw canonical single/double-sided policy, appearance-generation-owned opaque/alpha-mask classification, exact geometry/appearance generation leases, semantic validation, and a finite draw-packet budget. Pure snapshot admission retains CPU intent only and performs no shader admission, geometry materialization, pipeline creation, encoding, or submission.
- **Canonical 3D GPU raster core** (`three_d`) — added context-lazy exact three-group bind layouts, device-aligned view/object/appearance uniform arenas, a context-owned opaque-white fallback texture and filtering sampler, direct Naga shader-module materialization, a finite correctness-keyed pipeline cache, and private direct encoding with `Depth32Float` reverse-Z plus direct 1x/4x MSAA resolve. Current actual-GPU proof covers the Metal headless backend only.
- **Retained 3D viewport widget** (`Viewport3D`) — added an ordinary retained widget and admitted custom-render participant with explicit shared `Arc<Context3D>`, immutable view/snapshot revisions, sample-count 1/4, typed requested/committed/failure status, renderer-owned post-layout target metrics, private per-`SurfaceId` front/back/depth/MSAA resources, target-compatible last-good retention, incompatible/inactive withdrawal, submit-before-publication ordering, deferred removal lifetime, and full-UV opaque-WHITE production Image composition.

### Fixed
- **Per-surface 3D viewport publication** — replaced viewport-global front selection with renderer-owned target-bound Image bindings keyed by presentation identity and `SurfaceId`. Retained scenes now resolve only the exact target's committed front, omit missing or inactive bindings, and allocate request serials per actual target attempt.

## [0.192.0-beta] — 2026-08-21

### Added
- **Per-target custom-render preparation** (`SPEC-HAL-266A`) — added an opaque SPEC-HAL-295A public opt-in marker, typed participant registry, and canonical post-layout/pre-paint callback with immutable renderer-owned target metrics, shared production pixel coverage, retained-scene front-publication handling, and zero-participant structural absence.

### Changed
- **Plot target-resource lifecycle** — moved Plot texture sizing and target invalidation out of layout and `GpuResources::scale_factor` into the per-target preparation boundary, captured main-thread hit metadata exactly once at series admission, and added idempotent stable-metrics reuse plus inactive, scale-change, and distinct-surface behavior.

## [0.191.2-beta] — 2026-08-21

### Fixed
- **Image alpha association** (`SPEC-HAL-263A`) — normalized high-level straight-sRGB RGBA8 uploads to premultiplied-linear sampled content, corrected production Image modulation so texture alpha, tint alpha, and clip coverage each apply once, and documented the unchanged premultiplied-linear precondition for direct, deferred, nearest, and custom-front registration.

## [0.191.1-beta] — 2026-08-21

### Fixed
- **Custom-render authority and zero-participant absence** (`SPEC-HAL-295A`) — reconciled custom offscreen ownership with RenderGraph and GPU-lifetime authority, added worker publication, same-Queue sampling, concurrent producer, retirement, and test-only structural-absence proof, and made the Showcase Plot worker pool lazy so its default 2D section admits no custom-render workers or resources.

## [0.191.0-beta] — 2026-08-21

### Added
- **Explicit spatial wheel-zoom policy** (`SPEC-HAL-343A`, `SpatialWheelZoomPolicy`) — replaced the public modifier boolean with the closed `Disabled`, `Unmodified`, and `ControlOrMeta` policy plus one shared resolver. Node flow and Showcase retain Ctrl/Cmd+wheel zoom, standalone node graph, spatial viewport, and Timeline retain unmodified-wheel zoom, and routing, viewport math, Plot interaction, and spatial viewport animation remain unchanged.

## [0.190.0-beta] — 2026-08-15

### Added
- **Owner-correct retained node-flow surface** (`SPEC-HAL-294`, `SPEC-HAL-343`) — added public `NodeFlowSurface` and `NodeFlowSurfaceBinding` APIs. Owners publish immutable document/session/catalog/registry snapshots, the surface paints through node-graph/canvas/spatial mechanisms, and selection, rubber-band, node-move, wire, pan, zoom, context-menu, and capture-mode keyboard behavior is emitted as `NodeEditorIntent` values for owner application.
- **Complete rubber-band session intent transport** — added begin, update, and cancel session intents so a mounted surface can preserve external `NodeEditorSession` authority throughout box selection.

## [0.189.5-beta] — 2026-08-15

### Fixed
- **All-target example compatibility** — `navigation_host` now borrows the payload while describing a `NavigationIntent::NavigateTo` before applying the owned intent, and `shape_primitives` accounts for retained scene fragments when identifying scene-node variants.

### Removed
- **Unaccepted duplicate node-graph widget path** (`SPEC-HAL-284`, `SPEC-HAL-285`, `SPEC-HAL-286`, `SPEC-HAL-288`, `SPEC-HAL-293`, `SPEC-HAL-294`, `SPEC-HAL-338`, `SPEC-HAL-343`) — removed `NodeGraphSurface`, which bypassed the existing `NodeFlowView`/`NodeFlowCoordinator`, canvas/spatial projection, route-geometry, and external session-intent authority boundaries.

## [0.189.4-beta] — 2026-08-07

### Added
- **Readable anchored aligned order-book rows** (`SPEC-HAL-303`, `AlignedOrderBookRowLayout::ReadableAnchored`) — Plot-linked order books can keep exact Plot-owned price anchors while packing displayed rows to a readable labeled height. Hover and selection resolve through the displayed row geometry, and hidden-liquidity summaries preserve the exact source count.

### Fixed
- **Retained input and live-data repaint propagation** (`SPEC-HAL-354`, `EventRouter`, HF Data showcase) — handled widget events plus hover/focus/capture lifecycle callbacks now dirty the affected retained paint fragment through `WidgetTree`, so Controls and Inputs repaint on the event that mutates them. The HF Data showcase also routes status grids, readout tables, ListView/TreeView bodies, and live status labels through paint-only tree invalidation instead of mutating dirty flags directly.

## [0.189.3-beta] — 2026-07-24

### Fixed
- **Retained ListView selection projection** (`SPEC-HAL-347`, `SPEC-HAL-354`) — pointer, keyboard, focus, hover, and selection changes now repaint the exact retained `ListViewBody` fragment without layout. Consumer callbacks and the visible selected row stay in the same event generation, all projected rows remain present, and unchanged re-selection remains paint-clean.

## [0.189.2-beta] — 2026-07-23

### Fixed
- **Retained ListView horizontal projection** (`SPEC-HAL-186`, `SPEC-HAL-354`) — linked horizontal scrolling now republishes the presented coordinate before composite updates and repaints the bounded header/body/footer subtree without layout. Rows beyond the original viewport become visible immediately while pinned columns and header affordances remain fixed.

## [0.189.1-beta] — 2026-07-23

### Fixed
- **Live-edge follow re-arm** (`SPEC-HAL-296`) — completing a horizontal pointer pan with the latest candle inside the bounded trailing-edge band now re-arms follow mode and aligns immediately to the canonical 5% padding. Historical pans and discrete Space+scroll remain suspended, while explicit reset remains available.

## [0.189.0-beta] — 2026-07-23

### Added
- **Explicit live Plot follow default** (`SPEC-HAL-296`, `Plot::with_initial_follow_latest`) — live data channels arm follow-latest independent of builder order while consumers that intentionally open on historical data can opt out explicitly.

### Fixed
- **Stable live-candle edge tracking** — appended candles retain the current X span and remain at one 5% padded right edge. Manual X pan/zoom suspends following until explicit reset, box zoom now participates in that contract, Y-only aligned-book navigation preserves time follow, and reset uses the same live-edge placement without a second alignment jump.

## [0.188.0-beta] — 2026-07-23

### Added
- **Bounded development Agent control** (`SPEC-HAL-317`, `AgentServerProfile::DevelopmentUiControl`) — consumer applications can explicitly enable a localhost-only debug server that admits targeted `FindWidgets`, `WidgetState`, `FocusState`, and ordinary fused input while rejecting composition, snapshots, unbounded tree/list queries, window control, text measurement, and Tower sequences before engine enqueue.
- **Typed targeted Agent client** (`AgentClient`, `agent_control`) — external verification can perform a bounded `FindWidgets → WidgetState → Click/Focus/Scroll` workflow without coordinate clicking or full-tree context dumps. Discovery defaults to 32 results and caps at 64; widget-state child and text fields report bounded/truncated output.

### Security
- **Explicit Agent authority profiles** — the existing unrestricted server remains the `Full` Wireframe/Tower composition profile, while the development UI-control profile is fail-closed in release builds and preserves localhost binding plus canonical fused-input routing.

### Fixed
- **Complete synthetic input gestures** — Agent `Click`, `DoubleClick`, and `KeyPress` now enqueue matched down/up events through the canonical fused-input pipeline instead of leaving pointer or key state pressed after a down-only event.

## [0.187.1-beta] — 2026-07-23

### Fixed
- **Bounded update-only platform cadence** (`SPEC-HAL-354`, macOS/Windows App event loop) — continuous `Widget::needs_update()` demand now retains one stable 120 Hz redraw deadline across event-loop callbacks. Empty channel polling can no longer bypass `WaitUntil` through immediate `AboutToWait` redraw requests and invoke `Screen::update()` tens of thousands of times per second; input remains immediate and unchanged admitted frames still skip scene and GPU work.

## [0.187.0-beta] — 2026-07-23

### Added
- **Shared-state paint-subtree invalidation** (`SPEC-HAL-354`, `UpdateContext::invalidate_paint_subtree`) — update-owned presentation state consumed by descendant painters can regenerate the retained subtree without layout.

### Performance
- **Order-book update-only cadence** — `OrderBook` and `AlignedOrderBook` now poll attached channels through `needs_update` instead of declaring permanent animation. Bursts coalesce to their final visible state, identical snapshots remain idle, stable row topology repaints without layout, and only the row body owns continuing flash animation.

## [0.186.0-beta] — 2026-07-23

### Added
- **Retained widget scene fragments** (`SPEC-HAL-354`, `SceneNode::RetainedFragment`) — the App now caches immutable local, overlay, sidecar, and composed widget fragments by stable `TreeWidgetId`. Paint-only changes repaint the target widget and recompose only its ancestor path while clean parents, children, and siblings reuse their completed fragments; tree topology, theme/scale/focus context, transitions, debug presentation, drag overlays, and tooltips invalidate the required dependencies.
- **Update-only widget cadence** (`Widget::needs_update`) — synchronized data polling and asynchronous completion can keep Halcyon's update phase alive without claiming that the widget visually changes every frame. `Widget::needs_animation` remains the explicit per-frame paint contract.

### Performance
- **No-op retained frames** — an unchanged retained frame now skips scene assembly, SceneGraph diff, draw-plan construction, renderer compilation, GPU rendering, and presentation. Removed widget lifetimes evict retained fragments through WidgetTree structure generations.
- **Plot live-update classification** — Plot data channels and asynchronous offscreen rendering now use update-only cadence, coalesce channel bursts into paint-only invalidation, and present only when Plot state or a completed texture generation changes. A populated static Plot no longer forces continuous Halcyon repaint merely because it contains series data.

## [0.185.0-beta] — 2026-07-23

### Added
- **Retained value/paint invalidation contract** (`SPEC-HAL-354`, `WidgetTree::mark_paint_dirty`, `EventContext::invalidate_paint`, `UpdateContext::invalidate_paint`) — value-only changes now invalidate a widget's local paint output and ancestor composition path without dirtying layout or unaffected siblings. Update-driven animation uses the same paint-only route, while explicit update-time geometry changes retain a separate layout invalidation.

### Performance
- **True clean-root layout reuse** (`SPEC-HAL-026`, `SPEC-HAL-354`) — a clean root under identical constraints now returns its completed cached geometry before entering the layout pass or walking/stamping the subtree. Paint-only updates therefore perform zero layout work while preserving the finite completed-geometry invariant.

## [0.184.0-beta] — 2026-07-23

### Added
- **Retained vertical ScrollArea continuity** (`SPEC-HAL-131A`, `SPEC-HAL-190`, `VerticalScrollLink`) — consumers can retain one presented vertical coordinate across sequential `ScrollArea` reconstruction. The replacement exposes the retained coordinate to first-frame ListView/TreeView virtualization, then consumes it once after tight layout establishes final bounds, clamps through canonical `ScrollState`, and jumps logical and visual offsets together before paint. Fresh scroll intent cancels a pending restore, standalone behavior is unchanged, and live multi-widget coordination remains owned by `ScrollGroupId`.

## [0.183.2-beta] — 2026-07-23

### Performance
- **Bounded production scene-vector reuse** (`SPEC-HAL-005`, `SPEC-R-001`, `src/core/app.rs`, `src/core/scene/mod.rs`) — after draw-plan construction consumes the primary surface scene, Halcyon now clears its payload immediately and reuses the preceding frame's root and nested clip/layer/scroll vector storage. Identical dynamic scenes stop allocating fresh child vectors on every `end_clip`/`end_scroll`, while unused vectors are discarded on the next build rather than accumulated historically.
- **Allocation-free scene-text caller enforcement** (`SPEC-HAL-159`, `src/core/scene/mod.rs`) — the mandatory all-build caller allowlist now matches both Unix and Windows source paths in place instead of allocating a normalized path `String` for every emitted text node. Authorization behavior and panic enforcement are unchanged.

## [0.183.1-beta] — 2026-07-23

### Fixed
- **Bounded debug SceneCache generations** (`SPEC-HAL-083`, `src/core/scene_diff/mod.rs`) — removed scene identities no longer leave permanent generation tombstones in debug builds. Live entries receive cache-global monotonic generations, preserving stale-handle rejection across removal, clear, and same-key reuse while bounding generation metadata to current cache cardinality during rapid scene reconstruction.

### Verified
- **Production staging completion lifecycle** (`SPEC-HAL-040`, `src/tests/renderer_staging.rs`) — repeated unchanged renderer-owned path uploads now have explicit non-blocking compile/`advance_frame` coverage proving that pending submissions/remaps drain and retained staging capacity remains stable without the test-only per-frame blocking wait. The single-threaded App path cannot enqueue another frame while Metal drawable acquisition is backpressured.

## [0.183.0-beta] — 2026-07-23

### Added
- **ListView preferred-width persistence** (`SPEC-HAL-185`, `src/widgets/tabular_view/view_config.rs`, `src/widgets/list_view/list_view.rs`) — `ColumnConfigurationState` and the existing injected `ViewConfigStore` envelope now retain identity-keyed preferred widths. Committed drag and auto-size operations save only after resize completion; reconstruction applies finite positive widths through current column constraints before first layout, preserving a single owner for header/body geometry and requiring no consumer pointer-coordinate inference.

## [0.182.7-beta] — 2026-07-23

### Fixed
- **Live Plot follow suspension on pointer zoom** (`SPEC-HAL-296`, `src/plot/mod.rs`) — ordinary pointer-scroll zoom now clears `ViewState::follow_latest` before subsequent public `DataUpdate` channel drains, so a manually zoomed live viewport remains under user control until the existing explicit reset gesture re-enables auto-follow.

## [0.182.6-beta] — 2026-07-23

### Fixed
- **Bounded repeated-frame path staging** (`SPEC-HAL-040`, `src/renderer/pipelines/path_pipeline.rs`, `src/renderer/staging_pool.rs`, `src/renderer/wgpu_renderer.rs`) — path vertex, index, and instance uploads now use the renderer-owned staging lifecycle instead of creating three wgpu/Metal staging buffers through `Queue::write_buffer` on every compiled frame. Staging allocations use power-of-two size classes and best-fit reuse, so an unchanged repeated path workload stabilizes after warm-up rather than growing the pool because a smaller request consumed a larger reusable buffer.

## [0.182.5-beta] — 2026-07-22

### Fixed
- **Cross-frame renderer font-metrics reuse** (`SPEC-HAL-344`, `src/renderer/wgpu_renderer.rs`) — renderer text-quad compilation now routes per-pass metrics misses through `BackendTextEngine`'s bounded exact cache instead of bypassing it through the native backend. Repeated unchanged `compile_multi` frames therefore reuse the same CoreText metrics/font instance while preserving the renderer's explicit per-surface scale identity, local front cache, glyph shaping, rasterization, and quad geometry.

## [0.182.4-beta] — 2026-07-22

### Fixed
- **Cross-frame native font-metrics reuse during Plot paint** (`SPEC-HAL-104`, `SPEC-HAL-296`, `src/platform/backend_text_engine.rs`) — `BackendTextEngine` now retains a bounded exact metrics cache keyed by font identity, font-size bits, and scale bits. Repeated axis/crosshair label paint no longer calls macOS CoreText to construct a fresh native font per label on every frame; font/scale changes invalidate the cache while metric values, shaping, rasterization, and draw ordering remain unchanged.

## [0.182.3-beta] — 2026-07-22

### Fixed
- **Finite wired ScrollArea measurement** (`SPEC-HAL-088`, `src/widgets/scroll_area.rs`) — extended the finite measurement invariant to the ordinary `ScrollArea::new_unwired()` plus `wire_content()` path used by virtualized TreeView/ListView bodies. An unbounded parent measurement axis now retains a finite viewport and returned size before the bounded allocation pass, preventing public `SplitPane` composition from publishing infinite child rects or clips while preserving strict scene validation and bounded final layout.

## [0.182.2-beta] — 2026-07-22

### Fixed
- **Finite standalone-empty ScrollArea measurement** (`SPEC-HAL-088`, `src/widgets/scroll_area.rs`) — `ScrollArea::empty()` now treats an unbounded maximum constraint as measurement capability rather than final viewport geometry, reporting the finite incoming minimum until a bounded allocation pass arrives. Public `SplitPane` measurement can no longer retain an infinite ScrollArea child rect or clip into scene construction; bounded final layout, existing scroll ownership, and strict finite-scene validation remain unchanged.

## [0.182.1-beta] — 2026-07-22

### Fixed
- **Readable automatic aligned rows** (`SPEC-HAL-296`, `SPEC-HAL-303`, `src/plot/market_detail.rs`) — corrected the automatic grouping default from a one-pixel geometry floor to the existing ten-pixel label-bearing threshold. Automatic intervals remain exact and receive no round-number enlargement, but rows now retain readable price/quantity labels at every Plot zoom instead of remaining permanent hairlines. Explicit decimal/fixed-step grouping, shared price coordinates, exact-source interaction, and standalone OrderBook behavior are unchanged.

## [0.182.0-beta] — 2026-07-22

### Added
- **Explicit aligned price-grouping intent** (`SPEC-HAL-296`, `SPEC-HAL-300`, `SPEC-HAL-303`, `src/plot/market_detail.rs`) — `MarketDetailGrouping` now exposes automatic pixel-collision grouping, decimal-place grouping, and validated fixed-price-step grouping. Semantic grids remain globally stable, keep the shared reference as an exact boundary, and merge only the minimum adjacent bins required by the configured pixel floor while exact source levels remain authoritative for interaction.

### Changed
- **Helpful-only automatic grouping** — the aligned order book now defaults to an exact one-logical-pixel collision floor and no longer enlarges derived buckets for round-number aesthetics. Explicit `MarketDetailConfig::min_bucket_height`, quantity-depth animation settings, price formatting, shared Plot coordinates, and standalone OrderBook behavior remain consumer-controlled and unchanged.

## [0.181.2-beta] — 2026-07-21

### Fixed
- **Less aggressive aligned price grouping** (`SPEC-HAL-296`, `SPEC-HAL-300`, `SPEC-HAL-303`, `src/plot/market_detail.rs`) — aligned order-book grouping now rounds its Plot-derived minimum interval through a dense deterministic nice-step lattice whose consecutive steps differ by at most one third. This preserves the configured minimum row height and stable reference/global anchoring without the former `1/2/5` threshold jumps that could nearly double row height and merge otherwise readable levels. Exact source interaction, price-coordinate alignment, bounded projection work, explicit consumer configuration, and standalone OrderBook behavior are unchanged.

## [0.181.1-beta] — 2026-07-20

### Fixed
- **Aligned order-book row hover** (`SPEC-HAL-296`, `SPEC-HAL-303`, `src/widgets/aligned_order_book.rs`) — aligned levels now paint the active theme's local row-hover affordance on the exact bid/ask projected row under the pointer. Real empty liquidity gaps remain unhighlighted instead of resolving to a distant nearest level; exact hover events, explicit cross-widget reflection, selection precedence, and standalone OrderBook behavior are unchanged.

## [0.181.0-beta] — 2026-07-20

### Added
- **Aligned market-detail navigation and exact selection** (`SPEC-HAL-296`, `SPEC-HAL-303`, `src/plot/market_detail.rs`, `src/widgets/aligned_order_book.rs`) — wheel input over `AlignedOrderBook` now emits one bounded, coalesced logical-pixel price-pan intent through `MarketDetailLink`; Plot consumes it as the sole `ViewState` and Y-domain owner, with logarithmic-axis support and an explicit `with_wheel_price_pan(false)` opt-out. Primary click now retains a public `AlignedOrderBookSelection { side, price }` exact-source identity, highlights its visible projected group through active theme tokens, and never infers a market tick or silently reattaches when that source price disappears. Empty price intervals remain honest gaps and standalone OrderBook behavior is unchanged.

### Changed
- **Local aligned hover and interval-aware Plot ticks** (`SPEC-HAL-296`, `SPEC-HAL-303`, `src/plot/grid.rs`, `src/plot/mod.rs`, `src/widgets/aligned_order_book.rs`) — ordinary aligned-book hover still emits exact enter/leave events but no longer paints the blue horizontal guide or automatically publishes into the shared Probe; cross-widget hover reflection remains available through explicit consumer wiring. Default Plot axis labels now derive decimal precision from the actual tick interval, keeping adjacent large fractional prices distinct while preserving explicit consumer formatters.

## [0.180.1-beta] — 2026-07-20

### Fixed
- **Reference-attached grouped order-book rows** (`SPEC-HAL-296`, `SPEC-HAL-300`, `SPEC-HAL-303`, `src/plot/market_detail.rs`, `src/widgets/aligned_order_book.rs`) — an opted-in presented Plot reference now anchors the aligned grouping grid as an exact bucket boundary, so adjacent bid/ask groups move with the shared line instead of letting it slide through a fixed absolute range. Each row displays one grouped midpoint price rather than a changing lower–upper range; typed aggregation bounds and exact source levels remain available internally for projection evidence and interaction. Axis, reference, and row projection consume one immutable presentation snapshot, reference-generation invalidation remains bounded and generation-neutral when unchanged, unreferenced projection retains global nice-step anchoring, and standalone order books are unchanged.

## [0.180.0-beta] — 2026-07-20

### Changed
- **Stable aligned price-range rows** (`SPEC-HAL-296`, `SPEC-HAL-300`, `SPEC-HAL-303`, `src/plot/market_detail.rs`, `src/widgets/aligned_order_book.rs`) — aligned order-book grouping now uses deterministic globally anchored price ranges instead of quantity-weighted representative labels. The range interval is the smallest Plot-style `1/2/5 × 10ⁿ` step satisfying the configured minimum row height in the presented price transform, so source quantity churn cannot move or rename grouped rows while the interval is unchanged. Rows display formatted range bounds, expose their typed `ProjectedPriceRange` identity and deterministic axis midpoint, and continue to aggregate exact snapshot quantity without rewriting, interpolating, or synchronizing source prices. Exact source levels remain authoritative for hover/click, logarithmic price axes use logarithmic range anchors, horizontal depth animation remains independent, and standalone order books are unchanged. Gates: `market_detail` 19/19; `grip:verify package halcyon test market_detail refresh check build`.

## [0.179.5-beta] — 2026-07-20

### Fixed
- **Same-frame aligned row projection** (`SPEC-HAL-296`, `SPEC-HAL-303`, `src/plot/market_detail.rs`, `src/widgets/aligned_order_book.rs`) — `AlignedOrderBook` now detects when Plot publishes a completed presented-axis generation after the ladder's update traversal and performs one bounded paint-time projection from the existing exact book index. Row Y geometry, clipping summaries, and the optional reference line therefore paint from the same presented Plot transform regardless of sibling update order. Authoritative book snapshots remain unsmoothed and independent; the intentional horizontal quantity-depth animation is unchanged.

## [0.179.4-beta] — 2026-07-20

### Fixed
- **Same-frame aligned market-detail presentation** (`SPEC-HAL-295`, `SPEC-HAL-296`, `SPEC-HAL-299`, `SPEC-HAL-303`, `src/renderer/custom_render_pass.rs`, `src/plot/`, `src/widgets/aligned_order_book.rs`) — Plot workers now leave completed front-texture registration pending until Plot's next UI update, where the texture, rendered ViewState, Plot-owned presented Y-domain, and optional linked PriceLevel reference are committed together before sibling paint. `AlignedOrderBook` therefore uses the exact transform and reference belonging to the visible Plot frame, with no one-frame catch-up. Pending presentation prevents either double-buffer texture from being reused early. The opt-in API, pure-data `MarketDetailLink`, explicit styling, and standalone widgets are unchanged.

## [0.179.3-beta] — 2026-07-20

### Fixed
- **Frame-locked live market-detail reference** (`SPEC-HAL-296`, `SPEC-HAL-299`, `SPEC-HAL-303`, `src/plot/renderer.rs`, `src/plot/render_pool.rs`, `src/plot/mod.rs`, `src/plot/market_detail.rs`, `src/widgets/aligned_order_book.rs`) — Plot render jobs now reject mixed series/overlay generations, preventing a newer live candle from rendering with an older PriceLevel position. The optional linked reference is published only after the matching Plot frame swaps successfully, and the aligned ladder invalidates when that completed-frame publication changes. Existing opt-in configuration and pure-data ownership are unchanged.

## [0.179.2-beta] — 2026-07-20

### Changed
- **Synchronized live price reference and faster quantity fill** (`SPEC-HAL-296`, `SPEC-HAL-299`, `SPEC-HAL-303`, `src/plot/mod.rs`, `src/plot/market_detail.rs`) — a nominated market-detail reference level now renders and publishes its authoritative price without independent easing, keeping the Plot line and aligned order-book continuation on the live candle's update timeline. Ordinary price levels retain their existing smoothing. Default aligned visible-depth attack/release rates increased by another one third; explicit consumer-provided `MarketDetailConfig` rates remain unchanged.

## [0.179.1-beta] — 2026-07-20

### Changed
- **Faster market-detail motion** (`SPEC-HAL-296`, `SPEC-HAL-299`, `src/plot/mod.rs`, `src/plot/market_detail.rs`) — increased the frame-rate-independent Plot price-level smoothing rate and the default aligned visible-depth attack/release rates by one third. Explicit consumer-provided `MarketDetailConfig` rates remain unchanged.

## [0.179.0-beta] — 2026-07-20

### Added
- **Optional linked Plot price reference** (`SPEC-HAL-296`, `SPEC-HAL-299`, `SPEC-HAL-303`, `src/plot/market_detail.rs`, `src/plot/mod.rs`, `src/widgets/aligned_order_book.rs`) — consumers can nominate one existing Plot `PriceLevel` by ID for continuation through `AlignedOrderBook`. Plot publishes the selected level's smoothed display price, theme-resolved or explicit color, and solid/dashed/dotted style through the pure-data `MarketDetailLink`; the ladder paints it at the identical shared Y coordinate through its right edge without gaining price or annotation authority. Standalone and unselected linked widgets remain unchanged.

### Fixed
- **Aligned projected quantity labels** (`SPEC-HAL-303`, `src/widgets/aligned_order_book.rs`) — projected rows now display the already-aggregated bucket quantity without appending internal source-level multiplicity such as `×3` or `×9`. The source count remains available in projection data for evidence and exact-source interaction.

## [0.178.1-beta] — 2026-07-19

### Fixed
- **Aligned market-detail sparse-level projection** (`SPEC-HAL-296`, `SPEC-HAL-300`, `SPEC-HAL-303`, `src/plot/market_detail.rs`, `src/widgets/aligned_order_book.rs`) — aligned ladders now apply bounded presentation-only vertical pixel buckets even when a sparse snapshot occupies only a few pixels inside a wide Plot-owned price range, eliminating overlapping fixed-height bars without changing the authoritative `BookSide` snapshot or exact hover/click recovery. Bid and ask price labels stay on their own side of the center line and quantity labels are omitted when the available half-width cannot keep them disjoint. Linked Plot regression coverage proves that primary-drag retains local time pan and Space+primary-drag retains free time/price pan with Y publication to the ladder; the showcase now uses a sparse symmetric near-mid book and states both gestures. Gates: `market_detail`; `grip:verify package halcyon test market_detail refresh check build`.

## [0.178.0-beta] — 2026-07-18

### Added
- **Optional Plot/order-book market-detail alignment** (`SPEC-HAL-296`, `SPEC-HAL-300`, `SPEC-HAL-303`, `src/plot/market_detail.rs`, `src/widgets/aligned_order_book.rs`) — added the pure-data `MarketDetailLink`, Plot-owned shared price projection, and opt-in `AlignedOrderBook` while leaving standalone Plot and OrderBook behavior independent. The aligned ladder clips to the Plot Y domain, reports bounded above/below liquidity summaries, performs presentation-only pixel bucketing with exact source hover/click recovery, and smooths a robust horizontal depth scale independently from price. Unchanged axis publication is generation-neutral, repeated projection state is cached, snapshot bursts coalesce to one source-index rebuild per update, Plot render submissions deduplicate unchanged frame state, and the live showcase uses incremental candle/volume/EMA updates beside the aligned ladder. Gates: `market_detail` 6/6; `grip:verify package halcyon test market_detail refresh check build`.

## [0.177.0-beta] — 2026-07-18

### Added
- **Theme-aware Plot rendering** (`SPEC-HAL-296`, `src/core/theme.rs`, `src/core/widget/mod.rs`, `src/plot/`) — Plot-owned surfaces, grid, axes, labels, crosshairs, selections, built-in tools, annotation defaults, viewport-volume chrome, and built-in series defaults now resolve from the active Halcyon theme while preserving complete consumer color/style authority. Runtime theme switches update Plot render generation and repaint the offscreen target; the dark palette retains its prior values, and the light palette uses a restrained off-white canvas with higher-contrast chrome and no default volume scrim. The showcase proves both themes with one Plot configuration while retaining its live buy/sell-ratio volume coloring. Gates: `plot_theme` 3/3; `plot_spatial_overlay` 1/1; `cargo check --example showcase`; `grip:verify changed refresh check`.

## [0.176.0-beta] — 2026-07-17

### Added
- **Atomic interaction-domain retirement** (`SPEC-HAL-234`, `SPEC-HAL-235`, `SPEC-HAL-236`, `SPEC-HAL-237`, `SPEC-HAL-247`, `SPEC-HAL-248`, `src/core/event/mod.rs`, `src/core/focus_authority.rs`, `src/core/navigation_domain_router.rs`, `src/core/overlay_domain_coordinator.rs`, `src/core/navigation_orchestrator.rs`, `src/core/navigation_application.rs`) — added one application-owned prepared transaction that acquires a single domain token, proves the router's complete root/port/focus-claim set, removes every immutable domain frame-projection consumer, retires the complete borrow-free focus graph and restoration stack including lawful domain-local trees that are not router claims, reserves every overlay owner including inactive domain-local owners, unwinds delegation, and executes one all-root compositor batch before removing router, coordinator, and `InteractionDomains` records. Application-admitted roots now retain permanent same-domain participation from hidden admission through destruction, closing direct host/orchestrator/compatibility teardown bypass even without a ScreenHost binding. Preparation failure and drop preserve every authority; commit contains no fallible operation after focus retirement begins. Gates: `navigation_domain_retirement`; `navigation_domain_router`; `navigation_root_admission`; `navigation`; `root_retirement`; `frame_projection_binding_registry`; `projection`; `overlay_domain_coordinator`; `focus_authority`; `grip:verify changed refresh check`.

## [0.175.0-beta] — 2026-07-17

### Added
- **Guard-enforced frame-projection source participation** (`SPEC-HAL-237`, `SPEC-HAL-246`, `SPEC-HAL-247`, `SPEC-HAL-248`, `src/core/frame_projection_binding_registry.rs`, `src/model/navigation_host.rs`, `src/core/navigation_orchestrator.rs`, `src/core/navigation_application.rs`) — versioned frame bindings now retain unique resolved orchestrator participants, while each source holds a checked same-coordinator/domain reference count and independently abortable prepared deltas. Installation, replacement, removal, orphan invalidation, and multiconsumer retirement commit source claims with the binding/index mutation. Every routed navigation variant and whole-root retirement proves the matching domain guard at both NavigationHost commit and surface publication; bound direct host execution and orchestrator dispatch fail closed; and the standalone snapshot/authorize/prepare transaction executes the same guard-outside-token host/surface sequence without router generations. Gates: `frame_projection_binding_registry` 3/3 including enrollment exclusion by an unguarded prepared host commit, bound direct host/orchestrator rejection, guarded standalone execution, registered routed execution, and multi-binding final-release behavior; `navigation` 91/91; `root_retirement` 11/11; `projection` 112/112; `grip:verify changed refresh check`.

## [0.174.0-beta] — 2026-07-16

### Added
- **Guarded multicontext navigation frame projection** (`SPEC-HAL-246`, `SPEC-HAL-247`, `SPEC-HAL-248`, `src/core/frame_projection_binding_registry.rs`, `src/core/app.rs`) — made each orchestrator projection navigation-only, fallible, and exact over its committed path-to-surface mapping; added compositor-identity-bound frame projections with grouped navigation contexts and independently sourced overlay/focus testimony; and integrated one non-blocking guarded pull into both live frame loops. The runtime now validates registered-root focus association, every referenced surface and role, binding epochs, ScreenHost lifecycle, and compositor identity before complete publication; current-epoch failure clears stale testimony, while busy/stale retry preserves the prior view. Gates: `navigation_frame_projection` 5/5; `navigation_projection` 10/10; `frame_projection_binding_registry` 3/3; `root_retirement` 11/11; `pull` 6/6; `navigation` 91/91; `grip:verify package halcyon refresh check`.

## [0.173.0-beta] — 2026-07-16

### Added
- **Atomic delegation-linked navigation-root retirement** (`SPEC-HAL-234`, `SPEC-HAL-235`, `SPEC-HAL-236`, `SPEC-HAL-237`, `src/core/focus_authority.rs`, `src/core/overlay_domain_coordinator.rs`, `src/core/navigation_domain_router.rs`, `src/core/navigation_orchestrator.rs`, `src/core/navigation_application.rs`) — added a router-owned versioned retirement plan that removes the exact dependent delegation suffix top-first, supports primed-root replacement by a surviving formerly suspended chain member, and publishes one final effective root. Focus authority now prepares an exact-final graph retirement that can explicitly consume the current effective owner's top restoration suffix while retiring another root tree. Permanent overlay/focus retirement rejects `Suspend` before owner or focus mutation. The registry can reserve the retiring and surviving peer orchestrators together and execute peer overlay dismissal plus whole-root teardown in one compositor batch. All fallible focus, owner, projection, binding, claim, routing, and surface work completes before non-fallible commit; dropping preparation preserves the original chain and every authority. Gates: `root_retirement` 11/11; `retirement_` 15/15; `navigation_domain_router`; `focus_authority`; `overlay_domain_coordinator`; `navigation_delegation`; `grip:verify package halcyon refresh check`.

## [0.172.0-beta] — 2026-07-16

### Added
- **Atomic application-owned navigation delegation and return** (`SPEC-HAL-234`, `SPEC-HAL-235`, `SPEC-HAL-236`, `SPEC-HAL-237`, `src/core/navigation_domain_router.rs`, `src/core/navigation_orchestrator.rs`, `src/core/navigation_application.rs`) — added guarded prepared router push/pop reservations, exact source/target owner and destination focus-binding validation, cycle-safe nested delegation, and one non-fallible application commit for focus plus routing publication. Source-owned overlays exit to target focus before suspension, target-owned overlays remain authoritative across delegation, and return symmetrically exits a target overlay before restoring source focus. A separate prepared no-host-change overlay surface batch now applies the owner-issued top-first `Suspend` or `Dismiss` plan under the same orchestration lease. Unrelated peer overlays reject unchanged, prepared values abort without mutation, and suspended sources cannot route or obtain overlay activation eligibility. Gates: `navigation_delegation` 6/6; `navigation_domain_router` 5/5; `overlay_domain_coordinator` 8/8; `navigation_routed_overlay_exit` 7/7; `navigation` 86/86; `grip:verify package halcyon refresh check`.

## [0.171.0-beta] — 2026-07-16

### Added
- **Atomic application-owned navigation-root retirement** (`SPEC-HAL-234`, `SPEC-HAL-235`, `SPEC-HAL-236`, `SPEC-HAL-237`, `SPEC-HAL-248`, `src/core/navigation_application.rs`) — added one guarded prepared transaction for roots outside the active delegation stack that validates root/router/focus/owner generations, enumerates the complete frame-projection consumer set, prepares every binding removal or replacement, retires the exact root-owned focus graph and binding port, removes router claims and association, unregisters the overlay owner, and executes explicit whole-root surface teardown only after every external authority has detached. Active retiring-owner overlays use one compound dismissal/focus-retirement plan; peer restoration dependencies and live delegation references reject unchanged. Projection window access now captures immutable compositor identity so prepared ScreenHost clearing can coexist with exclusive compositor teardown. Gates: `root_retirement` 5/5 covering replacement commit, complete abort, active-owner overlay teardown, peer-dependency rejection, and priming replacement without surviving-peer focus theft; `frame_projection_binding_registry`; `overlay_domain_coordinator`; `navigation_domain_router`; `navigation_root_admission`; `navigation` 80/80; `grip:verify package halcyon refresh check`.

## [0.170.0-beta] — 2026-07-16

### Added
- **Guarded frame-projection binding authority** (`SPEC-HAL-237`, `SPEC-HAL-248`, `src/core/frame_projection_binding_registry.rs`) — added the application-owned, coordinator-bound registry for versioned per-ScreenHost projection-source bindings, exact root/domain reverse indexes, registered-root and standalone-orchestrator source validation, synchronous stale-view clearing, guarded orphan invalidation, and per-consumer prepared remove/replace operations that allow a root retirement to prepare every affected binding before any commit. ScreenHost projection testimony is explicitly coordination/verification data and remains separate from renderer input. Gates: `frame_projection_binding_registry` 3/3 including simultaneous preparation of two root consumers; `navigation_domain_router`; `navigation_root_admission`; `navigation_projection_boundary_guard`; `screen_host_projection`; `navigation_surface_lifecycle`; `grip:verify package halcyon refresh check`.

## [0.169.0-beta] — 2026-07-16

### Added
- **Atomic routed overlay exit and surface execution** (`SPEC-HAL-234`, `SPEC-HAL-236`, `SPEC-HAL-237`, `src/core/overlay_domain_coordinator.rs`, `src/core/navigation_orchestrator.rs`, `src/core/navigation_application.rs`) — added owner-issued top-first overlay surface-exit testimony, exact root/surface validation, and one compositor batch that applies `Suspend` visibility or `Dismiss` destruction with the navigation delta. Routed existing-domain and focus-graph transactions now deactivate the root-owned overlay, consume its exact restoration tokens, update focus/binding/router/history, and publish surfaces without an intermediate target; drop restores owner tokens, graph, claims, history, and every live overlay surface. `Suspend` retains owner-local records for reactivation while `Dismiss` purges the deactivated records that referred to destroyed surfaces; owner-catalog-wide surface uniqueness prevents inactive aliases to those destroyed surfaces, and fresh inactive stack admission supports surfaces created after atomic root admission. Active peer overlays reject owner-correctly without token or surface mutation. Gates: `navigation_routed_overlay_exit` 7/7; `navigation_routed_transaction`; `navigation_routed_focus_graph`; `navigation_domain_router`; `overlay_domain_coordinator`; `navigation_surface_lifecycle`; `grip:verify package halcyon refresh check`.

### Fixed
- **Router overlay eligibility set law** (`SPEC-HAL-236`, `src/core/overlay_domain_coordinator.rs`) — validates that every activated overlay focus domain is contained in the sealed root-owned set instead of incorrectly requiring the overlay stack to equal the root's complete focus-domain set.

## [0.168.0-beta] — 2026-07-16

### Added
- **Routed focus-graph membership transaction** (`SPEC-HAL-235`, `SPEC-HAL-237`, `src/core/navigation_application.rs`) — added the ordinary routed path for atomic focus-domain forest addition/removal and current-domain replacement. One prepared value now owns the exact focus graph, root binding membership change, unique router claim swap, host history commit, and surface transition; commit publishes graph, binding, claims, history, and surfaces under one router `Committing` interval, while drop restores the old graph/claims/host/surfaces and releases the domain token. Gates: `navigation_routed_focus_graph` 2/2; `navigation_routed_transaction`; `navigation_domain_router`; `focus_authority`; `navigation_surface_lifecycle`; `grip:verify package halcyon refresh check`.

## [0.167.0-beta] — 2026-07-16

### Added
- **Prepared ordinary routed navigation** (`SPEC-HAL-228`, `SPEC-HAL-231`, `SPEC-HAL-234`, `SPEC-HAL-235`, `SPEC-HAL-237`, `src/core/navigation_application.rs`) — added coherent application-owned route snapshots that bind router, root focus-port, host mechanics, registry residency, and orchestrator generations; outside-token guard authorization; exact guarded revalidation; and one prepared non-fallible existing-focus-domain transaction. The router now seals the exact prepared focus-binding replacement, reserves one compound generation, hides snapshots through `Committing`, and returns to `Stable` only after focus, binding, host history, and surface publication all commit. Abort releases the host, surface, binding, router, and domain leases without mutation. Gates: `navigation_routed_transaction` 5/5; `navigation_root_admission`; `navigation_domain_router`; `navigation_multiroot`; `grip:verify package halcyon refresh check`.

## [0.166.0-beta] — 2026-07-16

### Added
- **Atomic navigation-root admission** (`SPEC-HAL-235`, `SPEC-HAL-236`, `SPEC-HAL-237`, `src/core/navigation_application.rs`) — added the application-owned prepared root-admission transaction that holds one interaction-domain transition guard while it preflights router identities, stages a hidden registry root, reserves its overlay owner, atomically prepares the initial focus-domain graph and exact root focus-binding port, and prepares exclusive router claims. Commit publishes owner, focus, binding, router association, and the visible registry/compositor root through one non-fallible ordered path; abort or drop removes the hidden surface and releases every reservation. Gates: `navigation_root_admission` 4/4; `navigation_domain_router`; `overlay_domain_coordinator`; `focus_authority`; `navigation_multiroot`; `grip:verify package halcyon refresh check`.

## [0.165.0-beta] — 2026-07-16

### Added
- **Navigation-domain router and exclusive root focus claims** (`SPEC-HAL-235`, `SPEC-HAL-236`, `SPEC-HAL-237`, `src/core/navigation_domain_router.rs`) — added the application-owned per-interaction-domain router with checked routing generations, stable/committing snapshot publication, explicit priming, coherent scoped/unscoped route candidates, exact coordinator-bound root focus-binding ports, disjoint ancestry/subtree-closed focus-domain claims, sealed atomic root association and batch membership updates, ordered root claim/routing/owner retirement, and leased router eligibility for overlay activation. Focus graph preparations now carry checked non-reused identities so router, port, and focus commits cannot be paired across transactions. Gates: `navigation_domain_router` 5/5; `focus_authority` 10/10; `overlay_domain_coordinator` 8/8; `grip:verify package halcyon refresh check`.

## [0.164.0-beta] — 2026-07-16

### Added
- **Guarded overlay-domain coordinator** (`SPEC-HAL-236`, `src/core/overlay_domain_coordinator.rs`) — added the application-level single-stack authority for each live interaction domain: non-blocking generation-bound transition guards, concrete sealed owner lifecycle ports, two-phase domain/owner registration and retirement, exact router-managed eligibility testimony, prepared activation/deactivation, nested frame push/pop, compound exit-to-domain restoration, abort-safe focus-token custody, and guarded active-stack projection. Focus preparation now accepts borrowed owner-held restoration tokens so failed downstream preparation cannot consume owner capability. Gates: `overlay_domain_coordinator` 8/8; `grip:verify package halcyon refresh check`.

## [0.163.0-beta] — 2026-07-16

### Added
- **Central prepared focus-domain authority** (`SPEC-HAL-235`, `src/core/focus_authority.rs`, `src/core/event/mod.rs`) — extended each live interaction domain's existing `FocusManager` with an explicit focus-domain forest, unique target-to-domain/surface claims, deterministic active-target selection, checked non-reused restoration generations, and an exact LIFO restoration stack. Added crate-sealed domain capabilities plus abortable prepared/non-fallible commit seams for target mutation and selection, domain activation, nested overlay activation/exit/restoration, atomic parent+child graph admission, complete subtree swaps, and peer-safe retirement. New `InteractionDomainState` instances bind focus to their actual allocated domain identity. Gates: `focus_authority` 10/10; `focus` 210/210; `interaction_domain_isolation`; `grip:verify changed refresh check`.

## [0.162.0-beta] — 2026-07-16

### Added
- **Prepared navigation surface lifecycle transactions** (`SPEC-HAL-234`, `SPEC-HAL-237`, `SPEC-HAL-247`, `src/compositor/mod.rs`, `src/core/navigation_orchestrator.rs`) — added semantic-free exact compositor mutation batches, hidden and abortable ordinary surface staging, complete-entry surface identity, generation-checked guarded lifecycle publication, explicit overlay-first/deepest-first termination, compositor binding rejection, duplicate-safe hidden prepared root admission, and separate prepared whole-root teardown. Legacy dispatch now lowers through the prepared host and surface seams while central focus/overlay/binding/router publication remains explicitly pending. Gates: `navigation` 64/64; prepared abort, stale generation, full-entry identity, compositor binding, duplicate admission, and whole-root teardown proofs; `grip:verify changed refresh check`.

## [0.161.0-beta] — 2026-07-16

### Added
- **Prepared NavigationHost history mechanics** (`SPEC-HAL-228`, `SPEC-HAL-231`, `src/model/navigation_host.rs`) — added complete semantic navigation entries, validated root-to-current path intents, independent structural-up and chronological history traversal, derived availability projection, immutable versioned candidates, requested-intent-preserving guard replacement chains, candidate-bound one-shot authorization, checked host and guard-context revalidation, and an exclusive prepared non-fallible history commit. Legacy orchestration entry points remain as a compatibility boundary while the surface/focus/router compound transaction migrates. Gates: `navigation` 62/62; focused host history, replacement, lease, stale-generation, and foreign-host proofs; `grip:verify changed refresh check`.

## [0.160.0-beta] — 2026-07-16

### Added
- **Provider-authoritative ListView multi-column sorting** (`SPEC-HAL-353`, `src/widgets/tabular_view/data_adapter.rs`, `src/widgets/list_view/`, `examples/listview.rs`) — added revision-scoped bounded order windows, provider-declared column/custom sort capabilities, confirmed applied-sort testimony, baseline replacement, persisted requested preferences, and RowId-stable selection across order changes. ListView headers now provide quiet sortable affordances, provider-confirmed direction/priority labels, button-style hover/press feedback, plain-click replacement sorting, and Shift+click additive multi-sort without materializing large provider orders in memory. The one-million-row example proves bounded windows, custom and prohibited keys, multi-sort, clear/reset behavior, restart restoration, and the live click path. Gates: `listview_provider_sorting` 2/2; `listview_interaction_callbacks` 5/5; `cargo run --example listview -- --proof`; native `listview` runtime confirmation; `grip:verify changed refresh check`.

## [0.159.18-beta] — 2026-07-16

### Fixed
- **Gauge example blank window** (`SPEC-HAL-307`, `SPEC-HAL-163`, `src/widgets/gauge.rs`, `examples/gauge.rs`) — gives `Gauge` a finite square intrinsic size under unbounded layout probes and mounts the demo content under an explicit background surface, so the gauge example renders its gauges instead of collapsing or violating text/background scene coherence. Gate: `gauge_layout`; `cargo check --example gauge`; `cargo run --example gauge`; `grip:verify changed refresh check`.

## [0.159.17-beta] — 2026-07-14

### Fixed
- **ListView hydrated header layer ordering** (`SPEC-HAL-186`, `SPEC-HAL-192`, `src/widgets/list_view/list_view.rs`) — assigns fixed header background, labels, separators, and interaction affordances to explicit layers above body and scrollbar content, so hydrated rows cannot cover header chrome after the overlay paint pass. Gate: `listview_paints_fixed_header_chrome_above_hydrated_body`; `grip:verify changed refresh check`.

## [0.159.16-beta] — 2026-07-14

### Fixed
- **ListView scrolled body clip extent** (`SPEC-HAL-186`, `SPEC-HAL-110`, `src/widgets/list_view/body.rs`) — keeps ListViewBody’s internal scrollable-column clips in content space after the body layout result became viewport-sized, so hydrated scrolled rows do not lose scrollable-cell text while pinned cells remain visible. Gate: `listview_scrollable_cells_remain_inside_content_clip_after_scroll`; `grip:verify changed refresh check`.

## [0.159.15-beta] — 2026-07-13

### Fixed
- **ListView hydrated body viewport extent** (`SPEC-HAL-186`, `SPEC-HAL-110`, `src/widgets/list_view/body.rs`) — returns the ListView body’s assigned viewport height as `LayoutResult.size` while preserving full row extent in `content_size`, matching the virtualized body contract used by TreeView and PropertyEditor so hydrated rows cannot expand the ScrollArea child rect under adjacent fixed chrome. Gate: `listview_body_layout_size_stays_viewport_sized_after_hydration`; `grip:verify changed refresh check`.

## [0.159.14-beta] — 2026-07-13

### Fixed
- **ListView hydrated header chrome** (`SPEC-HAL-192`, `SPEC-HAL-193`, `src/widgets/list_view/list_view.rs`) — paints the fixed header background, labels, config affordance, and normal separators in the overlay phase after the ScrollArea body, so hydrated rows cannot visually collapse into or cover the header band. Gate: `listview_paints_fixed_header_chrome_above_hydrated_body`; `grip:verify changed refresh check`.

## [0.159.13-beta] — 2026-07-13

### Fixed
- **ListView hydrated header/body boundary** (`SPEC-HAL-192`, `SPEC-HAL-193`, `src/widgets/list_view/list_view.rs`) — moves the fixed header/body boundary into `paint_overlay()` so hydrated body rows painted by the ScrollArea child cannot cover it. Gate: `listview_paints_fixed_strong_header_body_boundary`; `grip:verify changed refresh check`.

## [0.159.12-beta] — 2026-07-13

### Fixed
- **ListView header/body boundary** (`SPEC-HAL-192`, `SPEC-HAL-193`, `src/widgets/list_view/list_view.rs`) — paints the ListView-owned header/body boundary as a fixed, pixel-snapped strong hairline so dense table headers remain visually separated from the first row without moving body geometry or requiring consumer-side padding. Gate: `listview_paints_fixed_strong_header_body_boundary`; `grip:verify changed refresh check`.

## [0.159.11-beta] — 2026-07-13

### Fixed
- **StatusGrid boundary padding** (`SPEC-HAL-350`, `src/widgets/readout.rs`) — reserves symmetric 8px top and bottom outer padding in `StatusGrid` so dense readout cells do not visually collapse into following widgets such as `ListView` headers. Gate: `status_grid_natural_height_reserves_symmetric_outer_padding`; `grip:verify changed refresh check`.

## [0.159.10-beta] — 2026-07-13

### Fixed
- **ListView resize clamp reversal** (`SPEC-HAL-165`, `SPEC-HAL-169`, `src/widgets/list_view/list_view.rs`) — re-anchors the active resize drag baseline when the substrate clamps a proposed width at min/max, so reversing direction after overdragging a narrow column immediately moves the column and its suffix instead of waiting for the cursor to cross the overshoot distance. Gate: `routed_resize_reverse_after_min_clamp_moves_subsequent_columns_immediately`; `routed_resize`.

## [0.159.9-beta] — 2026-07-13

### Fixed
- **ListView resize with elastic predecessors** (`SPEC-HAL-165`, `SPEC-HAL-169`, `src/engines/hierarchical/mod.rs`) — freezes frame-resolved elastic sibling widths in HTS before committing a user resize, preventing a flex column from reflowing and dragging unrelated relative columns while the target separator moves. Gate: `routed_resize_keeps_elastic_sibling_frozen_while_subsequent_columns_translate`; `list_view_resize`.

## [0.159.8-beta] — 2026-07-13

### Fixed
- **ListView resize sibling translation** (`SPEC-HAL-169`, `src/widgets/list_view/list_view.rs`, `src/widgets/list_view/core.rs`) — restores resize to a single-column width commit so columns to the right translate with the resized column instead of having the adjacent neighbor absorb the inverse delta, while preserving the elastic-sibling freeze from `0.159.6-beta`. Gate: `routed_resize_moves_subsequent_columns_without_resizing_neighbor`; `routed_resize_keeps_elastic_sibling_frozen_while_subsequent_columns_translate`; `grip:verify changed refresh check`.

## [0.159.7-beta] — 2026-07-13

### Fixed
- **ListView divider resize target** (`SPEC-HAL-180`, `src/widgets/list_view/list_view.rs`, `src/widgets/list_view/core.rs`) — stores resize gestures as adjacent-column divider pairs and transfers width from right to left as the pointer moves, so the grabbed boundary follows the cursor and late quote-table separators no longer appear to resize the wrong column. Gate: `routed_resize_transfers_width_between_adjacent_columns`; `routed_resize_transfers_late_separator_width_without_elastic_sibling_absorption`; `grip:verify changed refresh check`.

## [0.159.6-beta] — 2026-07-13

### Fixed
- **ListView resize with elastic siblings** (`SPEC-HAL-169`, `SPEC-HAL-180`, `src/widgets/tabular_view/mod.rs`, `src/model/tabular/column.rs`) — freezes visible flex columns at their resolved widths before applying a user resize, so elastic columns no longer absorb the target column delta and make the dragged separator appear to resize the wrong column. Gate: `routed_resize_freezes_elastic_siblings_before_applying_target_width`; `list_view_resize`; `grip:verify changed refresh check`.

## [0.159.5-beta] — 2026-07-13

### Fixed
- **SplitPane shared-edge idle seam** (`SPEC-HAL-267`, `src/widgets/split_pane.rs`) — stopped painting the idle SplitPane handle line so adjacent child-owned borders do not stack with an extra separator, while preserving the existing hover/drag/focus affordance, 3px layout gap, 9px hit band, and resize cursor behavior. Gate: `split_pane`; `grip:verify changed refresh check`.

## [0.159.4-beta] — 2026-07-13

### Fixed
- **Runtime debug log noise** (`SPEC-HAL-345`, `SPEC-HAL-128`, `src/core/app.rs`, `src/core/widget/mod.rs`, `src/core/event_router.rs`) — gated stale `[HAL-PASS]` layout-pass dumps behind explicit `HAL_PROFILE_LAYOUT_PASS` opt-in, skipped disabled per-child timing work, and suppressed empty bulk pointer-cancel `INFO` traces while preserving real cancellation logging. Gate: `grip:verify changed refresh check`.

## [0.159.3-beta] — 2026-07-13

### Fixed
- **StatusGrid readout label clipping** (`SPEC-HAL-350`, `src/widgets/readout.rs`) — derives StatusGrid label clip/hit region height from active readout typography metrics instead of fixed 12px rectangles, preserving dense row geometry while keeping paint and read-only text-selection regions aligned for inspector labels. Gate: `status_grid_label_clip_regions_cover_active_font_metrics`; `grip:verify changed refresh check`.

## [0.159.2-beta] — 2026-07-12

### Fixed
- **SplitPane separator affordance** (`SPEC-HAL-267`, `src/widgets/split_pane.rs`) — reduced the SplitPane layout gap to a thinner 3px handle with a 1px visual separator while preserving a 9px centered drag hit band, and kept the resize cursor requested across the full hover hit zone even when pointer routing targets an adjacent child pane. Gate: `split_pane`; `grip:verify changed refresh check`.

## [0.159.1-beta] — 2026-07-12

### Fixed
- **ListView range-drag anchor preservation** (`SPEC-HAL-347`, `src/widgets/list_view/core.rs`) — kept selection intents as the focus authority during drag extension so `ExtendToRow` no longer gets followed by a fallback `FocusTo` that moves the anchor to the previous cursor row. Range drag now extends from the original anchor across continued pointer moves. Gate: `range_drag_routes_through_policy_and_notifies_focused_row`; `grip:verify changed refresh check`.
- **SplitPane constrained minimum handling** (`SPEC-HAL-267`, `src/widgets/split_pane.rs`) — centralized SplitPane size/range bounds so layout, hit-testing, pointer drag, and keyboard Home/End proportionally clamp panels when the parent allocation is smaller than both panel minimums plus the handle. Workbench center/inspector pane resizing no longer panics from an invalid `f32::clamp` range or emits negative child constraints. Gate: `split_pane`; `grip:verify changed refresh check`.

## [0.159.0-beta] — 2026-07-12

### Added
- **Section surface family** (`SPEC-HAL-351`, `src/widgets/section_surfaces.rs`, `examples/section_surfaces.rs`) — added separate public section surfaces for tonal bands, metadata-bearing bands, and disclosure headers while keeping `SectionHeader` as the descriptive label primitive. `DisclosureSectionHeader` reflects externally supplied expanded state and emits only an explicit toggle request; `SectionBandHeader` and `SectionMetaHeader` remain passive visual surfaces. The disclosure header uses a compact inset disclosure control with row-level hover/press activation, up-inside activation with up-outside cancellation, keyboard activation, and control-rect focus geometry while still leaving expanded/content state external. `DisclosureIndicator` paints a pixel-snapped one/two-vertical-bar affordance instead of Unicode text glyphs or filled icon-path disclosure wedges. The standalone demo shows the section-header cases in an inspector-style surface without explanatory demo captions. Gate: `section_surface_family_keeps_distinct_header_authority`; `disclosure_section_header_requests_explicit_toggle_without_owning_state`; `disclosure_indicator_paints_vertical_bar_affordance`; `grip:verify changed refresh check`.
- **Transient scroll anchoring transactions** (`SPEC-HAL-352`) — added explicit `WidgetTree`/`ScrollArea` coordination for layout-changing disclosures. Collapse preserves the toggled header with transient tail slack separate from true content extent and scrollbar range; expansion minimally reveals newly opened content. User scrolling, committed programmatic/reveal/linked movement, resize, removal, a replacement transaction, or true-range growth retire or absorb slack at the owning boundary. Gate: `scroll_layout_anchor`; `cargo check --example section_surfaces`; native `section_surfaces` runtime confirmation.

## [0.158.2-beta] — 2026-07-12

### Fixed
- **TreeView row-hover lifecycle** (`SPEC-HAL-262`, `src/widgets/tree_view/body.rs`) — clear the identity-based row-hover projection through Halcyon's existing `on_hover_leave` lifecycle when the pointer moves to another widget or leaves the window, preventing a stale hover band after the cursor exits TreeView. Gate: `treeview_body_hover_leave_clears_identity_hover_projection`; `grip:verify changed refresh check`.

## [0.158.1-beta] — 2026-07-12

### Fixed
- **SectionHeader subtle-label legibility** (`SPEC-HAL-349`, `src/widgets/section_header.rs`) — kept `Subtle` case-preserving and secondary-colored while restoring the normal compact SectionHeader font size, so dense subsection labels remain quiet without reading as undersized metadata. Gate: `section_header_subtle_emphasis_preserves_case_without_undersizing_label`; `grip:verify changed refresh check`.
- **Readout field-label legibility** (`SPEC-HAL-350`, `src/widgets/readout.rs`) — replaced separate 10px literals in `StatusGrid` and `KeyValueStrip` with one theme-derived readout typography role, resolving canonical labels to 11px while preserving muted label and primary value semantics. Gate: `status_grid_and_key_value_strip_share_theme_owned_readout_typography`; `grip:verify changed refresh check`.

## [0.158.0-beta] — 2026-07-12

### Added
- **Optional tabular header bands** (`SPEC-HAL-348`, `src/widgets/list_view/list_view.rs`, `src/widgets/tree_view/tree_view.rs`) — added matching `set_header_visible()` / `header_visible()` consumer APIs to ListView and TreeView. Hidden headers resolve to zero height inside the existing closed composite, move the owned body viewport to the top edge, preserve footer and tabular mechanics, and update ListView natural height without consumer geometry reconstruction. Gates: `hidden_` package tests 11/11; `grip:verify changed refresh check`.
- **SectionHeader subtle emphasis** (`SPEC-HAL-349`, `src/widgets/section_header.rs`) — added `SectionHeaderEmphasis::Subtle` and builder/readback APIs for compact, case-preserving, secondary-color subsection labels without a background well, container semantics, or consumer-painted replacement. Gate: `section_header_subtle_emphasis_preserves_case_and_reduces_visual_weight`; `grip:verify changed refresh check`.

## [0.157.0-beta] — 2026-07-11

### Added
- **TooltipHost consumer wrapper** (`SPEC-HAL-226A`, `src/widgets/tooltip_host.rs`) — added the specified public, layout-transparent, non-hit-testable single-child tooltip host over the existing framework tooltip manager and placement authority. The showcase now consumes this owner primitive instead of maintaining a private duplicate. Gate: `consumer_contract_tooltip_host_preserves_child_layout_and_registers_owned_trigger`; `grip:verify changed refresh check`.
- **ListView natural-height query** (`SPEC-HAL-183`, `SPEC-HAL-186`, `src/widgets/list_view/list_view.rs`) — added `ListView::natural_height()` as a read-only aggregate of owner-authored header, row-oracle content, and footer height so consumers can size bounded short lists without reconstructing ListView geometry. Gate: `consumer_contract_listview_natural_height_uses_owned_band_and_row_geometry`; `grip:verify changed refresh check`.

## [0.156.0-beta] — 2026-07-11

### Added
- **StatusGrid tone presentation** (`SPEC-HAL-332`, `SPEC-HAL-096`, `src/widgets/readout.rs`) — added grid-level `StatusGridTonePresentation::EdgeRail` for narrow item-owned semantic edge accents while preserving the existing inline-marker default, per-item tooltips, and authoritative text. Edge rails use theme semantic colors and do not consume value-text width. Gate: `status_grid_edge_rail_paints_item_owned_semantic_accents`; `grip:verify changed refresh check`.

## [0.155.0-beta] — 2026-07-11

### Added
- **StatusGrid item presentation** (`SPEC-HAL-226A`, `SPEC-HAL-332`, `SPEC-HAL-096`, `src/widgets/readout.rs`) — `StatusGridCell`, `StatusGridWideRow`, and `StatusGridMetric` now accept per-item informational tooltip text and optional theme-backed `Info`, `Success`, `Warning`, or `Error` markers while retaining their textual values. StatusGrid computes and registers its own item trigger geometry, so consumers do not reconstruct readout layout. Gate: `status_grid_registers_item_tooltips_and_paints_semantic_markers`; `grip:verify changed refresh check`.

### Fixed
- **Composite tooltip trigger identity** (`SPEC-HAL-226A`, `src/model/tooltip.rs`) — `TooltipManager` now distinguishes trigger rectangles owned by the same widget and refreshes content for a stable trigger, allowing paint-only composite widgets to expose item-level tooltips through the existing framework lifecycle. Gates: focused tooltip-manager tests 2/2; `grip:verify changed refresh check`.

## [0.154.2-beta] — 2026-07-11

### Fixed
- **Retained flex mutation invalidation** (`SPEC-HAL-026`, `SPEC-HAL-158`, `src/core/widget_tree.rs`) — `WidgetTree::set_flex()` now marks the changed node and its ancestors layout-dirty when `FlexParams` change. A mounted ListView that starts with a zero-height body allocation now receives a new layout and realizes newly published rows when its consumer grows the flex allocation, without redraw forcing or caller-owned dirty manipulation. Gate: `empty_listview_realizes_rows_after_flex_height_growth`; `grip:verify changed refresh check`.

## [0.154.1-beta] — 2026-07-11

### Fixed
- **ListView focused-row owner compliance** (`SPEC-HAL-347`, `src/widgets/list_view/`) — reconciled focused-row observation with the existing `ListViewCore` + `OrderedSelectionState` storage architecture, routed range-drag updates through `IntentInput::DragMove`, made the multi-selection preset declare its established range-drag behavior, and atomically rejected policy changes after the first routed row interaction. Focus callbacks are restricted to pending consumer action/event delivery rather than direct UI or Halcyon `AppState` mutation. Gates: focused ListView interaction callbacks; focused intent-policy drag/factory oracles; `grip:verify changed refresh check`.

## [0.154.0-beta] — 2026-07-11

### Added
- **ListView focused-row consumer observation** (`SPEC-HAL-347`, `src/widgets/list_view/`) — added identity-authoritative `RowId` focus notification after internal borrow release, public `TabularIntentPolicy` configuration/readback, and a read-only focused-row query so pointer and keyboard navigation can drive consumer projections without event reconstruction.

### Fixed
- **ListView click activation delivery** (`src/widgets/list_view/body.rs`) — pointer clicks now deliver `ActivationMode::OnClick` intents through the existing activation callback while preserving independent focus notification and Enter/double-click activation behavior. Gates: focused ListView interaction callback tests 3/3; focused intent-policy click/keyboard/Enter/double-click oracles 4/4; `grip:verify changed refresh check` 0 errors.

## [0.153.8-beta] — 2026-07-05

### Fixed
- **ListView demo blank footer band** (`examples/showcase/tab_list_view.rs`, `examples/listview.rs`) — kept the structural ListView footer child mounted but zero-height in the showcase and standalone demos so the visible ListView surface no longer ends with a dead separator-only footer strip. Gates: `cargo check --example showcase`; `cargo check --example listview`; `grip:verify changed refresh check` 0 errors.

## [0.153.7-beta] — 2026-07-05

### Fixed
- **TreeView resize horizontal scroll freeze** (`src/widgets/tree_view/tree_view.rs`, `src/widgets/tree_view/body.rs`) — added TreeView’s shared horizontal-freeze flag and wired TreeView scroll areas through `ScrollArea::with_horizontal_freeze()` so horizontal wheel/trackpad deltas are frozen while a column resize is armed or dragging. Gates: `cargo test --lib tree_view_horizontal_scroll_freeze`; `cargo test --lib tree_view_finalize_resize_clears_horizontal_scroll_freeze`; `grip:verify changed refresh check` 0 errors.

## [0.153.6-beta] — 2026-07-05

### Changed
- **SPEC-PM-007 row-state layering authority** (`docs/specs/SPEC-PM-007-Clip-Hash-Ordering-Hazard.md`) — aligned the row-state overlay rule and audit table with the landed ListView body ordering from `90bf3812`: base background below row-state repaint, row-state below normal separators, and interaction separators highest.

### Fixed
- **TreeView footer paint layering** (`src/widgets/tree_view/footer.rs`) — split footer background, normal separator/top-border, and active/hovered separator layers so clipped footer chrome no longer shares the background layer. Gates: `cargo test --lib tree_view_footer_layers_keep_chrome_above_background`; `grip:verify changed refresh check` 0 errors.

## [0.153.5-beta] — 2026-07-03

### Fixed
- **ListView row highlight separator visibility** (`src/widgets/list_view/body.rs`) — kept normal body column separators above hovered/selected row overlays so column boundaries remain visible on highlighted rows. Gates: `cargo test --lib body_separator_layers_keep_separators_above_row_state`; `grip:verify changed refresh check` 0 errors.

## [0.153.4-beta] — 2026-07-03

### Fixed
- **ListView active separator held-row visibility** (`src/widgets/list_view/body.rs`) — raised active/hovered body resize separators above the row-state overlay so the row under the original pointer-down cannot cover the separator segment during drag. Gates: `cargo test --lib body_separator_layers_keep_active_separator_above_row_state`; `grip:verify changed refresh check` 0 errors.

## [0.153.3-beta] — 2026-07-03

### Fixed
- **ListView drag ghost zebra flicker** (`src/widgets/list_view/list_view.rs`) — split dragged-column ghost painting into strict shadow, surface, row-fill, chrome, and border layers so zebra row fills no longer share a draw layer with the ghost surface across clipped paint batches. Gates: `cargo test --lib drag_proxy_tests`; `grip:verify changed refresh check` 0 errors.

## [0.153.2-beta] — 2026-07-03

### Changed
- **Showcase Workbench toolbar removal** (`examples/showcase/tab_workbench.rs`) — removed the local File/Edit/View/Process/Help command buttons from the Workbench tab while preserving the right-aligned profile strip. Gates: `grip:verify changed refresh check` 0 errors; `cargo check --example showcase`.

## [0.153.1-beta] — 2026-07-02

### Fixed
- **ListView drag-reorder inline editor ghost** (`src/engines/cell_mechanics/mod.rs`, `src/widgets/list_view/body.rs`, `src/widgets/list_view/list_view.rs`) — parked always-mounted editor children for the dragged column during active column reorder and painted ComboBox-backed string cells with a ComboBox-shaped drag proxy, preventing stale inline editor chrome from staying overlapped at the original column. Gates: `grip:verify changed refresh check` 0 errors; focused `cargo test --lib` ComboBox drag proxy and editor parking tests 2/2.

## [0.153.0-beta] — 2026-06-21

### Added
- **Pre-beta cleanup and editor factory boundary wiring** (`src/engines/cell_source/mod.rs`, `src/widgets/cell_editor_factory.rs`, `src/widgets/gauge.rs`, `tests/`, `examples/`) — moved combo-box cell editor state into the engine cell-source boundary, added boolean and combo-box editor factory hooks, repaired stale routed input/focus/geometry tests and examples, resolved Halcyon compiler warnings, and fixed full-circle gauge endpoint label overpaint. Gates: `grip:check_health` 0 errors, 0 warnings; `cargo test --message-format=short` 3269 passed, 39 ignored.

## [0.152.5-alpha] — 2026-06-19

### Fixed
- **Toast dismiss affordance removal** (`src/widgets/toast.rs`, `src/tests/toast_manager.rs`) — removed the expanded toast close icon, hover square, border, dismiss-button geometry, and separate dismiss target after the Showcase close affordance repeatedly rendered without a visible icon; toast body clicks still dismiss visible toasts and the paint test now gates that no dismiss affordance is painted. Gates: `grip:check_health` 0 errors; focused toast paint/body-dismiss test 1/1; `cargo check --example showcase`.

## [0.152.4-alpha] — 2026-06-19

### Fixed
- **Toast close icon and text alignment** (`src/core/icons/mod.rs`, `src/widgets/toast.rs`, `src/tests/icon_builder.rs`, `src/tests/toast_manager.rs`) — toast dismiss controls now use a visibly sized built-in close icon path instead of a text fallback, the close glyph geometry is wider for compact rendering, and toast message text is vertically centered with action labels. Gates: `grip:check_health` 0 errors; focused toast paint test 1/1; focused icon registry/hash tests; `cargo check --example showcase`.

## [0.152.3-alpha] — 2026-06-19

### Fixed
- **Toast close affordance visibility** (`src/widgets/toast.rs`, `src/tests/toast_manager.rs`) — expanded toast dismiss controls now render a centered text close mark through the regular text path instead of short diagonal line strokes, making the close affordance visible on the light theme hover surface. Gates: `grip:check_health` 0 errors; focused toast paint test 1/1; `cargo check --example showcase`.

## [0.152.2-alpha] — 2026-06-19

### Fixed
- **Theme-aware toast rendering** (`src/widgets/toast.rs`, `src/tests/toast_manager.rs`) — toasts now draw from the active theme surfaces, typography, borders, controls, icons, and accent roles, and expanded toast close controls render direct close marks instead of depending on an absent icon entry. Gates: `grip:check_health` 0 errors; focused toast paint test 1/1; `cargo test toast`; `cargo check --example showcase`.

## [0.152.1-alpha] — 2026-06-19

### Fixed
- **Showcase workstation chrome pass** (`examples/showcase/tab_inputs.rs`, `src/widgets/groupbox.rs`, `src/widgets/tab_bar.rs`, `src/widgets/breadcrumbs.rs`) — tightened Inputs and Controls spacing, made `GroupBox::with_padding` and explicit flat borders affect rendered geometry, and reduced duplicate focus chrome on active tabs and breadcrumbs. Gates: `grip:check_health` 0 errors; `cargo check --example showcase`.

## [0.152.0-alpha] — 2026-06-19

### Added
- **SPEC-HAL-346 executable acceptance audit** (`src/tests/interaction_fuzz/acceptance.rs`, `src/tests/interaction_fuzz/mod.rs`, `src/tests/interaction_fuzz/chaos.rs`) — added a 50-criterion white-box acceptance audit that runs the real driver, intent expansion, CI smoke, chaos, replay, shrink, negative validation, visual trace, visual confirmation, failure corpus, and promotion paths against SPEC-HAL-346 acceptance criteria. Gates: `grip:check_health` 0 errors; focused acceptance audit test 1/1; `interaction_fuzz` lib filter 130/130.

## [0.151.0-alpha] — 2026-06-19

### Added
- **SPEC-HAL-346 run report and boundary report hardening** (`src/tests/interaction_fuzz/runner.rs`, `src/tests/interaction_fuzz/smoke.rs`) — runner output can now emit a typed run report for the required `run-report.json` artifact, smoke reports carry Constitution boundary profile/check status directly, and negative-validation case reports now carry and gate on case-level Constitution boundary metadata. Gates: `grip:check_health` 0 errors; focused smoke profile tests 3/3; focused runner tests 26/26; `interaction_fuzz` lib filter 129/129.

## [0.150.0-alpha] — 2026-06-19

### Added
- **SPEC-HAL-346 ListView resize oracle smoke registration** (`src/tests/interaction_fuzz/list_view_resize.rs`, `src/tests/interaction_fuzz/smoke.rs`) — the CI smoke confidence packet now declares and records ListView resize oracle evaluations for direction monotonicity, no opposite-direction first drag, scroll-aware separator hit-testing, and repeated drag consistency, using routed resize probes and laid-out separator probe snapshots. Gates: `grip:check_health` 0 errors; focused ListView resize tests 14/14; focused smoke profile tests 3/3; focused runner tests 25/25.

## [0.149.0-alpha] — 2026-06-19

### Added
- **SPEC-HAL-346 smoke oracle coverage** (`src/tests/interaction_fuzz/smoke.rs`) — the bounded CI smoke now declares and records universal finite-sink and incidental-navigation integrity oracle evaluations in addition to Constitution boundary checks, deriving U11 evaluations from executed intent trace indices and generator-declared incidental targets. Gates: `grip:check_health` 0 errors; focused smoke profile tests 3/3; focused runner tests 25/25.

## [0.148.0-alpha] — 2026-06-19

### Added
- **SPEC-HAL-346 TreeView smoke coverage** (`src/tests/interaction_fuzz/generators.rs`, `src/tests/interaction_fuzz/smoke.rs`, `src/tests/interaction_fuzz/tree_view_projection.rs`) — the bounded CI interaction smoke now executes `tv1` through the shared first-separator resize generator, requires all declared fixtures and root seeds before reporting coverage saturation, and asserts that the transaction budget exactly covers the declared scenario set. Gates: `grip:check_health` 0 errors; focused generator registry tests 11/11; focused fixture registry tests 2/2; focused smoke profile tests 3/3.

## [0.147.0-alpha] — 2026-06-19

### Added
- **SPEC-HAL-346 TreeView fixture registry entry** (`src/tests/interaction_fuzz/registry.rs`, `src/tests/interaction_fuzz/tree_view_projection.rs`, `src/tests/interaction_fuzz/generators.rs`) — `FIXTURE-346-TV1` is now addressable as `tv1` in the deterministic interaction fixture registry, builds a TreeView basic projection fixture, and carries a TreeView probe source id for generated targets. Gates: `grip:check_health` 0 errors; focused fixture registry tests 2/2; focused TreeView projection probe test 1/1.

## [0.146.0-alpha] — 2026-06-19

### Added
- **SPEC-HAL-346 TreeView projection probes** (`src/widgets/tree_view/tree_view.rs`, `src/widgets/tree_view/body.rs`, `src/tests/interaction_fuzz/tree_view_projection.rs`) — TreeView now exposes test-only tabular and hierarchy probe data after layout, including structural rects, column geometry, realized rows/cells, visible projection, expanded node IDs, disclosure hit zones, and indentation geometry. Gates: `grip:check_health` 0 errors; focused TreeView projection probe test 1/1.

## [0.145.0-alpha] — 2026-06-19

### Added
- **SPEC-HAL-346 generator version provenance** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/smoke.rs`, `src/tests/interaction_fuzz/chaos.rs`) — interaction fuzz profiles now require structured generator-version records, and chaos reports, chaos failure fields, and chaos corpus entries record chaos stream generator version for replay provenance. Gates: `grip:check_health` 0 errors; focused missing-generator-version profile test 1/1; focused CI smoke profile test 1/1; focused chaos profile test 1/1; focused chaos report test 1/1; focused chaos failure packet test 1/1; focused chaos failure corpus test 1/1; `interaction_fuzz` lib filter 127/127.

## [0.144.0-alpha] — 2026-06-19

### Added
- **SPEC-HAL-346 chaos generated command records** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/chaos.rs`) — chaos reports now serialize per-command generation metadata including primitive step index, replay command, command domain, pressure patterns, pointer motion policy, pointer sample ordinal, and frame/reconcile boundary flags. Gates: `grip:check_health` 0 errors; focused chaos report serialization test 1/1.

## [0.143.0-alpha] — 2026-06-19

### Added
- **SPEC-HAL-346 visual confirmation probe context** (`src/tests/interaction_fuzz/runner.rs`) — visual confirmation reports now carry probe snapshot artifacts copied from the replayed failure packet, preserving probe-derived diagnostic context without reading or mutating live runtime state. Gates: `grip:check_health` 0 errors; focused no-failure visual confirmation test 1/1; focused failure-packet visual confirmation test 1/1.

## [0.142.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 failure packet probe snapshots** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/list_view_resize.rs`, `src/tests/interaction_fuzz/chaos.rs`) — failure packets now carry serialized probe snapshot artifacts with provenance fields and payload JSON, the ListView resize probe proves it can be wrapped for failure-packet/report use, and chaos packet builders populate the new field explicitly. Gates: `grip:check_health` 0 errors; focused core failure-packet schema test 1/1; focused ListView probe artifact test 1/1; focused chaos failure-packet serialization test 1/1.

## [0.141.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 ListView structural probe fields** (`src/tests/interaction_fuzz/list_view_resize.rs`, `src/widgets/list_view/list_view.rs`) — ListView resize probe snapshots now serialize widget id, root/header/body/footer rects, horizontal and vertical scroll offsets, resize/hover state, expansion-region state, selected rows, and focused row from read-only test probes. Gates: `grip:check_health` 0 errors; focused ListView structural probe test 1/1.

## [0.140.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 ListView tabular probe coverage** (`src/tests/interaction_fuzz/list_view_resize.rs`, `src/widgets/list_view/list_view.rs`) — ListView resize probe snapshots now serialize column affordance/constraint metadata, realized row projections, and realized cell content states from read-only test probes. Gates: `grip:check_health` 0 errors; focused ListView probe shape test 1/1.

## [0.139.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 probe snapshot provenance** (`src/tests/interaction_fuzz/list_view_resize.rs`) — ListView resize probe snapshots now record source layer, source owner, layout epoch, frame number, settle boundary, and snapshot reason, and probe snapshot data now serializes for failure-packet/report use. Gates: `grip:check_health` 0 errors; focused probe provenance test 1/1.

## [0.138.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 probe-derived generator targets** (`src/tests/interaction_fuzz/generators.rs`) — default user-like generators now emit target-based intent commands with semantic target identity, coordinate space, source probe id, and deterministic probe epoch instead of relying on coordinate helper constructors. Replay serialization coverage now proves generated probe targets survive into artifact JSON. Gates: `grip:check_health` 0 errors; generator module tests 11/11; smoke module tests 3/3.

## [0.137.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 semantic intent target storage** (`src/core/interaction_fuzz.rs`) — intent commands now store versioned/probe-derived `InteractionIntentTarget` metadata instead of raw points, replay intent commands serialize target identity/probe fields with resolved coordinates, and coordinate helper constructors preserve compatibility by producing synthetic point targets. Gates: `grip:check_health` 0 errors; focused probe-derived target replay test 1/1; focused target metadata regressions 2/2; generator module tests 9/9.

### Fixed
- **Headless visual diagnostics schema assertion** (`src/core/interaction_fuzz.rs`) — failure-packet schema coverage now verifies the required `unavailable`, `reason`, and `profile_policy` fields for headless runs instead of requiring capture availability.

## [0.136.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 intent target trace metadata** (`src/core/interaction_fuzz.rs`) — intent traces and replay artifacts now record resolved target identity, target kind/class, logical coordinate space, resolved coordinate, source probe id, and probe epoch for coordinate-resolved intent targets. Gates: `grip:check_health` 0 errors; focused click target metadata replay test 1/1; focused drag endpoint metadata test 1/1.

## [0.135.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 click and text intent expansion** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/generators.rs`) — added `ClickTarget` and `TextAtTarget` intent commands with deterministic pointer-path expansion, primitive press/release/text traces, coverage classes, strict replay serialization, and exact primitive replay. Gates: `grip:check_health` 0 errors; focused click intent replay test 1/1; focused text intent replay test 1/1; generator module tests 9/9.

## [0.134.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 runner report Constitution boundary metadata** (`src/tests/interaction_fuzz/runner.rs`, `src/tests/interaction_fuzz/chaos.rs`) — replay, negative-validation, visual-trace, visual-confirmation, regression-promotion, and chaos-shrink reports now record Constitution boundary profile and boundary-check status so every runner-side report carries explicit authority metadata. Gates: `grip:check_health` 0 errors; focused runner report boundary tests 5/5; focused chaos shrink boundary test 1/1.

## [0.133.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 negative validation completeness gate** (`src/tests/interaction_fuzz/runner.rs`) — runner negative-validation gating now rejects truncated reports that omit any required validation class, preventing an all-passing partial report from authorizing a run. Gates: `grip:check_health` 0 errors; focused missing negative-validation class gate test 1/1.

## [0.132.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 failure packet visual diagnostics status** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/chaos.rs`) — failure packets now record explicit visual diagnostics availability state and chaos failure packets emit either a concrete visual trace artifact path or an unavailable-diagnostics artifact bound to the detector id. Gates: `grip:check_health` 0 errors; focused non-finite chaos failure packet test 1/1; focused capture-available visual artifact test 1/1; broad `interaction_fuzz::chaos` attempt hit the tool timeout before returning.

## [0.131.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 first generator set completion** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/generators.rs`, `src/tests/interaction_fuzz/smoke.rs`) — added wheel-at-target intent expansion/replay support and completed the initial deterministic generator set with repeated resize, inverse drag restoration, scroll-then-resize, focus traversal, overshoot correction, and hover-pause pointer paths. The CI smoke profile now reports coverage saturation across the full declared generator frontier and both declared root seeds. Gates: `grip:check_health` 0 errors; focused wheel intent replay test 1/1; generator module tests 9/9; bounded CI smoke artifact test 1/1.

## [0.130.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos pressure coverage reporting** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/chaos.rs`) — chaos reports now carry a dedicated pressure coverage summary with the required SPEC pressure dimensions, separate from semantic confidence coverage, and prove that the deterministic chaos stream saturates the declared pressure frontier without emitting a confidence packet. Gates: `grip:check_health` 0 errors; focused chaos pressure report tests 2/2.

## [0.129.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 strict replay artifact decoding** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/runner.rs`) — replay artifact deserialization now rejects unknown fields in the runner replay envelope, nested replay artifact, replay command variants, and replay trace steps instead of accepting schema drift silently. Gates: `grip:check_health` 0 errors; focused strict replay decoding test 1/1.

## [0.128.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 artifact schema identity enforcement** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/runner.rs`, `src/tests/interaction_fuzz/chaos.rs`, `src/tests/interaction_fuzz/smoke.rs`) — added canonical `halcyon.interaction_fuzz.v1` schema identity fields to serialized interaction fuzz profiles, replay artifacts, packets, reports, chaos corpus entries, and shrink artifacts, and rejects replay artifacts whose outer or nested schema identity does not match. Gates: `grip:check_health` 0 errors; focused schema identity tests 2/2.

## [0.127.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 negative validation coverage** (`src/tests/interaction_fuzz/runner.rs`) — extended negative validation to cover authority-violation hard-failure detection and shrinker preservation validators for injection-boundary and hard-failure-class mutation artifacts. Gates: `grip:check_health` 0 errors; focused negative-validation coverage tests 3/3.

## [0.126.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 replay artifact authority validation** (`src/tests/interaction_fuzz/runner.rs`) — added shared replay artifact schema/profile validation across replay, visual trace, and visual confirmation runner operations so diagnostics cannot render from unsupported schemas or mismatched replay/profile artifacts. Gates: `check_health` 0 errors; focused replay/visual authority tests 3/3; broad `interaction_fuzz::runner` attempt hit the tool call timeout before returning.

## [0.125.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos pointer motion policies** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/chaos.rs`) — added explicit chaos pointer motion policy metadata for sampled pointer movement, surfaced the policy set in chaos reports, and included motion-policy keys in chaos corpus coverage and frontier saturation. Gates: `check_health` 0 errors; focused chaos motion/report/halt tests 4/4; broad `interaction_fuzz::chaos` attempt hit the tool call timeout before returning.

## [0.124.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-129 no-stub enforcement for SPEC-HAL-346 promotion** (`tests/no_stub_tests_gate.rs`, `tests/*_invariants.rs`) — replaced SPEC-HAL-107-V empty invariant tests with executable assertions over layout, line/newline, selection, insertion, enter, and deletion behavior, and added an integration-test gate that fails empty or placeholder test bodies. Gates: `check_health` 0 errors; focused invariant/no-stub integration tests 35/35.

## [0.123.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 visual confirmation profile** (`src/tests/interaction_fuzz/runner.rs`) — added a visual confirmation report derived only from an existing replay artifact, recording replay command, primitive trace, command timeline, pointer path overlay, target timeline, frame-sequence status, and failure/no-failure status with profile-hash enforcement. Gates: `check_health` 0 errors; `interaction_fuzz::runner` 18/18; `interaction_fuzz` 105/105.

## [0.122.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 regression promotion gate** (`src/tests/interaction_fuzz/runner.rs`) — added a runner promotion report for shrunk failures that enforces `tests/` or `src/tests/` promotion targets, profile-hash continuity, deterministic shrunk-trace replay, invariant assertion identity, and SPEC-HAL-129 no-stub status. Gates: `check_health` 0 errors; `interaction_fuzz::runner` 15/15; `interaction_fuzz` 102/102.

## [0.121.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos corpus saturation halt** (`src/tests/interaction_fuzz/chaos.rs`) — added an explicit memory-only pressure-frontier corpus policy that halts chaos mode with `CorpusSaturation` after required primitive-family and pointer-pressure coverage enters the run corpus. Gates: `check_health` 0 errors; `interaction_fuzz::chaos` 27/27; `interaction_fuzz` 100/100.

## [0.120.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos no-new-coverage halt** (`src/tests/interaction_fuzz/chaos.rs`) — added deterministic chaos pressure-frontier tracking and `NoNewCoverageWindow` halting after required primitive-family and pointer-pressure coverage is saturated. Gates: `check_health` 0 errors; `interaction_fuzz::chaos` 26/26; `interaction_fuzz` 99/99.

## [0.119.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos settle-budget exhaustion detector** (`src/tests/interaction_fuzz/chaos.rs`) — added timeout/no-progress detection for dirty or deferred progress markers that exceed the declared settle iteration budget before reaching the settle boundary. Gates: check_health 0 errors; interaction_fuzz::chaos 25/25; interaction_fuzz 98/98.

## [0.118.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos unsupported command/domain skip detector** (`src/tests/interaction_fuzz/chaos.rs`) — added pre-injection detection for generated commands outside the declared primitive-family surface or disabled command domains, with replayable hard-failure packets recording the rejected family/domain, primitive trace, progress markers, and unsupported-domain evidence. Gates: check_health 0 errors; interaction_fuzz::chaos 24/24; interaction_fuzz 97/97.

## [0.117.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos replay-nondeterminism detector** (`src/tests/interaction_fuzz/chaos.rs`) — added exact primitive replay comparison after successful chaos streams, with replay-layer failure packets recording divergence class, expected and replayed artifact fingerprints, primitive trace, progress markers, and failed replay determinism status. Gates: check_health 0 errors; interaction_fuzz::chaos 22/22; interaction_fuzz 95/95.

## [0.116.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos authority-violation detector** (`src/tests/interaction_fuzz/chaos.rs`) — added a progress-marker-backed authority violation detector using the Constitution boundary evaluator, with replayable chaos failure packets that record the authority domain, failed C1 requirement, primitive trace, progress markers, and failed boundary status. Gates: check_health 0 errors; interaction_fuzz::chaos 20/20; interaction_fuzz 93/93.

## [0.115.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos router-contradiction detector** (`src/tests/interaction_fuzz/chaos.rs`) — added a progress-marker-backed event router contradiction detector for captured target vs routed target divergence, with replayable chaos failure packets that record the contradictory route state, primitive trace, and progress marker history. Gates: check_health 0 errors; interaction_fuzz::chaos 18/18; interaction_fuzz 91/91.

## [0.114.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos dirty-settle detector** (`src/tests/interaction_fuzz/chaos.rs`) — added a progress-marker-backed dirty-layout-at-settle-boundary detector using the existing layout-settled oracle, with replayable chaos failure packets that record dirty state, failed U7 requirement, primitive trace, and progress marker history. Gates: check_health 0 errors; interaction_fuzz::chaos 16/16; interaction_fuzz 89/89.

## [0.113.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos capture-teardown detector** (`src/tests/interaction_fuzz/chaos.rs`) — added a progress-marker-backed pointer capture release/teardown detector using the existing capture-owner oracle, with replayable chaos failure packets that record leaked capture owner class, failed U3 requirement, primitive trace, and progress marker history. Gates: check_health 0 errors; interaction_fuzz::chaos 14/14; interaction_fuzz 87/87.

## [0.112.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos panic/unwind detector** (`src/tests/interaction_fuzz/chaos.rs`) — wrapped chaos primitive injection in an unwind boundary and emit replayable hard-failure packets for panic/crash/unwind escapes with payload, failing command index, primitive trace, progress markers, and chaos failure fields. Gates: check_health 0 errors; interaction_fuzz::chaos 12/12; interaction_fuzz 85/85.

## [0.111.0-alpha] — 2026-06-18

### Changed
- **SPEC-HAL-346 chaos hard-failure oracle declaration** (`src/tests/interaction_fuzz/chaos.rs`) — promoted the implemented timeout/no-progress detector into the enabled chaos hard-failure oracle set and removed it from declared unsupported domains so chaos reports match the detector surface that now exists. Gates: check_health 0 errors; interaction_fuzz::chaos 10/10; interaction_fuzz 83/83.

## [0.110.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 declared timeout/no-progress detection** (`src/tests/interaction_fuzz/chaos.rs`) — added deterministic chaos timeout and no-progress detection from declared profile timeout policy plus recorded progress marker history, emitting replayable hard-failure packets with timeout budget, active command, observed elapsed values, and progress markers. Gates: check_health 0 errors; interaction_fuzz::chaos 10/10; interaction_fuzz 81/81.

## [0.109.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 contract smoke runner wiring** (`src/tests/interaction_fuzz/runner.rs`, `src/tests/interaction_fuzz/smoke.rs`) — exposed the canonical CI smoke fuzz profile and taught the runner `run(profile)` path to return a hash-checked confidence packet for contract smoke profiles, while preserving chaos reports for chaos modes. Gates: `check_health` 0 errors; `interaction_fuzz::runner` 13/13; `interaction_fuzz` 81/81.

## [0.108.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 visual trace diagnostics** (`src/tests/interaction_fuzz/runner.rs`) — expanded runner visual trace reports with deterministic pointer path samples, target timeline entries, replay-derived progress markers, and explicit failure-frame context while preserving visual diagnostics as non-authoritative. Gates: `check_health` 0 errors; `interaction_fuzz::runner` 12/12; `interaction_fuzz` 80/80.

## [0.107.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 negative validation gate** (`src/tests/interaction_fuzz/runner.rs`) — enforced negative validation profile/hash matching and failed-case rejection before run, replay, shrink, and visual trace reports can pass; shrink now requires the complete originating profile before producing a shrunk failure packet. Gates: `check_health` 0 errors; `interaction_fuzz::runner` 12/12; `interaction_fuzz` 80/80.

## [0.106.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 negative validation samples** (`src/tests/interaction_fuzz/runner.rs`) — replaced label-only negative validation with deterministic local invalid samples backed by existing finite-sink, capture, layout-settle, incidental navigation, Constitution boundary, artifact-layout, replay, timeout/progress, visual diagnostics, and reporter-overclaim validators. Gates: `check_health` 0 errors; `interaction_fuzz::runner` 10/10; `interaction_fuzz` 78/78.

## [0.105.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 runner operations** (`src/tests/interaction_fuzz/runner.rs`) — added the internal runner operation surface for profile-driven chaos runs, profile-hash-enforced replay reports, chaos failure shrink-to-packet output, deterministic negative validation reports, visual trace reports derived from replay artifacts, and required artifact layout paths. Gates: `check_health` 0 errors; `interaction_fuzz::runner` 6/6; `interaction_fuzz` 74/74.

## [0.104.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos corpus entries** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/chaos.rs`) — added typed chaos corpus deltas with fixture/surface identity, stream generator identity, RNG identity, primitive stream range, coverage keys gained, oracle sets, optional hard-failure class, and shrunk primitive trace capture for shrunk chaos failures. Gates: `check_health` 0 errors; `interaction_fuzz` 68/68.

## [0.103.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos primitive shrinker** (`src/tests/interaction_fuzz/chaos.rs`) — added deterministic replay-validated primitive stream shrinking for chaos hard failures, preserving injection boundary and hard-failure class while emitting a serialized shrink report with original/shrunk counts, failing-command indices, detector/class identity, and shrunk replay artifact. Gates: `check_health` 0 errors; `interaction_fuzz` 67/67.

## [0.102.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos failure packets** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/chaos.rs`) — added chaos-origin failure packet fields for surface state, stream policy, RNG identity, primitive stream range, detector/class, timeout budget, replay determinism status, snapshots, and unsupported domains, plus replay JSON support for non-finite rejected primitive command fields. Gates: `check_health` 0 errors; `interaction_fuzz` 66/66.

## [0.101.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 chaos smoke runner** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/chaos.rs`, `src/tests/interaction_fuzz/mod.rs`) — added chaos surface/report schemas, primitive family and pointer pressure classifications, deterministic seeded primitive stream generation, pre-injection command recording, explicit unsupported detector domain reporting, and proof that chaos pass reports do not emit confidence packets. Gates: `check_health` 0 errors; `interaction_fuzz` 65/65.

## [0.100.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 interaction fuzz profile schema** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/smoke.rs`) — added profile modes, deterministic SHA-256 profile hashes, explicit timeout policies, visual diagnostic status, Constitution boundary profile/check status, and profile-backed confidence/failure packet fields. Gates: `check_health` 0 errors; `interaction_fuzz` 60/60.
- **SPEC-HAL-346 blind-spot closure contract** (`docs/specs/SPEC-HAL-346-Oracle-Driven-Interaction-Fuzzing-and-Confidence-Smoke-Testing.md`) — specified interaction fuzz profiles, chaos pressure coverage, chaos surface schema, visual diagnostic artifact rules, timeout/no-progress detector mechanics, negative validation proof gates, and Constitution boundary audit requirements.
- **SPEC-HAL-346 chaos/bombardment fuzzing contract** (`docs/specs/SPEC-HAL-346-Oracle-Driven-Interaction-Fuzzing-and-Confidence-Smoke-Testing.md`) — specified chaos surfaces, seeded primitive command streams, sampled pointer pressure, hard-failure oracles, replayable chaos failure packets, chaos shrink/corpus behavior, CI chaos smoke reporting, and the rule that chaos passes cannot emit full confidence claims.

## [0.99.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 path exposure coverage completion** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/generators.rs`, `src/tests/interaction_fuzz/smoke.rs`) — added pointer path sample-count, coalesced/routed move, crossed hover target, crossed separator, capture transition, and incidental route target coverage keys and confidence-packet summary fields. Gates: `check_health` 0 errors; `interaction_fuzz` 57/57.

## [0.98.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 CI smoke confidence packet** (`src/tests/interaction_fuzz/smoke.rs`) — added a bounded deterministic CI smoke profile over fixed ListView fixtures, fixed seeds, semantic coverage saturation, strict Constitution invariant checks, and a serialized bounded confidence packet artifact on pass. Gates: `check_health` 0 errors; `interaction_fuzz` 57/57.

## [0.97.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 Constitution boundary checks** (`src/tests/interaction_fuzz/constitution.rs`) — added internal generator, probe, oracle, shrinker, replay, and reporter responsibility contracts with mechanical failures for forbidden boundary actions and reduced-boundary full-confidence claims. Gates: `check_health` 0 errors; `interaction_fuzz` 55/55.

## [0.96.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 shrunk trace replay** (`src/core/interaction_fuzz.rs`) — added a dedicated shrunk-trace replay artifact constructor and replay API that replays through primitive injection while preserving shrink seed metadata. Gates: `check_health` 0 errors; `interaction_fuzz` 51/51.

## [0.95.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 ListView resize oracles** (`src/tests/interaction_fuzz/list_view_resize.rs`, `src/core/app.rs`) — added internal R2 direction monotonicity, R4 first/subsequent drag direction, R5 scroll-aware separator hit-test, and R12 repeated resize consistency evaluators, with routed resize probe proof tests and a test-only frame layout tracking reset hook matching the runtime frame boundary. Gates: `check_health` 0 errors; `interaction_fuzz` 50/50.

## [0.94.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 ListView resize probe snapshot** (`src/tests/interaction_fuzz/list_view_resize.rs`) — added an internal laid-out ListView probe snapshot exposing resolved/preferred column widths, separator content positions, scroll-adjusted viewport positions, scroll-domain metadata, and separator hit half-widths. Gates: `check_health` 0 errors; `interaction_fuzz` 42/42.

## [0.93.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 incidental navigation integrity oracle** (`src/core/interaction_fuzz.rs`) — added named ORACLE-346-U11 evaluation over intended/incidental targets, active intent and primitive indices, path policy, side-effect suppression, and universal oracle results. Gates: `check_health` 0 errors; `interaction_fuzz` 40/40.

## [0.92.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 layout settled oracle** (`src/core/interaction_fuzz.rs`) — added named ORACLE-346-U7 evaluation over declared settle-boundary snapshots with dirty-state checks, fixture-declared continuous invalidation allowances, normalized dirty reason reporting, and serialized failure metadata. Gates: `check_health` 0 errors; `interaction_fuzz` 37/37.

## [0.91.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 capture owner validity oracle** (`src/core/interaction_fuzz.rs`) — added named ORACLE-346-U3 evaluation over explicit capture owner snapshots with owner existence, active-domain, capture-eligibility, and release/cancel path checks plus serialized failure metadata. Gates: `check_health` 0 errors; `interaction_fuzz` 34/34.

## [0.90.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 finite sink geometry oracle** (`src/core/interaction_fuzz.rs`) — added named ORACLE-346-U1 evaluation over declared sink geometry samples with read-only pass/fail status, derived values, first-failure sink metadata, and JSON-safe non-finite value reporting. Gates: `check_health` 0 errors; `interaction_fuzz` 32/32.

## [0.89.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 KBD1 generator** (`src/tests/interaction_fuzz/generators.rs`) — added a deterministic keyboard traversal-before-activation generator with seed-varied key paths, non-spatial keyboard target metadata, generator registry proof tests, and coverage checks for keyboard navigation intent execution. Gates: `check_health` 0 errors; `interaction_fuzz` 30/30.

## [0.88.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 keyboard navigation intent** (`src/core/interaction_fuzz.rs`, `src/tests/interaction_fuzz/generators.rs`) — added deterministic keyboard navigation intent expansion with KeyDown/KeyUp/Reconcile primitive traces, keyboard navigation coverage keys, replay serialization, trace metrics, and non-pointer path policy handling. Gates: `check_health` 0 errors; `interaction_fuzz` 29/29.

## [0.87.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 generator registry** (`src/tests/interaction_fuzz/generators.rs`, `src/tests/interaction_fuzz/mod.rs`) — added deterministic internal generator definitions for RSZ1, RSZ2, and PTH1 with fixture-scoped scenario generation, non-instant user-like pointer paths, seed records, intended/incidental target metadata, and driver coverage proof tests. Gates: `check_health` 0 errors; `interaction_fuzz` 28/28.

## [0.86.0-alpha] — 2026-06-18

### Fixed
- **SPEC-HAL-346 LV2 fixture contract** (`src/tests/interaction_fuzz/list_view_resize.rs`, `src/tests/interaction_fuzz/registry.rs`) — made LV2 start with a deterministic non-zero horizontal scroll offset and exposed that initial scroll in the internal fixture registry. Gates: `check_health` 0 errors; `interaction_fuzz` 24/24.

## [0.85.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 fixture registry** (`src/tests/interaction_fuzz/registry.rs`, `src/tests/interaction_fuzz/mod.rs`) — added the deterministic internal interaction fixture registry with LV1 and LV2 ListView resize fixtures and registry proof tests. Gates: `check_health` 0 errors; `interaction_fuzz` 24/24.

## [0.84.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 trace metrics** (`src/core/interaction_fuzz.rs`) — added deterministic primitive and intent trace metrics for emitted event counts, command classes, path policy classes, pointer sample counts, frame boundaries, threshold crossings, endpoint teleportation, sampled path use, and coverage key totals. Gates: `check_health` 0 errors; `interaction_fuzz` 22/22.

## [0.83.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 finite sink guard** (`src/core/interaction_fuzz.rs`) — added checked interaction execution APIs that reject non-finite primitive sink values before raw input injection, preserve driver state on rejected command batches or intent expansions, and report the exact primitive step and field. Gates: `check_health` 0 errors; `interaction_fuzz` 20/20.

## [0.82.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 replay and packet schemas** (`src/core/interaction_fuzz.rs`) — added serializable replay artifacts, primitive replay command records, intent expansion replay records, failure packet schema, confidence packet schema, bounded positive claim text, and coverage summaries that retain path policy and incidental target classes. Gates: `check_health` 0 errors; `interaction_fuzz` 17/17.

## [0.81.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 interaction coverage keys** (`src/core/interaction_fuzz.rs`) — added deterministic path exposure coverage tracking for intent command class, pointer path policy class, frame policies, path segments, threshold crossings, and endpoint teleportation mode. Gates: `check_health` 0 errors; `interaction_fuzz` 14/14.

## [0.80.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 intent path expansion foundation** (`src/core/interaction_fuzz.rs`) — added intent commands above primitive commands, deterministic pointer path policies, frame/coalescing policy records, intent-to-primitive trace ranges, and threshold-crossing segment tagging for drag intents. Gates: `check_health` 0 errors; `interaction_fuzz` 11/11.

## [0.79.0-alpha] — 2026-06-18

### Added
- **SPEC-HAL-346 interaction fuzzing trace and routed resize proof** (`src/core/interaction_fuzz.rs`, `src/core/event_router.rs`, `src/tests/interaction_fuzz/list_view_resize.rs`, `src/widgets/list_view/list_view.rs`) — added primitive command trace recording, moved the ListView resize fixture into the internal white-box interaction fuzz harness, routed uncaptured `DragStart` through hit-testing so real fused drag activation reaches ListView resize handling, and proved routed first-column resize through `EventRouter`. Gates: `check_health` 0 errors; `interaction_fuzz` 9/9; exact routed resize smoke 1/1; exact primitive trace smoke 1/1.

### Changed
- **SPEC-HAL-346 user-like path law** (`docs/specs/SPEC-HAL-346-Oracle-Driven-Interaction-Fuzzing-and-Confidence-Smoke-Testing.md`) — reinforced intent-level fuzzing so generated pointer interaction must use sampled near-realistic paths, declare coalesced versus routed frame policy, record threshold-crossing/path exposure coverage, and forbid endpoint teleportation outside primitive replay, shrinker minimization, or injection-boundary tests.

## [0.78.0-alpha] — 2026-06-15

### Changed
- **Showcase high-frequency data polish** (`examples/showcase/main.rs`, `examples/showcase/tab_live_data.rs`, `src/widgets/list_view/list_view.rs`, `src/widgets/readout.rs`, `src/widgets/tree_view/tree_view.rs`) — renamed the Showcase tab to HF Data, replaced the dead SignalStore-centered layout with dense high-frequency ListView, ReadoutTable, StatusGrid, and TreeView readouts, added retained paint-scoped refresh paths for changing readout/tree values, hardened ListView column reordering against unresolved geometry reads, and verified the tab visually at 120 fps. Gates: `check_health` 0 errors; `cargo build --example showcase`; `cargo build --release --example showcase`.

## [0.77.0-alpha] — 2026-06-15

### Changed
- **Showcase Live Data table refresh path** (`src/widgets/list_view/list_view.rs`) — added `ListView::refresh_visible_cells()` so live `DataSource` cell invalidations can refresh realized table cells without forcing per-frame layout, and hardened unbounded measurement so structural ScrollArea/Footer children are cleared with finite degenerate constraints before the final bounded layout pass.
- **Showcase Live Data operations polish** (`examples/showcase/main.rs`, `examples/showcase/tab_live_data.rs`) — rebuilt the Live Data tab around a full-width high-frequency ListView readout table, removed the SignalStore demo and redundant metrics panel from the tab, kept updates paint-scoped, removed the nested ListView wrapper that leaked infinite width into ScrollArea layout, and aligned the visual structure with SPEC-HAL-332/SPEC-HAL-332A sharp chrome and dense data surfaces. Gates: `check_health` 0 errors; `cargo build --example showcase`; `cargo build --release --example showcase`; debug LiveData-startup runtime probe stayed running past the previous immediate ScrollArea panic.

## [0.76.21-alpha] — 2026-06-15

### Fixed
- **SPEC-HAL-159 Showcase Node Graph text ownership fix** (`examples/showcase/tab_node_graph.rs`) — replaced debug-only graph text probes with widget-owned `TextEngine::draw()` painting so Node Graph frame labels, node labels, and HUD text render in release builds without authorizing scene-level UI text. Gates: `check_health` 0 errors; `cargo build --example showcase --release`; `cargo build --example showcase`.

## [0.76.20-alpha] — 2026-06-14

### Fixed
- **SPEC-HAL-345 Showcase ListView/TreeView fast-scroll remediation** (`Cargo.toml`, `docs/specs/SPEC-HAL-345-ListView-TreeView-Performance-Diagnostic-and-Remediation.md`) — set dev profile optimization to `opt-level = 1` after probes proved debug-built text shaping dominated fast-scroll CPU in Showcase ListView and TreeView. Gates: `check_health` 0 errors; ListView fast-scroll verified at 102-121 fps; TreeView fast-scroll verified at 117-121 fps.

## [0.76.19-alpha] — 2026-06-14

### Fixed
- **SPEC-HAL-344 Console FPS remediation** (`src/widgets/console.rs`, `src/renderer/wgpu_renderer.rs`, `examples/showcase/main.rs`) — cached Console wrap segmentation, retained shaped runs under a hard cap, removed duplicate paint-side shaping for unselected Console text, and cached renderer font metrics per text-quad build pass to eliminate per-primitive CoreText font construction in Showcase Console. Gate: `check_health` 0 errors; Console-active Showcase verified at 120-121 fps.

## [0.76.18-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 plot overlay parity proof** (`tests/spatial_projection.rs`) — added an integration invariant proving Plot spatial region and tag overlays preserve transformed geometry, contribute label bounds, and remain non-hit overlays outside plot mutation authority. Gates: `check_health` 0 errors; `plot_spatial_adapter_preserves_non_hit_overlay_geometry` 1/1.

## [0.76.17-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 node graph route plans** (`src/model/node_graph_projection.rs`, `tests/node_graph_projection.rs`) — enriched node graph spatial projections with SPEC-HAL-338-backed `SpatialRoutePlan` entries for forward right-angle wires while preserving the canvas compatibility bridge, and added parity proof that realized route geometry matches existing wire endpoints. Gates: `check_health` 0 errors; `node_graph_projection` 10/10.

## [0.76.16-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 spatial action authority proof** (`tests/spatial_projection.rs`, `tests/node_graph_projection.rs`) — added integration invariants proving Plot and Timeline spatial adapters reject mutation/action intents, while Node Graph keeps drag/delete authority outside the spatial adapter and only translates rubber-band geometry into a canvas intent. Gates: `check_health` 0 errors; targeted authority tests 5/5.

## [0.76.15-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Timeline time-geometry proof** (`tests/spatial_projection.rs`) — added an integration invariant proving Timeline spatial interval x/width projection is derived from `TimeGeometryEngine::x_of`, preserving TemporalSubstrate/domain authority for time-to-x mapping. Gates: `check_health` 0 errors; `timeline_spatial_adapter_uses_time_geometry_for_x_mapping` 1/1.

## [0.76.14-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Plot authority guard** (`src/tests/spatial_public_contract.rs`) — added an internal source-scan invariant proving `src/plot/spatial.rs` does not pull plot axes, series, probes, tools, or candle rendering authority into the spatial adapter. Gates: `check_health` 0 errors; `plot_spatial_adapter_excludes_plot_domain_rendering_authority` 1/1.

## [0.76.13-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Canvas compatibility spec amendment** (`docs/specs/SPEC-HAL-284-Infinite-Canvas.md`, `docs/specs/SPEC-HAL-285-Canvas-Item-Protocol.md`) — amended the dependent Canvas specs to define the lossless read-only `CanvasProjection` to `SpatialProjection` compatibility bridge while preserving public `Canvas*` behavior and keeping mutation authority outside the canvas/spatial substrate. Gate: `check_health` 0 errors.

## [0.76.12-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Spatial adapter purity guard** (`src/tests/spatial_public_contract.rs`) — added an internal source-scan invariant proving `SpatialAdapter` keeps projection and intent translation signatures on immutable domain snapshots, session snapshots, policy inputs, intents, and contexts. Gates: `check_health` 0 errors; `spatial_adapter_trait_accepts_only_immutable_inputs` 1/1.

## [0.76.11-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Spatial projection mutation guard** (`src/tests/spatial_public_contract.rs`) — added an internal source-scan invariant proving `SpatialProjection` exposes no public mutable fields, mutable accessors, or mutable-style public accessors. Gates: `check_health` 0 errors; `spatial_projection_public_api_has_no_mutation_channel` 1/1.

## [0.76.10-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Plot path hit parity proof** (`tests/spatial_projection.rs`) — added hit-threshold assertions for Plot Bezier, styled path, and path spatial projections so migrated path-like primitives preserve route hit affordances. Gates: `check_health` 0 errors; `plot_spatial_adapter_preserves_path_marker_hit_tolerances` 1/1.

## [0.76.9-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Spatial public contract guard** (`src/tests/spatial_public_contract.rs`, `src/tests/mod.rs`) — added an internal source-scan invariant proving public `Spatial*` type and enum variant names remain free of concrete domain vocabulary. Gates: `check_health` 0 errors; `spatial_public_names_remain_domain_neutral` 1/1.

## [0.76.8-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Timeline route authority proof** (`src/tests/timeline_spatial.rs`) — added an internal invariant proving timeline arrow projections preserve SPEC-HAL-338 right-angle route geometry in both the route map and rendered route primitive without widening the public API. Gates: `check_health` 0 errors; `timeline_spatial_adapter_preserves_spec338_arrow_route_geometry` 1/1.

## [0.76.7-alpha] — 2026-06-14

### Fixed
- **SPEC-HAL-343 Canvas compatibility bridge** (`src/model/spatial/mod.rs`, `tests/spatial_projection.rs`) — keyed bridged Canvas hit regions to their owning spatial item so Canvas hit-test parity survives when hit priority differs from the item visual role. Gates: `check_health` 0 errors; `spatial_projection` 28/28.

## [0.76.6-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Spatial visual guide proof** (`tests/spatial_projection.rs`) — added visual-policy assertions proving active and structural guide roles remain distinct from grid line roles and from each other for light and dark themes. Gates: `check_health` 0 errors; `spatial_projection` 27/27.

## [0.76.5-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Adapter action-routing proof** (`tests/node_graph_projection.rs`, `tests/spatial_projection.rs`) — added node graph, timeline, and plot adapter assertions proving shared spatial key commands are not translated into domain mutation authority; delete/action routing remains owned by the domain command layers. Gates: `check_health` 0 errors; `spatial_projection` 26/26; `node_graph_projection` 8/8.

## [0.76.4-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Plot overlay placement proof** (`src/plot/mod.rs`, `src/tests/mod.rs`, `src/tests/plot_spatial_overlay.rs`) — routed the chart-info HUD through a crate-private `chart_info_overlay_rect` helper backed by `SpatialOverlayPlan` and added an internal invariant test proving the Plot paint path preserves viewport-edge padding across viewport movement and resize. Gates: `check_health` 0 errors; `plot_chart_info_overlay_preserves_viewport_edge_padding` 1/1.

## [0.76.3-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Spatial interaction policy declarations** (`examples/showcase/tab_node_graph.rs`, `src/widgets/timeline_canvas.rs`, `tests/spatial_projection.rs`, `docs/specs/SPEC-HAL-294-Node-Flow-Editor.md`, `docs/specs/SPEC-HAL-296-Plot.md`, `docs/specs/SPEC-HAL-336-TimelineHeader-and-TimelineCanvas.md`) — aligned node graph scroll zoom with the documented Ctrl/Cmd-gated shared policy, removed unconsumed Timeline fit/reset key commands from its spatial policy, and documented node graph, timeline, and plot interaction-policy boundaries required by INV-343-I2. Gates: `check_health` 0 errors; `spatial_projection` 26/26; `check --example showcase` 0 errors.

## [0.76.2-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Canvas bridge identity boundary** (`src/model/canvas.rs`, `src/model/spatial/mod.rs`, `tests/spatial_projection.rs`) — moved Canvas entity stable identity decomposition into the Canvas compatibility layer so `src/model/spatial/` no longer depends directly on node graph port direction types, while preserving distinct spatial identities for input and output ports. Gates: `check_health` 0 errors; `spatial_projection` 26/26.

## [0.76.1-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Plot overlay placement** (`src/plot/mod.rs`) — routed the Plot chart-info HUD origin through `SpatialOverlayPlan` and `resolve_spatial_overlay_rect` while preserving the existing 8px top-left viewport placement and leaving axes, series, probes, tools, and chart transforms under Plot authority. Gates: `check_health` 0 errors; `spatial_projection` 25/25.

## [0.76.0-alpha] — 2026-06-14

### Added
- **SPEC-HAL-343 Spatial interaction policy parity** (`src/model/spatial/mod.rs`, `examples/showcase/tab_node_graph.rs`, `tests/spatial_projection.rs`) — added alternate pan gesture and rubber-band cursor policy fields, routed node graph demo pan, rubber-band, fit/reset key, selection-toggle, and cursor decisions through `SpatialInteractionPolicy`, preserved NodeGraphDocument/NodeEditorSession authority, and kept Timeline's Space+primary-only override explicit. Gates: `check_health` 0 errors; `spatial_projection` 25/25; `check --example showcase` 0 errors.

## [0.75.2-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Timeline interaction policy consumption** (`src/widgets/timeline_canvas.rs`, `tests/spatial_projection.rs`) — routed TimelineCanvas primary pan activation through `SpatialInteractionPolicy` while preserving Timeline's Space+primary-only pan rule, disabled rubber-band and route creation, and kept temporal mutation authority outside the spatial layer. Gates: `check_health` 0 errors; `spatial_projection` 25/25.

## [0.75.1-alpha] — 2026-06-14

### Fixed
- **SPEC-HAL-343 Plot spatial path parity** (`src/plot/spatial.rs`, `tests/spatial_projection.rs`) — rejected partial Plot path and styled-path spatial projections when any waypoint is outside the active Plot transform domain, preventing synthetic shared spatial hit geometry while preserving Plot axis and series authority. Gates: `check_health` 0 errors; `spatial_projection` 24/24. Full `cargo test` remains blocked by pre-existing `examples/boundary_test.rs` `layout_tree` arity mismatch.

## [0.75.0-alpha] — 2026-06-14

### Added
- **SPEC-HAL-343 Node graph spatial adapter** (`src/model/node_graph_projection.rs`, `tests/node_graph_projection.rs`) — added `NodeGraphSpatialAdapter` and `NodeGraphProjectionSession` so node graph exposes the native `SpatialAdapter` shape while delegating projection construction through the lossless Canvas bridge and translating spatial presses back to Canvas intents. Gates: `check_health` 0 errors; `node_graph_projection` 8/8.

## [0.74.4-alpha] — 2026-06-14

### Fixed
- **Timeline pan probe cleanup** (`src/widgets/timeline_canvas.rs`) — removed live pan debug stderr probes from timeline pointer handling so pan/space-drag interaction no longer emits diagnostic output during normal runtime. Gates: `check_health` 0 errors.

## [0.74.3-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Timeline route-plan sharing** (`src/widgets/timeline_canvas.rs`, `src/widgets/timeline_spatial.rs`) — moved timeline arrow route-plan construction behind the timeline spatial adapter helper and made paint consume the shared `SpatialRoutePlan` realization path, preserving TemporalSubstrate geometry authority and SPEC-HAL-338 route math ownership. Gates: `check_health` 0 errors; `spatial_projection` 23/23.

## [0.74.2-alpha] — 2026-06-14

### Changed
- **SPEC-HAL-343 Node graph showcase visual policy** (`examples/showcase/tab_node_graph.rs`) — routed showcase wire states, wire previews, and port anchor outlines through `SpatialVisualPolicy` route and handle roles while preserving node/frame domain colors and port type fills. Gates: `check_health` 0 errors; `cargo check --example showcase`.

## [0.74.1-alpha] — 2026-06-14

### Fixed
- **SPEC-HAL-343 Spatial occupied-region queries** (`src/model/spatial/mod.rs`, `tests/spatial_projection.rs`) — made `SpatialProjection::in_region` treat projected `TextBounds` as occupied geometry so shared fit, guide, alignment, and route consumers do not miss label-only regions outside item body bounds. Gates: `check_health` 0 errors; `spatial_projection` 23/23.

## [0.74.0-alpha] — 2026-06-14

### Added
- **SPEC-HAL-343 Spatial label bounds access** (`src/model/spatial/mod.rs`, `tests/spatial_projection.rs`) — added `SpatialProjection::label_bounds` and included projected `TextBounds` in `world_bounds` so shared fit, guide, alignment, and route consumers account for externally supplied label regions that extend beyond item geometry. Gates: `check_health` 0 errors; `spatial_projection` 22/22.

## [0.73.2-alpha] — 2026-06-14

### Fixed
- **SPEC-HAL-343 Canvas label constraint bridge** (`src/model/spatial/mod.rs`, `tests/spatial_projection.rs`) — preserved existing Canvas node/frame geometry while adding spatial `TextBounds` primitives for text-bearing compatibility items so shared spatial guide, fit, route, and label-avoidance logic can treat domain-provided label regions as occupied without remeasuring labels or changing Canvas hit behavior. Gates: `check_health` 0 errors; `spatial_projection` 21/21.

## [0.73.1-alpha] — 2026-06-14

### Fixed
- **Renderer text atlas bind-group invalidation** (`src/renderer/wgpu_renderer.rs`) — forced per-surface text bind groups to be rebuilt after pipeline generation changes so glyph atlas growth cannot leave new glyph UVs sampling stale atlas texture views across tabs or surfaces. Gates: `check_health` 0 errors; `text_bind_group_rebuild` 2/2.

## [0.73.0-alpha] — 2026-06-14

### Added
- **SPEC-HAL-343 Timeline spatial projection revision derivation** (`src/widgets/timeline_spatial.rs`, `src/widgets/timeline_canvas.rs`, `tests/spatial_projection.rs`) — added deterministic Timeline spatial projection revisions derived from Timeline-owned geometry, temporal frame state, lane resolution, and projection bounds so `TimelineCanvas::spatial_snapshot` no longer emits a placeholder revision while preserving read-only spatial adapter authority. Gates: `check_health` 0 errors; `spatial_projection` 20/20.

## [0.72.0-alpha] — 2026-06-14

### Added
- **SPEC-HAL-343 Timeline spatial projection adapter** (`src/widgets/timeline_spatial.rs`, `src/widgets/timeline_canvas.rs`, `tests/spatial_projection.rs`) — added a pure `SpatialAdapter` for Timeline snapshots that projects rows, lanes, bands, gridlines, intervals, arrows, markers, and the now marker into read-only `SpatialProjection` geometry after TemporalSubstrate-owned time/lane resolution, preserving Timeline paint/event paths and temporal mutation authority. Gates: `check_health` 0 errors; `spatial_projection` 18/18.

## [0.71.0-alpha] — 2026-06-14

### Added
- **SPEC-HAL-343 Plot spatial marker adapter** (`src/plot/spatial.rs`, `src/plot/mod.rs`, `tests/spatial_projection.rs`) — added a pure `SpatialAdapter` for Plot marker snapshots that projects price-level markers, regions, tags, circles, connector lines, Beziers, styled paths, and paths into read-only `SpatialProjection` geometry after Plot-owned coordinate mapping, preserving axes, series, probes, tools, and mutation authority in Plot. Gates: `check_health` 0 errors; `spatial_projection` 15/15.

## [0.70.0-alpha] — 2026-06-14

### Added
- **SPEC-HAL-343 spatial subsystem reconciliation** (`src/model/spatial/mod.rs`, `src/model/node_graph_projection.rs`, `src/widgets/timeline_canvas.rs`, `examples/showcase/tab_node_graph.rs`, `tests/spatial_projection.rs`) — added the neutral spatial projection substrate, lossless Canvas compatibility bridge, shared visual and interaction policies, node graph spatial projection bridge, and route-plan realization that consumes SPEC-HAL-338 route geometry without transferring route authority into the spatial layer. Gates: `check_health` 0 errors; `spatial_projection` 12/12; `interval_rect_single_lane` 1/1.

## [0.69.1-alpha] — 2026-06-14

### Fixed
- **Checkbox press flicker** (`src/widgets/checkbox.rs`, `src/tests/checkbox.rs`) — pinned hover chrome through the armed press, snaps the visual checkmark preview to the pending target on pointer down, and still commits the semantic checked value only on pointer up; added regression coverage for press preview, hover stability, commit, and cancel restore. Gates: `check_health` 0 errors; `test --lib checkbox` 5/5.

## [0.69.0-alpha] — 2026-06-13

### Added
- **Node graph alignment snapping** (`src/model/node_editor.rs`, `examples/showcase/tab_node_graph.rs`, `tests/node_editor_session.rs`) — added optional edge/center alignment snapping with guide projections for node/frame drag movement, compacted the node graph showcase HUD so status text cannot spill under the overlay, and covered snap resolution with focused model tests. Gates: `check_health` 0 errors; `alignment_snap` 2/2; `check --example showcase` 0 errors.

## [0.68.1-alpha] — 2026-06-13

### Fixed
- **SPEC-HAL-295 custom render pass closeout** (`docs/specs/SPEC-HAL-295-Custom-Render-Pass.md`, `src/renderer/custom_render_pass.rs`, `src/tests/custom_render_pass.rs`) — finalized the binding SPEC against the implemented lifecycle and closed stale pending-registration paths for immediate register, direct resize, direct destroy, deferred destroy, and repeated resize before frame drain; strengthened custom WGSL acceptance with readback validation. Gates: `check_health` 0 errors; `custom_render_pass` 14/14; `plot` 4/4.

## [0.68.0-alpha] — 2026-06-13

### Added
- **SPEC-HAL-342 temporal emphasis substrate** (`src/engines/temporal/emphasis.rs`, `src/engines/temporal/mod.rs`, `src/engines/temporal/substrate.rs`) — added `EmphasisRelation` (Identity/Containment), `EmphasisRole`, and `EmphasisState`: a transient hover/focus state, the sibling of selection (INV-335-S1). `resolve` builds the one-hop linked set (direct parent + direct children via `ContainmentSource`, INV-342-E3/E7), gated on focus/relation change (INV-342-E4), independent of selection (INV-342-E5). `TemporalSubstrate` holds an `EmphasisState` beside `selection` and exposes `emphasis()` / `set_emphasis_focus` / `resolve_emphasis`. Additive to SPEC-HAL-335 (`begin_frame`/snapshot untouched); containment arrives via parameter (FORBID-342-7). Gates: `test --lib emphasis` 4/4.

## [0.67.1-alpha] — 2026-06-13

### Fixed
- **SPEC-HAL-295 deferred unregistration ownership** (`src/renderer/custom_render_pass.rs`, `src/renderer/wgpu_renderer.rs`, `src/tests/custom_render_pass.rs`) — retained widget-owned front/back textures inside pending unregistrations until the renderer drains the unregister, preserving unregister-before-drop ordering for deferred resize and destroy paths. Gates: `check_health` 0 errors; `custom_render_pass` 9/9; `plot` 4/4.

## [0.67.0-alpha] — 2026-06-13

### Changed
- **SPEC-HAL-295 custom render pass integration** (`src/renderer/custom_render_pass.rs`, `src/renderer/wgpu_renderer.rs`, `src/plot/renderer.rs`, `src/plot/render_pool.rs`, `src/plot/mod.rs`, `src/tests/custom_render_pass.rs`) — added deferred image unregistration, deferred custom pass destruction, resize unregister-before-register ordering, COPY_SRC readback support, Plot renderer migration onto `CustomRenderPass`, Plot unmount cleanup, custom WGSL pipeline acceptance coverage, pixel readback validation, and deferred-destroy compile guards. Gates: `check_health` 0 errors; `custom_render_pass` 7/7; `plot` 4/4.

## [0.66.0-alpha] — 2026-06-13

### Added
- **SPEC-HAL-295 custom render pass lifecycle** (`src/renderer/custom_render_pass.rs`, `src/renderer/mod.rs`, `src/tests/custom_render_pass.rs`) — added a generic widget-owned custom render pass lifecycle with physical-pixel sizing, front/back offscreen texture ownership, back-texture command encoding and queue submission, front-texture view registration, resize re-registration, explicit unregister-before-drop destruction, and focused acceptance coverage for registration, double-buffer swap, resize, unregistration, clipping, z-order, and surrounding scene composition. Gates: `check_health` 0 errors; `custom_render_pass` 5/5.

## [0.65.0-alpha] — 2026-06-13

### Added
- **SPEC-HAL-340 temporal containment substrate** (`src/engines/temporal/containment.rs`, `src/engines/temporal/mod.rs`) — added `ContainmentPolicy` (Enforced/Emergent) and the object-safe `ContainmentSource` trait (`children_of`/`parent_of`/`has_children`/`interval`/`drain_invalidation_events`), a single-level parent/child relation over identity-keyed intervals (INV-340-C1/C2/C4/C6). Independent of `IntervalSource` — a flat source pays nothing (FORBID-340-1); re-exported from `engines::temporal`. Purely additive; no implementor or consumer; no existing SPEC-HAL-335 type touched. Gates: `check --lib` 0 errors.

## [0.64.0-alpha] — 2026-06-13

### Added
- **Node flow public view surface** (`src/model/node_editor.rs`, `tests/node_editor_session.rs`) — added the public `NodeFlowView` surface name over the existing coordinator authority model and enforced the read-only gate on shortcut/action intent resolution so graph, search, undo, and redo intents cannot bypass read-only mode. Gates: `check_health` 0 errors; `node_editor_session` 20/20.

## [0.63.0-alpha] — 2026-06-13

### Fixed
- **Node graph duplicate move undo** (`src/model/node_graph.rs`, `tests/node_graph_model.rs`) — normalized duplicate node targets in `MoveNodes` and `MoveFrameWithNodes` transactions so each node produces one delta entry with the original position and final target, preserving exact undo/redo. Gates: `check_health` 0 errors; `node_graph_model` 12/12.

## [0.62.0-alpha] — 2026-06-13

### Fixed
- **Node graph serialized frame validation** (`src/model/node_graph.rs`, `tests/node_graph_model.rs`) — rejected imported serialized graphs with non-positive frame sizes so deserialization cannot bypass the same frame invariant enforced by graph transactions; added validation coverage for unknown node types. Gates: `check_health` 0 errors; `node_graph_model` 10/10.

## [0.61.0-alpha] — 2026-06-13

### Fixed
- **Node graph compound validator authority** (`src/model/node_graph.rs`, `tests/node_graph_model.rs`) — changed compound transaction execution so a supplied `GraphValidator` validates the whole compound exactly once while built-in child transaction validation still executes atomically on the cloned graph. Gates: `check_health` 0 errors; `node_graph_model` 9/9.

## [0.60.0-alpha] — 2026-06-13

### Added
- **Node graph inline title edit authority** (`src/model/node_graph.rs`, `src/model/node_editor.rs`, `tests/node_graph_model.rs`, `tests/node_editor_session.rs`) — added frame title transactions and deltas with undo/redo replay, session-owned inline edit commit/cancel flow, node/frame title transaction planning, and cancel/read-only routing for active inline title edits. Gates: `check_health` 0 errors; `node_graph_model` 8/8; `node_editor_session` 18/18.

## [0.59.0-alpha] — 2026-06-13

### Added
- **Node graph frame creation and resize sessions** (`src/model/node_editor.rs`, `src/model/node_graph_projection.rs`, `tests/node_editor_session.rs`) — added projected-node-bounds frame creation transactions, session-owned frame resize preview state, frame size projection overrides, resize commit transaction planning, and cancel/read-only routing for active frame resize without document mutation during preview. Gates: `check_health` 0 errors; `node_editor_session` 17/17; `node_graph_projection` 5/5.

## [0.58.0-alpha] — 2026-06-13

### Added
- **Node graph frame collapse and drag planning** (`src/model/node_graph_projection.rs`, `src/model/node_editor.rs`, `tests/node_graph_projection.rs`, `tests/node_editor_session.rs`) — added collapsed-frame projection filtering for contained nodes, ports, and wires; added session-owned frame drag captures, frame preview offsets, and transaction planning that emits `MoveFrameWithNodes` for captured containment without mutating document authority. Gates: `check_health` 0 errors; `node_graph_projection` 5/5; `node_editor_session` 15/15.

## [0.57.0-alpha] — 2026-06-13

### Added
- **Node flow clipboard paste transaction** (`src/model/node_editor.rs`, `tests/node_editor_session.rs`) — added document-derived paste transaction generation from `NodeEditorSession.clipboard`, anchoring copied node/frame entities at the requested world position and reusing `DuplicateSelection` without mutating document authority during intent resolution. Gates: `check_health` 0 errors; `node_editor_session` 14/14.

## [0.56.0-alpha] — 2026-06-13

### Added
- **Node flow action binding model** (`src/runtime/actions.rs`, `src/model/node_editor.rs`, `tests/node_editor_session.rs`) — added stable node-flow `ActionId` values, declarative default binding records for SPEC-HAL-294 keyboard shortcuts, and coordinator action-to-intent resolution for selection, graph transactions, search, cancel, view, zoom, undo/redo, and keyboard nudging without widget/event handling. Gates: `check_health` 0 errors; `node_editor_session` 13/13.

## [0.55.0-alpha] — 2026-06-13

### Added
- **Node graph session drag projection** (`src/model/node_graph_projection.rs`, `src/model/node_editor.rs`, `tests/node_editor_session.rs`) — added session-owned node projection offsets and session revision salting so move drag previews alter disposable node, port, wire, and frame projection geometry without mutating `NodeGraphDocument` authority. Gates: `check_health` 0 errors; `node_editor_session` 10/10; `node_graph_projection` 4/4.

## [0.54.0-alpha] — 2026-06-13

### Added
- **Node flow coordinator model** (`src/model/node_editor.rs`, `tests/node_editor_session.rs`) — added a pure `NodeFlowCoordinator` surface that owns only `InfiniteCanvas`, `NodeFlowConfig`, and outbound intents while building disposable node graph/canvas projections from external document, session, catalog, registry, and execution overlay inputs. Added read-only intent gating for graph/search/connect edits, view helper intents for zoom-to-fit, zoom-to-node, pan, and reset, plus focused coverage for projection build, read-only filtering, and viewport intent helpers. Gates: `check_health` 0 errors; `node_editor_session` 9/9.

## [0.53.0-alpha] — 2026-06-13

### Added
- **Node editor session and intent model** (`src/model/node_editor.rs`, `src/model/mod.rs`, `tests/node_editor_session.rs`) — added external editor/session authority data for viewport, selection, drag, wire drag, rubber band, node search, clipboard, execution overlay, node-flow config, and outbound editor intents; added selection-to-graph transaction helpers, document-safe wire drag commit/reroute/remove intent helpers, node search catalog projection/filtering, and projection-state derivation from session/execution overlays. Gates: `check_health` 0 errors; `node_editor_session` 6/6.

## [0.52.0-alpha] — 2026-06-13

### Added
- **Node graph projection model** (`src/model/node_graph_projection.rs`, `src/model/mod.rs`, `tests/node_graph_projection.rs`) — added disposable node, port, wire, and frame projections derived from `NodeGraphDocument`, `NodeTypeCatalog`, and `PortTypeRegistry`; generated `CanvasProjection` items for node/port/wire/frame hit testing and draw order; implemented deterministic port layout, wire paths, frame containment derivation, overlay visual-state inputs, and projection errors for missing catalog/port data. Gates: `check_health` 0 errors; `node_graph_projection` 4/4; `node_graph_model` 7/7.

## [0.51.0-alpha] — 2026-06-13

### Added
- **SPEC-HAL-284/285 canvas coordinate and projection protocol** (`src/model/canvas.rs`, `src/model/mod.rs`, `tests/canvas_projection.rs`) — added the pure canvas model layer: world-space points and rects, explicit viewport transform, zoom clamping and zoom-to-fit helpers, advisory grid config, read-only projection records, semantic canvas entity IDs, hit regions, draw-order and hit-test ordering, world-bounds/region queries, path hit testing, and intent drainage. No widget, document, selection, or session authority added. Gates: `check_health` 0 errors; `canvas_projection` 9/9; `node_graph_model` 7/7.

## [0.50.0-alpha] — 2026-06-13

### Added
- **SPEC-HAL-293 node graph document authority** (`src/model/node_graph.rs`, `src/model/mod.rs`, `tests/node_graph_model.rs`) — added the pure serializable `NodeGraphDocument` / `NodeGraph` model, graph IDs, node/port/connection/frame data, catalog and port registry inputs, transaction-only mutation, deltas, bounded undo/redo, serde round-trip surface, catalog-aware validation, duplicate/cardinality/type/direction/dangling checks, and DAG topological sorting. Added focused integration coverage for transaction atomicity, cascading node deletion, undo/redo, compound duplication, serialization runtime-state exclusion, validation failures, and cycle reporting. Gates: `check_health` 0 errors; `node_graph_model` 7/7.

## [0.49.0-alpha] — 2026-06-12

### Added
- **SPEC-HAL-336 §7 control law — `VerticalPanBy`, wheel rework, hand tool** (`src/engines/temporal/substrate.rs`, `src/widgets/timeline_canvas.rs`, `examples/timeline_bound.rs`, `docs/specs/SPEC-HAL-335-TemporalSubstrate-Architecture.md`) — new `TemporalIntent::VerticalPanBy { dy }`: plain-wheel / hand-tool vertical pan crosses as a deferred intent, applied by the composition through the row provider's single vertical-scroll path (`ScrollArea::scroll_to_y`; INV-337-S1 / FORBID-337-4 — the canvas never scrolls rows, FORBID-337-2). Wheel arm reworked per §7: Ctrl/Cmd+wheel zooms about the cursor (up = in), Shift+wheel pans time from a vertical-wheel device, plain wheel is a two-axis pan (dx → engine, dy → deferred `VerticalPanBy` in bound mode; free mode has no vertical domain and bubbles). Hand tool (Display+ tier, grab precedes the policy gate): composition-fed Space-held state via new `TimelineCanvas::set_pan_modifier` (same boundary class as `set_row_geometry`; FORBID-337-1 clean — no callbacks, no handles), Space+drag pans both axes with content following the pointer (negated deltas); Up or Space release ends the pan. Space unbound from `ToggleFocused` (JC ruling 2026-06-12); toggling remains via Cmd/Ctrl+click. `VisibleRangeChanged` now fires only when the horizontal range actually changed. `timeline_bound` demo: Space tracking in `handle_input` (KeyDown/KeyUp + window `FocusChanged(false)` clear), fed per frame through `set_pan_modifier`; drained `VerticalPanBy` accumulates and applies via `scroll_to_y`; excluded from the `[intent]` log (wheel-frequency spam); header checklist and banner updated. SPEC-HAL-335 §10 listing gains the variant. Gates: `check --lib --tests` 0 errors; `intents_fifo_and_drain` 1/1; examples fail only on the pre-existing E0061 trio.

## [0.48.0-alpha] — 2026-06-12

### Added
- **`TemporalIntent::DeselectAll` — empty-canvas press emits the promised intent** (`src/engines/temporal/substrate.rs`, `src/widgets/timeline_canvas.rs`, `src/tests/temporal_substrate.rs`, `docs/specs/SPEC-HAL-335/336`) — SPEC-HAL-336 §7 "Click empty" promised a deselect *intent*; the `None` arm performed only the local engine clear. New `DeselectAll` variant; the empty-press arm now mirrors the `Select` arm's reduce-then-push shape per INV-335-I1 (local clear retained — free mode keeps working with no drain consumer). Folded SPEC-HAL-335 §10 audit corrections: enumeration gains the variant; `SelectModifiers` type drift corrected to `Modifiers`. 336 §7 row names the intent. `intents_fifo_and_drain` extended to four intents. Gates: `check --lib --tests` 0 errors; test 1/1; examples fail only on the pre-existing E0061 trio.

## [0.47.0-alpha] — 2026-06-12

### Changed
- **SPEC-PM-017 rung 1b — TreeView viewport-relative geometry, single f64 subtraction** (`src/widgets/scroll_area.rs`, `src/widgets/tabular_view/tree_paint.rs`, `src/widgets/tree_view/body.rs`, `examples/treeview.rs`, `examples/showcase/{tab_tree_view,tab_layout,tab_workbench}.rs`) — the widget half of the PM-017 remedy (INV-PM-017-3): one f64 subtraction converts content space to viewport space before any f32 narrowing; no content-magnitude value reaches paint, hit, or child geometry. ScrollArea gains `content_owns_vertical_translation` (builder `with_content_owned_vertical_translation()`): the scene Scroll node translates X only — `scroll_offset()` reports `(visual_x, 0.0)`; vertical scroll state, domain, scrollbars, smoothing, reveal, and sync_scroll dispatch unchanged. TreeViewBody hit-tests in f64, computes content height as a single f64 product, sizes the tree-column clip to the viewport, positions disclosure toggles at viewport-relative Y (restoring their hit rects at depth), paints rows/guides via the R1b form of INV-TV-PAINT-CONTENT-COORDS (viewport-relative `row_y_abs`), and retires the separator counter-scroll. `build_tree_guide_rects` gains `content_phase_parity` so the 1-on/1-off dot pattern stays content-anchored. All four TreeView ScrollArea mounts flagged (pipeline ListView unflagged — rung 2). Pixel-identical at shallow offsets, correct at deep offsets; the f32 sync channel (2px scroll quantization past 2^24) is the documented rung-3 limitation. Gates: `check --lib --tests` 0 errors; tree 86/86, hts 17/17, hierarchical 41/41, temporal 45/45, tabular 182/182; examples fail only on the pre-existing E0061 trio.

## [0.46.2-alpha] — 2026-06-12

### Fixed
- **ListView finality propagation — stop fabricating the ScrollArea's final pass** (`src/widgets/list_view/list_view.rs`) — Flex measures the ListView with a loose height bound (SPEC-PM-005 §7), and `ListView::layout` forwarded that as a tight height to its internal ScrollArea, fabricating the Stage-2.8 finality signal: the ScrollArea committed scroll state twice per frame with two different container heights, the measurement-bound commit landing last (PM-009 class — live probes showed max_y clamped at 103.0 instead of 137.2 and a 103↔111.6 end-stop oscillation: the Workbench pipeline table's unreachable last row and mushy stop). `layout()` now derives `height_is_tight` from its own incoming constraints and forwards `min_height = 0.0` when loose — the measurement pass skips the commit, the height-tight allocation pass commits exactly as before. One commit per frame, the correct one; no layout sizes change. TreeView/PE finality audit follows separately. Gate: check 0 errors; `list` 11/11; `scroll` 137 passed (2 pre-existing pm004 name-matches); showcase compiles clean; interactively confirmed (one PROBE-A line per frame, max_y=137.2 held, mush gone) before probe removal.

## [0.46.1-alpha] — 2026-06-12

### Fixed
- **Occlusion-aware scroll domain — content clears the opposing overlay scrollbar** (`src/widgets/scroll_area.rs`) — `max_scroll = content − viewport` used the full container as viewport while the overlay scrollbar occludes a `thickness` strip of it, leaving the last `thickness` px of content permanently under the bar at max scroll (Workbench pipeline table: last row half-hidden and unreachable, thumb stopping at the corner, wheel rubber-banding). `layout()` now computes a `domain_viewport` = container minus the occluded strip per axis (visibility from the previous frame's content extent — frame-driven convergence, SPEC-PM-005 discipline) and feeds it to `state.set_viewport_extent`, `smooth.set_viewport_size`, and the INV-SCROLL-DOMAIN-CLOSED assertion. Drag mapping, thumb math, and reveal inherit the corrected domain through `state.viewport_extent` with zero edits; child constraints and virtualization untouched — zero reflow. Both axes fixed symmetrically. Gate: check 0 errors; `scroll` filter 137 passed (2 fails = pre-existing pm004 batcher tests, name-matched only); `list` 11/11; showcase example compiles clean.

## [0.46.0-alpha] — 2026-06-12

### Changed
- **SPEC-PM-017 rung 1a — f64 scroll core in HTS + virtualization, single-source `first_sorted`** (`src/engines/hierarchical/{mod,virtualization}.rs`, `src/engines/virtualization/mod.rs`, `src/engines/cell_mechanics/mod.rs`, `src/engines/temporal/hts_rows.rs`, `src/widgets/tree_view/{body,tree_view}.rs`, `src/widgets/list_view/{body,core,list_view}.rs`, `src/widgets/property_editor/{body,property_editor}.rs`, `src/widgets/tabular_view/mod.rs`, `src/tests/hts_f64_scroll_core.rs` new, `src/tests/{mod,hierarchical_substrate}.rs`) — the engine half of the PM-017 remedy (ruling B: f64 document space, f32 view space). `HierarchicalTabularSubstrate` scroll state and its `scroll_to`/`scroll_offset` surfaces are f64 (INV-PM-017-1, amends SPEC-HAL-259 §5.5); fixed-height products and variable-height prefix sums compute/accumulate in f64 (INV-PM-017-2, FORBID-PM-017-2); the `VirtualizationStrategy` trait speaks f64 (legacy `VirtualizationEngine` shimmed at the boundary until rung 5). `VisibleWindow` gains f64 `first_y` and engine-published `first_sorted`; `visible_rows_with_total` returns the 3-tuple — TreeViewBody consumes `first_sorted` instead of re-deriving it from an f32 scroll division (the task's only behavioral delta; discharges the SPEC-PM-009 dual-source hazard). Every remaining f32 cast is a marked shim naming its removal rung (1b/2/3/4/5). Two precision tests pin exactness past 2^24 (scroll 16,777,300 → first 699,054, phase −4.0 exact). Landed as R1a + two corrective sets (missed ripple: ListView's third `visible_rows_with_total` caller, three tabular_view dyn call sites, five `cfg(test)` sites — `test_compute_visible_range` widened to mirror the trait by ruling). New law from the misses: gate 0 for signature changes is `cargo check --lib --tests`. Gates: lib+tests 0 errors; hts 17/17, hierarchical 41/41, tree 86/86, temporal 45/45, list 11/11, tabular 182/182; examples fail only on the pre-existing E0061 trio.

## [0.45.5-alpha] — 2026-06-12

### Removed
- **Diagnosis-era tracing in `try_restore_focus`** (`src/core/event_router.rs`) — six `tracing::warn!` calls removed under the logging law: the function logged at WARN on every path, including the routine cold-start no-memory case (multiple times per session), all four normal-lifecycle bail conditions, and the success path (SUCCESS at warn level — categorical diagnosis residue). Control flow, `take_last_focus` consumption order, do-not-re-store semantics, and `request_focus_for_widget` byte-identical. Gate: check 0 errors; `event_router` filter 7/7; `try_restore_focus:` zero hits in src/.

## [0.45.4-alpha] — 2026-06-12

### Removed
- **ALPHA-PROBE diagnostic residue in `Batcher::add_rect`** (`src/renderer/batcher.rs`) — leftover alpha-blending diagnosis instrumentation removed under the logging law: a `debug_assertions`-gated static-counter block logging the first 10 rects of every debug run at INFO in the renderer hot path. Observation-only; primitive emission untouched. Gate: check 0 errors; `batcher` filter 69 passed / 6 failed — the 6 are the pre-existing stale pm004/stage17 SDF-pipeline tests documented since 2026-06-08, unrelated to this removal.

## [0.45.3-alpha] — 2026-06-12

### Fixed
- **Selection toggle reachable on macOS — `ctrl || meta` in TimelineCanvas** (`src/widgets/timeline_canvas.rs`, `examples/timeline_strip.rs`) — the canvas's toggle branch checked `pe.modifiers.ctrl` only, but macOS converts Ctrl+primary-click into a secondary click at the OS level, so toggle attempts arrived as right-clicks and fired `ContextMenu` intents instead. Now `ctrl || meta` per the house convention (TreeView body, PE body, List policy) — Cmd+click toggles on macOS; Ctrl+click remains the macOS context-menu gesture, unchanged. The `timeline_strip` banner names the chord. Gate: check 0 errors; `timeline_strip` example compiles clean.

## [0.45.2-alpha] — 2026-06-12

### Fixed
- **TimelineCanvas claims wheel routing — `wants_scroll` override** (`src/widgets/timeline_canvas.rs`) — the router's scroll pipeline dispatches wheel only to the nearest hit-chain widget whose `wants_scroll()` returns true and drops the event when none exists; the canvas had no override, leaving its fully-implemented SPEC-HAL-336 §7 wheel arm (Ctrl = zoom about the cursor per the INV-335-G3 anchor law, Shift / native horizontal = pan) unreachable — zero `VisibleRangeChanged` intents across an interactive session. One trait-method override (`ScrollArea::wants_scroll` is the house precedent); plain vertical wheel still returns false from the arm and bubbles, preserving §7 row-provider forwarding for bound compositions. Gate: check 0 errors; `temporal` filter 45/45; examples fail only on the pre-existing E0061 trio.

## [0.45.1-alpha] — 2026-06-12

### Removed
- **TreeViewBody dead code** (`src/widgets/tree_view/body.rs`) — rustc-attested dead code removed: the parked SPEC-HAL-313 §3.4 `host: ExpandableRowHost` field (its variable-row-height precondition never landed; `ListViewBody` remains the live reference) with its constructor init and both dead imports (`row_expansion::{ExpandedHeightPolicy, ExpansionPolicy}`, `ExpandableRowHost`); the superseded `visible_ids` local (disclosure pruning uses the stricter `eligible_ids`); the vestigial paint-side `scroll_x` read (the DoubleClick arm's own read is the live one). Behavior-identical; the four named warnings gone (101 → 97). Gate: check 0 errors; `tree` filter 86/86.

## [0.45.0-alpha] — 2026-06-12

### Added
- **SPEC-HAL-337 §5 — the FORBID-337-2 adapter: `engines::temporal::hts_rows`** (`src/engines/temporal/hts_rows.rs` new, `src/engines/temporal/mod.rs`, `src/widgets/timeline_canvas.rs`, `src/tests/temporal_hts_rows.rs` new, `src/tests/mod.rs`) — the sanctioned HTS → timeline boundary: `hts_rows(&SharedHts) -> Vec<RowGeometry>` derives per-frame viewport-space row geometry exclusively from the substrate's published read surface (`visible_window` + `row_height` + `scroll_offset`, INV-337-R1); timeline widgets never hold `SharedHts` (FORBID-337-2) and no consumer re-implements the virtualization arithmetic. `RowGeometry` relocated from `widgets::timeline_canvas` to the engine (engines do not depend on widgets; the widget re-exports it so the old path stays valid — both prior references were canvas-internal). INV-HTSROWS-1 names the fixed-row-height contract (`visible_window` publishes no per-row heights; variable-height bound mode is a SPEC question first). Empty topology → empty `Vec` → free mode per the §7 seam. Three tests (unscrolled viewport mapping, scrolled partial first row in `(-height, 0]`, empty-topology free mode). Wiring into compositions is out of scope — the bound Gantt waits on the §9 selection ruling. Gate: check 0 errors; hts_rows 3/3; `temporal` filter 45/45; `check --examples` fails only on pre-existing E0061 arity drift (boundary_test / min_hit_test / border_focus), proven identical on clean HEAD.

## [0.44.0-alpha] — 2026-06-12

### Changed
- **SPEC-HAL-337 stage-3 companion — PropertyEditor `Tree(ToggleExpand)` deferral** (`src/widgets/property_editor/{core,property_editor}.rs`, `src/tests/property_editor_intent_deferral.rs` new, `src/tests/mod.rs`) — the last synchronous structural mutation leaves event routing (§4.1 / INV-337-M1): `apply_tree_intent` becomes `enqueue_tree_intent`, which keeps the bound-check and resolves the frame-local `row_index` to a node id at dispatch time (identity law — the queue carries node ids, never indices), pushing onto the new `pending_topology_toggles` field; a new `PropertyEditor::update` per-frame hook drains and applies via `toggle_by_id` before layout, with no choreography (the replaced arm had none). Permission gate keeps its event-time position inside the single funnel (INV-PE-INTENT-FUNNEL); new pub(crate) widget-level `dispatch_intent` is a pass-through into it, not a second entry path. Behavioral delta: NONE — both emitters already invalidate the frame whose update now applies the toggle. Doc-rot fixed alongside: §8.1 banner drops the stale "22 fields" count; `node_id_for_index`'s doc renames both stale `apply_tree_intent` mentions and drops the rotted `core.rs:468` line pin. Three INV-337-M1 tests (deferral until update, toggle round-trip, empty-queue no-op). Gate: check 0 errors; deferral 3/3; `property_editor` filter 7/7; `apply_tree_intent` zero hits in src/.

## [0.43.0-alpha] — 2026-06-12

### Added
- **SPEC-HAL-337 stage 3 — TreeView topology-intent funnel** (`src/widgets/tree_view/{body,tree_view,mod}.rs`, `src/tests/tree_view_intent_funnel.rs` new, `src/tests/mod.rs`) — structural topology mutation leaves event routing (§4.1 / INV-337-M1): new `TreeTopologyIntent` enum (`Toggle`/`Expand`/`Collapse`/`ExpandSubtree`/`CollapseSubtree`) and `topology_intents` queue on `TreeViewCore`; all five mutation sites (body DoubleClick arm, disclosure-toggle closure, Right-expand, Cmd/Ctrl+Right, Left-collapse, Cmd/Ctrl+Left keyboard arms) now enqueue + invalidate, and a new `TreeView::update` per-frame hook drains the queue before layout with `ensure_focus_visible`/`request_scroll_to_focus` choreography gated once per batch on `revision()` change. `enqueue_topology_intent` is the pub(crate) funnel entry; selection branches stay byte-identical (INV-337-M2); HTS remains sole topology authority (INV-TV-1). Documented deltas: Cmd/Ctrl+Right/Left consume whenever a focused row exists, and application moves to the same frame's update before layout. Three INV-337-M1 tests (deferral until update, toggle round-trip, empty-queue no-op). Gate: check 0 errors; funnel 3/3; `tree` filter 86/86.

## [0.42.0-alpha] — 2026-06-11

### Added
- **SPEC-HAL-337 stage 5 — HTS `visible_window()` read surface** (`src/engines/hierarchical/mod.rs`, `src/tests/hierarchical_substrate.rs`) — new `VisibleWindow { rows, first_y }` struct; the windowing orchestration moved verbatim from `visible_rows()` into `visible_window()`, which publishes the virtualization's y-offset as `first_y` instead of discarding it (INV-337-R1). `visible_rows()` is now a thin `.rows` projection — one orchestration, no second copy (`visible_rows_with_total` deliberately untouched, separate disposition). Four §14 tests (scroll origin, fixed-height arithmetic, parity with `visible_rows`, empty substrate). Consumer adoption is stage 6. Gate: check clean; hierarchical_substrate 41/41; full lib 1457 pass / 7 pre-existing fail.

## [0.41.0-alpha] — 2026-06-11

### Added
- **SPEC-HAL-337 stage 4 — PropertyEditor ownership surface** (`src/widgets/property_editor/property_editor.rs`, `src/tests/property_editor_shared_hts.rs` new, `src/tests/mod.rs`) — exact mirror of the stage-2 TreeView pattern: constructor splits into `new()` (mints + configures `DEFAULT_ROW_HEIGHT`, `HtsOwnership::Internal`) and `with_hts(SharedHts)` (injected substrate, `External`, configuration untouched), both through a shared private `with_handle()`. New `hts_handle()` (clones through `core.cms` — PE holds no substrate field, INV-337-O2) and `ownership()` accessors. Four INV-337-O1 tests including the injected-substrate-not-reconfigured pin. Gate: check clean; targeted 4/4; full lib 1453 pass / 7 pre-existing fail.

## [0.40.0-alpha] — 2026-06-11

### Removed
- **PropertyEditor v1 deleted — PE v2 is the sole implementation (JC ruling, 2026-06-11)** (`src/widgets/property_editor/{mod,property_editor,types}.rs` v1 content, `examples/property_editor.rs` v1 example, `src/widgets/mod.rs`) — the SPEC-HAL-260 v1 widget and its five public exports (`PropertyEditor`, `PropertyEditorHandle`, `PropertyDataSource`, `PropertyNode`, `PropertyValue`) are removed; sole external consumer was its own example. Closes the last INV-337-O2 violation (v1's by-value `HierarchicalTabularSubstrate` field). SPEC-HAL-260 supersession marking follows as a separate docs task.

### Changed
- **PE v2 promoted to the canonical name** (`src/widgets/property_editor_v2/` → `src/widgets/property_editor/`, `examples/property_editor_v2.rs` → `examples/property_editor.rs`, `examples/showcase/{tab_layout,tab_property_editor,tab_workbench}.rs`) — module renamed `widgets::property_editor_v2` → `widgets::property_editor`; example and showcase consumers updated. Content unchanged beyond path references.

## [0.39.0-alpha] — 2026-06-11

### Added
- **SPEC-HAL-337 stage 2 — TreeView ownership surface** (`src/engines/hierarchical/mod.rs`, `src/widgets/tree_view/tree_view.rs`, `src/tests/tree_view_shared_hts.rs` new, `src/tests/mod.rs`) — the §2 policy type `HtsOwnership` (`Internal`/`External`) lands beside the `SharedHts` alias with its first consumer. TreeView's constructor splits into `new()` (mints its own substrate, `Internal`) and `with_hts(SharedHts)` (injected substrate, `External`), both delegating to a shared private `with_handle()` — the relocated original constructor body, one code shape, no dual paths. New `ownership()` accessor; `ownership` field on the widget. Stage-1 accessors `hts()`/`hts_mut()`/`hts_handle()` untouched. Four INV-337-O1 tests prove handle identity and mutation visibility in both directions through both constructors. Gate: check clean; `tree_view_shared_hts` 4/4; `tree` filter 83/83.

## [0.38.0-alpha] — 2026-06-11

### Added
- **SPEC-HAL-338 step 1 — right-angle route geometry engine** (`src/engines/route_geometry/mod.rs` new, `src/engines/mod.rs`, `src/widgets/timeline_canvas.rs`, `src/tests/route_geometry.rs` new, `src/tests/mod.rs`) — extracts the timeline canvas's inline dependency-arrow waypoint arithmetic into a pure, stateless, allocation-free engine: `RouteDirection`, `RouteEndpoint`, `RouteParams`, `RightAnglePlan`, and `route_right_angle` (INV-338-G1 stub-out/H–V–H/terminal-inset law, INV-338-G2 backward double-back, INV-338-G3 legal degenerate segments, INV-338-P1/P2 purity). The canvas translates `IntervalEdge` to directed endpoints and consumes the plan; realization (`h_seg`/`v_seg`/`arrow_head`) is untouched and scene output is bit-identical (INV-336-R8). Seven invariant tests cover the plan law, backward targets, degeneracy, and determinism. No styling fields, no obstacle input, no style arms ahead of a consumer (FORBID-338-1..4).

## [0.37.0-alpha] — 2026-06-11

### Changed
- **SPEC-HAL-337 stage 1 — shared topology substrate handle (`SharedHts`)** (`src/engines/hierarchical/mod.rs`, `src/engines/cell_mechanics/mod.rs`, `src/widgets/tree_view/{tree_view,body}.rs`, `src/widgets/property_editor_v2/{core,property_editor,body}.rs`, `src/widgets/list_view/{core,body,footer,list_view}.rs`) — the topology substrate moves behind a single shared handle: `pub type SharedHts = Rc<RefCell<HierarchicalTabularSubstrate>>` (INV-337-O1, one instance per topology). `CellMechanicsSubstrate::new` now takes the handle; `hts()`/`hts_mut()` return frame-ephemeral `Ref`/`RefMut` guards (INV-337-A1) and `hts_handle()` exposes a clone; `hts_mut` is `&self` via interior mutability. TreeView holds its own handle clone so its `hts()`/`hts_mut()` signatures survive unchanged, plus a new `hts_handle()`. `ListViewCore::column_model()` returns `Ref<'_, ColumnModel>` via `Ref::map`. Consumer call sites adapted mechanically under bounded rules: `&*` coercions at `reduce_ordered_selection` oracle and `&ColumnModel` argument positions, guard hoists at chained-borrow statements, an if-let scrutinee bound to a local in `ListView::finalize_config` to end the guard before the mutable call, and twelve migration-born `unused_mut` removals. Ownership form only — no behavior change; stage 2 (`HtsOwnership`/`with_hts`) and stage 3 (mutation routing, 337 §4) follow. Gate: check clean; `try_enter_edit`/`tree`/`temporal` test filters green.

### Added
- **ListView real Checkbox cell mechanics consumer** (`src/widgets/list_view/body.rs`, `src/widgets/list_view/core.rs`, `src/engines/cell_mechanics/mod.rs`, `src/widgets/tabular_view/data_adapter.rs`, `examples/showcase/tab_list_view.rs`) — ListView bool cells now consume the extracted `CellMechanicsSubstrate` always-mounted Checkbox path used by PropertyEditor v2 instead of the fallback CMS bool painter. Inline CMS paint is suppressed for bool cells, real `Checkbox` children are mounted and synchronized from realized cells, and `DataSource::commit_cell` gives ListView a default-compatible commit seam. The Showcase ListView adds a `Checkbox` column backed by mutable demo state to prove the round-trip.

## [0.35.3-alpha] — 2026-06-10

### Changed
- **Password text field mask glyph** (`src/widgets/text_field.rs`) — changes SecureEntry masking from the full black-circle glyph to the smaller bullet glyph, keeping password geometry, caret movement, selection spans, and hit testing on the same cached mask-advance path.

## [0.35.2-alpha] — 2026-06-10

### Fixed
- **ListView CellMechanicsSubstrate consumer and tooltip orientation fallback** (`src/widgets/list_view/*`, `src/engines/cell_mechanics/mod.rs`, `src/engines/hierarchical/mod.rs`, `src/model/tabular/row_heights.rs`, `src/widgets/tabular_view/view_config.rs`, `src/model/tooltip.rs`) — moves ListView onto the extracted CMS/HTS stack used by PropertyEditor v2 and TreeView, including adapter-fed realized cell values, HTS column geometry wrappers, flat row topology projection, row virtualization, expansion, resize, visibility, reorder, and config wiring. `ListViewBody` now declares bounded viewport constraints so ScrollArea does not feed infinite height during measurement, fixing the Showcase/ListView freeze path. Tooltip explicit placements now stay on their orientation axis (`Above`/`Below`, `Left`/`Right`) while `Auto` retains cross-axis fallback, fixing macOS tooltip side jumps near tight vertical space.

## [0.35.1-alpha] — 2026-06-10

### Fixed
- **Overlay dialog hit geometry and read-only text selection dismissal** (`src/widgets/dialog.rs`, `src/core/event_router.rs`, `src/core/widget/mod.rs`, `src/widgets/label.rs`, `src/widgets/text_block.rs`, `src/widgets/readout.rs`) — exports dialog panel and button rectangles in absolute coordinates when a modal root is offset, restoring button activation in Showcase normal and strict dialogs. Adds a widget-level hook for clearing transient read-only text selection and routes primary outside pointer-downs through the focused widget subtree check, so selection in readout/text widgets clears when clicking elsewhere without clearing focus or adding a global selection manager. Covers `Label`, `TextBlock`, `ReadoutTable`, `StatusGrid`, and `KeyValueStrip`; adds pointer-focus regression coverage.

## [0.35.0-alpha] — 2026-06-10

### Changed
- **SPEC-HAL-333 Rev 3 conformance — remove the shortcut snapshot from MenuModel** (`src/model/menu_model.rs`) — per INV-A6 Rev 3 ("hints are derived, never stored") and INV-E3 (single shortcut authority), the per-binding `Shortcut` snapshot is retired with the native machinery it served: the `Binding` struct collapses to a bare `MenuItemId → ActionId` map entry and `bound_shortcut` is removed (its only consumers were three test assertions, adjusted). `bind` behavior is preserved exactly — same registry lookup, same `to_display_string` derivation, same unconditional display-hint slot replacement and clear-on-unregistered. The menu system now holds no `Shortcut` anywhere.

## [0.34.0-alpha] — 2026-06-10

### Changed
- **SPEC-HAL-333 Rev 3 conformance — retire native machinery, collapse MenuHost to the concrete bridge** (`src/platform/menu_host.rs` deleted, `src/platform/mod.rs`, `src/widgets/menu_bar.rs`, `src/widgets/mod.rs`, `src/core/app.rs`, `Cargo.toml`) — per the Rev 3 ruling (in-house everywhere, INV-S2 sovereign-total), the `MenuHost` trait, `MenuHostKind`, `resolve_menu_host_kind()`, the X11/registrar probes, and the `zbus` dependency are removed; the platform layer has no menu knowledge (INV-F3). `InWindowMenuHost` is renamed to the now-free `MenuHost` — a concrete unboxed bridge (INV-G1) with inherent `install`/`sync_item`/`drain_activations`; `occupies_in_window_bar` is retired (single realization). `AppRoot.menu_host` drops the `Box<dyn>`; the `App::run` kind-match collapses to direct construction. No behavior change — the in-window path is equivalent through the same mailbox flow. Also removed the orphaned `occupies_in_window_bar_is_true` test exercising the retired method (missed by the task's edit list; covered by its removal clause). Menu/toast/tree/showcase gates all pass.

## [0.33.3-alpha] — 2026-06-10

### Fixed
- **Menu emphasis — popover surface, selection highlight, connected open title** (`src/widgets/menu_surface.rs`, `src/widgets/menu_bar.rs`) — open menus barely read in both themes. Token corrections per SPEC-HAL-332: the popup panel paints the designated popover surface `surfaces.level3` instead of the in-content `list.row_background` (which left the panel darker than the bar it descends from); the popup frame steps up to `borders.strong`; highlighted/pressed rows take the sanctioned selection treatment (`row_selected` + `text_selected`) replacing the inverted hover-stronger-than-pressed pairing; the open bar title paints `surfaces.level3` so title and popup read as one connected surface. Applies uniformly to every MenuSurfaceOverlay consumer (menu bar, ComboBox dropdowns, context menus) — intended. Light-theme `level3` recalibration (SPEC-HAL-332 §9) deliberately not taken here.

## [0.33.2-alpha] — 2026-06-10

### Added
- **Showcase main menu — SPEC-HAL-333 demonstration** (`examples/showcase/menu.rs`, `examples/showcase/main.rs`) — installs a real application menu on Showcase enter (idempotent, INV-G1): File→Exit actually exits, Demo→Push Toast raises a toast, Demo→Disable "Push Toast" is the live INV-A5 demonstrator (disables the sibling item and rewrites its own label through the model's mutation→drain→host-sync path), Help→About raises a toast. Actions registered as 3001–3004 in the Showcase block; item ids 101/201/202/301. Deliberately excluded: checkable items (SPEC-HAL-223 popup check visuals are an open gap), submenus (overlay expansion unverified), shortcut hints (app-scope keyboard dispatch unverified), and a View→Toggle Theme item (pending ruling — would leave the header Switch stale).

## [0.33.1-alpha] — 2026-06-10

### Fixed
- **MenuBar empty-projection collapse** (`src/widgets/menu_bar.rs`) — Class G mounts the in-window host unconditionally, so with no MenuModel supplied the bar laid out at theme button height and the application carried a dead strip. The bar now collapses to zero height and paints nothing when its titles are empty (no menu ⇒ no strip, INV-G1/INV-G4 spirit); `bar_height = 0.0` makes every strip/hit path inert by construction, and ScreenHost follows the bar's reported height unchanged.

## [0.33.0-alpha] — 2026-06-10

### Added
- **Menu host wiring — SPEC-HAL-333 Rev 2 Class G** (`src/core/app.rs`, `src/widgets/screen_host.rs`, `src/widgets/menu_bar.rs`) — `AppContext` gains `set_menu_model`/`menu_model_mut` (INV-G1/INV-A5); `AppRoot` owns the active host (`set_menu_host`) and runs `sync_menu_host` at the end of `flush_pending_actions_all` (INV-G3, never layout/paint): install-once per model replacement, dirty-id mirroring to the host, activation drain dispatching through the Action System with engine-ranged ActionIds rejected (INV-E1, SPEC-HAL-176), and in-window mailbox apply with AppRoot marking the bar node layout-dirty (SPEC-HAL-158). `App::run` resolves the host kind post-window (INV-B5) and mounts the in-window bar at z 100; native match arms collapse to in-window per INV-B2 fallback-total until `MacOsMenuHost`/`DbusMenuHost` land. `ScreenHost` allocates the top strip for a registered bar child (`set_menu_bar_child`, INV-G2) — full width, bar-reported height, all other children tight on the remainder below; no bar registered = exact pre-menu layout (INV-G4). Test-only `MenuBarMailbox::push_activation_for_test` injector; 4 new `app_menu_host_tests`. Until an application supplies a model, the mounted bar is an empty strip.

## [0.32.0-alpha] — 2026-06-10

### Added
- **InWindowMenuHost — SPEC-HAL-333 Rev 2 in-window MenuHost realization** (`src/widgets/menu_bar.rs`, `src/model/menu_model.rs`, `src/widgets/mod.rs`) — adds the `InWindowMenuHost` controller implementing `MenuHost` through a shared `MenuBarMailbox` (the ToastManager/ToastWidget shared-state pattern, SPEC-HAL-269): `install` stages the full projection (deep copy with live enabled state via `project_items`, dropping stale per-item syncs), `sync_item` stages enabled+label mirrors (`StagedItemSync`; checked arrives with the named SPEC-HAL-223 popup-gap amendment), `drain_activations` drains the bar's buffered activations (INV-E2). `MenuBar::new` now takes the mailbox; `apply_mailbox` applies staged data and returns `bool` so the Class G caller marks layout-dirty — dirty authority stays external (SPEC-HAL-158). A sync hitting the open title re-shows the surface so the popup reflects it. No tree access crosses the `MenuHost` trait; the trait and `src/platform/menu_host.rs` are untouched. `find_row`/`find_row_mut` in `menu_model` widened to `pub(crate)` for reuse. 14 menu_bar tests; menu_model's 9 unaffected.

## [0.31.0-alpha] — 2026-06-10

### Added
- **MenuBar widget — SPEC-HAL-333 Rev 2 Class D + §4** (`src/widgets/menu_bar.rs`, `src/widgets/mod.rs`) — in-window menu bar host widget. The bar owns exactly the INV-D2 set: title open/close, hover-switch between titles while open, Left/Right traversal with wraparound, and flat title chrome (no fill at rest, hover/active-open fills, square `push_rect` only, all colors theme-resolved — INV-D3/D4/D5). All in-menu interaction delegates to one re-shown `MenuSurface`/`MenuSurfaceOverlay` (INV-D1); activations buffer in `pending_activations` for the host to drain via `take_activations` (INV-E2). `MenuBarPolicy` per SPEC §4 with canonical defaults (`ClickToOpen`, hover-switch on, traversal on, close-on-reactivate, dismiss on click-outside/Escape/activation); its `DismissPolicy` stays module-scoped — `core/overlay_root.rs` owns a different type of the same name (SPEC-HAL-127). 7 unit tests. `InWindowMenuHost` controller and AppRoot mounting are later tasks; popup check-state visuals are a named SPEC-HAL-223 gap pending state sync.

## [0.30.0-alpha] — 2026-06-10

### Added
- **MenuHost contract and host-kind resolution — SPEC-HAL-333 Rev 2 Class B** (`src/platform/menu_host.rs`, `src/platform/mod.rs`, `Cargo.toml`) — adds the `MenuHost` trait (`install`, `sync_item`, `drain_activations`, `occupies_in_window_bar`), the `MenuHostKind` enum, and `resolve_menu_host_kind()`: a pure probe with no construction, window, or widget-tree access (INV-B5). Compile-time arms select `MacOsNative` on macOS and `InWindow` on Windows/WASM; the Linux arm probes at runtime and yields `LinuxDbus` iff the session is X11 (`XDG_SESSION_TYPE == "x11"`, exact match) and `com.canonical.AppMenu.Registrar` is owned on the session bus — every probe failure path yields `InWindow` (INV-B1/B3). Adds `zbus = "5"` to the Linux target dependency table (pure Rust, no GTK — INV-F1/F2); the Linux probe was compile- and runtime-verified externally against zbus 5.16.0. Host construction stays out of the platform layer (Class G, later task).

## [0.29.0-alpha] — 2026-06-10

### Added
- **MenuModel — SPEC-HAL-333 Rev 2 model layer** (`src/model/menu_model.rs`, `src/model/mod.rs`) — single source of truth for the application menu, owning exactly three tables (INV-A1): structure (`MenuTitle` list of `MenuItem` trees, SPEC-HAL-222), bindings (`MenuItemId → ActionId` with a bind-time snapshot of the Action's structured `Shortcut` and derived display hints replacing any authored ones, INV-A4/INV-A6), and live state (`MenuItemState` enabled/checked seeded recursively from the item tree). Runtime mutation (`set_enabled`, `set_checked`, `set_label`) applies immediately and queues the item id in a deduplicated FIFO dirty queue drained via `drain_dirty` (INV-A5); `set_checked` applies only to items declared checkable via `set_checkable`. No pixels, geometry, theme, focus, platform state, or host concept. 9 unit tests.

### SPEC
- **SPEC-HAL-333 Revision 2** (`docs/specs/SPEC-HAL-333-Menu-Bar-Host-and-Native-Projection.md`) — supersedes Revision 1 in full. Defines `MenuItemState` and fixes its ownership in the MenuModel; binds dispatch and shortcut authority to the existing Action System (SPEC-HAL-020) instead of a parallel command registry; rewrites macOS projection (native selectors confined to NSApp standard roles, Edit menu routes through action dispatch); replaces the platform-event-channel re-entry with buffered drain on the frame loop; splits host resolution (`MenuHostKind` pure probe) from construction (`App::run`); gates the Linux probe on X11 session type; adds threading law (INV-C5), the accepted macOS tracking stall (INV-C6), and the Application Integration class (G).

## [0.28.1-alpha] — 2026-06-10

### Changed
- **ListView / TreeView / PropertyEditor v2 — Shift+click range selection commits on press** (`src/widgets/list_view/body.rs`, `src/widgets/tree_view/body.rs`, `src/widgets/tree_view/tree_view.rs`, `src/widgets/property_editor_v2/body.rs`, `src/widgets/property_editor_v2/core.rs`, `src/widgets/property_editor_v2/property_editor.rs`) — ports the List mechanism from 0.23.1-alpha to the remaining range-selection consumers. All three previously armed a drag session on Shift+pointer-down and deferred the selection commit to pointer-up (ExtendTo on click-only, DragSelect/DragEnd on drag). The range now commits immediately on press and the drag stays armed, so a subsequent move keeps extending from the same persistent anchor. Drag continuation dispatches `ExtendTo` instead of `DragSelect` — `DragSelect` re-anchors at the pressed row, which would discard the press-committed range, while `ExtendTo` recomputes `[anchor..target]` from the persistent `anchor_key` on every call and cannot double-count. `DragEnd` is removed with it (it belongs to the DragSelect session mechanism and clears `focus_key`). The dead `drag_did_extend` / `selection_drag_did_extend` flags are deleted; TreeView and PropertyEditor v2 gain `selection_drag_last` for List-style row-change detection during drag.

## [0.28.0-alpha] — 2026-06-10

### Added
- **Selectable section and auxiliary text primitives** (`src/widgets/section_header.rs`, `src/widgets/auxiliary_text.rs`, `src/widgets/text_block.rs`, `examples/showcase/tab_overlays.rs`) — adds `AuxiliaryText` as a compact muted supporting-copy widget, reworks `SectionHeader` into the compact all-caps visual style, enables read-only selection for section headers and auxiliary descriptions, and routes both widgets through wrapping `TextBlock` layout so long text reports natural wrapped height instead of clipping as a single line.

### Fixed
- **Overlay readout physical-pixel snapping** (`src/widgets/readout.rs`, `examples/showcase/tab_overlays.rs`) — routes reusable readout borders through the renderer's physical-pixel border path and snaps remaining one-pixel rules to the active scene scale factor so same-type readout edges render with consistent thickness.

## [0.27.9-alpha] — 2026-06-09

### Fixed
- **Context menu overlay routing** (`src/core/app.rs`, `src/widgets/context_menu.rs`, `src/widgets/menu_surface.rs`, `src/model/menu_surface.rs`, `examples/showcase/tab_overlays.rs`, `examples/context_menu.rs`) — routes context menus through the modal overlay stack, restores OverlayRoot ownership of Escape/outside-click dismissal, prevents base-screen click-through and keyboard handling while overlays are active, shares popup geometry between paint and outside-hit coverage, and blocks disabled menu item activation from pointer and keyboard paths.

## [0.27.8-alpha] — 2026-06-09

### Added
- **Showcase runtime subsystem demonstrations** (`examples/showcase/*`, `src/core/paint.rs`, `src/core/scene/mod.rs`) — adds live Showcase coverage for SecureEntry password masking, SPEC-R-003 shape primitives through `PaintSink`, local `SignalStore` subscriptions, and current Surface/MWA/Signal subsystem state. Extends `PaintSink` with circle and ellipse primitive forwarding so widget paint paths can submit the active shape primitives directly.

## [0.27.7-alpha] — 2026-06-09

### Fixed
- **Navigation projection in main app loop** (`src/core/app.rs`, `src/tests/navigation_projection_boundary_guard.rs`) — wires `NavigationSurfaceOrchestrator` into the winit `App::run` frame loop using the existing base surface and pulls projection into `ScreenHost` before view rebuild, matching the active `run_ui_loop` SPEC-HAL-248 path.

## [0.27.6-alpha] — 2026-06-09

### Fixed
- **SignalBus frame-phase guard** (`src/core/app.rs`) — enforces the SPEC-HAL-161 no-signal-delivery critical section in the production `App::run` layout/scene/paint path, mirrors `run_single_frame()` with an RAII guard, and adds an `INV-XWIN-003` regression test.

## [0.27.5-alpha] — 2026-06-09

### Fixed
- **Tooltip suppression during active interactions** (`src/core/app.rs`, `src/core/widget_tree.rs`, `src/model/tooltip.rs`) — dismisses and blocks tooltip re-entry while widgets are in keyboard capture, pointer capture, active drag, or popup/modal backdrop overlay states.

## [0.27.4-alpha] — 2026-06-09

### Fixed
- **RadioGroup option spacing** (`src/widgets/radio_group_widget.rs`) — adds a 1px vertical gap between stacked radio options and covers the layout height with a regression test.

## [0.27.3-alpha] — 2026-06-09

### Added
- **Analytic SDF shape renderer submission** (`src/renderer/wgpu_renderer.rs`, `src/renderer/batcher.rs`, `src/renderer/render_graph/`) — wires the active `ShapePipeline` into renderer resources, render graph dispatch, dedicated shape vertex/index buffers, and SDF quad generation for circle, ellipse, line, and polyline primitive batches while preserving existing path-based scene shape routing.

## [0.27.2-alpha] — 2026-06-09

### Fixed
- **ComboBox affordance polish** (`src/widgets/combo_box.rs`) — restores the right-side dropdown affordance above the recessed field fill and keeps the separator inside the editable body so it no longer cuts through the focused border.

## [0.27.1-alpha] — 2026-06-09

### Fixed
- **Checkbox and Switch hover feedback** (`src/widgets/checkbox.rs`, `src/widgets/switch.rs`) — wires the existing hover animation state into paint output and frame invalidation so binary selectors visibly respond to pointer hover.

## [0.27.0-alpha] — 2026-06-08

### Added
- **Analytic SDF shape pipeline contract** (`src/renderer/pipelines/shape_pipeline.rs`, `src/renderer/shaders/shape.wgsl`, `docs/specs/SPEC-R-003-GPU-Path-Rendering-Primer.md`, `docs/specs/SPEC-HAL-085A-Shader-Suite-v2-Binding.md`) — activates the dedicated shape shader/pipeline contract for circle, ellipse, line segment, and polyline segment distance fields. Adds a fixed `ShapeVertex` layout, `ShapePipeline::new` render-pipeline creation using renderer globals, premultiplied SDF coverage output, focused layout/shader tests, and ledger/spec updates that leave renderer draw submission and shape vertex/index generation as the next explicit Stage 2B work.

## [0.26.1-alpha] — 2026-06-08

### Changed
- **Glyph atlas max-size eviction** (`src/renderer/glyph_atlas.rs`, `docs/specs/SPEC-HAL-029-Glyph-Atlas-Cache.md`, `docs/specs/SPEC-HAL-066-Unified-Glyph-Raster-Pipeline.md`) — adds deterministic LRU eviction once the atlas reaches maximum texture size. Evicted glyph rectangles are zeroed, recycled, split, and coalesced without moving retained glyph UVs; cache hits now refresh recency. Updates the glyph atlas specs from no-eviction/future-eviction language to the active max-size eviction contract.

## [0.26.0-alpha] — 2026-06-07

### Added
- **Gauge background inheritance** (`src/widgets/gauge.rs`) — adds `GaugeConfig::background_color` so radial gauges can either inherit their parent surface by default or opt into an explicit background slab.

### Changed
- **Light theme surface hierarchy** (`src/core/theme.rs`) — trials the lighter neutral light-mode palette with clear root chassis, panel, content, border, hover, selection, and semantic tokens.
- **Root and section surface painting** (`src/widgets/panel.rs`, `src/widgets/groupbox.rs`, `examples/showcase/main.rs`) — full-bleed plain Panels now resolve to `surfaces.base0`, ordinary Panels keep `surfaces.content`, and light GroupBoxes paint a real panel fill instead of a transparent fieldset over the window chassis.

## [0.25.0-alpha] — 2026-06-07

### Added
- **ExclusionFlow layout widget** (`src/widgets/exclusion_flow.rs`) — adds a flow container that computes free bands around exclusion rectangles so child widgets can wrap around reserved regions without overlap. Includes white-box coverage for band splitting, segment sorting, ignored zero-area/outside exclusions, and layout avoidance behavior.

### Changed
- **Professional control chrome application (SPEC-HAL-332)** (`src/widgets/button.rs`, `src/widgets/text_field.rs`, `src/widgets/text_area.rs`, `src/widgets/combo_box.rs`, `src/widgets/spin_box.rs`, `src/widgets/tag_input.rs`, `src/widgets/segmented_control.rs`) — routes raised clickable, recessed editable, and selector surfaces through the shared `ControlChrome` contract for compressed fills, sharper borders, capped radii, and non-glowing focus treatment.
- **Showcase dense-workstation layout polish** (`examples/showcase/*`) — tightens several showcase panels with fixed-column rows, fill-width navigator behavior, and formatted workbench/control examples.
- **Internal development workflow** — Revised private repository operating rules; no change to framework behavior.

## [0.24.0-alpha] — 2026-06-07

### Added
- **ControlChrome contract scaffolding (SPEC-HAL-332 Phase 1)** (`src/core/theme.rs`) — shared control-chrome vocabulary so interactive controls can resolve fill/border/radius/focus from one source instead of re-deriving chrome per widget. Adds public types `ControlChromeCategory` (`RaisedClickable`/`RecessedEditable`/`BinarySelector`/`TrackRange`/`SectionContainer`), `ControlChromeState` (`Idle`/`Hovered`/`Pressed`/`Focused`/`Captured`/`Disabled`/`Selected`), `ControlValidationState` (`Neutral`/`Error`/`Warning`/`Success`), `ControlFocusStyle` (`None`/`HairlineRing`/`BorderShift`/`IndicatorStroke`/`EdgeStripe`), and the returned `ControlChromeSpec`. Adds `Theme::control_chrome(category, state, validation) -> ControlChromeSpec` and `Theme::chrome_radius(category, requested, is_round) -> f32`. The selector uses existing tokens only (no new colors), consumes no shadow/glow parameters (focus is hairline/stroke/border-shift, never a glow per Principle 8), clamps functional radii to ≤2px (round indicators exempt), and applies validation as a border-color override (Error > Warning > Success > Neutral). Scaffolding only — no widget consumes it yet and no visual behavior changes; `Theme::control_border` is left in place.

## [0.23.1-alpha] — 2026-06-06

### Changed
- **List — Shift+click range selection commits on press** (`src/widgets/list.rs`) — Shift+click previously computed the `ExtendToRow` intents in the pointer-down handler, discarded them, and armed a drag session whose only job was to decide at pointer-up whether to commit (no drag) or rely on drag-move (drag). Plain click and Ctrl+click already commit on down, so range selection was the lone asymmetric case. Both the direct (`handle_event`) and projected (`handle_projected_action`) Down/Select paths now dispatch the range intents immediately on press (`handle_intent` + `set_focus` + `fire_selection_changed`) and keep the drag armed so a subsequent move keeps extending from the same anchor. This is safe because `OrderedSelectionAction::ExtendTo` is replacing — it pins the anchor to the persistent `anchor_key` and recomputes `[anchor..target]` on every call, so commit-on-press plus re-apply on drag-move cannot double-count. The deferred pointer-up re-interpretation block is removed (range gestures still `return true`, never activating), and the dead `drag_did_extend` field, its initializer, its Move-handler write, and its reset in `finalize_range_selection` are deleted.

## [0.23.0-alpha] — 2026-06-06

### Added
- **Switch — configurable label side (SPEC-HAL-208A)** (`src/widgets/switch.rs`) — new `LabelSide` enum (`Leading` / `Trailing`, default `Trailing`) lets a Switch place its owned label before or after the track, with `with_label_side` (builder) and `set_label_side` (mutable) setters. The label is shaped once in `paint`; its width sets the leading track offset, so `Trailing` keeps the track at `rect.x` and is byte-for-byte identical to prior behavior (INV-H2). Track left/right edges and the thumb origin derive from a side-aware `track_x`; the label is drawn at the precomputed `label_x`. Paint order stays indicator-first regardless of side (INV-H4). `layout` (width = track + gap + text, side-symmetric) and `handle_event` (full-bounds hit) are untouched, so the unified hit region and focus enclosure hold for both sides (INV-H3). No new styling authority, synthetic nodes, or event forwarding (INV-H5).

## [0.22.6-alpha] — 2026-06-03

### Fixed
- **Renderer — path-clip slot stride grows on demand, killing the INV-PATH-CLIPS panic** (`src/renderer/wgpu_renderer.rs`) — `compile_multi` packed each target's clips into the path-clip slab at a fixed stride of `PATH_CLIPS_SLOT_SIZE` (1024 B = 63 clips + fullscreen), and `upload_path_clips_at_offset` wrote `1 + scene.path_clips.len()` `ClipRectUniform`s (16 B each) there. A maximized multi-pane scene legitimately produces ≥ 64 distinct path clips for a target; its write (e.g. 65 × 16 = 1040 B) overran the 1024 B slot into the next target's slot — caught by the `debug_assert` in debug (the observed panic), silent slot corruption in release. The slab already grew for target *count* (`ensure_path_clips_slab_capacity`) but the per-target slot *stride* was fixed. Per SPEC-HAL-081B (slab grows to any size, no fixed cap), the stride is now frame-dynamic: a new `path_clips_slot_stride` field grows grow-only to the frame's largest clip count (`1 + max path_clips`, aligned up to `PATH_CLIPS_SLAB_ALIGNMENT`, floored at `PATH_CLIPS_SLOT_SIZE`); `ensure_path_clips_slab_capacity` takes a `required_stride` and grows both slot count and stride (never shrinks); per-target offsets and the `INV-PATH-CLIPS` debug assert use the dynamic stride. No shader or bind-group-layout change (`array<ClipRect>` is runtime-sized and `has_dynamic_offset` was already set); the draw-time clip bind already passed the stored per-target offset. INV-PATH-CLIPS now holds for any clip count until allocation fails.

## [0.22.5-alpha] — 2026-06-03

### Fixed
- **List — row text is vertically centered from font metrics, not `run.height`** (`src/widgets/list.rs`) — `List::paint` centered each row's `"Item N"` label with `row_y + (row_proj.height - run.height) / 2.0`. `GlyphRun.height` is the content-tight glyph extent (`"Item N"` has no descenders, so it is shorter than ascent + descent), while the renderer anchors the baseline at `top + ascent` (text-engine INVARIANT 1) — centering the tight box and then adding the full ascent pushed the baseline down, so row text sat low in the row (vertical drift). `paint` now uses the canonical control-text placement (SPEC-HAL-092 §8A, INV-TYPO-CTRL-1): `compute_control_text_baseline_y(row_proj.height, metrics.ascent, metrics.descent, typography.baseline_offset)` over `font_metrics(font_size)`, then `text_y = baseline_y - ascent`. Row text is now optically centered, consistent across zebra and plain rows. Paint-only; no interaction or geometry change.

## [0.22.4-alpha] — 2026-06-03

### Fixed
- **TextArea — document glyph color resolves from the live theme on every repaint instead of the stale layout cache** (`src/widgets/text_area.rs`) — `paint` drew document glyphs with `run.style.color`, the color baked into the shaped paragraph by `TextLayoutCache::rebuild`. Across a light/dark theme switch that color went stale: `TextLayoutCache::set_color` does not invalidate the cache (asymmetric with `set_font_size`, which forces a rebuild), and the no-rebuild theme toggle (TASK-SHOWCASE-THEME-TOGGLE-NO-REBUILD-15) triggers neither relayout nor a document/width rebuild — so existing runs kept the previous theme's glyph color while background, border, caret, and placeholder (all resolved live from `cx.theme` in `paint`) re-themed correctly. Resizing the window changed the layout width → forced a cache rebuild that re-baked the then-current color → masked the bug. `paint` now resolves the glyph color live from `t.colors.typography.primary` (SPEC-HAL-002 §4.7) — the exact token `layout()` feeds the cache — so the document text re-colors on a theme-only repaint with no resize. No `cx.invalidate()` / relayout added; the no-rebuild toggle is preserved.

## [0.22.3-alpha] — 2026-06-03

### Fixed
- **ComboBox now fills its parent's bounded width instead of collapsing to intrinsic content width** (`src/widgets/combo_box.rs`) — `layout` computed `natural_width` as `intrinsic_width.max(min_width).min(max_width)`, so whenever the parent did not hand the control a tight width it rendered at its intrinsic content width (`padding * 2 + 60 + affordance`) — a narrow stub in ordinary column / property-cell / flex-cell layouts. The width was also state-dependent: it expanded when the popup opened or an item was selected and shrank when another control took focus. `layout` now fills the available width when the parent supplies a bounded `max_width` (`max_width.is_finite()` → `max_width.max(min_width)`), using the intrinsic width only when the available width is unbounded (`f32::INFINITY`). Every combo now fills its container and the width is stable across popup open/close and focus changes. Behavioral note: this changes ComboBox sizing app-wide — a combo intended to stay compact in a horizontal row must now be given an explicit width by its parent.

## [0.22.2-alpha] — 2026-06-03

### Fixed
- **ScrollArea INV-REVEAL-FIT — focus-into-view no longer yanks the scroll to top for a viewport-sized target** (`src/widgets/scroll_area.rs`, `src/core/app.rs`) — `post_layout_reveal_focused_widget` runs when focus changes and calls `resolve_reveal_rect` to bring the newly-focused widget into view. When focus lands on a widget that fills its ScrollArea — a `List`/`TreeView` host (`needs_viewport_constraint = true`, so its rect height == the viewport) sitting at content origin — the reveal saw `rect.y (0) < view_top` and the `ScrollAlign::Nearest` "above viewport" branch scrolled to content top. In the Showcase Layout tab this surfaced as "click a list item after scrolling → the item selects but the list jumps back to the top, once" (the first focus-acquisition; later clicks keep focus inside the List, so no reveal fires). The List's own reveal path was innocent — it correctly requested no scroll for the visible clicked row. `resolve_reveal_rect` now establishes **INV-REVEAL-FIT**: a target whose scroll-axis extent meets or exceeds the visible viewport cannot be scrolled into view (every alignment just pins to an edge), so the reveal is a no-op and the current scroll is preserved. A small off-screen focusable (row/control height ≪ viewport) still scroll-reveals normally.

## [0.22.1-alpha] — 2026-06-03

### Fixed
- **TabBar — first click after a manual scroll no longer selects the wrong tab** (`src/widgets/tab_bar.rs`) — `on_focus` (fired once when the first click focuses the bar) initialized `focus_index` to the active tab and then called `ensure_index_visible(focus_index)`, which snapped `scroll_x` to bring the active tab into view — running *before* the `Down` handler hit-tested the click. After scrolling the strip right and clicking, focus fired first and reset the scroll toward 0, so the hit-test (`content_x = local_x + scroll_x`) landed on a leftward tab. First-click-only, because `on_focus` fires once per focus acquisition. The reset was latent until [0.22.0-alpha] made `scroll_x` survive rebuilds (before that the strip was always at ~0 on first click, so the reset was a no-op). `on_focus` no longer force-scrolls; keyboard navigation still reveals the focused tab via `move_focus → ensure_index_visible`, so the only behavioral change is that Tab-focusing into the bar no longer auto-scrolls the active tab into view (the first arrow press does).

## [0.22.0-alpha] — 2026-06-03

### Added
- **`TabBar::scroll_x` / `TabBar::set_scroll_x`** (`src/widgets/tab_bar.rs`) — public get/set for the tab strip's horizontal scroll offset (the `scroll_x` field was private). A consumer that recreates the `TabBar` on rebuild can read the live bar's offset and restore it onto the freshly built bar so the strip does not snap back to the start; the next `layout` re-clamps to the current scrollable range. TabBar itself was not at fault — `scroll_x` is preserved through `set_tabs` / `set_active_tab` / activation and only clamped (never reset) in `layout`. Wired into the Showcase (`examples/showcase/main.rs`) to persist tab-strip scroll across the rebuild a tab selection triggers: selecting a tab writes to `tab_sink`, which flags a rebuild that constructs a fresh `TabBar` (`scroll_x` = 0); the showcase now captures the offset each frame in `update` and restores it in `build`, so scrolling the strip and clicking a tab (or selecting via breadcrumb / keyboard) no longer jumps it back to the first tab.

## [0.21.6-alpha] — 2026-06-03

### Fixed
- **PropertyEditor SPEC-PM-009 — `Float` cell editor seeds from the display authority, not a second format** (`src/widgets/cell_editor_factory.rs`) — a `PropertyValue::Float` cell paints via `PropertyValue::display_text()` (`format!("{:.2}", f)` → `"2.20"`), but `make_float_field` seeded the edit-entry `TextField` with `format!("{}", initial)` — Rust's default `f64` Display, which drops trailing zeros → `"2.2"`. Two independent formatters for one value, so the editor seed diverged from the painted display (e.g. "Gamma" showed `2.20` but the editor opened on `2.2`). Float was the only affected variant — Integer/String/Color/Choice/KeyBinding already seed from the same form their display uses; only `f64`'s default Display differs from `{:.2}`. `make_float_field` now seeds from `PropertyValue::Float(initial).display_text()`, the same authority the cell display uses, so the editor text matches the painted value and follows automatically if the float precision ever changes.

## [0.21.5-alpha] — 2026-06-03

### Fixed
- **ComboBox SPEC-PM-009 — popup pointer model reads painted geometry instead of re-deriving it** (`src/widgets/combo_box.rs`) — `paint_overlay` is the popup's placement authority: it flips the popup above the control when there is no room below, widens it past the control width, and shifts it left on right-edge overflow. `handle_event` ignored all of that and assumed the popup sat directly below the control at control width — `popup_row_hit` resolved rows only at `local_y >= control_height`, and the `Down` handler's dismiss test was `local_y` within `[0, control_height + popup_height]`. When the popup flipped up (e.g. a ComboBox scrolled toward the bottom of a PropertyEditor viewport), a pointer over a row had negative `local_y`, so `popup_row_hit` returned `None` (no hover, no commit) and the `Down` handler read the click as outside and dismissed the popup — the "works, then breaks after I scroll" report; widened / left-shifted popups broke the same way. Fix (single geometry authority): `paint_overlay` records the popup's actual rect in control-local space once per frame (new `popup_local_rect`, full un-animated height); `popup_row_hit` derives the row origin from that rect, and the `Down` arm hit-tests `over_control` / `over_popup` and the popup scrollbar geometry against it rather than re-deriving from `control_height` / `last_layout_width`. Rows hover and commit whether the popup opens below or flips above, and the dismiss test fires only for clicks genuinely outside both surfaces.

## [0.21.4-alpha] — 2026-06-02

### Fixed
- **Plot INV-FLEX-MEASURE-UNBOUNDED — `Plot` no longer requests a `u32::MAX` offscreen texture when embedded as a `Flex` child** (`src/plot/mod.rs`, SPEC-PM-005 §7) — `Plot::layout` sizes its offscreen texture from `constraints.max_height`, but a `Flex` measures fill children with an unbounded main axis (`f32::INFINITY`) unless they declare `needs_viewport_constraint() = true` (SplitPane, ScrollArea, List/ListView). The `Plot` consumed `max_height` without declaring the flag, so `f32::INFINITY as u32` saturated to `u32::MAX`; the existing `pw/ph > 0` guard let it through and `Device::create_texture` panicked with a wgpu validation error (`Dimension Y value 4294967295 exceeds the limit of 8192`). Switching to the Showcase Plot tab — the first context to measure a `Plot` as a flexible `Flex` child — crashed on entry; `plot_live` (`mount_child`) and `plot_quad` (`SplitPane`) hand the `Plot` tight finite bounds and never triggered it. Two-part fix mirroring the 0.21.2/0.21.3-alpha `needs_viewport_constraint` defense for `ScrollArea`/`ListView`: (1) `Plot` now declares `needs_viewport_constraint() = true`, so a `Flex` measures it with a bounded main axis; (2) `Plot::layout` only (re)allocates the texture when both dimensions are finite (`size.width.is_finite() && size.height.is_finite()`), so when an ancestor `Flex` is itself measured unbounded the `INFINITY` never reaches `init_gpu` — the correct finite size arrives in the `Flex` re-layout pass (`src/widgets/flex.rs:343-358`). The new guard is inert in non-`Flex` contexts (finite bounds always satisfy it), so `plot_live`/`plot_quad` render unchanged.

## [0.21.3-alpha] — 2026-05-26

### Fixed
- **ListView INV-LIST-VIEWPORT-1 — `ListView::layout` guards the substrate-viewport write against non-finite measurement** (`src/widgets/list_view/list_view.rs`) — completes the invariant defense begun on the ScrollArea side in 0.21.2-alpha. `ListView` declares `needs_viewport_constraint() = true`, so a parent's unbounded measurement pass (the typical Flex measure of a flexible child) arrives with `available_height = INFINITY`. The `body_height` arithmetic below then yields `INFINITY`, and writing that into `substrate.body_viewport` corrupts every downstream `visible_rows()` projection — paint, hit-test, and child layouts all walk the entire dataset, O(N) per frame. `layout` now returns early on `!available_height.is_finite()` with a `content_size` height of `0.0`, skipping the substrate update and child layouts; the subsequent tight/allocation pass arrives with finite constraints and lays out normally. Where 0.21.2-alpha stops `INFINITY` at the constraint-propagation boundary (ScrollArea → virtualized child), this stops it at the substrate-write boundary (ListView → its own substrate), so a virtualized `ListView` measured directly is protected as well.

## [0.21.2-alpha] — 2026-05-26

### Fixed
- **ScrollArea INV-LIST-VIEWPORT-1 — virtualized children no longer receive `INFINITY` height during unbounded measurement** (`src/widgets/scroll_area.rs`, SPEC-HAL-086 §8 / SPEC-PM-005 / SPEC-HAL-131) — when a content child declares `needs_viewport_constraint() = true` (TreeViewBody, ListViewBody, ConsoleBody, List, ListView) and ScrollArea's own incoming `max_height` is non-finite — the typical Flex measurement pass for a flexible child — `max_h` now resolves to `0.0` instead of passing `INFINITY` downward. Previously the child saw `available_height = INFINITY`, virtualization computed every row as visible (`compute_fixed_height` returned `last = sorted_count`), and the body materialized all rows in a single layout pass (10k+ disclosure widgets at `NODE_COUNT=100k`, ~2.5 s per measurement pass — the source of the periodic scroll freeze). With the degenerate `0.0` viewport, virtualization returns zero visible rows, the body performs no per-row work, and `content_size` (`total_count × row_height`) stays accurate for the scroll domain; the subsequent tight/final pass arrives with proper constraints and lays out normally. Measured on `examples/treeview.rs` at 100k nodes: per-pass layout dropped from 2510 ms / 2070 ms to a single 9 ms startup sort, with no layout pass exceeding 5 ms during scroll and the body never materializing more than 42 rows. Width handling (SPEC-HAL-086 Stage 8) and the existing non-virtualizing `INFINITY` path are unchanged.

## [0.21.1-alpha] — 2026-05-18

### Added
- **`KeybindEditor` Backspace-clear** (SPEC-HAL-329 §5.5 / INV-KBE-BACKSPACE-CLEARS) — in capture mode, `Backspace` with no modifiers held clears `value` to `None`, fires `on_change(None)`, clears `last_conflict`, and releases capture (new `KeyOutcome::ClearAndRelease`). `Backspace` with any modifier held is captured as a normal binding via the standard commit path.

### Fixed
- **`KeybindEditor` single-click capture entry** — Pointer Down (Primary) now explicitly calls `cx.request_keyboard_capture()` and returns `true`, matching the canonical `TextField` pointer pattern (`src/widgets/text_field.rs:1212`). Previously the widget returned `false` expecting framework auto-engagement that never occurs, so clicking the editor did not enter Capture. Public API (SPEC-HAL-329 §7) unchanged.

### Changed
- **SPEC-HAL-329 amended** (`docs/specs/SPEC-HAL-328A-Category-K-Amendment.md`): §3.2 Backspace caveat; §5.2 Backspace-clear transition row and single-click pointer row; §5.4 Backspace carve-out; new §5.5 (Backspace clear behavior); §7.3 handle_event description; §9 INV-KBE-BACKSPACE-CLEARS. The double-click capture-entry deviation (transient §5.6 / INV-KBE-DOUBLE-CLICK-ENTRY) was introduced then fully reverted — single-click entry is retained, consistent with SPEC-HAL-328 §3.1 and PropertyEditor activation.

## [0.21.0-alpha] — 2026-05-18

### Added
- **`KeybindEditor` widget** (`src/widgets/keybind_editor.rs`) — SPEC-HAL-329, Category K (Single-Stroke Capture) per SPEC-HAL-328 §11.K. Standalone widget editing `Option<Shortcut>` via one-stroke keyboard capture: capture entry flips to a prompt, the first non-modifier Key Down builds a verbatim `Shortcut` from the event + tracked modifier state, fires `on_change`, and programmatically releases capture. Producer of `Shortcut` values only — does not touch `BindingRegistry` (INV-KBE-NO-REGISTRY). Escape/Tab are framework-reserved and never consumed (INV-KBE-NO-OVERRIDE-ESCAPE/TAB); commit is exactly one explicit user keystroke (INV-KBE-EXPLICIT-COMMIT); value display is canonical via `Shortcut::to_display_string()` (INV-KBE-DISPLAY-CANONICAL). Public API: `new`, `with_value`/`with_placeholder`/`with_capture_prompt`/`with_conflict_check`/`on_change` builders, `value`/`set_value`/`cancel`, plus optional `ConflictInfo` display callback. Re-exported as `halcyon::widgets::{KeybindEditor, ConflictInfo}`. 10 inline conformance tests covering §9 invariants and §10 forbidden behaviors.

## [0.20.27-alpha] — 2026-05-16

### Added
- **`paint_linear_gauge` public free function** (`src/widgets/linear_gauge.rs`) — extracted from the private `LinearGauge::paint_horizontal` / `paint_vertical` methods so external call sites (e.g. custom `CellPainter` implementations in tabular-view consumers) render identical gauge output without instantiating a widget. Public dispatcher `pub fn paint_linear_gauge(cx, rect, cfg, value)` plus private `paint_linear_gauge_horizontal` / `paint_linear_gauge_vertical`; `LinearGauge::paint` now delegates to it. Single source of truth, pre-empts SPEC-PM-009 (Dual-Source Geometry Drift). Behavior byte-for-byte unchanged. SPEC-HAL-311.

## [0.20.26-alpha] — 2026-05-11

### Fixed
- **SpinBox PM-009 dual-source geometry in paint** — `SpinBox::layout()` computes a content-aware `value_width = max(widest_text+2*pad, "999"+2*pad, geom.value_width)` and lays the editing field out at that width, but `paint()` was re-reading the raw theme constant `geom.value_width`. When the content-aware width was greater than the theme floor, the editing field was painted into a narrower rect than it was laid out for; center-aligned text positioned at the wrong offset. Paint now reads `self.cached_value_width` and `self.cached_control_width` populated by layout — single source.
- **SpinBox editing_field cache rebuild after `set_text`** — `enter_edit_mode` calls `self.editing_field.set_text(&self.value_text)` which marks the cache stale. The framework's next layout pass picks it up, but paint can run first. Added an explicit `self.editing_field.rebuild_layout_cache(text_width, font_size, text_engine)` immediately after `set_text` — same pattern as ComboBox `attempt_commit` (1afbcf29). New cached fields: `cached_value_width`, `cached_font_size`, `cached_text_field_padding_x` (synced from theme in layout, accessed from event handlers which have no theme access).

## [0.20.25-alpha] — 2026-05-11

### Fixed
- **TextField paint defensive guard for stale layout cache** — patching every per-site `mark_stale` in `handle_event` (0.20.24-alpha) covers the six text-mutation arms but is fragile against future text changes and against hosts that mutate the embedded TextField (SpinBox `set_text` on double-click, ComboBox fresh-TextField after `attempt_commit`, IME commit arm, etc.). Added a single defensive check in `TextField::paint`: when `self.layout_dirty.get()` is true, the non-empty text branch bypasses the boundary-cache reads entirely and renders via `cx.text_engine.measure(...)` + `shape(...)` + `draw(...)` directly. The next frame's layout pass rebuilds the cache and the normal `draw_with_backgrounds` path (selection highlights + caret) resumes. Trade-off: selection/caret are not drawn for the one frame after a mutation that didn't synchronously rebuild — imperceptible in practice.

## [0.20.24-alpha] — 2026-05-11

### Fixed
- **TextField crash on typing (P0)** — every text-mutation arm in `TextField::handle_event` marked the layout cache stale and called `cx.invalidate_layout()`, but the framework can paint before re-layout runs in the same frame. Paint then hit `text_width_from_boundaries()` → `boundaries()` → `assert_fresh()` and panicked. Reproducible by typing any character into any TextField in `card_demo` or `showcase`. Fix: after every `self.state.layout_cache.mark_stale()` inside the handler, immediately call `self.rebuild_layout_cache_now(cx.text_engine())` — the same pattern already used at the pre-read rebuild sites. Cache is rebuilt at the current `last_layout_width`; the subsequent `invalidate_layout()` still schedules a real re-layout next frame. Six call sites updated: Backspace, Delete, Space, Cut, Paste, text input. (Note: there is also a 7th `mark_stale` in the IME commit arm; the spec for this task enumerated only the 6 above, so the IME site is left as-is for a follow-up.)

## [0.20.23-alpha] — 2026-05-11

### Added
- **`SurfaceColors::card` theme token** — dedicated elevated-card surface, calibrated per theme so a single value reads correctly in both. Previous attempts using `base0.darken(...)`, `surfaces.content`, or `surfaces.level1` all failed to land in the right perceptual zone in both themes simultaneously (linear-space math diverges at brightness extremes).
  - Light: `#F2F3F5` (6 sRGB levels above base0 `#ECECEC`, not white).
  - Dark: `#18181B` (7 sRGB levels above base0 `#111113`, Material-style elevation).

### Changed
- **`Panel` Card-mode bg** now uses `surfaces.card` (was `surfaces.content`). Combined with the visible drop shadow already in place (offset `(0, 1.5)`, spread `0.5`, blur `6.0`, alpha `0.15`), Cards read as elevated surfaces against the canvas in both themes without becoming pure white in light theme. Plain `Panel::new()` and explicit `with_background` overrides unchanged. Doc comment on the `background` field updated.
- **`GroupBox` caption chip default** now uses `surfaces.card` (was `surfaces.level1`). Chip now matches Card surfaces when GroupBox sits on a Card, and remains visible against the base0 canvas. Explicit `GroupBox::with_background(c)` still wins (chip = `c`).

## [0.20.22-alpha] — 2026-05-11

### Changed
- **Card background darken amount dialed back further** — `Panel::paint()` Card-mode default tint now uses `surfaces.base0.darken(0.004)` (was `0.008`). Resulting fill is ~`#EBEBED` — only ~1 unit darker than `base0` (`#ECECEC`). Ghost-level tint: the boundary registers via shadow + border, not via color difference. Plain `Panel::new()` background unchanged.

## [0.20.21-alpha] — 2026-05-11

### Fixed
- **ComboBox stale-cache crash on popup selection** — `attempt_commit` replaces the embedded `text_field` with a fresh `TextField::new().with_text(text)` whenever the user picks an option. `with_text` marks the new TextField's `layout_cache` stale, but the framework's next layout pass may not run before the next paint, so `paint` → `text_width_from_boundaries` → `boundaries()` tripped `assert_fresh` and crashed. The earlier `c8eb02b` fix added `text_field.layout()` inside ComboBox's own `layout()` method, which works for layout-time text changes but not for event-time text changes (popup commits).

### Added
- `TextField::rebuild_layout_cache(layout_width, text_engine)` — public hook for hosts that mutate or replace the embedded TextField during event handling. Sets `last_layout_width`, rebuilds the cache, clears `layout_dirty`. Distinct from the existing private `rebuild_layout_cache_now` because it accepts an explicit width (a freshly-constructed TextField has `last_layout_width = 0`, which would otherwise produce a degenerate cache).

ComboBox `attempt_commit` now calls `self.text_field.rebuild_layout_cache(self.last_layout_width.get() - self.affordance_width, cx.text_engine())` immediately after replacing the TextField, using the content width cached from the most recent layout pass. `cx.invalidate_layout()` is still called so a proper re-layout schedules for the next frame, but paint between this event and that re-layout no longer trips the stale-cache assertion.

## [0.20.20-alpha] — 2026-05-11

### Changed
- **Card background darken amount dialed back further** — `Panel::paint()` Card-mode default now uses `surfaces.base0.darken(0.008)` (was `0.015`). Resulting tint is ~`#EAEAEC` — only ~2 units darker than `base0` (`#ECECEC`). Almost-invisible elevation: the contained area's boundary registers without a visible color change. Plain `Panel::new()` background unchanged.

## [0.20.19-alpha] — 2026-05-11

### Changed
- **Card background tint dialed back from `surfaces.level2` to `surfaces.base0.darken(0.015)`** — `level2` (`#D8DAE0`) was reading as a separate dark block ("dark territory") rather than a contained area. The default Card-mode background is now ~`#E8E8EA` — only ~4 units darker than the page surface (`base0` = `#ECECEC`) — a barely-perceptible elevation that reads as a deliberate enclosure without dominating the page. Plain `Panel::new()` background unchanged (`surfaces.content`).

### Examples
- `examples/card_demo.rs`, `examples/showcase.rs` — root content `Flex::column` now sets `.with_background(cx.app.theme.colors.surfaces.base0)` so the page reads with the same warm-grey wash as the wireframe example. Card backgrounds are now visible *against* this page tint, instead of merging with a default-white window background.

## [0.20.18-alpha] — 2026-05-11

### Fixed
- **Control backgrounds painted at z=2 (containers stay at z=0–1)** so child fills always sit above their container regardless of paint order — same-z fights between Panel/Card (z=0) and child controls (z=0) had been letting the parent grey win on some draws. Convention now in force across the input widgets:
  - Containers (Panel, GroupBox): bg z=0–1
  - Controls: bg z=2, borders z=3, hover z=3–4, focus rings z=5 (`LAYER_FOCUS`)
- Concrete edits:
  - `src/widgets/text_field.rs` — bg `0 → 2`, sculpted-border edges (4 rects) `1 → 3`.
  - `src/widgets/combo_box.rs` — bg `0 → 2`, sculpted-border edges (4 rects) `1 → 3`.
  - `src/widgets/text_area.rs` — rounded border `0 → 3`, inset bg `1 → 2`. Inner shadow at z=2 left as-is (paint-order keeps it above bg since it's emitted after).
- Already-correct sites confirmed (no edit): `checkbox.rs` (bg z=2, border z=3), `switch.rs` (track z=2, border z=3), `spin_box.rs` (zone bgs z=2, border z=3), `button.rs` (gradient z=2), `slider.rs` (track z=2, thumb z=5).

## [0.20.17-alpha] — 2026-05-11

### Fixed
- **Panel/Card background no longer covers child controls** — `paint()` was calling `stack.paint(cx.scene, panel_rect, state, 2)`, putting the container's background fill at z=2/3. Children paint after the parent in traversal order but at z=0 (e.g., `TextField` does `push_rect(rect, bg, 0, ...)`), so the parent's higher-z gray fill covered every child input background — TextFields, Checkbox boxes, ComboBox/SpinBox input areas all blended into the Card. Lowered the decoration stack base z-index to 0 so child paint at z=0 ends up visually on top via later draw order. Also dropped the Card-mode drop-shadow z from 1 to 0 so it sits at or below the (now z=0) decoration stack rather than between the background and child content. Panel-only change; no widget z-indices touched.

## [0.20.16-alpha] — 2026-05-11

### Fixed
- **Slider thumb (Track style) now paints above the track fill** — `paint_track_style` was emitting the thumb `push_rounded_rect` at z=4, the same z as the filled portion of the track. With matching z-indices the renderer's tie-breaking was painting the thumb under the rail, hiding the circle. Bumped the thumb to z=5 so it sits one layer above the fill (z=4), the borders (z=3), and the track background (z=2). `paint_bar_style` (handle z=4 above fill z=3) and `paint_radial_style` (single `push_radial_gauge`) were already correctly stacked and untouched.

## [0.20.15-alpha] — 2026-05-11

### Changed
- **Checkbox box has a flat 1px `borders.default` border** instead of the prior sunken sculpted border (shadow top-left + highlight bottom-right). Background remains `surfaces.content`. The flat border reads cleanly against any container surface — page, GroupBox, or elevated Card — so the box is no longer effectively invisible inside a Card whose background sits close to the box fill. The previously-unused `_border_color` (which lerped to `borders.focus` on hover) is gone; border color is now plain `borders.default` per the task spec.
- **Audit of TextField / TextArea / ComboBox / Switch backgrounds**: each already filled with `theme.colors.surfaces.content` (verified at `text_field.rs:842`, `text_area.rs:1667`, `combo_box.rs:974` for `InputBgState::Default`, `switch.rs:309`). Spec's literal condition for changing the Switch off-track ("if it matches the Card background") was not triggered — `surfaces.content` (`#19191C`) and `surfaces.level2` (`#202024`) are distinct hex values — so no Switch change was made. No widget bg changes in this version.

## [0.20.14-alpha] — 2026-05-11

### Changed
- **Card visual polish** — `Panel` paint() now lifts Card-mode panels off the page surface and gives them a defined edge:
  - Subtle drop shadow (`ShadowMode::Drop`, offset `(0, 1)`, blur 3.0, color `from_linear(0,0,0,0.08)`, z=1) pushed before the background when `title.is_some()`.
  - Border color in Card mode is now `colors.borders.default` (visible outline). Plain `Panel::new()` honors the constructor-set `self.border.color` (transparent by default — preserves prior look for callers that didn't opt into a colored border).
  - `Panel::card()` corner radius bumped 2px → 4px so the rounding actually reads.
  - Title bar background tint deferred per the spec's "skip if complex" allowance — `push_rect` is single-radius and per-corner rounding via that path would have required additional plumbing; shadow + visible border were sufficient to distinguish Cards from GroupBoxes.

## [0.20.13-alpha] — 2026-05-11

### Changed
- **Button background uses a 4-stop Fresnel/Aero-style vertical gradient** — replaced the flat 2-stop `paint_vertical_gradient(top, bottom)` call with a multi-stop `push_gradient_rect(&GradientDef { stops, ... })` to produce a glossy raised look: bright highlight band concentrated in the top 45%, sharp transition near the midpoint, darker base in the bottom half. Per-state stops:
  - Normal: `(0.00, +0.10) → (0.45, +0.06) → (0.50, -0.01) → (1.00, -0.04)`
  - Hovered: `(0.00, +0.14) → (0.45, +0.08) → (0.50, +0.01) → (1.00, -0.02)`
  - Pressed (inverted, "pressed in"): `(0.00, -0.04) → (0.50, -0.01) → (0.55, +0.06) → (1.00, +0.10)`
  - Linear, p0=(0,0), p1=(0,1). The downstream sculpted-edge highlight/shadow rects and focus ring are unchanged.

## [0.20.12-alpha] — 2026-05-10

### Added
- **Card preset on Panel** — extended `Panel` with an optional painted `title: Option<String>` field, `with_title()` builder, and a `Panel::card(child, title)` constructor (2px corner radius, 12px padding, transparent border that paints from theme, elevated `surfaces.level2` background applied dynamically when `title` is `Some`). Layout reserves a `title_height = font_size + 8.0` strip at the top of the content area; `paint()` renders the title text in `colors.typography.primary` at the panel's top-left, with a 1px `colors.borders.subtle` separator spanning the content width below it. Plain `Panel::new()` is unchanged — no title, no separator, content surface as before.
- **Widget factory registration** — `Card` added to `src/agent/widget_factory.rs` (alongside `GroupBox`) with `title` (string) and optional `radius` (px) config keys; placeholder `NodeId(0)` child wired by the agent's `add_child` flow, same as `GroupBox`.
- **MCP catalog entry** — `Card — title (string), radius (px, default 2)` listed under LAYOUT; widget count bumped 29 → 30.

## [0.20.11-alpha] — 2026-05-10

### Fixed
- **ComboBox TextLayoutCache STALE crash on click in showcase Configuration tab** — ComboBox already called `self.text_field.layout(cx, text_constraints, &[])` early in its `layout()` method, but with loose constraints derived from the incoming `constraints.max_width` (which can be much larger than the eventual `natural_width` that ComboBox returns to its parent — e.g., when laid out inside a wide Flex row). The TextField's cached `last_layout_width` therefore did not match the actual paint rect width (`rect.width - affordance_width` at paint line 1012), and any geometry read from the cache after a subsequent mutation could trip the stale-cache assertion. Mirrored the SpinBox `editing_field.layout(cx, Constraints::tight(...), &[])` pattern: after `natural_width` is finalized, re-lay out the TextField with tight constraints (`field_width = natural_width - affordance_width`, `result.size.height`) so the cache is built at the exact width paint will use.

## [0.20.10-alpha] — 2026-05-10

### Fixed
- **HierarchicalBlockVirtualization integer overflow on PropertyEditor startup** — same bug class as 0.20.9-alpha, second site at `src/engines/hierarchical/virtualization.rs:68`. `compute_fixed_height` panicked with `attempt to add with overflow` because `((viewport_bottom / h).floor().max(0.0) as usize + 1)` overflows when the float-to-`usize` cast saturates to `usize::MAX` on degenerate early-layout inputs. Replaced `+ 1` with `.saturating_add(1)`; `.min(row_count)` clamps to a valid index. The hierarchical virtualization engine no longer panics; PropertyEditor demo starts cleanly.

## [0.20.9-alpha] — 2026-05-10

### Fixed
- **VirtualizationEngine integer overflow on EventLog startup** — `compute_fixed_height` panicked with `attempt to add with overflow` at `src/engines/virtualization/mod.rs:175`. The expression `((viewport_bottom / h).floor().max(0.0) as usize + 1)` overflows when the float-to-`usize` cast saturates to `usize::MAX` (which happens on degenerate early-layout inputs where the viewport extent is huge or non-finite relative to row height); the subsequent `+ 1` then wraps. Replaced the bare `+ 1` with `.saturating_add(1)`. The downstream `.min(row_count)` clamps the result back into a valid index range. The virtualization engine no longer panics on degenerate viewport/height combinations; `cargo run --example eventlog` now starts cleanly.

## [0.20.8-alpha] — 2026-05-10

### Changed
- **WGT-1: GroupBox caption now sits on a fieldset-style chip** — the existing border-with-gap pattern left the caption hovering in a transparent void over the etched border. Paint now fills the gap area (between `gap_start` and `gap_end`, with a small vertical pad above and below the caption baseline) with `theme.colors.surfaces.base0` at z=5 — above the border (z=4), below the glyphs. The caption reads as a deliberate HTML fieldset/legend chip instead of "text floating across the border line". No layout change, no theme tokens added, no API change. Skipped when the label is empty (no chip to draw).

## [0.20.7-alpha] — 2026-05-10

### Changed
- **WGT-3: ProgressBar default `track_thickness` is now visually prominent** — was 4 / 4 / 6 logical px across compact / regular / comfortable density themes (paint as a hairline that read as "barely visible"). Bumped to 10 / 12 / 16 logical px to match the visual weight of TrackBar's `bar_height` (16 / 20 / 24). Track-style ProgressBars in compose_dashboard now read as bars, not lines. Theme-only change; no widget code touched, so callers that override `track_thickness` are unaffected.

## [0.20.6-alpha] — 2026-05-10

### Fixed
- **GroupBox top padding actually clears the caption** — `content_y` was `extra_above + border_thickness + padding.top`, which is roughly `label_height/2 + half-border + padding`. Because the caption is centered on the border, its bottom edge sits at `rect.y + label_height`, so the first child crowded the title (zero or negative gap for typical font sizes — most visibly "Current Deployment" overlapping "Rollout Progress" in compose_dashboard). `content_y` is now `label_height.max(border_thickness) + padding.top`, putting the first child a full `padding.top` below the caption's bottom edge.

## [0.20.5-alpha] — 2026-05-10

### Fixed
- **SegmentedControl focus ring follows the active segment on click** — `Pointer Down` updated `self.selected` but not `self.focused_index`, so the focus ring stayed on whatever the keyboard had last visited. Pointer activation now sets both, matching the keyboard activation path.
- **SegmentedControl Enter/Space bounds-checks `focused_index` before indexing `self.segments`** — added a `fi < self.segments.len()` short-circuit before `self.segments[fi].enabled`. Defensive against any path that could leave `focused_index` stale; the existing handler would have panicked on a raw index. The arrow/Home/End handlers already go through `next_enabled` / `first_enabled` / `last_enabled`, which return values in `[0, len)`, so they are unaffected.

## [0.20.4-alpha] — 2026-05-10

### Fixed
- **`process_create_split_layout` now honours `ratio` and stretches panel children** — the assembly was throwing the `ratio` away (`_ratio: Option<f32>`) and giving both panels `grow: 1.0`, so the split was always 50/50 regardless of the requested ratio. Panels also lacked `align_items: "stretch"`, so children sized to intrinsic width on the first frame and only grew to panel width after a popup interaction triggered a relayout (visible width jump on ComboBox in a sidebar). The function now sets `leading_grow = ratio.unwrap_or(0.5)`, `trailing_grow = 1.0 - leading_grow`, and adds `align_items: "stretch"` to both panel configs. A 0.22 ratio now produces a ~22% / ~78% split with children filling the panel width from the first frame.

## [0.20.3-alpha] — 2026-05-10

### Fixed
- **TabSwitcher inactive tabs no longer paint over active content** — `paint_node` iterated children unconditionally; zero-size layout did not stop the framework from descending into the subtree, so inactive tab content rendered on top of the active tab at (0,0). The traversal now skips children whose `absolute_paint_rect` has both `width ≤ 0` and `height ≤ 0`. Layout still runs on those nodes (TabSwitcher lays them out with `Constraints::tight(Size::zero())`), so `INV-PAINT-REQUIRES-LAYOUT-EPOCH` is still satisfied; only paint is skipped. Normal widgets always have non-zero rects, so this only affects content multiplexers that deliberately collapse children.

## [0.20.2-alpha] — 2026-05-10

### Fixed
- **ComboBox no longer jumps width after popup selection** — `ComboBox::layout` was returning `intrinsic_width.min(constraints.max_width)`, which under stretch parents (`min_width == max_width == parent_width`) returned the smaller intrinsic width and ignored the parent's stretch request. After a popup interaction triggered a relayout, something else expanded the ComboBox to parent width and the visible width jumped. The intrinsic width is now treated as a floor: `width = intrinsic.max(min_width).min(max_width)`. Stretch parents fill from the first frame; unbounded / grow=0 parents still get the intrinsic width.

## [0.20.1-alpha] — 2026-05-10

### Fixed
- **TextField INV-H.VIS1 panic on zero-width viewport** — `ensure_focus_visible` now returns early when `viewport_width < 1.0`. During rapid composition (e.g. compose_dashboard building dozens of widgets in a frame), TextField can receive a transient layout pass with `max_width == 0` before the tree is fully laid out; the existing post-condition asserted the focus target is visible inside the viewport, which cannot hold for a zero-width viewport. The assertion is unchanged for non-zero viewports.

## [0.20.0-alpha] — 2026-05-10

### Added
- **`TabSwitcher` widget (`src/widgets/tab_switcher.rs`)** — content multiplexer for tab layouts. Owns no widgets of its own; takes children directly from the tree and lays out only the one at `active.get()`. Inactive children are still laid out with `Constraints::tight(Size::zero())` so their `layout_epoch` is stamped (paint invariant) but their rect collapses to `RectF::zero()`, so they paint nothing visible. Selection is shared via `Rc<Cell<usize>>` with the matching tab selector.
- **`SegmentedControl::add_invalidate_target(TreeWidgetId)`** — register a sibling widget (e.g. a `TabSwitcher`) to mark layout-dirty whenever the selection changes. Invalidation runs after `on_change` fires for both pointer and keyboard activation. Required because the framework's `cx.layout_child` short-circuits clean siblings — a sibling switcher would not see the new selection without explicit invalidation.

### Changed
- **`process_create_tab_layout` (assembly) — tab content now actually switches.** Previously the assembly placed all content regions as siblings of the SegmentedControl in a single Flex column, with section headers between them, and never wired the selector — content was always visible (designer-mode placeholder). The new shape:
  1. Assembly root (Flex column).
  2. `SegmentedControl` selector with an `on_change` closure that writes to a shared `Rc<Cell<usize>>` and an invalidation target wired to the new `TabSwitcher`.
  3. Divider.
  4. `TabSwitcher` parented under the assembly root.
  5. Content regions parented under the `TabSwitcher` (one per tab).
  
  The `AssemblyResult.ids` shape is unchanged (`tab_bar_id` + `content_ids`); the `content_ids` order still matches the input `tabs` order. The previous per-tab "Tab: X" `SectionHeader` and inter-tab dividers are gone — they are no longer meaningful with a real switcher.

## [0.19.1-alpha] — 2026-05-10

### Fixed
- **Slider edit-mode entry no longer panics with `TextLayoutCache STALE`** — same shape as the SpinBox 0.17.1 fix. `enter_value_edit()` calls `value_field.set_text()`, marking the embedded TextField's layout cache stale; the field is owned (not in the widget tree) so the framework's layout traversal does not reach it on its own. `enter_value_edit` now calls `cx.invalidate_layout()` so `Slider::layout()` runs before paint and rebuilds the cache via the existing `if self.value_editing { self.value_field.layout(...) }` branch. Without this, double-clicking the value chip panicked when paint read the stale cache before any layout pass had run.

## [0.19.0-alpha] — 2026-05-10

### Added
- **`Slider::with_value_field_side(ValueFieldSide)`** — pick which side of the track the editable value field appears on. New `pub enum ValueFieldSide { Left, Right }` (default `Right`). With `Left`, the value chip renders at `rect.x` and the track starts at `rect.x + value_field_width + label_gap`; with `Right` (default), the track starts at `rect.x` and the value chip follows the track (and label, if any).
- **`widget_factory` Slider config: `value_side: "left" | "right"`** — JSON-composed sliders can pick the side without a code change.
- **Slider track now reserves space for label + value field** — `paint_track_style` and `paint_bar_style` compute `actual_track_w = rect.width - label_extras - value_extras`, so the track no longer extends across the value chip area. Required for the side switch to work; also makes the value chip stay inside the slider's own bounds (and the cached hit-test rect).

### Changed
- **`Slider::paint_label` signature** — now takes `(track_x, actual_track_w)` instead of just `track_length`, so it can position the label after the track and the value field per `value_field_side`. Internal change; no public API impact.

### Fixed
- **Slider hit-testing accounts for the track offset** — `x_to_value` and `x_to_value_bar` now subtract a new `cached_track_offset_x` from the local pointer x, so dragging works whether the value chip is on the left or right of the track.

## [0.18.0-alpha] — 2026-05-10

### Added
- **`TextField::with_text_align(TextAlign)`** — opt-in horizontal text alignment within the viewport. Reuses the canonical `core::paragraph::TextAlign` (`Left` / `Center` / `Right`, default `Left`). Center/Right only apply when text fits the viewport; once text overflows the viewport, alignment is ignored and the field reverts to the existing left-anchored scroll behavior. Caret position and selection backgrounds inherit the alignment offset since they are drawn relative to the run's `text_x`. Placeholder text and the empty-text caret are aligned by the same rule.

### Changed
- **SpinBox edit-mode TextField is now centered** — the embedded `editing_field` is initialized with `with_text_align(TextAlign::Center)` so the value text remains centered in the value region during editing, matching the centered display-mode rendering. Removes the visual jump that previously occurred when entering edit mode.

## [0.17.1-alpha] — 2026-05-10

### Fixed
- **SpinBox edit-mode entry no longer panics with `TextLayoutCache STALE`** — `enter_edit_mode()` calls `editing_field.set_text()`, which marks the embedded TextField's layout cache stale. Because the TextField is owned (not in the widget tree), the framework's layout traversal does not reach it on its own. `enter_edit_mode` now calls `cx.invalidate_layout()` so SpinBox's own `layout()` runs before paint and rebuilds the cache via the existing `if self.editing { self.editing_field.layout(...) }` branch. Without this, double-clicking the value area panicked when paint read the stale cache before any layout pass had run.

## [0.17.0-alpha] — 2026-05-10

### Changed
- **SPEC-HAL-210 INV-C2 amended** — `Slider` may now expose an **optional editable** value field. Replaces the prior "no editable value field" rule. Edit mode is opt-in, entered explicitly (double-click), commit is explicit (Enter), cancel is explicit (Escape), focus loss reverts (no implicit commit), parse failure reverts silently, typed input is parsed and clamped to the domain on commit. Editing must reuse the embedded TextField infrastructure (SPEC-HAL-101, ComboBox pattern). Slider drag and keyboard adjustment continue to operate on the slider proper, independent of the value field.

### Added
- **Slider editable value field (SPEC-HAL-210 INV-C2 amended)** — when `with_value_display()` is enabled, the value label is rendered as a sunken-input chip; double-clicking it activates an embedded `TextField` (owned, not in the tree) with the current value pre-selected. `Enter` commits parsed-and-clamped `f32`, `Escape` reverts, focus loss reverts, parse failure reverts silently. While editing, slider drag and arrow keys are routed to the TextField for caret/cursor; clicking the track during an edit cancels it and starts a normal drag at the click position. Field width is sized once from the widest formatted text in `[min, max]` so it does not resize as the value changes. New private fields on `Slider`: `value_field: TextField`, `value_editing: bool`, `value_edit_start: f32`, `cached_value_field_rect: Cell<RectF>`. New private fns: `max_value_text`, `enter_value_edit`, `commit_value_edit`, `cancel_value_edit`.
- **`widget_factory` Slider config: `show_value: bool`** — when present and true, the factory calls `Slider::with_value_display()` so JSON-composed sliders can opt into the editable value chip without a code change.

## [0.16.0-alpha] — 2026-05-10

### Added
- **SpinBox direct text-edit mode (SPEC-HAL-212 INV-C3)** — double-click the value area, or press Enter/F2 while focused, to type a value directly. Embedded `TextField` (ComboBox pattern: owned, not in the widget tree) handles the editing; `Enter` commits the parsed/clamped value, `Escape` reverts. Pressing `+`/`−` or losing focus during an edit reverts the in-progress text (no implicit commit). Input is filtered to digits and `-`. Layout sizes the editing field at the existing computed value width; paint swaps the centered value text for the embedded field when `editing` is true. New private fields on `SpinBox`: `editing_field: TextField`, `editing: bool`, `edit_start_value: i64`. New private fns: `enter_edit_mode`, `commit_edit`, `cancel_edit`.

## [0.15.30-alpha] — 2026-05-09

### Added
- **Agent Playground binary (`src/bin/playground.rs`)** — one-binary launch combining a blank GPU canvas, the agent WebSocket server, and an embedded Tower agent. CLI: `--load <file>`, `--auto <objective>`, `--collab`, `--no-tower`, `--llm-url <url>` (accepts full URL or `host:port` shorthand), `--port` (agent ws, default 9317), `--control` (Tower control TCP, default 9318). `--auto`/`--collab`/`--no-tower` are mutually-exclusive paths checked in `Args::parse`.
- **`agent::TowerMode` enum** (`src/agent/tower.rs`) — `Explore` (existing autonomous-exploration loop, kept as the default for backwards compatibility with `tower_embedded_demo` and SPEC-HAL-318 callers), `Directed` (control-channel only — no main loop, parks waiting for `tower-ctl` / MCP `halcyon_instruct_agent`), `Autonomous { objective }` (playground composition cycle), `Collaborative` (stdin propose/approve/execute). Re-exported from `agent::mod`. `Tower::run` dispatches on `mode` immediately after spawning the control listener; non-`Explore` modes early-return after their loop completes.
- **`agent::playground_loop` module (`src/agent/playground_loop.rs`)** — composition loops shared by Tower's Autonomous and Collaborative modes:
  - `run_autonomous(objective, …, max_exchanges)` — Plan → Compose → Inspect → Refine. The plan-phase LLM call breaks the objective into ≤6 screen descriptions; for each screen, Compose generates JSON commands via `COMPOSITION_SYSTEM_PROMPT`, the Inspect/Refine round prompts for `{"status":"DONE"}` or `{"status":"REFINE","commands":<JSON>}` and executes any returned commands, looping up to `MAX_REFINE_ITERATIONS` (4) per screen or until the LLM-call budget (`max_exchanges`) is exhausted.
  - `run_collaborative(…)` — read instruction from stdin, snapshot the tree, propose composition commands, render them through `pretty_print_actions` (one line per op showing `parent_id`, `widget_type`, `widget_id`, key config fields), prompt `[y/N]`, execute on `y`. `q`/`quit`/EOF/empty-line ends the session.
  - Both loops reuse `LlmClient`, `WsDispatcher`, `Directive/Task/Operation`, `value_to_instruction`, `Tower::extract_json`, `Tower::is_valid_action`, `read_event_blocking`, and the existing `Snapshot`/`BlueprintSnapshot` flow. `processing` and `widget_count` on shared `TowerState` are updated so `tower-ctl status` reports correctly while a playground loop runs.
- **`TowerConfig::mode` field** — defaults to `TowerMode::Explore`, so existing `..TowerConfig::default()` callers (e.g. `examples/tower_embedded_demo`) are source-compatible.
- **`pub(super)` exposure** for `extract_json`, `is_valid_action`, `value_to_instruction`, `read_event_blocking`, `COMPOSITION_SYSTEM_PROMPT`, `TowerState`, and `SharedWs` so the sibling `playground_loop` module can reuse them without duplication.

## [0.15.17-alpha] — 2026-05-07

### Changed
- **Compact tree snapshot now uses explicit `{ ... }` scope braces (SPEC-HAL-319)** — the default rendering branch in `write_compact_node` opens a brace at the end of the header line for any node that has indented children, and closes it at the parent's indent. Leaves emit no braces. Row-bracket notation (`Flex(row) [c1, c2]`) and collapsed Label+Input pairs are unchanged — they already carry explicit scoping. Removes ambiguity for both human readers and LLM parsers about where each container ends.

## [0.15.16-alpha] — 2026-05-07

### Added
- **`AgentAction::Snapshot { format }` — compact tree format (SPEC-HAL-319)** — `Snapshot` now carries an optional `format: Option<String>` (`#[serde(default)]`, defaults to `None`). When `format == Some("compact")`, the engine produces an indented text outline of the widget tree (~200 tokens for 100 widgets) instead of the full JSON blueprint, and emits it as `BlueprintSnapshot { data: Value::String(...) }`. Existing payloads (`{"Snapshot":{"name":"foo"}}`) keep the full-JSON behaviour.
- **`build_compact_tree` (`src/core/app.rs`)** — server-side walker that produces the compact format. Skips `ScreenHost` / `ScrollArea` wrappers, abbreviates widget types (`L`, `TF`, `CB`, `SB`, `Sl`, `Chk`, `Sw`, `Btn`, `GB`, `SA`, `Div`, `FS`, `Spacer`, `PB`), collapses Label+Input vertical pairs to `Flex → L"text" + Type`, and renders horizontal Flex rows as `Flex(row) [c1, c2, c3]`. Direction tag shown only for rows; spacing only when non-zero. Bounds and hints omitted (LLM doesn't need them).
- **`Flex::axis()` / `Flex::spacing()` getters** — small public accessors so the compact tree can read direction/spacing without duplicating Flex internals.
- **`examples/designer` `ctree` command** — sends `Snapshot { name: "compact", format: "compact" }` and prints the response. `tree` help text updated to distinguish the two formats.

## [0.15.15-alpha] — 2026-05-07

### Added
- **`examples/designer` — interactive REPL for live UI composition (SPEC-HAL-319)** — connects to a running halcyon canvas via the agent WebSocket and lets you `add` / `cfg` / `rm` / `hint` / `tree` widgets from the terminal. Response loop whitelists composition responses (`CompositionResult`, `BlueprintSnapshot`, `CompositionError`) and skips the canvas's continuous noise events; a 200ms TCP read timeout (set on `MaybeTlsStream::Plain`) lets the loop re-check the 3-second deadline without blocking. `WouldBlock`/`TimedOut` IO errors are mapped to `continue`.

### Changed
- **`ConfigureWidget` now also drives flex params (SPEC-HAL-319)** — `process_pending_composition` calls `widget_factory::extract_flex_params` on the JSON config and applies the result via `WidgetTree::set_flex(node_id, …)` after `configure_widget`. Sending `{"ConfigureWidget":{"widget_id":N,"config":{"grow":1.0}}}` now sets the grow factor on the tree node and triggers relayout in the same frame.
- **`examples/canvas` Flex root carries `EdgeInsets::all(8.0)`** — gives canvas content 8px breathing room from the window edges (the `surfaces.base0` background still extends edge-to-edge underneath).

### Fixed
- **ComboBox text-area click no longer immediately reopens the popup** — `on_capture_enter` opens on **initial** capture entry only (gated on `!was_captured`); re-entry while still captured (which fires after `close_on_text_click` closes the popup) no longer reopens it. Click-to-toggle now works in both directions for `ComboBox::simple()`-built dropdowns.

## [0.15.14-alpha] — 2026-05-07

### Fixed
- **GroupBox etched border pixel-snapped (SPEC-HAL-093 v3)** — border thickness was visibly inconsistent across sides and instances because `border_y = rect.y + extra_above` and the title-gap coordinates carried fractional offsets from font-metric math. `paint` now derives `bx`/`by`/`bw`/`bh`/`bottom_y` (and `gap_start`/`gap_end`) by `.round()`-snapping the source rect, then composes all five outer (shadow) and five inner (highlight) 1px segments off those snapped values. The etched groove is unchanged in shape; it just renders crisply on the device pixel grid now.

## [0.15.13-alpha] — 2026-05-07

### Changed
- **Compact density mode tightened (SPEC-HAL-093 v3)** — `GeometrySpec::for_density(Compact)` now uses `padding_y = 2.0` (was `4.0`) and `text_field.padding_x = 3.0` (was `4.0`). Compact `control_height` drops from 28px to 24px (`ceil(line_height(20) + 2*2)`), pulling every derived control (TextField, Button, ComboBox, SpinBox) tighter. Targets professional desktop density (VS Properties / classic desktop). Regular and Comfortable modes untouched.

## [0.15.12-alpha] — 2026-05-07

### Changed
- **Scrollbar thumb visual refinement (SPEC-HAL-093 v3)** — thumb now reads as a subtle handle, not a heavy blob.
  - **Lighter resting color** in the light theme: `scrollbars.thumb` `0xA0A4AA` → `0xC0C4CA`; the old shade promoted to `thumb_hover` (replacing `0x808488`). Dark theme untouched.
  - **2px "gloved" radius** on the thumb: `paint_scrollbars` switches from `push_rect` to `push_rounded_rect(_, _, RoundedCorners::uniform(2.0), …)` on both axes. Softens the hard 90° ends without becoming a pill.
  - **Grip ridges are now tonal hints, not glyphs** — derived from the thumb color via a 0.06 linear-space shift (`Color::from_linear(thumb.r ± 0.06, …)`) instead of full-contrast `borders.shadow`/`borders.highlight`. Count adapts: 3 ridges on tall/wide thumbs (`> 40`), 2 on medium (`> 25`), none on tiny. Width capped at 6px (was 8), spacing 2px (was 3), still drawn as dark/light pairs for the sculpted-edge effect.

## [0.15.11-alpha] — 2026-05-07

### Added
- **`AgentAction::ConfigureWidget` is now executable end-to-end (SPEC-HAL-319)** — previously declared but silently dropped. A new `PendingComposition::ConfigureWidget` variant carries the action across Phase 1, and a free `configure_widget` function applies the JSON config to the resolved widget during Phase 2 (Deferred Mutation). Phase 2 marks the node layout-dirty and emits a `CompositionResult { action: "ConfigureWidget", success: true, … }` event back to the agent. Initial property coverage: Flex `spacing`, Label `text`, TextField `placeholder`, Slider `style` + `value`, ProgressBar `value` — easy to extend per widget by adding a downcast block.
- **In-place setters for ConfigureWidget targets** — `Flex::set_spacing`, `TextField::set_placeholder`, `Slider::set_display_style`, `Slider::set_value`. Mirror the existing builder-style methods so the agent path can mutate widgets after they're mounted.

### Changed
- **Scrollbar thumb is now sharp-cornered with grip ridges (SPEC-HAL-093 v3)** — `ScrollArea::paint_scrollbars` swaps `push_rounded_rect` for `push_rect` on both axes (R-0, consistent with Halcyon's flat-corner identity) and overlays three classic grip ridges centered on the thumb. Each ridge is a `borders.shadow`/`borders.highlight` pair faded in step with `scrollbar_fade_alpha`, giving the sculpted-edge affordance of a professional desktop scrollbar. Vertical thumb paints horizontal ridges; horizontal thumb paints vertical ridges. Tiny thumbs (height ≤ 10px / width ≤ 10px) skip the grip to avoid crowding.

## [0.15.10-alpha] — 2026-05-07

### Fixed
- **`INV-BG-TEXT-COHERENCE` false positive when canvas root is a ScrollArea (SPEC-HAL-163)** — `DrawPlanBuilder::build_multi` counted BG-affecting scene nodes only at the top level of `scene.root`, missing rects nested inside `Clip` / `Layer` / `Scroll` containers. A new `count_bg_affecting` helper recurses through those container variants. Unblocks ScrollArea-rooted screens whose Flex background sits inside the ScrollArea's clip.

### Changed
- **Slider factory now defaults to `TrackDisplayStyle::Bar` (SPEC-HAL-278)** — agent-composed Sliders render as filled bars with edge handles (dense data UI look) instead of the thin track + circle thumb. The factory accepts a `"style"` config key with values `"track"`, `"bar"`, `"pill"`, or `"radial"`; absent or unknown values fall back to `Bar`.
- **`examples/canvas`** — root widget is once again a wired `ScrollArea` (`new_unwired` + Flex content child + `wire_content`) so canvas content scrolls vertically when it overflows the window. Background is `surfaces.base0` (warm gray) per SPEC-096 v3. Now safe because the BG-coherence count is recursive.

## [0.15.9-alpha] — 2026-05-07

### Added
- **`BorderColors::highlight` / `BorderColors::shadow` (SPEC-HAL-093 v3)** — sculpted-edge tokens carried by the theme. Light theme: `highlight=0xFAFAFA`, `shadow=0xA0A4AB`. Dark theme: `highlight=0x3A3D44`, `shadow=0x111316`. Together they paint two-tone etched (groove) and embossed edges without per-widget shadow shaders.

### Changed
- **Light theme rebalanced for warm-gray base (SPEC-HAL-096 v3)** — `surfaces.base0` is now `0xECECEC` (warm gray, not white) and `surfaces.content` is `0xFFFFFF` so input fields/lists/cells visibly stand out from the canvas. `chrome`, `level1`, `level2`, `level3` and the `borders` palette (subtle/default/strong/focus) updated to the v3 values for higher contrast and a more professional desktop feel.
- **`RadiusSpec::default()` flattened (SPEC-HAL-093 v3)** — sharp corners are the default. `r0=0`, `xs=2` (tooltips, dropdown menus), `s=3` (popovers, dialog bodies), `m=4` (modal dialogs only), `l=4` (capped at R-M). The `l` field is kept to avoid breaking call sites but no longer carries a larger radius — Halcyon has no large radii.
- **`GroupBox` border now an etched (groove) two-tone edge (SPEC-HAL-093 v3)** — the previous flat 1px border is replaced by an outer `borders.shadow` line and an inner `borders.highlight` line offset 1px inward. Title-gap geometry preserved, both lines on z-layer 4. `with_border()` color overrides are no longer consulted for the etched edge.

## [0.15.8-alpha] — 2026-05-07

### Changed
- **PropertyEditor validation-error tooltip** — rebuilt as a proper rounded tooltip below the editing cell: SDF-rounded border (overlay rect pass, outline only so it doesn't occlude text), `surfaces.level3` fill carried by a `TextBackgroundSpan` so the fill renders before glyphs in the text pass, error glyphs at `LAYER_OVERLAY + 2`. Replaces the prior flat text-with-background error strip. Tooltip geometry uses 6×3 padding and a 4px corner radius driven from local constants.

## [0.15.7-alpha] — 2026-05-07

### Reverted
- **`examples/canvas`** — wired ScrollArea root from 0.15.6 triggers `INV-BG-TEXT-COHERENCE`: ScrollArea owns surface 1, text primitives land on it, but the inner Flex's background rect is not counted as BG-affecting on that surface. This is a scene-graph surface-assignment issue, not a composition bug. Reverted to a bare `Flex::column()` root so canvas + compose_settings start cleanly. Vertical scrolling will return once the surface-assignment side is resolved.

## [0.15.6-alpha] — 2026-05-07

### Added
- **Agent factory: `grow`/`shrink`/`basis` flow through `AddWidget` config (SPEC-HAL-319)** — `widget_factory::extract_flex_params(&Value) -> Option<FlexParams>` reads the keys from a JSON config; `process_add_widget` applies the result via `WidgetTree::set_flex` immediately after `add_child`. Halcyon's grow lives on the parent's view of the child tree node, so the factory cannot set it at construction; this hook bridges that gap for runtime composition.
- **`examples/compose_settings`** — `add_labeled_field_row` helper that mirrors `add_labeled_field` but tags the column with `"grow":1.0`. All horizontal in-row callers (Names, Locale, Port/Timeout/Protocol, Theme/Font, Sidebar/Opacity, Cache/Threads) switched to the row helper so field columns now share the row width proportionally instead of shrinking to minimum content.

### Changed
- **`examples/canvas`** — root widget is once again a properly-wired `ScrollArea` (`ScrollArea::new_unwired()` + a Flex content child + `wire_content`) so canvas content can scroll vertically when it overflows the window. Replaces the bare `Flex::column()` revert from 0.15.5.

## [0.15.5-alpha] — 2026-05-06

### Changed
- **`examples/compose_settings`** — major rewrite: four sections (User Profile, Network, Appearance, Advanced) with horizontal field rows (First/Last name, Port/Timeout/Protocol, Theme/Font size, Sidebar width/Opacity, Cache size/Thread count) and dedicated multi-checkbox rows. New section closes with `Divider` + Reset Defaults / Cancel / Save button row. Every field carries an agent hint.

### Reverted
- **`examples/canvas`** — `ScrollArea::empty()` as the canvas root panicked at startup (deferred-wiring enforcement fires before layout, even though `is_fully_wired` was patched to return true in standalone mode). Reverted to a bare `Flex::column()` root so canvas + compose_settings start cleanly. ScrollArea root will need a different approach (likely a real wired ScrollArea with a placeholder content child) before it can come back.

## [0.15.4-alpha] — 2026-05-06

### Changed
- **`examples/compose_settings`** — proper Labels above every input (no more placeholders-as-labels) and complex nested layouts: First / Last name side-by-side, Port / Timeout side-by-side, vertical `Flex(column, spacing=2)` label-then-field columns assembled by a new `add_labeled_field` helper. Exercises GroupBox + nested Flex composition end-to-end.

## [0.15.3-alpha] — 2026-05-06

### Changed
- **GroupBox dynamic-children stacking now respects theme spacing** — the SPEC-HAL-319 standalone branch in `GroupBox::layout()` adds `cx.theme.spacing.s` between consecutive children instead of butting them flush. The wired-content path is unchanged.

## [0.15.2-alpha] — 2026-05-06

### Fixed
- **GroupBox / Container / ScrollArea ignore the `children` parameter (SPEC-HAL-319)** — their `layout()` methods previously drove off the internal wired-content NodeId only, which is `None` in the `::empty()` / `standalone_empty` paths used by runtime composition. Children added later via `WidgetTree::add_child` were never laid out and tripped `INV-PAINT-REQUIRES-LAYOUT-EPOCH`. Each `layout()` now consults the `children: &[NodeId]` slice (the live-tree truth) and, in the standalone branch, stacks the live children vertically using `cx.layout_child` + `cx.set_child_rect`. The pre-existing wired paths (single content child) are preserved bit-for-bit, including Container's INV-ZERO-2 corruption panic.

### Removed
- DIAG-1/2/3 probes from `src/core/app.rs` — no longer needed now that the root cause (widgets ignoring `children`) is identified and fixed.

## [0.15.1-alpha] — 2026-05-06

### Fixed
- **Composition actions now execute in Phase 2 (SPEC-HAL-319 + SPEC-HAL-002 §3.8/§4.2)** — `AddWidget`, `RemoveWidget`, `ClearChildren`, `SetHint`, and `Snapshot` were mutating the widget tree mid-frame during the agent action drain (Phase 1), which violated the constitutional invariant "structural operations only between frames" and was the root cause of `INV-PAINT-REQUIRES-LAYOUT-EPOCH` panics on dynamically added widgets. Phase 1 now queues the actions into a new `PendingComposition` buffer; the buffer is drained immediately after `run_deferred_mutation_phase` (Phase 2), so the upcoming layout pass naturally visits all new nodes within the same frame. Fixes the layout/paint-epoch mismatch on `compose_settings`, removes the resize-time panic, and gives all dynamically composed widgets non-zero bounds.

## [0.15.0-alpha] — 2026-05-06

### Added
- **Composition-compatible widget constructors (SPEC-HAL-319)** — childless variants for the container/control widgets that previously demanded a child `NodeId` at construction, so the agent factory can mount them empty and the agent fills them later via `WidgetTree::add_child`. New constructors: `Button::with_text(text)`, `Container::empty()`, `GroupBox::empty(label)`, `ScrollArea::empty()`, `ComboBox::simple(options: Vec<String>)`. ComboBox's standalone variant ships with a built-in `PickHighlightedResolver` (commits the highlighted option, rejects with `NoMatch` otherwise) — fine for wireframing; production code should pass a real `ResolverHandle`.
- `widget_factory::create_widget` now constructs Container, GroupBox, ScrollArea, Button, ComboBox in addition to the previous 13 types (SplitPane still excluded — needs two children at construction).

### Changed
- **Button**: `pub child: NodeId` → `pub child: Option<NodeId>`; added `pub text: Option<String>`. `Button::new(child)` keeps its public signature and wraps internally; layout/paint pick text from `self.text` first, then fall back to the child Label's `text_info`. No external code reads `button.child` directly.
- **Container**: `pub child: NodeId` → `pub child: Option<NodeId>`. Layout returns zero child size when standalone — final size is driven by `fixed_width`/`fixed_height`. The existing structural-corruption panic for wired-but-take-failed children is preserved.
- **GroupBox**: `pub content: NodeId` → `pub content: Option<NodeId>`. Layout falls through to zero child size when standalone; the labeled border still paints.
- **ScrollArea**: SPEC-HAL-154's Deferred-wiring contract makes "create empty, populate later" architecturally forbidden, so the empty path uses a new internal `standalone_empty: bool` flag rather than bypassing Deferred. `is_fully_wired` returns true and `seal` skips Deferred sealing in that mode; layout short-circuits to a viewport-sized empty result.

## [0.14.0-alpha] — 2026-05-06

### Added
- **SPEC-HAL-319 Phase 1: Runtime UI Composition** — agents can now mutate the widget tree at runtime via the agent channel. New `AgentAction` variants: `AddWidget { parent_id, widget_type, index, config, hint }`, `RemoveWidget`, `ClearChildren`, `MoveWidget`, `ConfigureWidget`, `SetHint`, `Snapshot { name }`. New `AgentEventKind` variants: `CompositionResult { action, success, widget_id, message }` and `BlueprintSnapshot { name, data }`. Engine processes Add/Remove/ClearChildren/SetHint/Snapshot in the agent action drain; Move/Configure are declared but not yet wired.
- **`crate::agent::widget_factory`** — `create_widget(type_name, config) -> Option<Box<dyn Widget>>` and `available_types() -> &'static [&'static str]`. Constructable widgets in this phase: Flex, TextField, TextArea, Checkbox, Switch, Slider, SpinBox, RadioGroup, Label, ProgressBar, Divider, FixedSpacer, FlexSpacer. Container/Panel/GroupBox/ScrollArea/SplitPane/Button/ComboBox excluded for now because their constructors require a child `NodeId` or a `SelectionResolver` — to be addressed by adding empty/standalone constructors in a follow-up.
- **`crate::agent::mock_data`** — deterministic seeded mock data for wireframing: `names(n, seed)`, `emails(n, seed)`, `list_items(domain, n, seed)`, `tree(depth, breadth, seed)`, `table_rows(n, seed)`, `options(domain)`. Pure-Rust LCG, no extra deps.
- **`examples/canvas`** — empty design surface bound to the agent server on `:9317`. Connect a WebSocket client and send `AddWidget` commands to compose UI live.

## [0.13.1-alpha] — 2026-05-06

### Changed
- **Tower pipelined main loop (SPEC-HAL-318)** — replaced the post-action `action_delay_ms` sleep with a 50 ms immediate-drain window so Tower can call the LLM while the Halcyon side is still animating. Events generated during LLM inference accumulate in the WebSocket buffer and are picked up by the next iteration's `drain_events`, so the LLM-thinking time effectively masks the animation/sequence time. Single-action sends now increment `total_actions` (previously only sequence sub-actions did). Removed the per-event `ValidationError` / `FocusChanged` finding detection from the post-action block — those will be reintroduced under the objective system. Feedback wording tightened (`"Action sent."`, `"No immediate UI response."`, `"Events:\n…"`).
- **Engine typing animation char-count fix** — guard now uses `text.chars().count() > 1` so a single non-ASCII multi-byte character still goes through the immediate path instead of being queued.
- **`DEFAULT_SYSTEM_PROMPT`** — minor tightening; dropped the "TypeText sends the full text — the app handles the visual pacing" line and the trailing two-sentence reminder, in favor of a single line about the UI summary.

## [0.13.0-alpha] — 2026-05-06

### Added
- **Agent typing animation (SPEC-HAL-318)** — when the agent sends `TypeText` with more than one character, the engine queues the chars and emits them as `FusedInputEvent::TextInput` one-or-a-few per frame instead of all at once. Pacing is adaptive: `(queue_len / 60).max(1)` chars per frame, capping a long string at ~1 second visible animation. While the queue is non-empty the engine forces `FrameReason::Animation` so the loop doesn't idle. Single-char `TypeText` still goes through the normal immediate path.
- **`AgentAction::Sequence(Vec<AgentAction>)`** — Tower-side macro action. The engine never sees this variant; Tower expands it into individual sub-actions on the WebSocket with `TowerConfig::sequence_delay_ms` (default 80 ms) between them. `is_valid_action` accepts a Sequence when the array is non-empty and every entry is one of the known action variants.
- **`TowerConfig::sequence_delay_ms: u64`** — inter-action delay inside a Sequence, default 80.

### Changed
- **`DEFAULT_SYSTEM_PROMPT`** — rewritten to teach the model about Sequences, the `i`-key capture-mode gotcha, and the engine-side typing animation.

## [0.12.0-alpha] — 2026-05-06

### Added
- **Tower query-aware navigation (SPEC-HAL-318)** — Tower now queries the UI on startup to build a world model and re-queries focus state after every action. The LLM receives a structured widget summary (IDs, types, hints, focus state) prepended to the initial prompt and a `Focused: widget N (mode: ...)` line after each exchange, so it sees `TextField(5) hint='Email address' focused=true` instead of raw event JSON. New private helpers on `Tower`: `query()` (blocks for `QueryResult`) and `query_ui_summary()`. `DEFAULT_SYSTEM_PROMPT` updated to teach the model how to use the new context. WebSocket-only (FORBID-318-1 preserved).

## [0.11.0-alpha] — 2026-05-06

### Added
- **Agent semantic hints on tree nodes (SPEC-HAL-317)** — `WidgetNode::agent_hint: String` plus `WidgetTree::set_agent_hint(node_id, hint)` let Screen/form builders annotate widgets with contextual meaning ("Email address", "Submit button", etc.). The hint lives on the node, not the widget, so the same `TextField` type can mean different things in different forms. The agent query API surfaces it: `AgentWidgetInfo::hint` on `visible_widgets`/`find_widgets`, and a conditional `"hint"` field on `widget_state` (only when set). `agent_demo` annotates its label, two text fields, and click target.

## [0.10.1-alpha] — 2026-05-06

### Changed
- **Agent query: meaningful widget type names** — added `debug_type_name()` overrides to 17 widgets that previously fell through to the default `"Widget"` string: TextField, TextArea, Checkbox, Switch, Slider, SpinBox, RadioButton, RadioGroup, ProgressBar, List, TagInput, TabBar, FixedSpacer, FlexSpacer, Panel, Divider, ScreenHost. The agent query API (SPEC-HAL-317 Layer 3) now reports each widget's actual type so agents can distinguish interactive widgets from spacers and filter `find_widgets` queries usefully.

## [0.10.0-alpha] — 2026-05-06

### Added
- **SPEC-HAL-317 Layer 3: Agent Query API** — agents can now read live UI state via `AgentAction::Query(AgentQuery)`. Six query types: `VisibleWidgets`, `FocusState`, `TabOrder`, `WidgetTree`, `WidgetState { widget_id }`, `FindWidgets { widget_type, label_contains }`. Responses arrive as `AgentEventKind::QueryResult { query, data }`. New public types: `AgentQuery`, `AgentWidgetInfo`. Strictly read-only (INV-317-2): no mutations to tree, focus, router, or domain state.
- **Tower `Query` control command** — `TowerCommand::Query { query_json }` placeholder; full forwarding through Tower's WebSocket is a Phase 2 enhancement.
- **`tower-ctl query <widgets|focus|tab|tree>`** — forwards a query JSON to the Tower control channel.

## [0.9.1-alpha] — 2026-04-30

### Fixed
- **PropertyEditor: RefCell double borrow** — `activate_editing` held an immutable `core.borrow()` (via `if let Some(prop) = ...nodes.get(&nid)`) while attempting `core.borrow_mut()` for keybind capture. Replaced with a scoped boolean extraction so the immutable borrow ends before the mutable one begins. Pressing Enter on a row no longer panics.

### Changed
- **Caret thickness** — `Theme::caret_thickness` 1.0 → 1.5 px.

## [0.9.0-alpha] — 2026-04-30

### Added
- **PropertyEditorHandle** — external control handle obtained via `PropertyEditor::handle()`. Cloneable; lets a consumer call `set_filter` / `clear_filter` / `invalidate_data` from outside the widget tree (e.g., from a TextField callback).
- **TextField `set_on_change`** — fires after every text mutation (typing, Backspace, Delete, Space, Cut, Paste, Undo, Redo) with the current text. Separate from `set_on_submit`. Enables live-as-you-type behavior in consumer widgets.
- **PropertyEditor demo: live filter** — search bar at the top of the demo wires `set_on_change` to `PropertyEditorHandle::set_filter` for live filtering.
- **PropertyEditor demo: validation** — `SettingsSource` overrides `validate()` for Auto-save Interval, Proxy Port, Font Size, Corner Radius. Invalid commits keep the editor open and render the error inline.

### Changed
- **Caret thickness** — `Theme::caret_thickness` 2.0 → 1.0 px.

## [0.8.0-alpha] — 2026-04-30

### Added
- **PropertyEditor filter API** — `set_filter(query)` / `clear_filter()`. Case-insensitive substring match on labels; section headers retained when any child matches. Consumer provides the UI (TextField, search bar, etc.).
- **PropertyEditor multi-value columns** — backward-compatible `PropertyDataSource::value_columns()`, `cell_value(node_id, column)`, `commit_cell(node_id, column, value)` (defaulted). PropertyEditor builds Name + N value columns from the data source. Existing single-column consumers unchanged.
- **PropertyEditor validation feedback** — `PropertyDataSource::validate(node_id, value)` (defaulted, always-valid). On commit, validation runs first; failure stores `validation_error`, keeps the editor open, and renders the message inline below the editing cell in `theme.colors.semantic.error`.
- **PropertyEditor `invalidate_data()`** — forces a full re-read of `properties()` on the next layout pass for external state changes not reflected in `revision()`.

### Changed
- **PropertyEditorCore** — added `filter_query`, `filter_revision`, `value_column_ids`, `editing_column`, `validation_error` fields. `commit_value()` routes through `commit_cell(editing_column)` and updates `PropertyNode::value` only for column 0. DoubleClick determines the editing column from the click x-position. Editing widget positions in the editing-column slot. Paint iterates all value columns; column 0 reads `PropertyNode::value`, columns N>0 read `data_source.cell_value(nid, N)`.

## [0.7.0-alpha] — 2026-04-24

### Changed
- **ListView expansion heights** — removed show-more/show-less machinery. Expansion areas now size to full natural content height via `ExpandedHeightPolicy::Natural`. Breaking: removed `RowExpansion::is_detail_expanded`, `toggle_detail_expanded`, `clear_detail_expanded` and the `detail_expanded` field.
- **ListView body layout** — measures newly mounted expansion children *before* `resolve_layout_geometry`, eliminating the one-frame overflow glitch on first expand.
- **EventLog** — switched to `ExpandedHeightPolicy::Natural`.

### Fixed
- **Scroll lerp tail snap** — `ScrollContainer::update_visual` snaps visual offset to target when within 0.5 logical pixels, eliminating the sub-pixel jitter tail at the end of smooth scrolls.
- **TextBlock truncation** — paint uses `rect.height` for available vertical space, so children that report natural size render only what fits their allocated rect.



### Added
- **Read-only text selection subsystem** (SPEC-HAL-315 Phase 1+2) — `core::text_selection` module. `ReadOnlyTextSelection` state + `TextSelectionPolicy` (Disabled/Enabled/OnCapture/OnModifier). Free functions `hit_test_run`, `x_for_offset`, `selection_spans`, `find_word_boundaries` operating on `GlyphRun`. Single-click/drag character selection, double-click word, triple-click select-all, shift+click extend, Ctrl+A, clipboard extraction. 17 unit tests. Infrastructure only — no widget integration yet.
- **Sparkline widget** (SPEC-HAL-308) — Phase 1 fill mode. Miniature inline chart via scene graph Path. No GPU pipeline overhead.
- **PlotColorScheme** — Plot-owned dark/light color presets. Plot's visual theme is independent of the widget theme.
- **Chart info coloring** — OHLCV labels dim, values bright, Close colored green/red by candle direction. Instrument name brightened.
- **OrderBook spread percentages** — bid/ask percentage labels at edges of the spread ratio bar.
- **OrderBook mirrored zebra** — stripe pattern continuous across spread row boundary.
- **SPECs 305-308** — Console Widget, Pie/Donut Chart, Gauge Widget, Sparkline Widget.

### Changed
- **OrderBook visual overhaul** — 12px font, 19px rows, no headers, no bottom bar, spread row as green/red ratio bar with centered delta.
- **OrderBook architecture** — rewritten to ScrollArea + OrderBookBody (ListView pattern). Space re-centers on spread. Initial scroll to midpoint.
- **OrderBook hover** — blue tint (`accents.primary.with_alpha(0.25)`) + 3px left-edge accent bar. Reads through depth bars.
- **Plot `[f32; 4]` → `Color`** — all public color fields in the Plot system migrated to `Color` type.

### Fixed
- **LabelConfig defaults** — `from_linear` → `from_srgb` (sRGB values were being interpreted as linear).
- **OrderBook sRGB colors** — replaced `Color::from_linear` with `Color::from_hex` / `Color::from_srgb` throughout.
- **Flash flickering** — bulk guard on flash effects (>30% consumed/new/changed = skip).
- **Measure tool label** — repositioned to top-right corner instead of hypotenuse midpoint.

---

*Changelog coverage between the early-development retrospective below and the later entries above remains to be reconstructed.*

## [Early development] — 2025-12-14

### Added

- **Project and retained UI foundation** — established the Rust 2024 package with `winit` windowing and `wgpu` rendering, then separated core, platform, renderer, runtime, utilities, and widgets. Added the persistent `WidgetTree`, parent/child ownership, mount/unmount hooks, tree-assigned `TreeWidgetId`, separate `WidgetInternalId`, and explicit layout, paint, and event contexts. [1]
- **Application and navigation model** — added `App`/`AppConfig`, `Screen`, `ScreenHost`, screen and modal stacks, enter/leave callbacks, queued navigation commands, rebuild flags, modal-first input handling, Escape dismissal, and pointer shielding. Migrated the examples to the shared application setup. [1]
- **Frame execution infrastructure** — introduced `EngineTicker`, frame reasons, active/idle tracking, render throttling, and frame timing for scrolling and caret animation. Added an optional UI-thread loop, `SceneFrame` transfer through a three-slot `SceneSwapchain`, and a fixed-step high-frequency scheduler configured for 120 Hz fixed steps by default, with capped catch-up work. [1]
- **Layout and basic widgets** — added constraints and box-model primitives; two-pass `Flex::row()`/`Flex::column()` with grow/shrink, alignment, spacing, and wrapping; and `FlowGrid` for responsive rows. Built the initial `Button`, `Label`, `TextBlock`, `Container`, `Panel`, fieldset-style `Legend`, `Divider`, `FixedSpacer`, and `FlexSpacer` set. Dirty-layout propagation, constraint/size caching, and node snapshots supported incremental work. [2]
- **Scrolling and viewports** — added `ScrollArea` and a shared `ScrollContainer` with content/viewport sizes, clamped target and visual offsets, wheel scrolling, interpolation, scrollbar dragging, and gutter-click drag initiation. Added scene scroll nodes, overlay hit regions for scrollbars, propagated viewports, and explicit visible/clip overflow modes. [2]
- **Input routing and focus** — introduced raw-to-fused input processing, pointer/scroll/drag coalescing, multi-click detection, scroll momentum, modifier tracking, capture/target/bubble pointer routing, exclusive pointer capture, and spatial-grid hit-test acceleration. Added focus/blur and hover callbacks, Tab and directional navigation, Enter/Space activation, modal focus scopes, and guarded focus restoration after window reactivation. [3]
- **Actions and theme data** — added an action registry, shortcut dispatch, metadata search and fuzzy ranking, and a modal action palette. Replaced the early theme structure with light/dark colors, typography, interaction settings, and geometry for Compact, Regular, and Comfortable densities. [3]
- **Scene batching and GPU buffer reuse** — added scene flattening, batch keys and deterministic batch sorting, persistent rectangle vertex/index buffers, and buffer reuse when batch structure matched. Structural changes rebuilt geometry buffers. [4]
- **Clipping and initial path rendering** — added nested clip state, cumulative scene scroll transforms, logical-to-physical GPU scissors, and per-corner rounded clipping in the rectangle shader with configurable antialiasing. Introduced path commands, quadratic/cubic curve flattening, simple-polygon fill triangulation, stroke meshes, GPU path buffers, and renderer preparation/draw encoding for path transforms, colors, clip IDs, and stroke flags. [4]
- **Glyph atlas and renderer bookkeeping** — added padded glyph storage, skyline packing, atlas growth/repacking, and cache keys covering font identity, size, glyph, DPI, format, and subpixel phase. Added frame epochs, GPU-resource metadata, staging-pool infrastructure, atlas identity tracking, surface recovery, and rectangle/text pipeline and binding revalidation. [4]
- **Text shaping, fonts, and paragraph layout** — introduced rustybuzz shaping, glyph clusters, advances and offsets, shape caching, font assets and identities, family/weight/style resolution, and a paragraph fallback API. Added hard line breaks, wrapping, alignment, paragraph runs, `TextBlock`, and cached editable-text layout. Added CoreText and DirectWrite rasterizers, a FreeType backend, and a Glyphon fallback path for WebAssembly. Windows and macOS SystemUI loading used the corresponding native font APIs, with embedded Inter retained as an explicit fallback. [5]
- **Shared text-editing substrate** — added the text document, cursor, selection, cluster boundaries, insertion/deletion operations, word/line/paragraph movement and selection, vertical navigation, caret affinity, and hit testing. Added invariant suites for UTF-8 boundaries, cursor positions, selection anchors, hard breaks, soft wraps, deletion modifiers, and preferred horizontal position. [5]
- **TextField and TextArea** — implemented the single-line and multiline controls on the shared editing substrate. Added text insertion/deletion, keyboard navigation and selection, pointer caret placement, drag and word selection, caret blinking, TextField horizontal caret visibility, and TextArea wrapping, vertical navigation, auto-height work, clipped painting, and scroll-to-caret behavior. [5]
- **Clipboard, history, and shared editing policy** — connected OS text copy/cut/paste through `EventContext` and an arboard-backed clipboard. Added undo/redo snapshots for text, cursor, and selection, compatible-edit merging, redo invalidation after a new edit, and newline normalization for single-line paste. Consolidated common behavior in `text_edit_delegate`. Added `TextEditingPolicy` for single-line/multiline differences, and `TextEditPolicy` with `decide_text_edit_policy` for enabled/read-only decisions on focused Key/Text events. Composition-event handlers and undo bookkeeping were also introduced, with the platform limitation recorded below. [5]
- **Architecture documents, examples, and regression suites** — established the RFC collection and subsequently consolidated the Constitution, amendment rules, and architectural contracts. Added examples for responsive grids, scrolling, navigation/modals, actions, shaping, text quality, and editable controls. Added unit and integration suites for widgets, layout, input fusion, focus, clipping, paths, text editing, and TextArea routing/scroll/paint/pointer behavior. [1–6]

### Fixed and stabilized

- **Coordinate and DPI handling** — corrected stale pointer positions, layout-container interception, logical/physical resize and scissor conversions, glyph rasterization scale, absolute geometry propagation, and scroll-aware hit testing. Consolidated coordinate work in layout/traversal and platform conversion paths. [2–4]
- **Layout and scrolling interactions** — corrected second-pass Flex constraints, FlowGrid sizing, Legend label geometry, ScrollArea width/viewport handling, scrollbar drag mapping, and animation scheduling during scrolling. [1–3]
- **Text geometry and layout** — corrected text measurement/cache invalidation, baseline and bearing placement, glyph clipping/padding, repeated scaling/snapping, cluster offsets when slicing runs, and paragraph hard-break ordering. Added regression coverage for layout after Enter and hard line breaks. [4–5]
- **Editing interaction** — corrected caret placement and selection geometry, capture/blur cleanup, UTF-8 boundary handling, platform-specific word-navigation modifiers, vertical preferred-position preservation, and TextArea event routing and caret visibility. Added tests for deletion modifier semantics and selection behavior at line boundaries. [3, 5]
- **Frame and window behavior** — corrected initial redraw handling, render-time accounting for idle throttling, window focus handling, and focus restoration eligibility. [1, 3–4]

### Historical limits

- **Renderer work was still partial.** Rectangle buffer reuse performed full geometry overwrites rather than fine-grained changed-range uploads. Rectangle, path, and text used separate draw stages within the main render pass; deterministic batch sorting did not establish unified painter order across those stages. Several shape/image variants and offscreen layers remained scaffolds. Rounded clipping did not mean rounded primitives and borders were fully rendered, and the initial path pipeline did not establish general multi-path correctness. GPU lifetime tracking primarily managed metadata, not ownership of GPU handles. [4]
- **Font coverage was platform-dependent.** Windows/macOS SystemUI discovery was implemented; Linux SystemUI discovery was not. FreeType still loaded a fixed DejaVu Sans path instead of the bootstrap font bytes. Paragraph fallback existed as an API, while the display/editing widgets still used their single-shaper layout route. Complete Unicode bidi layout and a finished LCD/subpixel compositor are not claimed for this period. [5]
- **Native IME remained unfinished.** The widgets contained composition-event and atomic-history helpers, but pre-edit text was not rendered and the platform adapter did not yet construct/route native IME events into them. [3, 5]
- **Verification is historical source reconstruction.** The listed tests were present in the dated source. This entry does not report a new execution of those historical builds or tests, or assign later platform acceptance to the early implementation.

### Historical sources

The references identify files as they existed in the **2025-12-14** source snapshot, together with their dated development record; they do not substitute today's file contents or specifications for that historical state.

1. `Cargo.toml`; `src/core/app.rs`; `src/core/widget.rs`; `src/core/widget_tree.rs`; `src/core/scene.rs`; `src/core/scheduler.rs`; `src/utils/id.rs`; `src/lib.rs`; `examples/rfc018_demo/`.
2. `src/core/layout.rs`; `src/core/widget_tree.rs`; `src/core/scroll_container.rs`; `src/widgets/mod.rs`; `src/widgets/flex.rs`; `src/widgets/flow_grid.rs`; `src/widgets/scroll_area.rs`; `src/widgets/scrollbar.rs`; `src/widgets/legend.rs`; `examples/flow_grid_megatest.rs`; `examples/flow_grid_legends.rs`.
3. `src/platform/window.rs`; `src/core/event.rs`; `src/core/event_router.rs`; `src/core/input_fusion.rs`; `src/core/hit_test.rs`; `src/core/theme.rs`; `src/runtime/actions.rs`; `src/runtime/builtins.rs`; `src/core/action_palette_modal.rs`; `src/widgets/action_palette.rs`; `tests/event_routing_tests.rs`.
4. `src/core/scene.rs`; `src/renderer/batcher.rs`; `src/renderer/wgpu_renderer.rs`; `src/renderer/glyph_atlas.rs`; `src/renderer/gpu_arena.rs`; `src/renderer/staging_pool.rs`; `src/renderer/path.rs`; `src/renderer/path_renderer.rs`; `src/renderer/pipelines/path_pipeline.rs`; `src/renderer/shaders/rect.wgsl` and the renderer modules' inline tests.
5. `src/core/text_shaper.rs`; `src/core/paragraph.rs`; `src/core/editing.rs`; `src/core/editing_history.rs`; `src/core/text_edit_delegate.rs`; `src/core/text_edit_policy.rs`; `src/platform/font_manager.rs`; `src/platform/font_resolver.rs`; `src/platform/font_manager_text_engine.rs`; `src/platform/system_font_provider.rs`; `src/platform/native_font_backend.rs`; `src/platform/native_font_backend_coretext.rs`; `src/platform/native_font_backend_directwrite.rs`; `src/platform/native_font_backend_freetype.rs`; `src/platform/native_font_backend_glyphon.rs`; `src/platform/clipboard.rs`; `src/widgets/text_document.rs`; `src/widgets/text_layout_cache.rs`; `src/widgets/text_block.rs`; `src/widgets/text_field.rs`; `src/widgets/text_area.rs`; the corresponding `tests/` suites.
6. `docs/rfcs/RFC-HAL-002-Constitution-and-Architectural-Invariants.md`; `docs/rfcs/RFC-HAL-002A-Constitution-Amendment-Guidelines.md`; the early `docs/rfcs/` collection and `examples/`.
