# SPEC-HAL-317A: **Semantic Agent Event Model**

### *Amendment to SPEC-HAL-317: Refined Event Categories*

**Author:** JC
**Status:** Draft
**Amends:** SPEC-HAL-317 §4 (Event Channel)

---

# 1. Purpose

This amendment replaces SPEC-HAL-317's raw input events
(PointerDown, KeyDown, TextInput) with semantic interaction
and outcome events. The agent receives meaningful
information about what happened, not hardware noise.

---

# 2. Principle

**The agent sees results, not inputs.**

A keystroke is noise. A committed value is signal.
A pointer coordinate is noise. A clicked button is signal.
A mouse-move is noise. A hover-enter on a specific widget
is signal.

Events are emitted by widgets at the point where semantic
meaning is established, not by the engine at the point
where raw input arrives.

---

# 3. Revised AgentEventKind

```rust
#[derive(Debug, Clone)]
pub enum AgentEventKind {
    // ================================================================
    // Interaction Events — what the user DID
    // ================================================================

    /// User clicked a widget (pointer down + up without drag).
    Clicked {
        widget_id: u64,
        widget_type: &'static str,
    },

    /// Pointer entered a widget's bounds.
    HoverEnter {
        widget_id: u64,
        widget_type: &'static str,
    },

    /// Pointer left a widget's bounds.
    HoverLeave {
        widget_id: u64,
    },

    /// Keyboard focus moved.
    FocusChanged {
        old: Option<u64>,
        new: Option<u64>,
    },

    /// User started editing a value (double-click, Enter, 'i', etc).
    EditingStarted {
        widget_id: u64,
        widget_type: &'static str,
    },

    /// User cancelled editing (Escape).
    EditingCancelled {
        widget_id: u64,
    },

    /// User committed a new value (Enter, selection, auto-commit).
    ValueCommitted {
        widget_id: u64,
        widget_type: &'static str,
        old_value: String,
        new_value: String,
    },

    /// Selection changed in a list, combo, tree, or similar.
    SelectionChanged {
        widget_id: u64,
        widget_type: &'static str,
        selected_index: Option<usize>,
        selected_text: String,
    },

    /// Expandable node toggled (TreeView, accordion, collapsible).
    ExpandToggled {
        widget_id: u64,
        node_label: String,
        expanded: bool,
    },

    /// Popup/dropdown opened.
    PopupOpened {
        widget_id: u64,
        widget_type: &'static str,
    },

    /// Popup/dropdown closed.
    PopupClosed {
        widget_id: u64,
    },

    /// User scrolled a container.
    Scrolled {
        widget_id: u64,
        direction: ScrollDirection,
    },

    // ================================================================
    // Outcome Events — what HAPPENED as a consequence
    // ================================================================

    /// Validation failed on a widget.
    ValidationError {
        widget_id: u64,
        message: String,
    },

    /// Validation error cleared (value corrected or editing cancelled).
    ValidationCleared {
        widget_id: u64,
    },

    /// Screen navigation occurred.
    ScreenPushed {
        screen_name: String,
    },

    /// Screen popped from navigation stack.
    ScreenPopped,

    /// A frame was rendered (kept from Phase 1 — useful for
    /// performance monitoring, emitted at low frequency).
    FrameRendered {
        frame_number: u64,
        duration_us: u64,
    },

    // ================================================================
    // Consumer Events — domain-specific outcomes
    // ================================================================

    /// Application-level event emitted by the consumer.
    /// Examples: "form_saved", "trade_executed", "connection_lost",
    /// "settings_updated", "file_exported".
    Custom {
        tag: String,
        payload: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}
```

---

# 4. Emission Points

Events are emitted BY WIDGETS, not by the engine dispatch
loop. The `AgentEventSender` is accessible via `EventContext`.

## 4.1 EventContext Integration

```rust
impl EventContext {
    /// Emit a semantic agent event. No-op if no agent is connected.
    pub fn emit_agent_event(&self, kind: AgentEventKind) {
        if let Some(ref tx) = self.agent_tx {
            crate::core::agent::emit(tx, kind);
        }
    }
}
```

## 4.2 Widget Emission Examples

