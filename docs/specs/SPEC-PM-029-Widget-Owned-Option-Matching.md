# SPEC-PM-029: Widget-Owned Option Matching

**Status** Recorded failure and binding prevention under SPEC-PM-000.
**Date** 2026-11-18.
**Owner** Selection model and projection adapters.
**Governing design** HAL-002 §3.11 and HAL-201 §7.

## Observed failure

At `4efc76e4`, `ComboBox::replace_snapshot` built a lowercase label cache in the widget. `update_search_highlight` compared the query with that cache and `compute_ghost_completion` matched a label again to extract the suffix. Both paths interpreted option labels inside the widget even though they produced only presentation hints.

Presentation hints can obscure which layer interprets the data. The prevention applies to option controls consuming model projections, including the ComboBox.

## Prevention

**INV-PM-029-001** The model or projection adapter interprets the raw query against the current ordered options. The widget consumes the resulting highlight ID and ghost suffix. It may resolve a supplied ID to its displayed label to qualify the ghost's source.

**INV-PM-029-002** The legacy prefix preview preserves the unfiltered order and current draft. Explicit activation alone reaches the selection resolver. A provider may publish a distinct filtered projection through its existing contract.

Widget-owned normalized label indexes, prefix comparisons, and query-derived suffix extraction are forbidden.

`combo_legacy_type_to_search_model_preview_preserves_ghost_and_options` exercises both projection paths, the source-qualified Unicode suffix, loading and source replacement, and explicit resolver entry through the mounted ComboBox.
