# Implement Device Video Channel Runtime

## Goal

Add the real device-side video channel runtime so negotiated sessions produce binary video packets for the host render bringup path.

## Requirements

* Stay within `sources/hscrcpy_server/**`.
* Build on the existing session listener/runtime and current video transport state model.
* Do not redesign the session handshake; focus on the `video` plane after `session_ready`.
* Minimum target:
  * activate a real video listener/runtime after session negotiation
  * accept the host video-channel connection
  * write binary packets with the existing host-expected header semantics:
    * `codec`
    * `pts_us`
    * `is_keyframe`
    * `payload_length`
  * produce at least a minimal viable JPEG packet path first, while keeping H264 compatibility in structure
* Preserve the existing negotiated codec path and control/session behavior.

## Acceptance Criteria

* [ ] Host no longer fails immediately reading the video packet header because the device actually writes binary video packets.
* [ ] The runtime stays aligned with the negotiated codec and session id.
* [ ] The implementation remains compatible with later real capture/encode integration.
* [ ] If DevEco IDE validation is needed, the exact manual steps are reported.

## Write Scope

* `sources/hscrcpy_server/**` only
