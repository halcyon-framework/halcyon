# SPEC-HAL-239: System File Dialog Bridge — Intent and Authority

**Author:** JC
**Status:** Draft — independent acceptance required
**Implementation status:** Not Implemented
**Layer:** Application Runtime / Native OS Mediation
**Proposed scope:** Single existing-file Open for the canonical application's primary window
**Revision date:** 2026-10-07

---

## 0. Purpose

This revision proposes one explicit native capability: choose one existing file
and deliver an owned, correlated result to a registered application consumer.
The OS supplies chooser UI and native transport. Halcyon retains authority over
its own routing, focus, capture, modality transitions, and result lifetime.

All normative language below describes the proposed contract after acceptance;
it does not activate authority or callable APIs. The dependency clarifications
in §7 require independent approval before implementation. The original draft's
Save, directory, and multi-selection scope remains deferred and unadmitted.

### 0.1 Governing owners and live baseline

SPEC-HAL-002/002A remain supreme. SPEC-HAL-139 §§D/E supplies explicit OS
capability and native confirmation mediation; SPEC-HAL-138 §§D/E owns the
intent/validation/confirmation/resolution pipeline. SPEC-HAL-082A limits native
windows to transport. SPEC-HAL-151 owns interaction domains and explicitly
defers OS modality beyond focus gating. SPEC-HAL-127/128/137/137B/140/176/328
retain their focus, capture, routing, and keyboard authority. SPEC-HAL-162
retains teardown-before-delivery and no-postmortem rules.

At `552f8d61d68bcc6f39179062789e47b3317c75e7` / `0.204.1-beta`,
`AppContext` and `ScreenContext` expose no native file-selection operation.
`AppRoot::flush_pending_actions_all` supplies the existing deferred screen
handling boundary; it does not retain results for screens that were removed.
`App::run` owns native windows and the `EventLoopProxy<()>` wake path.
`AppEvent` currently has only `Custom(&'static str)`. The generic confirmation
types described by HAL-138 are not an implemented public pipeline. These facts
identify extension points, not an already available picker.

---

## 1. Invariants

### INV-239.1: OS UI Is Canonical

The native backend owns chooser presentation and native handles. The chooser
is not a Halcyon `Dialog`, widget, overlay subtree, or second UI system.

OS parentage, focus notifications, or native window disabling MUST NOT decide
Halcyon focus, capture, routing, or resumption. This preserves
`INV-SOV-OS-TRANSPORT-ONLY`, `INV-SOV-SEMANTIC-OWNERSHIP`, and
`FORBID-SOV-AUTHORITY-BLEED`.

### INV-239.2: Intent-Driven Invocation

Only an explicitly enabled runtime capability and a live registered consumer
may request Open. An ordinary Button may record application intent; its widget
callback MUST NOT instantiate or call a native chooser. The application submits
the request from deferred handling through its `AppContext`.

No native API or product-local picker is a lawful substitute for this boundary.

### INV-239.3: Async and Non-Blocking

Request admission returns immediately. Native waiting MUST NOT block Halcyon's
event loop, input handling, update, layout, paint, or rendering. Completion is
edge-triggered through an owned pending record and the canonical event-loop
wake; timers, per-frame polling, and continuous-frame demand are forbidden.

Every admitted request resolves exactly once to `Selected`, `Cancelled`, or
`Failed`. Pre-admission rejection has no request and no later completion.
Dismissal is cancellation, never a null value or operational failure.

### INV-239.4: Scope-Limited Access

Selection conveys only the access result defined in §6. It MUST NOT imply
readability, persistent permission, schema validity, or a stable file identity.
Halcyon does not open/read/decode the selected artifact for the application.

An unsupported access profile rejects before admission. An owned path MUST NOT
masquerade as a sandbox access token or an already opened file handle.

### INV-239.5: No Visual Composition

The chooser has no Halcyon layout, paint, clip, Z, hit-test, or component
identity. Native presentation cannot bypass the engine-owned input gate in §5.

### INV-239.6: Durable Correlation

A request belongs to a runtime, primary `WindowId`, and registered consumer,
not to an initiating Screen or Widget. Navigation MUST NOT discard its result.
At most one terminal result is taken for a request; duplicate/late native
records cannot replace that result or target a dead UI object.

### INV-239.7: Phase and Lifetime Separation

