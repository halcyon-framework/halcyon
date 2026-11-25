# SPEC-HAL-245: Embedded Asset Picker — Projection and Control Integration

**Status:** Draft
**Layer:** UI Composition / ViewModel Integration / Control Embedding
**Scope:** Using asset browsers as embedded controls within forms or property panels
**Owner:** Application Layer
**Applies To:** Forms, inspector panels, property editors, dialogs

---

## 0. Purpose

Defines how to embed asset pickers as part of larger UI structures, allowing selection from a subset of assets scoped to the form's context.
Examples: select an icon, background image, font asset, audio file.

---

## 1. Invariants

### INV-245.1: Embedded Pickers Use Host ViewModel

The asset picker does not own or store selected values. It writes to the parent control ViewModel.

- Picker is projection-only
- Target is explicitly passed (e.g., via lens)

### INV-245.2: Asset Scope Is Contextual

Embedded pickers must specify their asset type and scope upfront — they do not default to global asset space.

- Examples: `only_fonts`, `only_audio`, `icons_only`
- Filters are defined by the host context

### INV-245.3: Picker Is Stateless Except During Interaction

Embedded picker must not store its own local persistent state outside of active use.

- ViewModel provides current value and scope
- Picker shows UI only on demand (e.g., popup)

### INV-245.4: Selection Is Intent-Based

Selection dispatches `Intent::SetPropertyValue` or similar binding-level intent.

- No direct mutation of values
- Compatible with form validation pipelines

---

## 2. UI Modes

- **Inline Selector** — shows current selection and opens popup browser
- **Dropdown Panel** — vertical expansion into filtered grid/list
- **Dialog Mode** — full asset browser dialog on demand

---

## 3. Responsibilities

- Parent provides value lens or binding target
- Picker renders current preview
- Popup/browser scoped by embedded filters

---

## 4. Out of Scope

- Global Asset Browser
- Drag/drop asset injection
- Remote asset sync
