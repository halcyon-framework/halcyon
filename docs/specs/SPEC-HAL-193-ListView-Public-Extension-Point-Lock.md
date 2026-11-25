# SPEC-HAL-193: ListView Public Extension Point Lock

## Status

**LOCKED** — Binding.

**Amended by:** SPEC-HAL-347, which narrowly admits ListView's existing-policy configuration/readback, read-only focused-row query, and single focused-row callback. All other locks in this SPEC remain binding.

**Also amended by:** SPEC-HAL-348, which narrowly admits header-visibility configuration/readback without opening header rendering or composite structure.

## Purpose

SPEC-HAL-192 locks ListView's internal architecture (closed composite, direct substrate).
This SPEC locks ListView's **external surface** — what traits it implements, what configuration paths exist, and what is explicitly forbidden from future extension.

Without this, complexity can re-enter legally via "just one hook" or "just one trait impl".

---

## 1. Allowed Trait Implementations

ListView MAY implement:

| Trait | Rationale |
|-------|-----------|
| `Widget` | Required for widget tree participation |
| `WidgetDowncast` | Required for runtime type queries |
| `Debug` | Optional; diagnostics only |
| `Clone` | Optional; if needed for snapshot/restore |

---

## 2. Forbidden Trait Implementations

ListView MUST NOT implement:

| Trait | Rationale |
|-------|-----------|
| `AuthorityOwner` | SPEC-HAL-192 lock; ListView is not an authority provider |
| `Container` (if exists) | ListView has no pluggable children |
| `Extensible<T>` / `Extension<T>` | No plugin/hook mechanism |
| `EventSink` / `EventSource` | No pub/sub; intents are forwarded to substrate |
| Custom listener/callback traits | No on_select, on_scroll, etc. |

**INV-193-TRAIT-LOCK**: Any new trait implementation on ListView requires SPEC overturn.

---

## 3. Allowed Constructor/Configuration Paths

The following are the ONLY public entry points:

| Method | Purpose |
|--------|---------|
| `new(source: S)` | Create ListView with DataSource |
| `with_columns(columns)` | Builder: configure columns at construction |
| `add_column(title, width)` | Add column after construction |
| `add_column_with_config(...)` | Add column with full geometry config |
| `set_item_height(height)` | Uniform row height |
| `set_scroll_y(scroll_y)` | External scroll sync |
| `set_header_visible(visible)` | Show or omit the owner-rendered header band |
| `header_visible()` | Read the configured header-band presence |

**INV-193-CTOR-LOCK**: No other configuration entry points are permitted.

Explicitly forbidden:
- `with_selection_mode(...)` — ListView does not own selection policy
- `with_focus_mode(...)` — ListView does not own focus policy
- `with_row_factory(...)` — No pluggable row widgets
- `with_cell_factory(...)` — No pluggable cell widgets
- `on_select(callback)` — No callback hooks
- `on_scroll(callback)` — No callback hooks
- `register_extension(...)` — No extension mechanism

---

## 4. Non-Extensible Components

The following are explicitly sealed from external extension:

### 4.1 Rows

- Row identity is owned by `DataSource` (via RowId)
- Row layout is owned by `TabularSubstrate`
- Row painting is internal to ListView's `paint()`
- **No** `RowDelegate`, `RowFactory`, or `RowRenderer` trait

**INV-193-ROW-SEALED**: Row rendering is internal. No external row customization.

### 4.2 Cells

- Cell identity is `(RowId, ColumnId)` from substrate
- Cell values come from `DataSource.read_cell()`
- Cell painting is internal to ListView's `paint()`
- **No** `CellDelegate`, `CellFactory`, or `CellRenderer` trait

**INV-193-CELL-SEALED**: Cell rendering is internal. No external cell customization.

### 4.3 Header

- Header is owned by `TabularSubstrate.header()`
- Header painting is internal to ListView's `paint()`
- **No** `HeaderDelegate` or custom header rendering

**INV-193-HEADER-SEALED**: Header rendering is internal. No external header customization.

### 4.4 Footer (Not Implemented)

- If footer is added, it follows the same pattern
- **No** `FooterDelegate` or custom footer rendering

**INV-193-FOOTER-SEALED**: Footer (if any) is internal. No external footer customization.

### 4.5 Intents

- Intents are forwarded via `self.substrate.handle_intent()`
- **No** `IntentInterceptor`, `IntentFilter`, or custom intent handling
- **No** `on_intent(callback)` hooks

**INV-193-INTENT-SEALED**: Intent flow is internal. No external interception.

---

## 5. Allowed Query Methods

Read-only accessors that expose substrate state:

| Method | Returns |
|--------|---------|
| `substrate()` | `&TabularSubstrate` |
| `adapter()` | `&DataAdapter<S>` |
| `row_count()` | `usize` |
| `row_ids()` | `&[RowId]` |
| `contains_row(row_id)` | `bool` |
| `scroll_y()` | `f32` |
| `content_height()` | `f32` |
| `column_ids()` | `Vec<ColumnId>` |
| `column_title(column_id)` | `Option<&str>` |

Mutable accessors (use with care):

| Method | Returns |
|--------|---------|
| `substrate_mut()` | `&mut TabularSubstrate` |
| `adapter_mut()` | `&mut DataAdapter<S>` |

**Note**: Mutable accessors exist for integration (e.g., tests, external scroll sync). They do NOT constitute extension points.

---

## 6. Future Change Requirements

Any change to this SPEC requires:

1. **Explicit SPEC overturn** referencing SPEC-HAL-193
2. **Justification** why the closed composite model is insufficient
3. **Update** to SPEC-HAL-192 if internal architecture is affected
4. **Source scan test update** if new forbidden patterns emerge

---

## 7. Invariant Summary

| ID | Statement |
|----|-----------|
| INV-193-TRAIT-LOCK | New trait impls on ListView require SPEC overturn |
| INV-193-CTOR-LOCK | Only listed constructor/config methods permitted |
| INV-193-ROW-SEALED | Row rendering is internal, no external customization |
| INV-193-CELL-SEALED | Cell rendering is internal, no external customization |
| INV-193-HEADER-SEALED | Header rendering is internal, no external customization |
| INV-193-FOOTER-SEALED | Footer (if any) is internal, no external customization |
| INV-193-INTENT-SEALED | Intent flow is internal, no external interception |

---

## 8. Relationship to Other SPECs

- **SPEC-HAL-190**: Defines when authority machinery is required (ListView: not required)
- **SPEC-HAL-191**: Defines ListView-to-TreeView generalization boundary
- **SPEC-HAL-192**: Locks ListView's internal architecture (closed composite)
- **SPEC-HAL-193** (this): Locks ListView's external extension surface
