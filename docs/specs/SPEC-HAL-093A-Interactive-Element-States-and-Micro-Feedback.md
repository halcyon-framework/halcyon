# **SPEC-HAL-093A — Interactive Element States & Micro-Feedback**

**SPEC-HAL-093A**
**Interactive Element States & Micro-Feedback**
**Author:** Halcyon Architect
**Status:** PROPOSED
**Version:** 1.0

**Extends:** SPEC-HAL-093 — Visual Appearance & Radii System

**Depends On:**

* SPEC-HAL-090 — Logical Grid & Compact Layout Rules
* SPEC-HAL-093 — Visual Appearance & Radii System
* SPEC-HAL-094 — Component Geometry Specification
* SPEC-HAL-095 — Iconography Rules
* SPEC-HAL-096 — Color System & Semantic Palette
* SPEC-HAL-097 — Motion & Micro-Animation Rules
* SPEC-HAL-098 — Cursor and Pointer System
* SPEC-HAL-100 — Widget Interaction State Machine

---

## **Implementation Status**

*Not implemented.*

This SPEC extends SPEC-HAL-093 with detailed **visual appearance of micro-interaction states** across all Halcyon components:

* resize handles and splitters
* column/row manipulation in grids
* drag-and-drop feedback
* text selection and carets
* sort indicators
* overflow indicators
* tooltips
* loading states
* focus traversal feedback
* validation states
* empty states
* search highlighting
* notification badges

These details are critical for professional-grade UI that feels responsive and precise.

---

# **1. Motivation**

Professional desktop applications require pixel-perfect feedback for every interaction. Users must:

* know exactly what they can interact with
* see immediate visual response to hover/press
* understand drag operations through clear feedback
* never wonder "is this draggable?" or "where will this drop?"

Missing or inconsistent micro-feedback makes software feel unfinished.

This SPEC covers the "last 10%" of visual polish that separates amateur UI from professional tools.

---

# **2. Resize Handles & Splitters**

### **2.1 Splitter (Panel Divider)**

| Property           | Value                              |
|--------------------|------------------------------------|
| Visual width       | 1px (Border-Subtle)                |
| Hit area width     | 5px (centered on visual)           |

**States:**

| State    | Visual                                      |
|----------|---------------------------------------------|
| Default  | 1px Border-Subtle                           |
| Hover    | 1px Border-Default + 3px Accent-Muted gutter|
| Dragging | 1px Accent-Primary + 3px Accent-Muted gutter|

The "gutter" is a subtle colored zone on each side of the divider line, indicating the resize affordance.

### **2.2 Column Resize Handle (Grid/Table)**

| Property           | Value                              |
|--------------------|------------------------------------|
| Visual width       | 1px (Border-Subtle)                |
| Hit area width     | 7px (centered on border)           |
| Hit area height    | Full header height                 |

**States:**

| State    | Visual                                      |
|----------|---------------------------------------------|
| Default  | 1px Border-Subtle (column separator)        |
| Hover    | 2px Accent-Primary, centered                |
| Dragging | 2px Accent-Primary + ghost column edge line |

**Ghost Line During Drag:**

```
width:   1px
color:   Accent-Primary @ 60% opacity
extends: Full table height (below header)
```

### **2.3 Row Resize Handle (if supported)**

| Property           | Value                              |
|--------------------|------------------------------------|
| Visual height      | 1px (Border-Subtle)                |
| Hit area height    | 5px                                |
| Hit area position  | Bottom edge of row                 |

States mirror column resize.

### **2.4 Window Resize Grips**

| Property           | Value                              |
|--------------------|------------------------------------|
| Corner grip size   | 16 × 16 px (logical)               |
| Edge hit area      | 4px from window edge               |
| Visual indicator   | None (invisible grip)              |

No visible resize grip marks — cursor change is the only affordance.
Cursor definitions per SPEC-HAL-098.

### **2.5 Dialog/Panel Resize**

