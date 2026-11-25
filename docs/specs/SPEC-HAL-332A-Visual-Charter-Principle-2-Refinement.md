# SPEC-HAL-332A — Visual Charter Principle 2 Refinement: Tonal Compression and Saturation Discipline

### *Tonal Hierarchy clarified to accept both data-lighter and data-darker patterns within a compressed dynamic range; pure-saturation prohibition made explicit.*

**Author:** JC
**Status:** Draft (Binding upon adoption)
**Amends:** SPEC-HAL-332 (Professional Visual Charter), §4 Principle 2
**Amendment Type:** A (Clarifying)

---

# 1. Purpose

Refine the wording of SPEC-HAL-332 Principle 2 to accurately describe the substance of the charter's tonal-hierarchy rule.

The original Principle 2 ("Tonal Hierarchy: Data Recesses Into Chrome") prescribed a single directional pattern — chrome brighter, data darker — and rejected the inverse. An audit of Halcyon's dark theme against this principle (see AUDIT-HAL-332-DARK-THEME, §Principle 2) found that the dark theme operates with the **opposite** direction: data is *lighter* than chrome (`surfaces.content` = 0x19191C at ~26 luminance vs `surfaces.level1` = 0x161618 at ~23 luminance). The dark theme nonetheless reads as charter-compliant in spirit because every surface sits within a compressed luminance range (10-42 of 255), no element is at saturation, and the chrome reads as structure while data reads as substance.

The substantive rule the dark theme actually demonstrates is not "data must be darker than chrome." It is: **no element may dominate adjacent surfaces through saturation; the relationship between chrome and data must be intentional and consistent within a theme; and the surface stack must occupy a compressed dynamic range without saturated endpoints.**

This amendment rewrites Principle 2 to state that rule directly. The change is **clarifying** — it does not weaken the charter, does not change which themes are compliant, and does not change the diagnosis of the light-theme problem that motivated SPEC-HAL-332 in the first place. The light-theme defect (`surfaces.content` = 0xFFFFFF dominating gray chrome) violates the refined principle as plainly as it violated the original: pure white *is* pure saturation.

---

# 2. Amendment Type Justification

Per SPEC-HAL-002A §3, this amendment qualifies as **Type A — Clarifying**.

- Behavior preserved: the dark theme, which charter §10 implicitly endorses by listing the showcase status bar as "correctly pitched," now reads as compliant rather than as a charter exception.
- No invariant weakened: every violation the original Principle 2 ruled out is still ruled out under the refined wording (light-theme white surfaces remain rejected; consumer-app card pattern remains rejected).
- No new invariant added: the rule was already implicit in the rejection of saturated dominance; this amendment makes it explicit.
- No downstream SPEC change required beyond the wording substitution.

This is **not** a Type B (extending) amendment because no new prohibition or invariant is introduced. The refined principle's constraints are strictly a subset-equivalent of the original's actual constraints once "dominance via saturation" is understood as the operative concept.

---

# 3. Scope

**Sections touched:** SPEC-HAL-332 §4 Principle 2 (text body).

**Sections preserved unchanged:** SPEC-HAL-332 §1 (Motivation), §2 (Audience), §3 (Genre), §4 Principles 1, 3-11, §5 (Anti-Principles), §6 (Decision Protocol), §7 (Decision Log), §8 (Alignment with Existing SPECs), §9 (Open Recalibration Items), §10 (Test of Adoption). All other principles and protocols remain as adopted.

**Implementing SPECs affected:** SPEC-HAL-096 Color System. No change to the recalibration agenda — the §9 recalibration items still apply, only the language of the principle they implement is refined.

---

# 4. Refined Principle 2 Text

The following replaces SPEC-HAL-332 §4 Principle 2 in its entirety upon adoption.

---

### Principle 2 — Tonal Compression and Saturation Discipline

The surface hierarchy of every default theme must satisfy three constraints simultaneously:

**4.1 No saturated endpoints.** No surface in a default theme may be pure black (`#000000`) or pure white (`#FFFFFF`), nor approach them within roughly 8 luminance levels. The surface stack occupies a compressed dynamic range with deliberate margins at both extremes.

*Rationale.* Saturated surfaces dominate every adjacent element regardless of intent. A pure-white data area on any gray chrome reads as a card slammed onto the page — the consumer-app pattern Halcyon rejects. A pure-black panel against any near-black canvas produces unreadable transitions. The compressed-range constraint forecloses both failure modes.

**4.2 No dominance through saturation differential.** No surface may differ from its immediate neighbor by more than approximately one-third of the total dynamic range used by the theme. A data surface within ~10-15 luminance levels of its panel reads as a layered relationship; one that's 30+ levels brighter reads as a competing visual element. The eye must perceive the surfaces as belonging to the same composition, not as foreground-versus-background.

*Rationale.* The visual harm in the light-theme defect was not direction (data lighter than chrome) but magnitude (data 17+ levels brighter than chrome, while data itself was at saturation). Constraining the differential rules out the same defect regardless of direction.

