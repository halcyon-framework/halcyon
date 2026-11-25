# SPEC-PM-028: Detached Command Custody and False Completion

**Author:** JC
**Status:** Recorded failure and binding prevention under SPEC-PM-000.
**Date:** 2026-11-13.
**Owner:** Renderer submission and transient-resource lifetime.
**Governing design:** HAL-385 section 9, coordinated HAL-040 and HAL-081B. HAL-002/002A are unchanged.

## Observed failure

HAL385's first full candidate kept a mutable public raw CommandBuffer beside private frost-use custody. A caller could retain the original encoded command using mem::replace and submit or cancel an empty replacement while leaving the custody attached to the output. The owner then certified the wrong command. Queue completion was genuine, but it did not prove settlement of the command that referenced the backing.

The actual Metal regression frosted_glass_raw_command_replacement_retains_unconfirmed_backing retained the original unsubmitted command, completed its replacement and observed the next Compile reuse the same backing: before=(1,27288), after=(1,27288). It failed 0/1 with actual child exit 101. This proves premature reuse, not an observed crash, corruption or physical-display failure.

The original regression, candidate-source and restoration records were retained privately. Archived candidate patch SHA256: 955f6fea911bc634a94258b7112c2e8749131343b0b0cef28dd6632408d34e47. These are historical verification records, not public reproduction artifacts linked by this document. The unsafe production candidate was withdrawn; prior positive pixel tests did not close this lifetime defect.

## Why it recurs

A private credential authenticates its own record, not whichever mutable object currently sits next to it. Reaching the correct submit function and receiving a real queue callback are insufficient when original-command custody was split earlier. Surface identity, preparation identity and wrapper survival cannot repair that split.

## Prevention

**INV-PM-028-001:** The exact executable command and the lifetime obligation it certifies remain inseparable until the owner submits that command or disposes of it unsubmitted.

**INV-PM-028-002:** Raw export irrevocably removes reusable-custody authority before exposing the raw command. Exported or submission-uncertain resources stay protected and charged; reconstruction never restores that authority.

**INV-PM-028-003:** Completion evidence belongs to the original submitted command on its actual queue. Completing or cancelling a replacement, unrelated work or another target is not a substitute.

**INV-PM-028-004:** Test the substitution boundary and actual reuse, not only ordinary correct submission. Real pixel correctness does not establish lifetime correctness.

Forbidden: detached lease plus replaceable raw command; late custody lookup by SurfaceId; Debug/address guesses of CommandBuffer identity; reclaiming exported work from wrapper Drop; test-only authentication shortcuts; relabeling the recorded RED as a visual PASS.

This record adds no rendering feature, scheduler or alternative lifetime service. HAL-385 section 9 owns the precise corrective API. Proposed PM-022 remains at its existing status and is not activated by this record.