For resizable dialogs and floating panels:

| Property           | Value                              |
|--------------------|------------------------------------|
| Minimum size       | 200 × 120 px                       |
| Resize edges       | All 4 edges + 4 corners            |
| Hit area           | 4px                                |
| Visual feedback    | None until drag starts             |
| During drag        | 1px Accent-Primary outline         |

---

# **3. Data Grid Interactions**

### **3.1 Column Header**

| Property           | Compact    | Balanced   |
|--------------------|------------|------------|
| Height             | 28px       | 32px       |
| Padding            | P-S (6U)   | P-S (6U)   |
| Font               | T-S, 500   | T-S, 500   |
| Background         | Surface-1  | Surface-1  |
| Border bottom      | 1px Border-Default |          |

**States:**

| State         | Visual                                  |
|---------------|-----------------------------------------|
| Default       | Surface-1, Text-Primary                 |
| Hover         | Surface-2, Text-Primary                 |
| Pressed       | Surface-2 −4%, Text-Primary             |
| Sorted        | Text-Primary + sort icon (Icon-Accent)  |
| Resize hover  | 2px Accent-Primary on right edge        |

### **3.2 Sort Indicator**

| Property           | Value                              |
|--------------------|------------------------------------|
| Icon size          | I-XS (10px)                        |
| Position           | Right side of header text          |
| Gap from text      | S-XS (4U)                          |
| Ascending icon     | Chevron up (▲)                     |
| Descending icon    | Chevron down (▼)                   |
| Color              | Icon-Accent                        |

**Multi-column Sort:**

```
Primary sort:   Icon-Accent @ 100%
Secondary sort: Icon-Accent @ 60% + small "2" badge
Tertiary sort:  Icon-Accent @ 40% + small "3" badge
```

### **3.3 Column Reorder (Drag)**

| Property           | Value                              |
|--------------------|------------------------------------|
| Drag threshold     | 4px movement                       |
| Ghost appearance   | Header cell @ 80% opacity          |
| Ghost offset       | Centered on cursor                 |
| Drop indicator     | 2px Accent-Primary vertical line   |

### **3.4 Cell Selection**

| Property           | Value                              |
|--------------------|------------------------------------|
| Selected cell bg   | Accent-Muted                       |
| Selected cell border | 1px Accent-Primary (if focused)  |
| Selected row bg    | Accent-Muted (full row)            |
| Range selection    | Accent-Muted for all cells in range|
| Range border       | 1px Accent-Primary around range    |

**Multi-Select Feedback:**

```
Ctrl+Click:  Toggles individual cell/row
Shift+Click: Extends selection to clicked cell/row
Drag:        Creates rectangular range selection
```

### **3.5 Frozen Columns/Rows**

| Property           | Value                              |
|--------------------|------------------------------------|
| Freeze divider     | 2px Border-Strong                  |
| Shadow (optional)  | 4px horizontal shadow @ 15% opacity|
| Shadow direction   | Right of frozen columns            |

The shadow provides depth cue that frozen area is "above" scrolling content.

### **3.6 Cell Editing**

| Property           | Value                              |
|--------------------|------------------------------------|
| Edit mode border   | 2px Accent-Primary                 |
| Edit mode bg       | Surface-0 (slightly lighter)       |
| Caret              | 1px Accent-Primary, blinking       |
| Selection          | Selection-Background               |

---

# **4. Text Selection & Caret**

### **4.1 Selection Colors**

| Token                  | Dark Theme | Light Theme |
|------------------------|------------|-------------|
| Selection-Background   | `#2D4A6B`  | `#B8D4F0`   |
| Selection-Text         | Text-Primary (unchanged) |       |

Selection background is derived from Accent-Muted but may be adjusted for readability.

### **4.2 Caret (Text Cursor)**