Native callbacks carry only owned data and opaque correlation identities.
They MUST NOT retain or mutate `WidgetTree`, Widget, Screen, `ScreenContext`,
`AppContext`, or application `Rc` state. Only the engine's legal deferred phase
publishes results and invokes live application handling. Capture is never
resurrected by native completion or focus gain.

---

## 2. Proposed Public Boundary

The following names/signatures are **proposed symbols, not existing APIs or
runnable examples**. They extend current application owners; no generic job,
intent registry, or alternate event framework is introduced.

| Proposed surface | Exact responsibility |
| --- | --- |
| `AppConfig::with_existing_file_open(self) -> Self` | Explicit capability opt-in before `App::run`; disabled by default. |
| `AppContext::register_file_open_consumer(&mut self) -> Result<FileOpenConsumer, FileOpenAdmissionError>` | Register an application-owned consumer bound to this runtime's primary window. |
| `AppContext::request_open_existing_file(&mut self, consumer: &FileOpenConsumer, options: OpenExistingFileOptions) -> Result<FileOpenRequestId, FileOpenAdmissionError>` | Validate and reserve one request/result slot; enqueue native work without showing UI in the caller. |
| `FileOpenConsumer::take_result(&mut self) -> Option<FileOpenCompletion>` | Move the completed owned result out once; pending/empty returns `None`. Use on a ready notification or deliberate screen entry, never as a polling loop. |
| `AppEvent::FileOpenReady { consumer_id: FileOpenConsumerId, request_id: FileOpenRequestId }` | Typed deferred notification through existing `Screen::handle_app_event` for the current live application screen; carries no borrowed UI or native state. |

`FileOpenConsumer` is an opaque, non-clone application-thread handle. The
application retains it in state that outlives individual Screens. It contains
no UI references or native chooser handles. Dropping it retires its registration.
`FileOpenConsumerId` and `FileOpenRequestId` are opaque, runtime-qualified,
checked identities, never raw widget IDs or reusable screen addresses.

`OpenExistingFileOptions` contains only an optional owned title in this profile.
Absent title uses native default wording. Unrepresentable titles, including
embedded NUL, reject as `InvalidOptions`; no truncation or implicit substitution.
Directory hints, extension/type filters, accessory controls, and other modes
require a later explicit contract; none are needed to read a selected file.

`FileOpenCompletion` contains `consumer_id`, `request_id`, the originating
`WindowId`, and `FileOpenOutcome`. Its closed outcome surface is:

- `Selected(SelectedFile)`: exactly one owned native filesystem `PathBuf`.
- `Cancelled(FileOpenCancellation)`: `Dismissed`, `ConsumerRetired`,
  `WindowClosed`, or `RuntimeDestroyed`.
- `Failed(FileOpenFailure)`: a typed `BackendUnavailable`, `NativeFailure`, or
  `InvalidNativeResult`, with owned platform error evidence where available.

`FileOpenAdmissionError` distinguishes `CapabilityDisabled`,
`UnsupportedPlatform`, `UnsupportedAccessProfile`, `UnsupportedWindow`,
`RuntimeClosed`, `WindowClosed`, `ForeignConsumer`, `ConsumerRetired`, `Busy`,
`ResultPending`, `InvalidOptions`, and `IdentityExhausted`. Failure MUST NOT
queue native work, reserve an ID/result slot, or emit a ready notification.
Capability/access-profile rejection is decided before request admission;
native initialization failure after admission is a terminal `Failed`.

---

## 3. Request, Completion, and Retirement

### 3.1 Bounded admission

There is one active native Open operation per canonical runtime, bound to its
primary window. Other windows reject as `UnsupportedWindow`; another active
request rejects as `Busy`. There is no waiting list. A consumer with an
unconsumed result rejects a new request as `ResultPending`.

Admission validates capability, supported access profile, live owner, options,
and identity capacity, then atomically reserves both the operation and its
terminal slot. Storage is one slot per explicitly registered live consumer,
not a growing result history. The runtime remains busy until the operation is
terminal **and** its native UI/backend has quiesced. Taking a terminal result
alone does not authorize a second chooser while the first is still closing.

### 3.2 Pipeline and legal processing

1. Deferred application handling submits the explicit selection intent. The
   capability/owner/options checks are its bounded HAL-138 validation step.
2. Before native presentation, the engine establishes §5's input gate and
   capture/focus transitions. `App::run`, using its actual parent window
   transport, asks the framework-owned backend to present the chooser.
