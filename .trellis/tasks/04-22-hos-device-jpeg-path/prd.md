# Implement Device JPEG Video Path

## Goal

Add the HarmonyOS-side JPEG baseline path on top of the companion foundation so the device can produce a first usable video payload path for bring-up and fallback.

## Requirements

* Stay within `sources/hscrcpy_server/**`.
* Use the architecture contract as the source of truth for session/video semantics.
* Build on the new native-core and N-API boundary from the device foundation task.
* Implement only the JPEG baseline path for the device side.
* Do not add `H.264` in this task.
* Keep ArkTS shell thin; video-path logic belongs in native code.

## Acceptance Criteria

* [ ] Device-side code has an explicit JPEG video-path module or equivalent boundary.
* [ ] Session bootstrap / preview logic can describe or activate the JPEG path.
* [ ] Host-facing data shape is aligned with the architecture contract well enough for the host JPEG task to consume.
* [ ] The implementation remains clearly scaffold-to-baseline, not mixed with future H.264 logic.

## Write Scope

* `sources/hscrcpy_server/**` only

## Verification Notes

* If DevEco IDE compile/package verification is needed, report the exact manual IDE action for the user.
