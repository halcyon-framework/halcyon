# SPEC-PM-023: Composite Editor Widget Ownership and Duplicated Lifecycle

**Author:** JC  
**Status:** Binding — adopted 2026-10-29  
**Owner:** Core Architecture  
**Applies To:** ComboBox, SpinBox, Slider and TagInput under HAL-101 sections 2.4–2.5; their engine transaction, construction and factory/choice integration  
**Governing law:** HAL-002/002A, PM-000, PM-014; coordinated HAL-101 sections 2.4–2.5 and 8.4.6

## Pathology and prevention

The observed source pathology is a host directly containing a TextField Widget outside WidgetTree and manually replicating its lifecycle. A stored node ID followed by direct peer access, a parent context reused as child authority, or an extra host-driven child Update does not correct that ownership failure. For the expressly admitted HAL-101 section 2.4 path only, the engine owns the actual child and delegated session; no host manually implements peer lifecycle. Preserve PM-014's four outcomes: current owner-qualified keyboard Capture before editing, cache metrics from the real text/font/width owner, pointer forwarding gated to the actual text region, and complete caret/selection/motion cleanup on exit. Their new mechanism is the qualified tree/engine owner, not manual host calls into a peer Widget. Legacy embedded paths remain subject to PM-014 and gain no exception or new feature permission. No native failure or conformance result is asserted by this source-based record.

## Ordered transaction and readiness prevention

A parent context forwarded to a child, a host borrow surviving child dispatch, a result delivered at a later frame, or a callback moved across its existing clear/reset point is not a correct ownership migration. A Layout sync/installation site must not be preserved behind a renamed helper. Test the actual ordered engine transaction and ready publication, not adjacent API calls.

## Adoption and evidence boundary

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under the supervising technical reviewer's coordinated adoption, with JC's final ownership and veto retained. The exact three source bindings and adopted scope remain in the retained adoption record. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.

The exact ownership record comes from section 4.11 of the immutable ownership/session packet, SHA-256 `aa4911e8c31a2b0d589f6ae25d768f5de510e79107964ab5d1605c575b179b76`; the exact additional prevention rule comes from section 8 of the immutable engine transaction packet, SHA-256 `80f16e49ba46a2e379a6678cbb62deb1d9f530a9bea7cc538f78b79bba6aa94c`. PM-014 remains verbatim. This source-based pathology record asserts no native failure or executable conformance result.
