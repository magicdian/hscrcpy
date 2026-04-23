# Implement Device Input Control Path

## Goal

Add the HarmonyOS-side control message handling and input injection scaffold on top of the companion foundation so the device can accept host-issued control events through the session channel.

## Requirements

* Stay within `sources/hscrcpy_server/**`.
* Use the architecture contract as the source of truth for `control_event` message shape and authorization semantics.
* Build on the current native-core and N-API boundary.
* Move device-side control support from contract-only placeholders toward a real baseline path.
* Keep ArkTS thin; input/control handling belongs in native code or clearly isolated boundary code.
* Do not add H.264 logic in this task.

## Acceptance Criteria

* [ ] Device-side code has an explicit control/input path or module boundary.
* [ ] Session preview/startup data can represent `control.enabled=true` without using the old JPEG-only rejection path.
* [ ] Input injection constraints or unsupported cases are surfaced explicitly in code/comments/contracts.
* [ ] The implementation remains separable from future video codec work.

## Write Scope

* `sources/hscrcpy_server/**` only

## Verification Notes

* If DevEco IDE compile/package verification is needed, report the exact manual IDE action for the user.
