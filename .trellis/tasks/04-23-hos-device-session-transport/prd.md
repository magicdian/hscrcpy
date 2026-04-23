# Implement Device Session Transport Runtime

## Goal

Add the HarmonyOS-side runtime `session` / `video` transport plumbing so the companion can move beyond preview-only negotiation and prepare actual handshake/control/video traffic delivery.

## Requirements

* Stay within `sources/hscrcpy_server/**`.
* Keep ArkTS shell thin; native code owns runtime transport setup.
* Build on the existing device foundation, JPEG/H.264 negotiation, and control baseline.
* Introduce concrete device-side runtime channel behavior for:
  * session/control-plane readiness
  * video-plane activation after session readiness
  * runtime framing boundaries that remain compatible with the architecture contract
* Do not assume the final host implementation already exists; keep seams explicit and testable.

## Acceptance Criteria

* [ ] Device code has explicit runtime transport/channel ownership beyond preview-only data shaping.
* [ ] Session-plane and video-plane activation rules match the architecture contract.
* [ ] Existing negotiation and control behavior remain intact.
* [ ] If DevEco IDE validation is needed, the exact manual step is reported.

## Write Scope

* `sources/hscrcpy_server/**` only
