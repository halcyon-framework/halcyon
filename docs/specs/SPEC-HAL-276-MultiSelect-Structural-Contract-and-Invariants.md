# SPEC-HAL-276 v3: **TagInput — Multi-Value Selection and Tag Creation Widget**

### *Unified Specification: Selection, Filtering, Ghost Completion, Custom Tags, Polish*

**Author:** JC
**Status:** Implemented
**Version:** 3 — updates v2 to reflect implemented polish features
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-136 (Transactional Selection and Explicit Commit Semantics)
* SPEC-HAL-199 (Popup & Anchored Selection Controls)
* SPEC-HAL-265 (Icon System — multi-layer icons for affordances)

**Supersedes:** SPEC-HAL-276 v1, SPEC-HAL-276 v2, SPEC-HAL-276A, SPEC-HAL-276B, SPEC-HAL-276C, SPEC-HAL-277

---

# 1. Purpose

TagInput is the multi-value selection widget. It is the counterpart to ComboBox
(single-value). It selects zero or more items from a predefined list, AND/OR
accepts custom user-created tags, depending on policy.

Use cases: filters, permissions, category assignment, tag selection, feature
toggles, role assignment, email recipients, GitHub labels, Jira tags.

---

# 2. Architectural Decision: New Widget, Not ComboBox Mode

ComboBox is single-select. TagInput differs in every dimension:

| Aspect | ComboBox | TagInput |
|--------|----------|----------|
| Selection | One item | Zero to many |
| Display (closed) | Single text value | Chips / count / summary |
| Popup click | Selects + closes | Toggles + stays open |
| Commit | Implicit on select | Immediate per-action |
| Enter key | Commits highlighted | Toggles highlighted |

---

# 3. Composition Architecture

**INV-276-ARCH1: No Text Reimplementation**

TagInput MUST NOT reimplement text editing. No manual caret rendering, no
manual text shaping for input, no manual cursor tracking. The framework's
TextField handles all text editing.

**INV-276-ARCH2: Composition Pattern**

TagInput owns the visual shell, host semantic policy, chips and ghost-completion projection. WidgetTree alone owns its one real TextField child for single-line text entry. The host retains a sealed copied relation and source-qualified owned observations, never a TextField Widget or child-state borrow. Engine traversal paints the child; TagInput shall not call inline_input.paint or render another input caret/text implementation.

**INV-276-ARCH3: Rect Handoff**

Ordinary Layout arranges the real text-entry child in the remaining region after chips through the existing child layout/rect APIs. It does not create/remove the child, settle semantic commands or borrow child editing state. Paint consumes the actual child geometry; changed chip inset or incompatible source/geometry suppresses old motion canonically. No Paint-computed semantic region is published as pre-Update authority.

**INV-276-ARCH4: Event Delegation**

Existing TagInput policy owns its intercepted tag/popup commands. Text operations are delegated only through the engine-qualified section 2.4 session. Each operation's owned current result settles before refiltering, ghost recomputation, tag creation/toggle/clear, callback or next dependent event. Preserve original input provenance and current same-batch command order; no host direct handle_event call, child reference or next-frame value substitution is admitted.

**INV-276-ARCH5: Shared Inline-Entry Presentation.** Only the current single-line inline text-entry region may consume HAL-101 section 8.4.6 after lawful ownership admission. TagInput continues to reuse the same editing/text presentation implementation and must not draw an independent caret or reshape substrings. Current chip layout and inline origin/viewport qualify every consuming Paint; changed chip inset, line arrangement, ghost completion or source association that cannot preserve the exact admitted presentation snaps canonically. Tag creation/selection, popup behavior, filter and ghost-completion semantics remain unchanged. This addition does not legalize a host-owned Widget or activate a multiline transition.

**FORBID-276-ARCH5.** No transition across chip reflow, animation of ghost-completion or tag meaning, reused old session source, or new tag-specific editing engine.

The closed host adapter preserves this widget's current command, resolver/value/tag, callback and clear/reset order under HAL-101 section 2.5; ordinary standalone TextField Enter/Cancel shall not replace host policy. Runtime child-dependent API calls require the scoped owner context; no no-context peer replacement or silently queued sync is admitted.

