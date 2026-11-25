# **SPEC-HAL-071 — Declarative Animation Language & State Machine DSL**

**SPEC-HAL-071**
**Declarative Animation Language & State Machine DSL**
**Author:** Halcyon Architect
**Status:** Draft
**Version:** 1.0

**Depends On:**

* SPEC-HAL-024 Absolute Logical Space
* SPEC-HAL-034 Rendering Pass Graph
* SPEC-HAL-048 Text Layout Engine
* SPEC-HAL-070 Timeline Animation System

**Implements:**
A **declarative animation language + compile-time macro DSL** for defining:

* animation timelines
* keyframes
* easing curves
* multi-track transitions
* widget state machines
* nested animation graphs
* synchronized multi-widget sequences

070 defines the *engine*.
071 defines the *language* that feeds it.

---

## **1. Motivation**

070 provides raw building blocks:

* Timelines
* Tracks
* Keyframes
* Graph transitions

But writing all this manually is:

* verbose
* error-prone
* unreadable for complex widgets
* hard to debug
* impossible to visually inspect

071 introduces a **domain-specific animation language** that compiles to SPEC-070 structures.

This becomes Halcyon’s animation “truth source”.

---

## **2. Goals**

### **Primary**

* A concise, readable animation DSL
* Compile-time validation
* Zero runtime overhead
* Works for widgets, components, scenes
* Supports sequences and sync groups

### **Secondary**

* Reusable presets
* Variables & expressions
* Conditional branches
* Nested timelines

---

## **3. DSL Overview**

### **3.1 Core Syntax Goals**

The language must feel like:

* CSS keyframes
* Figma timelines
* Motion graphics scripts
* Rust macros

But with complete determinism.

### **3.2 Example: Simple Button Animation**

```
anim! {
    states {
        idle {
            opacity: 1.0;
            scale: 1.00;
        }
        hover {
            opacity: 1.0 => 1.0;
            scale: 1.00 => 1.05 @ ease_out;
        }
        pressed {
            scale: 1.05 => 0.96 @ ease_in;
            glow:  0.0  => 0.3  @ cubic(0.3, 0.7, 0.2, 1.0);
        }
    }

    transitions {
        idle   -> hover   on hover_enter;
        hover  -> idle    on hover_leave;
        hover  -> pressed on mouse_down;
        pressed-> hover   on mouse_up;
    }
}
```

This expands entirely into the SPEC-070 engine:

* timelines
* tracks
* keyframes
* transitions

No runtime parsing.

---

## **4. Language Definition**

### **4.1 High-Level Grammar (Informal)**

```
AnimationFile =
    Option<import*> "\n"
    "anim!" "{" 
       StatesBlock
       TransitionsBlock
       Option<SequencesBlock>
    "}"

StatesBlock =
    "states" "{" State* "}"

State =
    Ident "{" Track* "}"

Track =
    Property ":" Keyframe ("=>" Keyframe)? "@"? Easing? ";"

Keyframe =
    Number

TransitionsBlock =
    "transitions" "{" Transition* "}"

Transition =
    Ident "->" Ident "on" Trigger ";"

Trigger =
    Ident | string-literal

SequencesBlock =
    "sequences" "{" Sequence* "}"
```

This is intentionally simple.

---

## **5. Supported Properties**

Matches SPEC-070:

```
opacity
scale
rotation
position.x
position.y
color
shadow.offset
glow.radius
text.color
outline.thickness
```

---

## **6. Easing Functions**

### **Built-in**

```
linear
ease_in
ease_out
ease_in_out
```

### **Cubic**

```
cubic(x1, y1, x2, y2)
```

Matches CSS cubic-bezier.

---

## **7. Complex Examples**

### **7.1 Modal Popup**

```
anim! {
    states {
        hidden {
            opacity: 0.0;
            scale: 0.95;
            blur: 6.0;
        }

        shown {
            opacity: 0.0 => 1.0 @ ease_out;
            scale: 0.95 => 1.0 @ ease_out;
            blur: 6.0 => 0.0 @ ease_out;
        }
    }

    transitions {
        hidden -> shown on show;
        shown  -> hidden on hide;
    }

    sequences {
        open_modal {
            at 0ms: trigger show;
            at 200ms: play sound "whoosh";
        }
    }
}
```

---

### **7.2 TextField Focus**

```
anim! {
    states {
        unfocused {
            cursor_opacity: 0.0;
            outline_thickness: 1.0;
        }

        focused {
            cursor_opacity: 0.0 => 1.0 @ linear.repeat(500ms);
            outline_thickness: 1.0 => 2.0 @ ease_out;
        }
    }

    transitions {
        unfocused -> focused on focus_gained;
        focused   -> unfocused on focus_lost;
    }
}
```

---

## **8. Sequences and Sync Groups**

### **8.1 Sync Groups**

```
sequences {
    login_screen_enter {
        group header {
            at 0ms: trigger "fade_in_title";
            at 50ms: trigger "fade_in_subtitle";
        }

        group form {
            at 120ms: trigger "fade_in_username";
            at 180ms: trigger "fade_in_password";
            at 250ms: trigger "fade_in_button";
        }
    }
}
```

This allows **choreography across the scene**.

---

## **9. Compile-Time Behavior**

### **9.1 Macro Expands Into:**

* `Timeline` structs
* `Track` structs
* `Keyframe` arrays
* `AnimationGraph`
* `StateNode`
* `Transition`
* `Sequence` structures

### **9.2 Validation Errors (examples)**

* Unknown property
* Unknown state
* Unknown trigger
* Non-monotonic keyframe times
* Missing easing after “=>”
* Cyclic graph without explicit cycle flag
* Fractional ms unacceptable at compile time

---

## **10. Runtime Behavior**

Zero parsing.
Zero strings.
Zero allocations.

All data is directly embedded as `const` and referenced by widgets.

---

## **11. Testing Matrix**

### **Language-Level**

* Lexing of all constructs
* Macro expansion correctness
* Error conditions checked

### **Runtime-Level**

* States transition correctly
* Timelines evaluate correctly
* Interruptions behave predictably
* Sequences fire triggers as expected

---

## **12. Examples Included**

* Button
* TextField
* Popup
* Modal
* Tabs
* Toasts
* Scene page transition
* Multi-widget orchestra

---

## **13. Implementation Plan**

### **Phase 1 — Macro Parser**

* Tokenizer
* State/block grammar
* Error reporting

### **Phase 2 — AST to Engine Mapping**

* Build Timelines
* Build Tracks
* Build Keyframes
* Build Graph

### **Phase 3 — Sequence Compiler**

* Time → trigger graph

### **Phase 4 — Runtime API**

* `AnimController` for widgets
* Hooking into widget lifecycle

### **Phase 5 — Integrate With Renderer**

* Write instance parameters
* Synchronize with frame clock

### **Phase 6 — IDE Visualization (Optional)**

* Real-time timeline editor
* Sync with Halcyon debug overlay

---

## **14. Future Work**

* External `.anim` files (not just macros)
* VFX transition language
* Audio sync integration
* Behavior trees for complex UI choreography