| Property           | Value                              |
|--------------------|------------------------------------|
| Width              | 1px (logical)                      |
| Color              | Accent-Primary                     |
| Blink rate         | 530ms on, 530ms off                |
| Blink start delay  | 0ms (immediate)                    |
| Height             | Line height of current font        |

**Caret States:**

| State              | Behavior                           |
|--------------------|------------------------------------|
| Focused            | Blinking                           |
| Typing             | Solid (no blink) during input      |
| Selection active   | Hidden (selection handles shown)   |
| Unfocused          | Hidden                             |

### **4.3 Text Selection Handles (Future Touch Support)**

| Property           | Value                              |
|--------------------|------------------------------------|
| Handle shape       | Teardrop / lollipop                |
| Handle size        | 20 × 20 px touch target            |
| Handle color       | Accent-Primary                     |
| Handle position    | Start and end of selection         |

Not implemented in initial desktop version; reserved for future tablet/touch mode.

---

# **5. Drag-and-Drop Feedback**

### **5.1 Drag Initiation**

| Property           | Value                              |
|--------------------|------------------------------------|
| Drag threshold     | 4px movement from mousedown        |

### **5.2 Drag Ghost**

| Property           | Value                              |
|--------------------|------------------------------------|
| Opacity            | 70%                                |
| Scale              | 100% (no shrink)                   |
| Offset from cursor | (8, 8) px                          |
| Background         | Surface-3                          |
| Border             | 1px Border-Default                 |
| Shadow             | Z-Popover level                    |
| Max size           | 200 × 60 px (clip if larger)       |

**Multi-Item Drag:**

```
Ghost shows first item + badge with count
Badge: 16px circle, Accent-Primary bg, Text-Inverse
Badge position: Top-right corner of ghost, offset (-4, -4)
```

### **5.3 Drop Zone Indicators**

| State              | Visual                                  |
|--------------------|-----------------------------------------|
| Valid drop zone    | 2px dashed Accent-Primary border        |
| Valid (hovering)   | 2px solid Accent-Primary + Accent-Background fill |
| Invalid drop       | None (cursor shows invalid per 098)     |

### **5.4 Drop Position Indicators**

For ordered lists/trees:

| Indicator          | Visual                                  |
|--------------------|-----------------------------------------|
| Insert above       | 2px Accent-Primary line above item      |
| Insert below       | 2px Accent-Primary line below item      |
| Insert into (nest) | Item background = Accent-Muted          |

Line indicators include small "arrow" endcaps (4px triangles) pointing toward insertion point.

### **5.5 Tree Drag-and-Drop**

| Property           | Value                              |
|--------------------|------------------------------------|
| Auto-expand delay  | 800ms hover to expand collapsed node |
| Scroll zone height | 32px from top/bottom edge          |
| Scroll speed       | 100px/sec                          |

---

# **6. Scroll Feedback**

### **6.1 Scrollbar States**

| State              | Track           | Thumb             |
|--------------------|-----------------|-------------------|
| Default (idle)     | Transparent     | Scrollbar-Thumb @ 0% (hidden) |
| Hover (on area)    | Scrollbar-Track | Scrollbar-Thumb   |
| Hover (on thumb)   | Scrollbar-Track | Scrollbar-Thumb-Hover |
| Dragging           | Scrollbar-Track | Scrollbar-Thumb-Hover |
| Scrolling (wheel)  | Scrollbar-Track | Scrollbar-Thumb   |

**Fade Timing:**

```
Appear:     100ms ease-out
Disappear:  300ms delay, then 200ms fade-out
```

### **6.2 Overscroll Feedback**

| Property           | Value                              |
|--------------------|------------------------------------|
| Overscroll effect  | None (hard stop)                   |
| Bounce             | Disabled (not mobile)              |
| Edge shadow        | Optional: 8px gradient fade @ 10% |

Edge shadow appears when content is scrolled and more content exists in that direction.

### **6.3 Scroll Shadows**