---

# 4. Data Model

```rust
pub struct TagItem {
    pub id: u64,
    pub label: String,
    pub group: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TagEntry {
    Item(u64),
    Custom(String),
}
```

**INV-276-D1: External Item List**
Items provided via `set_items(Vec<TagItem>)`. Widget does not generate items.

**INV-276-D2: Selection Is a Set**
Selection is `HashSet<TagEntry>`. No duplicates within the same variant.

**INV-276-D3: Working Selection**
Working selection cloned from committed on popup open. Both sets stay in sync
through immediate commit on every mutation. The working set exists for
consistent paint-time references during popup-open rendering.

**INV-276-D4: Selection Order**
Parallel `Vec<TagEntry>` preserves insertion order for chip display.

**INV-276-D5: Disabled Items**
`enabled: false` items are visible but not toggleable. If previously selected,
they remain selected but cannot be deselected through the UI.

**INV-276-D6: Group Support**
`TagItem.group` is an optional string. Items with the same group render under
a shared header in the popup. Items are sorted by group in `refilter_from_input`.
Group headers are non-interactive — navigation skips them.

---

# 5. Tag Creation Mode

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagCreationMode {
    #[default]
    SelectOnly,    // Select from predefined items only
    FreeFormOnly,  // Custom tags only, no popup, no predefined list
    Hybrid,        // Select from list + create custom tags
}
```

**INV-276-TC1: Mode Behavior Matrix**

| Mode | Popup on capture | Type-to-filter | Ghost completion | Enter (no match) | Inline input |
|------|-----------------|----------------|------------------|-----------------|-------------|
| SelectOnly | Yes | Yes | Yes | No-op | Yes (filter) |
| FreeFormOnly | No | N/A | No | Create tag | Yes (create) |
| Hybrid | Yes | Yes | Yes | Create tag | Yes (both) |

---

# 6. Interaction Policy

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagInputPolicy {
    #[default]
    Full,        // All interactions: popup, toggle, chip ×, filter
    ToggleOnly,  // Popup + toggle only, no chip ×, no filter
    Locked,      // Display only, no interaction, chips without ×
    ReadOnly,    // Plain text display, not focusable
}
```

**INV-276-POL1:** Full — all interactions active.
**INV-276-POL2:** ToggleOnly — popup + toggle, no filter, no chip ×.
**INV-276-POL3:** Locked — visual chips, no interaction.
**INV-276-POL4:** ReadOnly — plain text, not focusable.

---

