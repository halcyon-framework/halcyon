# SPEC-HAL-122

**Menu & MenuItem Interaction Contract**

**Status:** Design — Approved
**Scope:** Menu-based command invocation and hierarchical menu navigation
**Applies to:** MenuBar, Menu, MenuItem, SubMenu, ContextMenu
**Non-Goals:** Visual styling, theming, animation, accessibility APIs (handled separately)

---

## 1. Problem Statement

Menus fail when interaction rules are implicit or borrowed inconsistently from buttons, lists, or trees.

Common failures include:

* activation on hover
* focus leaks when menus close
* inconsistent keyboard navigation
* accidental selection vs activation coupling
* broken submenu timing

This SPEC defines a **strict interaction contract** for menus as **command-routing state machines**, not lists or trees.

---

## 2. Core Definitions

### 2.1 Menu

A transient container presenting a list of **MenuItems** for command invocation.

### 2.2 MenuItem

A single actionable or structural entry in a menu.

Types:

* **CommandItem** — triggers an action
* **CheckboxItem** — toggles state
* **RadioItem** — selects within group
* **SubMenuItem** — opens nested menu
* **Separator** — non-interactive

---

## 3. Focus Model

### INV-F1 — Menu Focus Trap

While a menu is open:

* Focus MUST remain within the menu hierarchy
* Tab/Shift+Tab MUST NOT escape the menu
* Focus returns to invoking widget on close

---

### INV-F2 — Single Focused Item

At most **one MenuItem** may be focused at any time.

* Focus is required for keyboard activation
* Hover MAY move focus (see INV-H1)

---

## 4. Activation Semantics

### INV-A1 — Activate on Release Only

MenuItems activate:

* on pointer **release**
* on keyboard **Enter** or **Space** release

Never on press.

---

### INV-A2 — Hover Is Not Activation

Hovering a MenuItem:

* MAY move focus
* MUST NOT activate
* MUST NOT commit selection

---

## 5. Submenu Behavior

### INV-SM1 — Explicit Submenu Opening

SubMenus open via:

* pointer hover with dwell delay (configurable)
* Right Arrow key
* Enter on SubMenuItem

---

### INV-SM2 — Submenu Close Rules

SubMenus close when:

* pointer leaves parent menu tree
* Left Arrow pressed
* parent menu closes

Closing a parent MUST close all descendants.

---

## 6. Keyboard Navigation

### INV-K1 — Vertical Navigation

* Up/Down: move focus between enabled MenuItems
* Disabled items are skipped
* Separators are skipped

---

### INV-K2 — Horizontal Navigation (MenuBar)

* Left/Right moves between top-level menus
* Wrap-around allowed
* Focus preserved when switching menus

---

### INV-K3 — Escape Behavior

* Esc closes current menu
* If submenu: closes submenu only
* If top-level menu: closes entire menu stack

---

## 7. Disabled State

### INV-D1 — Disabled Items Are Inert

Disabled MenuItems:

* cannot be focused
* cannot be activated
* must be skipped in navigation

---

## 8. Event Emission

### INV-E1 — Single Activation Event

A MenuItem activation emits exactly one event:

* `on_activate(item_id)`

No duplicates. No hover events.

---

## 9. Responsibility Boundaries

### Menu / MenuSystem owns:

* open/close state
* focus trapping
* submenu stack management
* keyboard routing

### MenuItem owns:

* enabled/disabled state
* item type (command, checkbox, etc.)
* activation intent

### Menu / MenuItem MUST NOT own:

* visual rendering
* layout geometry
* command execution logic

---

## 10. Forbidden Behavior

Menus MUST NOT:

* activate on hover
* allow focus outside menu while open
* activate disabled items
* leave submenus orphaned
* reuse List or Tree interaction semantics

---

## 11. Test Requirements

Any Menu implementation MUST include tests for:

* focus trap enforcement
* keyboard navigation
* submenu open/close timing
* activation on release only
* escape handling
* disabled item skipping

---

## 12. Reference Implementation

Planned widgets:

* `MenuBar`
* `Menu`
* `MenuItem`
* `ContextMenu`

---

## 13. Final Note

Menus are **command routers**, not selection widgets.

Any implementation that treats a Menu like a List or Tree is **incorrect by definition**.