| Position           | Visual                              |
|--------------------|-------------------------------------|
| Top (scrolled)     | 8px vertical gradient, #000 @ 8%    |
| Bottom (more below)| 8px vertical gradient, #000 @ 8%    |
| Left (scrolled)    | 8px horizontal gradient, #000 @ 8%  |
| Right (more right) | 8px horizontal gradient, #000 @ 8%  |

Shadows indicate scrollable content exists beyond visible area.

---

# **7. Tooltips**

### **7.1 Appearance**

| Property           | Value                              |
|--------------------|------------------------------------|
| Background         | Surface-3                          |
| Border             | 1px Border-Default                 |
| Radius             | R-S (3px)                          |
| Padding            | P-XS vertical, P-S horizontal      |
| Shadow             | Z-Popover level                    |
| Font               | T-S (12px)                         |
| Text color         | Text-Primary                       |
| Max width          | 280px                              |

### **7.2 Timing**

| Property           | Value                              |
|--------------------|------------------------------------|
| Show delay         | 500ms                              |
| Show delay (fast)  | 200ms (after recent tooltip)       |
| Fast mode duration | 1500ms since last tooltip hidden   |
| Hide delay         | 0ms (immediate on mouse leave)     |
| Transition         | 100ms fade-in, 50ms fade-out       |

### **7.3 Positioning**

| Property           | Value                              |
|--------------------|------------------------------------|
| Preferred position | Below trigger, centered            |
| Fallback order     | Above → Right → Left               |
| Offset from trigger| 4px                                |
| Screen edge margin | 8px                                |

**Arrow/Pointer:** None. Halcyon tooltips are arrow-less rectangles.

### **7.4 Rich Tooltips**

For complex tooltips (e.g., keyboard shortcuts):

| Property           | Value                              |
|--------------------|------------------------------------|
| Max width          | 320px                              |
| Header font        | T-S, 500 weight                    |
| Body font          | T-S, 400 weight                    |
| Shortcut display   | Monospace, Surface-2 bg, R-XS radius |

---

# **8. Overflow Indicators**

### **8.1 Tab Bar Overflow**

When tabs exceed available width:

| Property           | Value                              |
|--------------------|------------------------------------|
| Indicator          | Chevron button on overflow side    |
| Button size        | 24 × full tab bar height           |
| Icon               | I-S chevron (< or >)               |
| Background         | Surface-1                          |
| Hover              | Surface-2                          |
| Scroll behavior    | Click scrolls by 1 tab width       |

**Gradient Fade:**

```
32px horizontal gradient fade on overflow edge
From: Surface-1 @ 100%
To:   Surface-1 @ 0% (transparent)
```

### **8.2 Menu Overflow**

When menu exceeds screen height:

| Property           | Value                              |
|--------------------|------------------------------------|
| Scroll arrows      | 24px tall zones at top/bottom      |
| Arrow icon         | I-XS chevron (▲ or ▼)              |
| Scroll speed       | 200px/sec while hovering arrow     |
| Arrow visibility   | Only when overflow exists          |

### **8.3 Breadcrumb Overflow**

| Property           | Value                              |
|--------------------|------------------------------------|
| Collapse indicator | "..." (ellipsis) button            |
| Button style       | Ghost button                       |
| Click behavior     | Shows dropdown of collapsed items  |
| Collapse strategy  | Collapse middle items first        |

---

# **9. Loading States**

### **9.1 Spinner**

| Property           | Value                              |
|--------------------|------------------------------------|
| Size (default)     | 16 × 16 px                         |
| Size (large)       | 24 × 24 px                         |
| Color              | Accent-Primary                     |
| Track color        | Border-Subtle @ 30%                |
| Animation          | 800ms linear rotation              |
| Stroke width       | 2px                                |

### **9.2 Progress Bar (Determinate)**

| Property           | Value                              |
|--------------------|------------------------------------|
| Height             | 4px                                |
| Track background   | Surface-2                          |
| Fill color         | Accent-Primary                     |
| Radius             | R-XS (2px) or full-round           |
| Transition         | 150ms ease-out on value change     |

