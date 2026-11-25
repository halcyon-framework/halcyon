# **SPEC-HAL-072 — Declarative Widget Style System (Halcyon StyleSheets v1)**

**SPEC-HAL-072**
**Declarative Widget Style System (Halcyon StyleSheets v1)**
**Author:** Halcyon Architect
**Status:** Draft
**Version:** 1.0

**Depends On:**

* SPEC-HAL-024 Absolute Logical Space
* SPEC-HAL-032 Hit-Test Acceleration
* SPEC-HAL-046 Widget Model v2
* SPEC-HAL-048 Text Layout Engine
* SPEC-HAL-022 Layout Pass Model
* SPEC-HAL-071 Animation DSL
* SPEC-HAL-090 Logical Grid & Compact Layout Rules
* SPEC-HAL-096 Color System & Semantic Palette

**Implements:**
A unified **style language** (like CSS but sane, deterministic, Rust-native) for Halcyon:

* Declarative styling for all widgets
* States + variants
* Inheritance
* Resolve-once style trees
* Integration with animations (SPEC-071)
* Integration with theme engines
* Zero runtime parsing

This is the **styling backbone** for the entire UI framework.

---

## **1. Motivation**

Halcyon widgets currently:

* define appearance *inside* Rust code
* have ad-hoc style fields
* cannot be themed consistently
* duplicate logic across Button, Panel, TextBlock, InputField
* have no dynamic “variant/state → style” semantics
* lack a global theming mechanism

072 introduces:

* A **style language**
* A **resolver**
* A **compiled stylesheet tree**
* A **widget style inheritance model**

The goal is to give Halcyon:

* theming
* skins
* variants
* dark/light switching
* animated style transitions
* consistent style rules

---

## **2. Goals**

### **Primary**

* Stylesheets defined in a macro DSL
* Deterministic compiler producing static Rust data
* No string-based lookups at runtime
* State-dependent property resolution
* Widget-type-based selectors
* Variant selectors (e.g. “Button.primary”, “Button.danger”)

### **Secondary**

* Integration with SPEC-071 animations
* Hot-reload support (future SPEC)
* Style inspector/debug overlay
* Internal resolution cache

---

## **3. High-Level Model**

A stylesheet defines:

1. **Scopes** (e.g. global, dark, light, high-contrast)
2. **Selectors** (widget type, variant, state)
3. **Properties** (colors, padding, radii, borders, typography)
4. **States** (hover, pressed, focused, disabled)
5. **Transitions** (via animation DSL)

---

## **4. Syntax Overview (DSL)**

### **Example Sheet**

```
style! {
    scope default {
        Button {
            padding: 8px 14px;
            radius: 6px;
            bg: #303030;
            text.color: #ffffff;
            font.size: 15px;

            state hover {
                bg: #3A3A3A;
            }

            state pressed {
                bg: #222222;
                scale: 0.96;
            }

            variant primary {
                bg: #0A84FF;
                text.color: #ffffff;

                state hover { bg: #0B95FF; }
                state pressed { bg: #0870D0; }
            }
        }

        Panel {
            bg: #1E1E1E;
            padding: 12px;
            radius: 8px;
        }
    }
}
```

### **Everything expands at compile time**.

---

## **5. Selectors**

### **5.1 Widget Type**

```
Button {
    ...
}
```

Matches all Button widgets.

### **5.2 Variant**

```
Button.primary {
    ...
}
```

### **5.3 State**

Inside a type or variant:

```
state hover {
    bg: ...
}
```

States match widget boolean flags:

```
hover
pressed
focused
disabled
selected
```

### **5.4 Combined**

```
Button.primary state disabled {
    opacity: 0.5;
}
```

---

## **6. Supported Properties**

### **Layout**

```
padding
margin
radius
border.width
border.color
shadow
outline
```

### **Typography**

```
font.family
font.size
font.weight
font.style
text.color
line.height
```

### **Color / Background**

```
bg
color
gradient
opacity
```

### **Effects**

```
shadow.blur
shadow.offset
glow
blur
scale
rotation
```

### **Animation Integration**

```
transition: anim_ref("button_hover");
```

(Integrates with SPEC-071’s animation graph.)

---

## **7. Inheritance Model**

Halcyon uses a *CSS-like cascade*, but deterministic and simplified:

* Widget type rules apply first
* Variant overrides type
* State overrides variant
* Inline widget overrides all (rare, discouraged)

No magic.
No browser insanity.
No specificity games.

---

## **8. Resolution Model**

### **Step 1 — Preprocess into TRIE-like Selector Tree**

Compile-time.

### **Step 2 — Widget Creation**

Widget has:

* type: “Button”
* variant: Optional<&'static str>
* states: Bitmask

The style resolver:

1. Finds type block
2. Merges variant block
3. Applies active states
4. Produces a **StyleSnapshot** (static struct)

### **Step 3 — Render Pass**

Renderer receives resolved properties:

* bg_color
* text_color
* border
* radii
* shadow
* typography
* etc.

No lookups.

---

## **9. Integration With Animations (SPEC-071)**

Animations modify style properties:

* scale
* text.color
* opacity
* bg
* shadow
* etc.

072 defines:

```
transition: anim_ref("button_press");
```

Which gives the widget a **style-affecting animation**.

---

## **10. Theming System (Multi-Scope)**

Example:

```
scope light {
    Button { bg: #F0F0F0; text.color: #000; }
}
scope dark {
    Button { bg: #222; text.color: #FFF; }
}
```

Runtime switches:

```
ui.set_theme("dark");
```

Instant re-resolution.

---

## **11. Testing Matrix**

### **Selector-Level**

* Type matching
* Variant matching
* State matching
* Precedence rules

### **Property-Level**

* Padding merging
* Radii
* Border correctness
* Typography resolution
* Color parsing

### **Integration-Level**

* Animated property overrides
* Theming switch re-resolution
* Behavior under nested widgets

---

## **12. Implementation Plan**

### **Phase 1 — Parser Macro**

* Tokenize DSL
* Build AST
* Pre-validate

### **Phase 2 — Selector Tree Builder**

* Construct trie:

    * widget → variant → state

### **Phase 3 — Resolver Engine**

* Merge properties according to precedence
* Build `StyleSnapshot` struct

### **Phase 4 — Widget Integration**

* Each widget stores a handle to its style snapshot
* On theme change: resolve again

### **Phase 5 — Animation Binding**

* Style properties become animation tracks
* Animations override resolved values

### **Phase 6 — Styling Debug Overlay**

Optional:

* Inspect widget
* View resolved style
* View ancestry

---

## **13. Future Work**

* External `.style` files
* Hot reload
* Visual style editor
* Token-based design system
* Automatic dark/light derivation