**4.3 Intentional and consistent direction within a theme.** Within a given theme, the choice of whether data sits lighter than chrome (Bloomberg pattern, Halcyon's current dark theme) or darker than chrome (DaVinci Resolve pattern, classical instrumentation displays) is a deliberate design decision. Whichever direction is chosen, it must be applied consistently across all data-bearing surfaces within that theme. Data surfaces must not pick directions individually.

*Rationale.* Both directions exist in the §3 reference applications. Both are valid for the genre. What is invalid is *inconsistency* — a List that's lighter than its panel while a TreeView in the same theme is darker than its panel. The eye reads the inconsistency as visual chaos.

**Anti-pattern explicitly rejected:** pure-white data surfaces lifted off a gray panel surround. This is the "white card on gray page" pattern of every SaaS product. Under 4.1, pure white is forbidden. Under 4.2, a saturation differential against gray chrome would be excessive even at near-white. Under 4.3, mixing this pattern with any other data-surface direction is invalid. Three independent gates close on the same defect.

**Audit method.** Compliance is checked by:
- Listing every surface token in the theme (`surfaces.*`, `list.row_background`, `list.header_background`, `list.subheader_background`, and any per-widget surface overrides).
- Computing approximate luminance for each (R+G+B÷3 of the hex bytes).
- Verifying no value is below ~12 or above ~243 (the ~8-level margins at both extremes).
- Verifying adjacent-surface differentials do not exceed one-third of the theme's used range.
- Verifying all data-bearing surfaces (TreeView body, List body, PE body, TableView body, TextField body) sit on the same side of their containing panel (all lighter, or all darker — not mixed).

**Implementing SPEC.** SPEC-HAL-096 Color System (surface hierarchy values for both light and dark themes).

---

# 5. Consistency Check

The refined Principle 2 is checked against the audit findings:

| Theme element | Original Principle 2 | Refined Principle 2 |
|---|---|---|
| Dark theme `surfaces.content` lighter than `surfaces.level1` | **Violation** (data must be darker than chrome) | **Compliant** (4.1 no saturation, 4.2 small differential of 3 levels, 4.3 consistent direction across all data surfaces) |
| Light theme `surfaces.content = 0xFFFFFF` | **Violation** (data brighter than chrome) | **Violation** (4.1 pure saturation, 4.2 excessive differential against panel) |
| Light theme `list.header_background = 0xDFE1E5` against `surfaces.level1 = 0xE4E5E8` | **Implicit problem** (sections invisible) | **Implicit problem** under Principle 4 (typographic hierarchy) — Principle 2 governs surfaces, Principle 4 governs hierarchy markers. The audit correctly attributed the section-invisibility issue partly to Principle 2 (chrome and band too close) and partly to Principle 4 (no weight/case differentiation). The refined Principle 2 does not weaken this diagnosis. |

The refined principle rules in everything the dark theme does, rules out everything the light theme defect does, and does not change the diagnosis of any other charter-relevant defect.

---

# 6. Charter Coherence

The amendment is checked against the rest of SPEC-HAL-332:

- **§3 Genre:** the refined principle accommodates both Bloomberg (data lighter) and DaVinci Resolve (data darker) patterns, which are both in the genre. The original principle implicitly excluded Bloomberg, which contradicted §3. Refinement restores coherence.
- **§5 Anti-Principles:** "Cards" remains forbidden. The 4.1 + 4.2 + 4.3 stack independently rules out the visual signature of cards (saturated surface, large differential, inconsistent with the rest of the theme's surface direction).
- **§6 Decision Protocol:** unchanged. Step 2 now checks the refined Principle 2 wording rather than the original.
- **§9 Open Recalibration Items:** unchanged. The light-theme recalibration still needs to satisfy the principle; the principle simply states its requirements more accurately now.
- **§10 Test of Adoption:** the test point "the light theme presents data surfaces equal-to-or-darker than panel surfaces, with no pure white in default themes" is updated by this amendment to: *"the light theme contains no pure-white surfaces; light-theme surface differentials remain within the per-theme compressed-range bound; light-theme data surfaces follow a single consistent direction relative to chrome (either all lighter or all darker, chosen deliberately)."* This is a clarifying restatement of the same intent — the light theme is still required to fix the white-card defect — and is the only §10 wording affected by this amendment.

---

# 7. Adoption

Upon adoption:

1. SPEC-HAL-332 §4 Principle 2 is replaced in-place with the text in §4 of this amendment.
2. SPEC-HAL-332 §10 Test of Adoption updates the light-theme test point per §6 above.
3. The dark theme is declared charter-compliant without requiring source change.
4. The light theme remains non-compliant and the §9 recalibration items proceed as originally planned, now with the refined principle as their governing target.
5. This amendment is filed at `docs/specs/SPEC-HAL-332A-Visual-Charter-Principle-2-Refinement.md`. SPEC-HAL-332 itself is updated in place per the in-place amendment convention (HAL-002A §4 Step 6).

---

**End of SPEC-HAL-332A.**