### **9.3 Progress Bar (Indeterminate)**

| Property           | Value                              |
|--------------------|------------------------------------|
| Height             | 4px                                |
| Track background   | Surface-2                          |
| Fill color         | Accent-Primary                     |
| Animation          | Sliding highlight, 1.5s loop       |

### **9.4 Skeleton Loading**

| Property           | Value                              |
|--------------------|------------------------------------|
| Background         | Surface-2                          |
| Highlight          | Surface-3 (shimmer)                |
| Animation          | 1.5s shimmer sweep, left to right  |
| Radius             | Match target component radius      |
| Height             | Match target component height      |

---

# **10. Focus Traversal**

### **10.1 Focus Ring**

| Property           | Value                              |
|--------------------|------------------------------------|
| Style              | 1px solid Border-Focus             |
| Offset             | 0px (on border, not outside)       |
| Radius             | Match component radius             |

**Alternative (high contrast):**

```
2px solid Border-Focus
Offset: 2px outside component
```

Configurable via accessibility settings.

### **10.2 Focus Order Indicator (Dev Mode)**

For debugging focus order:

| Property           | Value                              |
|--------------------|------------------------------------|
| Badge              | Tab index number                   |
| Badge size         | 16px circle                        |
| Badge color        | Warning background                 |
| Badge position     | Top-left corner, offset (-8, -8)   |

Only visible when dev tools enabled.

---

# **11. Context Menus**

### **11.1 Appearance**

| Property           | Value                              |
|--------------------|------------------------------------|
| Background         | Surface-3                          |
| Border             | 1px Border-Default                 |
| Radius             | R-M (4px)                          |
| Shadow             | Z-Popover level                    |
| Min width          | 160px                              |
| Max width          | 320px                              |
| Padding            | P-XS (4U) vertical                 |

### **11.2 Positioning**

| Property           | Value                              |
|--------------------|------------------------------------|
| Trigger            | Right-click position               |
| Offset             | (0, 0) — menu corner at cursor     |
| Screen edge margin | 8px                                |
| Flip behavior      | Flip to opposite side if clipped   |
| Submenu offset     | (-4, 0) overlap with parent        |
| Submenu delay      | 150ms hover before opening         |

### **11.3 Animation**

| Property           | Value                              |
|--------------------|------------------------------------|
| Open               | 100ms fade + scale(0.95 → 1.0)     |
| Close              | 50ms fade-out                      |
| Origin             | Top-left (or flip corner)          |

### **11.4 Keyboard Shortcut Display**

Shortcuts shown in menu items:

| Property           | Value                              |
|--------------------|------------------------------------|
| Font               | Mono-S (12px)                      |
| Color              | Text-Secondary                     |
| Position           | Right-aligned in menu item         |
| Gap from label     | S-L (10U) minimum                  |
| Modifier format    | `Ctrl+S`, `Cmd+Shift+P`            |
| Modifier symbols   | Platform-native (⌘⇧⌥ on Mac)       |

**Shortcut Key Styling (in tooltips):**

```
Background: Surface-2
Border:     1px Border-Subtle
Radius:     R-XS (2px)
Padding:    2px 4px
Font:       Mono-S
```

---

# **12. Validation & Error States**

### **12.1 Input Validation**

| State              | Border          | Icon           | Message          |
|--------------------|-----------------|----------------|------------------|
| Default            | Border-Default  | None           | None             |
| Valid              | Border-Default  | Success (✓)    | None (optional)  |
| Warning            | Warning         | Warning (⚠)    | Below input      |
| Error              | Error           | Error (✕)      | Below input      |

### **12.2 Validation Icon**

| Property           | Value                              |
|--------------------|------------------------------------|
| Size               | I-S (12px)                         |
| Position           | Right side of input, inside border |
| Padding from edge  | P-XS (4U)                          |

### **12.3 Validation Message**