3. Native Choose is explicit confirmation of **selection only**. Native Cancel
   or dismissal cancels that intent. No timeout, focus change, or successful
   presentation confirms selection. No separate in-tree confirmation is added.
4. The backend copies the result into an owned, correlated pending record and
   wakes normal processing with the existing `EventLoopProxy<()>` mechanism.
   It cannot mutate engine/application state even on a main-thread callback.
5. The canonical application deferred service drains records after teardown
   reconciliation and before screen pending-action handling/layout. It resolves
   the request atomically, publishes the consumer's terminal slot, and issues
   the typed ready notification only to current live application handling.
   Result consumption may enqueue normal navigation; rebuilding occurs only in
   the existing legal mutation phases.

This service MUST run on the wake even if the parent is unfocused, no animation
is active, or a retained scene is reused. `UserEvent(())` currently only requests
render progress; implementation must add the owned deferred drain, not assume
that scheduling a redraw publishes the result.

The ready notification is a hint to take a durable result, not its sole storage.
Screen replacement neither changes the consumer/request identity nor consumes
the result. A later Screen may take it on entry if the earlier hint had no live
recipient. A dead Screen is never called. The current drop-oldest `SignalBus`
queue and dead-widget-filtered intent buffers MUST NOT be the sole result store.

### 3.3 Terminal races and release

The engine-owned request state admits one terminal transition. Teardown is
reconciled before pending native completions: if the owner is already closing,
cancellation wins over an unprocessed native selection. A previously published
terminal result is never overwritten. Duplicate, stale, or foreign records are
discarded with their owned payloads released; they cannot cause another take or
ready notification.

- Dropping the consumer retires it, cancels a pending operation, closes native
  presentation asynchronously, and releases any unconsumed result. There is no
  callback to the destroyed consumer; its terminal cancellation is discarded
  with that registration.
- Window/runtime closing rejects new work and terminates admitted pending work
  as `WindowClosed`/`RuntimeDestroyed`. An independently surviving consumer
  retains that terminal result, or an already published result, until one take
  or drop. It retains no runtime, window, worker, or native UI handle.
- Backend closure and callback retirement are explicit. Final native parent
  teardown waits for backend quiescence through normal event-loop processing,
  never by blocking an engine callback. No detached chooser/worker may keep an
  owner alive or publish into a retired queue. Late records release their data.

Application import/read/decode/install is outside this request lifetime.
The consuming application owns its broader one-import-in-flight policy and
decides when, or whether, to install the selected content.

---

## 4. Existing Integration Owners

`AppConfig` owns opt-in; `AppContext` owns registration/admission; `App::run`
owns native transport, synchronized completion ingress, and wake servicing.
`AppRoot::flush_pending_actions_all` and `Screen::handle_app_event` are the
existing application-handling integration points. `ScreenContext` borrows the
existing `AppContext`; it does not become an asynchronous callback target.

Capture cancellation reuses `EventRouter::cancel_all_pointer_sessions_with_tree`
and the existing capture-leave lifecycle. Focus transitions remain with
`FocusManager`, the active interaction domain, and current input-root authority.
The backend never receives those owners. No renderer/Image admission mailbox,
generic job scheduler, or parallel UI tree is repurposed for file selection.

The source's native child-window confirmation path is not a native file chooser
and does not supply this operation's durable application result lifetime.

---

## 5. Native Presentation and Halcyon Input Authority

The admitted operation gates the primary window from request service until
terminal/native quiescence. The gate runs before Screen/widget routing,
application/global shortcuts, and native-menu action dispatch. It prevents new
application input while preserving resize/redraw, lifecycle, close, wake, and
completion processing. OS parent disabling alone is insufficient.

Before presentation, the engine cancels pointer captures, gesture sessions,
and relative-pointer ownership with existing lifecycle notifications. Keyboard
capture leaves through its existing lifecycle; the domain returns to Navigation.
Focus is suspended/cleared through the central owner, never copied into a new
widget or backend focus cache. Capture visuals receive ordinary invalidation.

Tab, Escape, text, and pointer input while the chooser is active belong to its
native UI and MUST NOT also trigger Halcyon widgets or app shortcuts. Native
Escape/dismissal maps to `Cancelled(Dismissed)`; it cannot also exit Halcyon.
An engine window-close/shutdown request remains lawful and follows §3.3.