# 7. Display Modes (Closed State)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagDisplayMode {
    #[default]
    Chips,       // Tag chips with × buttons
    CountBadge,  // "N selected"
    Summary,     // "Tag A, Tag B + 2 more"
}
```

**INV-276-CH1:** Chips flow left-to-right. Overflow: "+N more" badge.
**INV-276-CH2:** Chip × deselects immediately (direct commit, no popup).
**INV-276-CH3:** Empty + not captured → placeholder text.
**INV-276-CH4:** Chip × shows hover feedback (circular background + darkened ×
text) when moused over. Provides clear affordance that × is clickable.

---

# 8. Unified Input Model

**INV-276-UI1: One Input, One Popup, Zero Zones**

There is ONE real WidgetTree-owned TextField child under HAL-101 section 2.4 positioned after chips.
It simultaneously: accepts typed text, filters the popup, shows ghost
completion, and creates custom tags on commit.

The popup is a dumb filtered list. No input elements. No zones. No mode
switching.

**INV-276-UI2: On Capture Enter**

1. The engine admits the current delegated text session and notifies the real child under section 2.4; its qualified caret becomes available without creating independent child Focus/Capture. TagInput does not call the child's capture/focus callbacks directly.
2. Popup opens (SelectOnly, Hybrid) or stays closed (FreeFormOnly)
3. All items shown (no filter yet), first enabled item pre-highlighted
4. User can immediately type or use arrow keys

**INV-276-UI3: Keyboard Routing**

| Input | Action |
|-------|--------|
| Character keys | Insert into inline input → popup filters live → ghost recomputed |
| Down arrow | Highlight next item (opens popup if closed, for Hybrid) |
| Up arrow | Highlight prev item (opens popup if closed, for Hybrid) |
| Enter (highlighted item) | Toggle item. Clear input. Refilter. Stay open. |
| Enter (no highlight, non-empty, Hybrid/FreeFormOnly) | Create custom tag. Clear input. Refilter. Stay open. |
| Enter (no highlight, non-empty, SelectOnly) | No-op. |
| Enter (empty, no highlight) | Close popup. Commit. Release capture. |
| Backspace (non-empty) | Delete character. Refilter. |
| Backspace (empty) | No-op. Tags removed via × only. |
| Ctrl/Alt+Backspace | Word-delete backward (wired to `apply_delete_word_back`). |
| Ctrl/Alt+Delete | Word-delete forward (wired to `apply_delete_word_forward`). |
| Escape (non-empty input) | Clear input. Refilter. Stay open. |
| Escape (empty) | Close popup. Release capture. |
| Tab | Close popup. Commit. Release capture. Move focus. |
| Space | Insert space character. NOT toggle. |
| Home / End / Left / Right | Cursor movement in inline input. |

**INV-276-UI4: Pointer Routing**

| Input | Action |
|-------|--------|
| Click chip × | Remove from committed set directly |
| Click control area | Enter capture (if not captured) |
| Click popup item | Toggle item. Preserve filter + highlight. Stay open. |
| Click outside | Close popup. Commit. Release capture. |
| Click Select All icon | Toggle all enabled items on |
| Click Clear All icon | Remove all `TagEntry::Item` entries (preserve Custom) |
| Click dropdown affordance | Toggle popup open/close |
| Scroll in popup | Scroll item list |
| Hover popup item | Update highlight + recompute ghost |
| Hover affordance icon | Visual feedback (background highlight + color shift) |
| Hover chip × | Visual feedback (circular background + darkened × text) |

---

# 9. Ghost Completion

**INV-276-GH1: Prefix Match**

When inline input text is a case-insensitive prefix of the highlighted item's
label, a ghost suffix renders after typed text in `typography.muted` color.
Visual only — never mutates input text.

Example: type "Ru" → highlighted "Rust" → ghost "st" in muted after "Ru".

**INV-276-GH2: Enter with Ghost**

Enter when ghost exists → toggles the highlighted item (properly-cased label
is what gets selected, not the user's typed text).

**INV-276-GH3: No Match Signal**

When input text doesn't prefix-match any item:
- No ghost shown
- Hybrid mode: control background changes to `accents.background` (signals
  "will create new tag")
- SelectOnly mode: no visual change (creation not available)

---

# 10. Tag Creation

**INV-276-CR1: Explicit Trigger Only**

Tags created only on explicit action (Enter, configured trigger keys).
No auto-creation on blur, timeout, or focus loss. Per SPEC-HAL-002 §4.11.

**INV-276-CR2: Whitespace Trimming**

Input trimmed before creation. Empty after trim → no tag.

**INV-276-CR3: Validation**

If `tag_validator` is set, trimmed input is validated before creation.
Failure → no tag created, input NOT cleared (user can correct).
Border turns `semantic.error` color on validation failure (see §15).

**INV-276-CR4: Duplicate Rejection**

When `allow_duplicate_tags` is false (default) and trimmed input matches
existing Custom tag, creation rejected. The existing chip flashes red for
200ms to indicate the duplicate (see §14).

**INV-276-CR5: Immediate Commit**

Every toggle and every custom tag creation immediately syncs both
`working_set/working_order` AND `committed_set/committed_order`.
`on_change` fires on every toggle/create. There is no deferred commit.
The working/committed split exists only as an implementation detail —
both sets are always identical after any mutation.

**INV-276-CR6: Input Cleared on Success**

After successful tag creation, inline input cleared.

**INV-276-CR7: Backspace Safety**

Backspace on empty input is a no-op. Tags are removed exclusively via
the × button on chips. This prevents accidental destruction of
intentional selections.

---

# 11. Trigger Key Configuration

```rust
#[derive(Debug, Clone, Copy)]
pub struct TagTriggerKeys {
    pub enter: bool,   // default: true
    pub comma: bool,   // default: false
    pub tab: bool,     // default: false
}
```

**INV-276-TK1: Enter Dual Purpose (Hybrid)**

Highlighted item exists → toggle it. No highlight + non-empty input →
case-insensitive exact match check → if match, toggle; if no match, create.
Empty input → close popup.

**INV-276-TK2: Tab Trigger**

When `tab: true`, Tab creates tag (non-empty input) and stays in widget.
When `tab: false`, Tab commits and moves focus.

---

# 12. Escape Cascade

| State | Action |
|-------|--------|
| Input has text | Clear input. Refilter. Stay open. |
| Input empty, popup open | Close popup. Release capture. |
| Input empty, popup closed | Release capture. |

Two Escapes worst-case: clear → close.

---

# 13. Commit Semantics

**INV-276-C1: Immediate Commit**

Every explicit user action (Enter toggle, Enter create, click toggle,
chip × remove) immediately updates both working and committed sets and
fires `on_change`. There is no deferred batch commit.

**INV-276-C2: Working Set Is Shadow Copy**

`open_popup` clones committed → working. Both sets stay in sync through
immediate commit on every mutation. The working set exists so that
`paint` can reference a consistent selection during popup-open rendering.

**INV-276-C3: Escape Does Not Revert**

Escape closes the popup. Since all toggles/creates were already committed,
there is nothing to revert. The user's selections are preserved.

**INV-276-C4: on_change Fires Per Action**

`on_change` fires on every toggle and every custom tag creation, not
only on popup close. Applications receive real-time selection updates.

---

# 14. Caret and Animation

**INV-276-AN1:** The host reports only its actual host-owned unsettled effects. The ordinary engine traversal separately observes the real child's Capture-qualified blink and admitted motion demand. Inactive delegation cannot sustain an invisible caret channel, and the host shall not invoke or duplicate child needs_animation/Update.

**INV-276-AN2:** The engine's `animation_needed` recheck must run AFTER event
routing to catch state changes from the current frame.

**INV-276-AN3:** The empty-text caret MUST render via the text pipeline
(`draw_with_backgrounds` with an empty text run + caret background span),
not via the rect pipeline (`push_rect`). This is required because host
widgets (TagInput, ComboBox) paint their background via the path pipeline,
which renders after the rect pipeline. A rect-pipeline caret would be
buried under the path-pipeline background regardless of z_index.

**INV-276-AN4:** An intercepted host key may issue an owned caret-activity request through the same current engine-qualified session before the next dependent presentation. The child alone resets its blink through its lawful existing activity owner. No host calls a child method or supplies invented activity/Capture facts.

**INV-276-AN5:** Chip flash on duplicate rejection. When a duplicate tag is
rejected (INV-276-CR4), the existing chip's background turns `semantic.error`
for 200ms. The flash is driven by `chip_flash: Option<(TagEntry, u64)>` and
`last_frame_ms`. `needs_animation` returns true during an active flash.

---

# 15. Rendering

**INV-276-R1:** Control: rounded rect background + border. Chips + embedded
TextField inside. Affordance area on right edge (hidden for FreeFormOnly).

**INV-276-R2:** Four-state border via `Theme::control_border(focused, captured, error)`.
Error state (validation failure) renders `semantic.error` border color.
Normal states: default → focus → capture. Color-only signaling. No width changes.

**INV-276-R3:** Popup: overlay below control. Item rows with checkbox + label.
Group header rows in muted text (non-interactive, navigation skips them).
Scrollbar when needed. "No matches" row when filter yields zero results.
No filter field inside popup.

**INV-276-R4:** Ghost completion: separate text run after the TextField's text,
positioned at `chip_x + text_padding + input_text_width`, in muted color.

**INV-276-R5:** Control background changes to `accents.background` when
Hybrid mode, input non-empty, and no items match the filter. This signals
"will create a new custom tag" without changing text color.

**INV-276-R6: Affordance Icons**

The affordance area contains up to three regions, right-to-left:

1. **Dropdown indicator** (always, except FreeFormOnly) — hamburger bars
   (two bars closed, one bar open). Inherited from ComboBox visual language.
2. **Clear All icon** (when items > threshold) — checkbox with × overlay.
   Multi-layer icon (Base + Highlight + Shadow). Painted in `semantic.error`.
   Removes only `TagEntry::Item` entries; preserves `TagEntry::Custom`.
3. **Select All icon** (when items > threshold) — stacked checkboxes with
   checkmark. Multi-layer icon. Painted in `accents.primary`.

All affordance regions show hover feedback (rounded background via
`controls.background_hover`) and press feedback (`controls.background_pressed`).
Icon colors shift on hover (brighten +30% lerp toward white) and press
(darken ×0.7 RGB).

**INV-276-R7: Chip × Hover Feedback**

When the mouse hovers over a chip's × region, a circular background appears
behind the × character (`controls.background_hover`, border-radius = 50%)
and the × text color shifts from `typography.muted` to `typography.primary`.
This provides clear affordance that the × is clickable.

---

# 16. Paste Splitting

**INV-276-PS1: Delimiter Detection**

When an `Event::Text` arrives with content longer than 1 character and
containing configured trigger delimiters (comma if `trigger_keys.comma`,
tab if `trigger_keys.tab`), the text is split on those delimiters.

**INV-276-PS2: Per-Segment Processing**

Each segment is trimmed. Empty segments are skipped. For each non-empty
segment:
1. Case-insensitive exact match against items → toggle as `TagEntry::Item`
2. No match + Hybrid/FreeFormOnly → create as `TagEntry::Custom`
3. No match + SelectOnly → skip

**INV-276-PS3: Atomic Commit**

All segments are processed, then both working and committed sets are synced
atomically. `on_change` fires once for the entire paste operation.

---

# 17. Group Headers

**INV-276-GR1: Item Grouping**

Items with `group: Some(name)` are sorted by group name in `refilter_from_input`.
Within each group, original item order is preserved.

**INV-276-GR2: Header Rendering**

Group headers render as non-interactive separator rows in muted text. They
are visually distinct from item rows (no checkbox, no highlight).

**INV-276-GR3: Navigation Skip**

Arrow key navigation and popup_row_hit skip group header rows. Headers
are paint-only — they have no effect on selection or highlighting logic.

**INV-276-GR4: PopupRow Enum**

The popup display list uses `enum PopupRow { GroupHeader(String), Item(u64) }`.
This is rebuilt in `refilter_from_input` and cached in `popup_rows`. Paint
and visible_rows read from this cached list.

---

# 18. Theme Geometry

```rust
pub struct TagInputGeometry {
    pub chip_height: f32,
    pub chip_padding_x: f32,
    pub chip_gap: f32,
    pub chip_radius: f32,
    pub chip_close_size: f32,
    pub popup_max_visible_rows: usize,
    pub popup_padding: f32,
    pub radius: f32,
}
```

---

# 19. Widget State

```rust
pub struct TagInput {
    id: WidgetInternalId,
    items: Vec<TagItem>,
    committed_set: HashSet<TagEntry>,
    committed_order: Vec<TagEntry>,
    working_set: HashSet<TagEntry>,
    working_order: Vec<TagEntry>,
    filtered_ids: Vec<u64>,
    popup_open: bool,
    highlighted_index: Option<usize>,
    popup_scroll_row: usize,
    display_mode: TagDisplayMode,
    policy: TagInputPolicy,
    max_selections: Option<usize>,
    placeholder: String,
    focused: bool,
    in_capture: bool,
    tag_creation_mode: TagCreationMode,
    trigger_keys: TagTriggerKeys,
    tag_validator: Option<fn(&str) -> bool>,
    allow_duplicate_tags: bool,
    on_change: Option<Box<dyn FnMut(&[TagEntry])>>,
    control_height: f32,
    last_layout_width: f32,
    affordance_width: f32,
    scrollbar_v: ScrollbarV,
    chip_rects: Cell<Vec<ChipRect>>,
    ghost_completion: Option<String>,   // visual only
    validation_error: bool,             // border turns semantic.error
    chip_flash: Option<(TagEntry, u64)>, // (entry, start_ms) — 200ms flash
    last_frame_ms: u64,                 // cached for chip flash timing
    select_all_threshold: usize,        // default: 10
    popup_rows: Vec<PopupRow>,          // cached group-aware display list
    hovered_icon: Option<AffordanceIcon>, // hover tracking for affordance area
    pressed_icon: Option<AffordanceIcon>, // press tracking for affordance area
    hovered_close: Option<TagEntry>,    // hover tracking for chip × buttons
}
```

The host stores a sealed copied child-role relation and source-qualified owned draft observations/configuration values under HAL-101 section 2.4. The TextField Widget/document/cursor/selection/cache/history/IME/presentation reside exclusively in the real tree child. This is a state-ownership contract, not a prescribed new Rust type or public schema.

---

# 20. Builder API

```rust
TagInput::new()
    .with_items(items)
    .with_selection(&[id1, id2])
    .with_display_mode(TagDisplayMode::Chips)
    .with_policy(TagInputPolicy::Full)
    .with_tag_creation(TagCreationMode::Hybrid)
    .with_max_selections(5)
    .with_placeholder("Select or type...")
    .with_trigger_keys(TagTriggerKeys { enter: true, comma: true, tab: false })
    .with_tag_validator(|s| s.len() >= 2)
    .with_allow_duplicate_tags(false)
    .on_change(|tags: &[TagEntry]| { ... })