| Property           | Value                              |
|--------------------|------------------------------------|
| Font               | T-XS (11px)                        |
| Position           | Below input, left-aligned          |
| Gap from input     | S-XS (4U)                          |
| Error color        | Error (from 096)                   |
| Warning color      | Warning (from 096)                 |
| Max lines          | 2 (truncate with ellipsis)         |

### **12.4 Form-Level Error Summary**

| Property           | Value                              |
|--------------------|------------------------------------|
| Background         | Error @ 10% opacity                |
| Border             | 1px Error                          |
| Radius             | R-S (3px)                          |
| Padding            | P-M (8U)                           |
| Icon               | I-M Error icon                     |
| Position           | Top of form, or inline             |

---

# **13. Empty States**

### **13.1 List/Grid Empty State**

| Property           | Value                              |
|--------------------|------------------------------------|
| Icon size          | I-XL (20px) or 32px                |
| Icon color         | Icon-Muted                         |
| Title font         | T-M (13px), 500 weight             |
| Title color        | Text-Secondary                     |
| Description font   | T-S (12px)                         |
| Description color  | Text-Muted                         |
| Alignment          | Centered in container              |
| Vertical position  | 33% from top (not dead center)     |

### **13.2 Empty State Content**

```
[Icon]

Title (1 line)
Description (1-2 lines, optional)

[Action Button] (optional)
```

### **13.3 Search "No Results"**

| Property           | Value                              |
|--------------------|------------------------------------|
| Icon               | Search icon, Icon-Muted            |
| Title              | "No results found"                 |
| Description        | Include search query in message    |
| Suggestion         | "Try different keywords" or similar|

---

# **14. Search & Highlighting**

### **14.1 Search Match Highlighting**

| Property           | Value                              |
|--------------------|------------------------------------|
| Background         | Warning @ 40% opacity              |
| Text color         | Text-Primary (unchanged)           |
| Border             | None                               |
| Radius             | 2px (subtle rounding)              |

### **14.2 Current Match (Focused)**

| Property           | Value                              |
|--------------------|------------------------------------|
| Background         | Warning @ 70% opacity              |
| Border             | 1px Warning                        |
| Text color         | Text-Primary                       |

### **14.3 Search Input**

| Property           | Value                              |
|--------------------|------------------------------------|
| Icon               | Search (🔍) I-S, left side         |
| Clear button       | × icon, appears when text present  |
| Placeholder        | "Search..." or context-specific    |
| Results count      | Right side, Text-Secondary         |
| Navigation         | ▲▼ buttons or Enter/Shift+Enter    |

### **14.4 Find-in-Page Bar**

| Property           | Value                              |
|--------------------|------------------------------------|
| Position           | Top-right or top of content area   |
| Background         | Surface-2                          |
| Border             | 1px Border-Default                 |
| Shadow             | Z-Overlay level                    |
| Animation          | Slide down, 150ms                  |

---

# **15. Notification Badges**

### **15.1 Count Badge**

| Property           | Value                              |
|--------------------|------------------------------------|
| Shape              | Pill (fully rounded)               |
| Min size           | 16px diameter (for single digit)   |
| Height             | 16px                               |
| Padding            | 4px horizontal (for multi-digit)   |
| Background         | Error or Accent-Primary            |
| Text color         | Text-Inverse                       |
| Font               | T-XS (11px), 600 weight            |
| Max display        | "99+" for counts over 99           |

### **15.2 Badge Position**

| Context            | Position                           |
|--------------------|------------------------------------|
| Icon button        | Top-right, offset (-4, -4)         |
| Tab                | After tab label, S-XS gap          |
| List item          | Right side, before chevron         |
| Tree node          | After node label                   |

### **15.3 Status Dot**

| Property           | Value                              |
|--------------------|------------------------------------|
| Size               | 8px diameter                       |
| Border             | 2px Surface-0 (creates outline)    |
| Position           | Bottom-right of avatar/icon        |
| Colors             | Success, Warning, Error, Info      |