Completion never restores pointer, relative-pointer, gesture, or keyboard
capture. On native quiescence the gate may be removed, but normal application
input still requires the engine's active-window/focus gate. OS focus gain alone
does not resume capture or override a still-active native operation.

When the parent is legally active again, central focus restoration may return
to the previously focused live eligible widget **only in the same current input
root/domain**. If navigation, a new modal root, or teardown invalidated that
target, keep the current lawful focus or leave it unset for normal focus entry.
No stale Screen/widget is revived and no fallback focus/capture is guessed.
This is a native-Open scoped policy proposal, not HAL-328 transient-overlay
capture suspension/resumption.

## 6. Access Result and Native Backend Profile

### 6.1 Initial access profile

The proposed initial targets are ordinary **non-App-Sandbox macOS desktop**
and **non-brokered Win32 desktop** processes. The backend must establish that
this profile applies before admission; an unknown or unsupported profile
rejects as `UnsupportedAccessProfile`. Linux, App Sandbox/security-scoped
access, and brokered/packaged access are not silently treated as equivalent.

`SelectedFile` owns a lossless platform `PathBuf`, copied before native result
objects are released. Its lifetime is independent of the chooser, Screen,
consumer, and runtime. There is no bridge-created access token, open handle,
permission lease, bookmark, or expiration clock. The application may perform
the subsequent user-authorized read using its existing process permissions.
Selection does not elevate them or guarantee that read will succeed: ACL/TCC,
file removal/replacement, sharing, and I/O errors remain application failures.
No bridge TOCTOU guarantee or persistent file identity is claimed.

Native file selection constrains the chooser to one existing filesystem file;
it does not promise that the same path still names a readable regular file when
the application opens it. The application owns bounded byte capture, schema
validation, normalization, and installation. Supporting a target that needs an
access lease instead requires a separately accepted extension to this result
contract before that target is admitted.

### 6.2 Proposed framework-owned native mechanics

These are platform design choices grounded in native API documentation, not
implemented Halcyon backends or native proof.