**Button** — in click handler:
```rust
cx.emit_agent_event(AgentEventKind::Clicked {
    widget_id: cx.widget_id.raw(),
    widget_type: "Button",
});
```

**TextField** — in commit handler (not every keystroke):
```rust
cx.emit_agent_event(AgentEventKind::ValueCommitted {
    widget_id: cx.widget_id.raw(),
    widget_type: "TextField",
    old_value: self.original_text.clone(),
    new_value: self.text().to_string(),
});
```

**ComboBox** — on selection (not every j/k step):
```rust
cx.emit_agent_event(AgentEventKind::SelectionChanged {
    widget_id: cx.widget_id.raw(),
    widget_type: "ComboBox",
    selected_index: Some(idx),
    selected_text: self.selected_text().to_string(),
});
```

**PropertyEditor** — on editing lifecycle:
```rust
// On activation:
cx.emit_agent_event(AgentEventKind::EditingStarted {
    widget_id: cx.widget_id.raw(),
    widget_type: "PropertyEditor",
});

// On commit:
cx.emit_agent_event(AgentEventKind::ValueCommitted {
    widget_id: cx.widget_id.raw(),
    widget_type: "PropertyEditor",
    old_value: old.to_string(),
    new_value: new.to_string(),
});

// On validation error:
cx.emit_agent_event(AgentEventKind::ValidationError {
    widget_id: cx.widget_id.raw(),
    message: error.clone(),
});
```

## 4.3 Consumer Emission

Application code emits domain events:

```rust
cx.emit_agent_event(AgentEventKind::Custom {
    tag: "order_placed".into(),
    payload: format!("BTC 0.5 @ 101000"),
});
```

---

# 5. What is NOT Emitted

- Raw pointer coordinates (PointerDown, PointerMove, PointerUp)
- Raw key events (KeyDown, KeyUp)
- Raw text input (TextInput)
- Every scroll pixel delta (only direction summary)
- Widget creation/destruction (query the tree instead)
- Layout metrics (query via Layer 3)
- Paint/render internals

These are available through the raw event channel
(Phase 1) for local fast agents. Semantic events are
the only events exposed through MCP to Claude.

---

# 6. Phase 1 Raw Events: Kept or Removed?

**Kept** as a separate channel tier. The raw channel
exists for:
- Local 7B models doing real-time processing
- Replay/recording tools
- Performance profiling

The semantic events are the primary channel for
intelligent agents. Both channels share the same
`AgentEventSender` — the enum simply has both raw
and semantic variants. The MCP layer filters to
semantic only.

Alternatively: remove raw events entirely from
`AgentEventKind` and keep them as a separate
`RawInputEvent` stream. This is architecturally
cleaner but requires two channels. Decision: JC.

---

# 7. Invariants

**INV-317A-1:** Semantic events are emitted at the point
where meaning is established, not where input arrives.
A ValueCommitted event fires in the widget's commit
handler, not in the engine's key dispatch.

**INV-317A-2:** Interaction events carry the widget_type
as a static string. The agent knows "Button was clicked,"
not "widget 42 was clicked." No runtime allocation for
the type string.

**INV-317A-3:** ValueCommitted carries both old and new
values. The agent sees the delta without querying state.

---

# 8. Forbidden Behaviors

**FORBID-317A-1:** Widgets MUST NOT emit agent events for
intermediate states. A TextField does NOT emit on every
keystroke. A ComboBox does NOT emit on every highlight
step. Only final, committed, meaningful state changes
produce events.

**FORBID-317A-2:** Agent event emission MUST NOT allocate
in the hot path when no agent is connected. The
`if let Some(ref tx)` guard short-circuits before any
String allocation.

---

# 9. Migration from Phase 1

Phase 1 emits: PointerDown, KeyDown, TextInput,
FocusChanged, FrameRendered.

After this amendment:
- **FocusChanged** — kept as-is (already semantic)
- **FrameRendered** — kept as-is (useful for monitoring)
- **PointerDown** — removed, replaced by widget-level Clicked
- **KeyDown** — removed, replaced by widget-level events
- **TextInput** — removed, replaced by ValueCommitted

The Phase 1 emission points in `app.rs` (the input
event match block) are removed. New emission points
are added inside widget event handlers.
