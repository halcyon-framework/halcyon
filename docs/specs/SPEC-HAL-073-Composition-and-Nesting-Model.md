# **SPEC-HAL-073 — Composition & Nesting Model for Declarative Styles (Variant Trees v1)**

**SPEC-HAL-073**
**Composition & Nesting Model for Declarative StyleSheets (Variant Trees v1)**
**Author:** Halcyon Architect
**Status:** Draft
**Version:** 1.0

**Depends On:**

* SPEC-HAL-046 Widget Model v2
* SPEC-HAL-071 Animation DSL
* SPEC-HAL-072 Declarative StyleSheets
* SPEC-HAL-024 Absolute Logical Space
* SPEC-HAL-092 Unified Typography Rules

**Implements:**
A deterministic, hierarchical **style inheritance & composition system** that allows:

* nested widgets to inherit parent style contexts
* variant propagation
* local overrides
* cross-widget style composition
* style “mixins”
* shadow scopes (internal/private styles)
* global → theme → layout → widget → state cascade

073 defines **how styles combine across the tree**, which 072 deliberately left at single-widget scope.

---

# **1. Motivation**

StyleSheets (072) cover per-widget styling, but **real UI design requires composition**:

* A Panel must style all children (default padding, bg, radii)
* A theme must define global typography, defaults, spaces
* A component (e.g. Card, Dialog, Frame) must expose a **style boundary**
* Widgets must inherit typography, colors, and layout from ancestors
* Variants should cascade downward (Button inside Toolbar inherits Toolbar variant)

The current system has only flat selectors — not enough.

SPEC-073 adds:

* hierarchical style scopes
* parent-driven defaults
* variant propagation
* controlled overrides
* style mixins

This makes the styling system actually usable in full UI design.

---

# **2. Goals**

### **Primary**

* Deterministic cascade from root → leaf
* Every child widget receives a merged style context
* Parent provides implicit defaults
* Child can override, but cannot “escape” hierarchy
* Variant propagation rules (e.g., `Panel.brand → all children brand`)

### **Secondary**

* Mixins — reusable property bundles
* Shadow scopes for components
* Layout-driven inheritance (Panels/Frames propagate spacing)
* Developer-defined “style contexts”

---

# **3. Style Context Model**

### **3.1 Style Context Object**

Each widget carries:

```
struct StyleContext {
    pub scope: &'static str,          // default, dark, light, high_contrast
    pub variant: Option<&'static str>,
    pub mixes: SmallVec<[StyleMixin; 4]>,
    pub overrides: StyleSnapshot,     // inline overrides
    pub inherited: StyleSnapshot,     // merged from parent
}
```

The effective style is:

```
effective_style = inherited + variant_rules + local_overrides
```

### **3.2 Context Inheritance**

At widget creation:

```
child.cx.scope    = parent.cx.scope
child.cx.variant  = parent.cx.variant (unless child sets one)
child.cx.mixes    = parent.cx.mixes + child mixes
child.cx.inherited = parent.effective_style (unless property marked non-inheritable)
```

This gives Halcyon true **hierarchical style behavior**.

---

# **4. Variant Propagation Rules**

Variants propagate **top-down** unless explicitly blocked.

Example:

```
Toolbar.variant("compact") → all children compact unless override
```

Propagation is:

1. Parent sets variant V
2. Child inherits V
3. Child may override with its own variant
4. “variant none” resets inheritance (rare use)

---

# **5. Shadow Scopes**

Some widgets (e.g. TextField) want internal styling that does NOT leak to children.

SPEC-073 introduces:

```
shadow scope
```

When entering a shadow scope:

* inherited style is frozen
* parent mixins do NOT propagate
* only widget-type-level rules apply

Example usage:

```
TextField {
    shadow {
        InputCaret { color: #0A84FF; }
        Placeholder { color: #666; }
    }
}
```

Shadow scopes prevent theme pollution.

---

# **6. Mixins**

Mixins are reusable property bundles.

### **Definition**

```
mixin rounded {
    radius: 8px;
    shadow: 0px 2px 6px rgba(0,0,0,0.25);
}
```

### **Usage**

```
Button {
    use rounded;
    padding: 10px 16px;
}
```

### **Propagation**

Mixins propagate like variants unless they are “local”:

```
mixin rounded(local) { ... } 
```

Local mixins do NOT propagate.

---

# **7. Property Inheritance Rules**

Each property has an inheritance flag:

```
bg: NON-INHERITED
padding: INHERITED
font.size: INHERITED
text.color: INHERITED
radius: INHERITED
border.*: NON-INHERITED
shadow: NON-INHERITED (unless explicitly inherited)
```

Default rules:

* Typography → inherited
* Spacing → inherited
* Colors → inherited
* Background, border, shadow → not inherited

This matches real design patterns.

---

# **8. Cross-Widget Composition**

073 defines the rule for composition within composite widgets:

### **Case: Component exposing internal slots**

Example:

```
Card {
    header: Header {}
    body: Body {}
    footer: Footer {}
}
```

Each slot inherits:

* scope
* variant
* mixins
* inherited properties

But internal shadow scopes inside components protect internals.

This creates a clean separation between:

* external influence
* internal structure

---

# **9. Compilation Model**

### **Compile-time**

* Resolve mixins
* Validate inheritance flags
* Precompute selector trees
* Generate `STATIC StyleSheet` tables

### **Runtime**

* StyleContext propagation
* Resolution per widget
* On theme/variant changes → invalidate subtree
* Fast merge (SIMD friendly small structs)

Everything is predictable and fast.

---

# **10. Testing Matrix**

### **Hierarchy Tests**

* Parent → child inheritance
* Variant propagation
* Mixins propagation
* Shadow scope isolation
* Proper override merging

### **Property Tests**

* Typography inheritance
* Padding propagation
* Color inheritance
* Non-inherited properties isolation

### **Integration Tests**

* Components with nested children
* Global theme switching
* Variant combos
* Shadow scope behavior

---

# **11. Implementation Plan**

### **Phase 1 — StyleContext Struct**

Add into core widget struct.

### **Phase 2 — Inheritance Engine**

Propagate:

* scope
* variant
* mixins
* inherited properties

### **Phase 3 — Variant/Mixin Resolution**

Compile mixins into property sets.

### **Phase 4 — Shadow Scope Support**

Implement push/pop of scope during widget build.

### **Phase 5 — Component Integration**

Expose style boundary for composite widgets.

### **Phase 6 — Rewrite Button/Panel/TextBlock**

Make first widgets use 072+073 fully.

---

# **12. Future Work**

* Theme packs
* Hot reload
* Animated theme transitions
* Style inspector overlay