**Windows:** use `IFileOpenDialog` with `FOS_FORCEFILESYSTEM`,
`FOS_FILEMUSTEXIST`, and `FOS_PATHMUSTEXIST`; omit multi-select and folder mode.
Give `Show` the actual parent HWND. Execute its modal wait in a backend-owned
COM STA with message dispatch, not on Halcyon's event-loop thread. Cancellation
must reach `IFileDialog::Close` on its owning apartment; COM interfaces cannot
cross apartments without marshalling. Only copied IDs, owned data, wake, and
the native cancellation channel cross the boundary.
[Microsoft options](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/ne-shobjidl_core-_fileopendialogoptions),
[STA rules](https://learn.microsoft.com/en-us/windows/win32/com/single-threaded-apartments).

Map `Show`'s `HRESULT_FROM_WIN32(ERROR_CANCELLED)` to dismissal, and other
failure HRESULTs to typed native failure. On success, obtain the single result
and `SIGDN_FILESYSPATH`, copy its UTF-16 into owned Windows path storage without
lossy Unicode conversion, then release the COM string with `CoTaskMemFree` and
release interface objects on their owner. `Close` is cancellation, not success
with a missing result.
[Show result](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-imodalwindow-show),
[path ownership](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellitem-getdisplayname),
[Close contract](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-ifiledialog-close).

**macOS:** use AppKit `NSOpenPanel` on the main thread, attached asynchronously
to the actual parent NSWindow with `beginSheetModalForWindow:completionHandler:`.
Explicitly allow files only, disable multiple selection and directory creation,
and treat file packages as directories. On accepted selection, require one
filesystem URL and copy its filesystem representation into an owned native
path while the URL is live. On dismissal, report cancellation. Native callback
processing only queues the correlated record. Dismiss/order out the panel on
the AppKit owner before declaring native quiescence.
[Apple Open-panel flow](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/UsingtheOpenandSavePanels/UsingtheOpenandSavePanels.html),
[sheet completion](https://developer.apple.com/documentation/appkit/nssavepanel/beginsheetmodal(for:completionhandler:)),
[filesystem representation](https://developer.apple.com/documentation/foundation/nsurl/filesystemrepresentation).

Current source already uses `winit` window transport and main-thread
`objc2`/AppKit menu projection on macOS. It has no implemented chooser backend;
the current direct Windows dependencies do not establish `IFileOpenDialog`
support, nor do the existing AppKit feature selections establish Open-panel
support. Required target bindings belong to later framework implementation.
No consumer-local native call or third-party picker is authorized by this draft.

## 7. Required Independent Authority Acceptance

This draft cannot override accepted owners by implication. The following exact
clarifications must be independently approved and, where necessary, reflected
in their owning clauses before implementation. No other SPEC is edited here.

| Existing owner/clause | Proposed bounded clarification requiring acceptance |
| --- | --- |
| HAL-239 scope and public boundary | Adopt only §§1–6's primary-window single-file profile, durable consumer/result lifetime, explicit capability, and initial desktop path-access targets. Save/directory/multi-select remain deferred. |
| HAL-138 §D.3 and §E | For this native Open operation, an engine-gated external native chooser may substitute for the in-tree confirmation overlay. “Input capture enforced” means exclusive operation gating, not a Widget pointer/keyboard capture owner. Preserve validation and atomic selection resolution; no generic confirmation framework adoption. |
| HAL-139 §§D/E | Recognize this explicit capability and HAL-138 specialization. Native UI confirms selection only; application read/install is a separate decision. OS UI cannot own engine focus/capture/modality. |
| HAL-151 §§I/P/Q and primary-only §O.1 | Admit only this operation's primary-domain native input inhibition/resumption despite the existing deferred OS-modal scope. Do not claim general multi-window modal trees or native dialog hierarchy implementation. |
| HAL-176 §9.4; HAL-328 §§3.9/3.10; HAL-137/137B/140 | For this explicit native operation, release capture before presentation and never resume it on completion/focus gain. Preserve ordinary focus-loss rules and central live-target restoration; the in-tree transient-overlay suspension rule does not apply. |

HAL-002/002A, HAL-082A, and HAL-162 require no amendment: native transport,
legal mutation, sole tree ownership, and teardown-before-delivery remain intact.
HAL-235/236 are Draft at the inspected baseline; reusing implemented central
focus machinery does not adopt their broader proposed contracts.

JC's remaining decision is acceptance of this precise profile and the listed
dependent clarifications. If sandboxed/brokered access is required for the first
consumer, the proposed path-only profile is insufficient and must be revised
before adoption. No runtime work is authorized while these clauses are Draft.

## 8. Required Future Implementation Proof

Owner-local mechanical proof should extend the colocated `src/core/app.rs`
application/screen lifecycle tests and the existing
`src/tests/interaction_domain_isolation.rs` / `src/tests/focus_authority.rs`
surfaces for affected domain transitions. Native backend tests remain with their
platform owner; a fake backend can prove queue/lifecycle behavior only.

The minimal coherent implementation must prove:

- explicit opt-in and atomic pre-admission rejection; one active operation,
  reserved result slot, and reopen after terminal/native quiescence plus take;
- exactly one terminal result/one take despite duplicate completion, navigation,
  consumer retirement, and window/runtime shutdown; initiating Screen removal
  cannot lose a result for a surviving consumer;
- owned-only native ingress, no postmortem UI callback, explicit native/payload
  release, and deferred wake service while unfocused or reusing retained scenes;
- pointer/relative/keyboard capture cancellation, gated widget/shortcut/menu
  dispatch, legal central focus return, and no capture resurrection;
- lossless owned path lifetime beyond native result release, with read/schema
  errors remaining with the consumer.

Separate **real Windows and macOS native runs** must each show selection,
dismissal/Cancel, operational error handling, repeat Open, and engine event-loop
progress during native presentation. Each run must also show navigation while
an operation is pending, surviving-consumer result retrieval, and owner/window
shutdown without duplicate completion or orphaned native UI. Operational-error
proof must identify a real failing native call or a declared backend fault
injection at that native boundary; user cancellation cannot stand in for error.
Record target OS/access profile and distinguish mechanical from native evidence.
macOS success proves nothing about Windows or Linux. No such proof exists yet.

## 9. Out of Scope

Save, directory/multi-selection, asset browsing, filter/accessory DSLs,
portable security-token emulation, automatic grants, file loading/decoding,
product import semantics, a generic job/confirmation scheduler, modal rewrites,
input/response timing, ActionId allocation, and new UI/rendering subsystems are
not admitted. Platform chooser navigation does not authorize Halcyon to load
cloud content, resolve assets, or parse application artifacts.
