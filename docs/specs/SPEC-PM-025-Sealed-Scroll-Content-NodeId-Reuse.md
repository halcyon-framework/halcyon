# SPEC-PM-025 — Reuse of a sealed ScrollArea content slot

- **Status** Recorded postmortem under existing authority
- **Date** 2026-11-10
- **Owner** Core Architecture
- **Scope** Removal of wired content while its sealed ScrollArea survives

## Verified failure

The original read-only reproduction sealed a live ScrollArea on first Layout.
Removing its original child retired
`TreeWidgetId(3)` and concrete mount generation `3`. Adding a different child
under the surviving ScrollArea reused `NodeId(2)` with `TreeWidgetId(4)` and
mount generation `4`. The sealed `Deferred<NodeId>` still returned `NodeId(2)`.
Layout consumed the new child and post-layout anchor processing returned
normally. The unchanged parent and recycled slot hid the loss of the wired
logical child. [HAL-087](SPEC-HAL-087-Widget-Identity-Model.md)

## Prevention under existing authority

`WidgetTree::remove_node` checks the requested node and its live immediate
parent before any tree mutation. If that parent is a sealed ScrollArea and its
wired content is the requested node, the operation hard-fails in every build
before unlinking, retirement, registry changes or slot reuse. A current parent
and slot alone cannot establish continuity of a retired wired child.
[HAL-154 §§B–F](SPEC-HAL-154-Sealed-After-Layout-Deferred-Wiring-Contract.md)

Whole-subtree teardown retires the host and its content together. Pre-seal
removals, unrelated siblings and standalone empty hosts keep their ordinary
removal path. `replace_widget_only` preserves the logical node and its
`TreeWidgetId` while advancing the concrete mount. The resolver realization
check remains a separate post-layout responsibility.
[HAL-087](SPEC-HAL-087-Widget-Identity-Model.md) ·
[PM-024](SPEC-PM-024-Anchor-Realization-Guard-Scope.md)

The observed failure was isolated to the tree fixture. The historical consumer
failure's exact wired pair and physical scanout remain unqualified.
