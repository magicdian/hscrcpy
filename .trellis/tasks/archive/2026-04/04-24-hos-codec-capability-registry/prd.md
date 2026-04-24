# Route-Neutral Codec Capability Registry

## Goal

Add a route-neutral codec capability model that combines route/device encoder support and host decoder support without hard-coding the universe to H.264/H.265/JPEG. Current playback remains H.264-first.

## Requirements

* Represent codec names as an extensible registry/descriptor model.
* Include known names such as `h264`, `h265`, `h266`, `vp9`, `av1`, and `jpeg`.
* Keep `h264` as the only required end-to-end implementation target for this phase.
* Model route/device encoder capability separately from host decoder capability.
* Select the first user-preferred codec supported by both sides.
* Produce fallback reasons for diagnostics.
* If live preview depends on `ffplay`, host capability probing may initially use `ffplay` availability/support.

## Acceptance Criteria

* [ ] Codec descriptors can represent future codecs without changing the selection data model.
* [ ] Selection tests cover H.264 success, preferred codec fallback, and no shared codec.
* [ ] Diagnostics can print route-supported codecs, host-supported codecs, selected codec, and fallback reason.
* [ ] Route adapters can consume the same codec selection result.
* [ ] H.265/H.266/VP9/AV1 actual playback remains out of scope.

## Dependencies

* Can run after or alongside `04-24-hos-route-abstraction-cli`.

## Out of Scope

* Real H.265/H.266/VP9/AV1 decoding/rendering.
* Device-side AVCodec probing under the HAP route.
* Official route codec reverse engineering beyond H.264.

## Technical Notes

* Likely files: `crates/hscrcpy-contracts/src/session.rs`, `crates/hscrcpy-host/src/video/**`, `crates/hscrcpy-host/src/session/startup.rs`.
* Existing `VideoCodec` enum may need a compatibility layer or an extensible descriptor wrapper.