```

---

# 21. Acceptance Criteria

1. Click → popup opens, caret visible in control area after chips
2. Type → popup filters in real-time, ghost completion appears
3. Down/Up → navigate filtered list, ghost updates, group headers skipped
4. Enter (highlighted) → toggle item, clear input, stay open
5. Enter (no match, Hybrid) → create custom tag chip, clear input
6. Enter (empty) → close popup, commit
7. Escape (text) → clear text, refilter, stay open
8. Escape (empty) → close, release capture (no revert — already committed)
9. Backspace (empty) → no-op (tags removed via × only)
10. Ctrl/Alt+Backspace → word-delete backward
11. Chip × → remove immediately
12. Chip × hover → circular background + darkened × text
13. Click popup item → toggle, popup stays open
14. Ghost renders correctly-cased suffix in muted color
15. Hybrid no-match → control background changes
16. FreeFormOnly → no popup, inline input only
17. SelectOnly → no tag creation, filter only
18. Caret blinks continuously when captured, even during arrow key navigation
19. No manual text/caret rendering — TextField handles all text
20. Validation failure → border turns red, input preserved for correction
21. Duplicate rejection → existing chip flashes red for 200ms
22. Paste "a, b, c" with comma trigger → three tags created atomically
23. Group headers render as non-interactive muted rows
24. Select All icon → all enabled items checked (when items > threshold)
25. Clear All icon → all Item entries removed, Custom tags preserved
26. Affordance icons show hover/press visual feedback
27. Select All / Clear All hidden when items ≤ threshold

---

# 22. Non-Goals (Future Work)

- Drag-to-reorder chips
- Hierarchical items (nested groups)
- Async item loading
- Multi-column popup
- Icons per item
- Backspace-to-highlight-chip confirmation pattern (highlight → second backspace deletes)
- Tooltip on affordance icons
- Cursor icon change on hover

---

# 23. Consistency Matrix (SPEC-HAL-002A Required)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. All state in TagInput. |
| B — Phase Separation | Preserved. Events → state; paint → render. |
| C — ID-based Identity | Preserved. TagEntry identity. |
| D — Pure Data | Preserved. All types pure data. |
| E — GPU-First | Preserved. No pipeline changes. |
| F — Sovereignty | Preserved. No OS dependencies. |

No permanent invariants weakened.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under the supervising technical reviewer's coordinated adoption, with JC's final ownership and veto retained. The exact three source bindings and adopted scope remain in the retained adoption record. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.
