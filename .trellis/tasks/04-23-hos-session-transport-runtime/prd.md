# Implement Session Transport Runtime

## Goal

Land the runtime `session` / `video` transport path between the Rust host and HarmonyOS companion on top of the negotiated contracts, replacing preview-only behavior with concrete runtime channel setup.

## Requirements

* Respect the architecture contract as the source of truth for:
  * `session` vs `video` channel split
  * handshake ordering
  * JSON control-plane semantics on `session`
  * binary payload semantics on `video`
* Split implementation across host and device write scopes instead of coupling both sides into one task.
* Keep `control_event` traffic on the session/control plane.
* Do not collapse transport logic into ArkTS UI files.

## Acceptance Criteria

* [ ] Host and device child tasks have explicit runtime ownership.
* [ ] The runtime path advances the project beyond preview-only negotiation.
* [ ] Channel setup semantics remain aligned with the architecture contract.
* [ ] The work stays compatible with the H.264 mainline + JPEG fallback model.

## Children

* `04-23-hos-device-session-transport`
* `04-23-hos-host-session-transport`