### **15.4 Unread Indicator**

| Property           | Value                              |
|--------------------|------------------------------------|
| Shape              | Circle                             |
| Size               | 6px diameter                       |
| Color              | Accent-Primary                     |
| Position           | Left of item label, or inline      |

---

# **16. Undo/Redo Feedback**

### **16.1 Change Flash**

When undo/redo affects visible content:

| Property           | Value                              |
|--------------------|------------------------------------|
| Highlight color    | Accent-Primary @ 30%               |
| Animation          | Flash on, 150ms hold, fade 300ms   |
| Scope              | Affected element(s) only           |

### **16.2 Toast Notification**

Optional toast for undo confirmation:

| Property           | Value                              |
|--------------------|------------------------------------|
| Message            | "Action undone" / "Redo: [action]" |
| Position           | Bottom-center                      |
| Duration           | 2000ms                             |
| Action             | "Redo" / "Undo" button             |

---

# **17. Accessibility Considerations**

### **17.1 Reduced Motion**

When `prefers-reduced-motion` is set:

| Change                                              |
|-----------------------------------------------------|
| Disable all non-essential animations                |
| Replace slide/scale with instant appear/disappear   |
| Keep functional animations (progress, loading)      |
| Reduce animation durations by 75%                   |

### **17.2 High Contrast Mode**

| Change                                              |
|-----------------------------------------------------|
| Border-Focus → 2px solid, high contrast color       |
| Accent-Muted → higher opacity for visibility        |
| Increase Border-Default contrast                    |
| Disable subtle shadows                              |

### **17.3 Focus Visibility**

| Change                                              |
|-----------------------------------------------------|
| Focus ring always visible (not just keyboard)       |
| Thicker focus ring (2px instead of 1px)             |
| Focus ring offset (2px outside element)             |

---

# **18. Timing Constants**

All interaction timing values for consistency:

| Constant                   | Value    | Usage                        |
|----------------------------|----------|------------------------------|
| `HOVER_DELAY`              | 0ms      | Immediate hover response     |
| `TOOLTIP_DELAY`            | 500ms    | Before tooltip appears       |
| `TOOLTIP_DELAY_FAST`       | 200ms    | After recent tooltip         |
| `TOOLTIP_LINGER`           | 1500ms   | Fast mode window             |
| `DRAG_THRESHOLD`           | 4px      | Movement before drag starts  |
| `DOUBLE_CLICK_TIME`        | 400ms    | Max time between clicks      |
| `LONG_PRESS_TIME`          | 500ms    | Touch long-press threshold   |
| `AUTO_EXPAND_DELAY`        | 800ms    | Tree node expand on drag hover |
| `SCROLL_FADE_DELAY`        | 300ms    | Before scrollbar fades       |
| `CARET_BLINK_ON`           | 530ms    | Caret visible duration       |
| `CARET_BLINK_OFF`          | 530ms    | Caret hidden duration        |
| `ANIMATION_FAST`           | 100ms    | Quick transitions            |
| `ANIMATION_NORMAL`         | 150ms    | Standard transitions         |
| `ANIMATION_SLOW`           | 250ms    | Emphasized transitions       |

---

# **19. Implementation Plan**

### Phase 1 — Core Interactions

* Splitter resize states
* Column resize states
* Basic drag-and-drop feedback

### Phase 2 — Grid Enhancements

* Sort indicators
* Cell selection
* Frozen column dividers
* Column reorder

### Phase 3 — Text Handling

* Selection colors
* Caret styling
* Cell editing mode

### Phase 4 — Feedback Polish

* Tooltips
* Overflow indicators
* Loading states
* Scroll shadows

### Phase 5 — Accessibility

* Focus ring options
* High contrast mode support
* Reduced motion support

### Phase 6 — QA

* Test all states at all DPI levels
* Verify timing feels responsive
* Cross-platform consistency
