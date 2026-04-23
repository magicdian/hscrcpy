# Implement Device H264 Mainline

## Goal

Add the HarmonyOS-side `H.264` mainline path and explicit JPEG fallback negotiation on top of the companion baseline so the device can advertise and shape the preferred MVP video path.

## Requirements

* Stay within `sources/hscrcpy_server/**`.
* Use the architecture contract as the source of truth for capability negotiation and session/video semantics.
* Build on the current device foundation, JPEG baseline, and input-control baseline.
* Add an explicit `H.264` mainline path without removing or obscuring the JPEG fallback path.
* Keep ArkTS thin; codec and negotiation logic belong in native code.

## Acceptance Criteria

* [ ] Device-side code has an explicit H.264 path or module boundary.
* [ ] Device capability advertisement can expose both `h264` and `jpeg` where appropriate.
* [ ] Session preview/startup data can represent mainline selection and JPEG fallback coherently.
* [ ] The result preserves the current architecture split and remains compatible with future deeper codec work.

## Write Scope

* `sources/hscrcpy_server/**` only

## Verification Notes

* If DevEco IDE compile/package verification is needed, report the exact manual IDE action for the user.
