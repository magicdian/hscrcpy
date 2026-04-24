# Official Scrcpy gRPC Adapter

## Goal

Add an isolated Rust adapter for the official scrcpy gRPC service while preventing generated protobuf/gRPC types from leaking into CLI or renderer layers.

## Requirements

* Add Rust protobuf/gRPC mapping for observed `scrcpy.proto`.
* Implement adapter calls for `onStart`, `onEnd`, and `onRequestIDRFrame`.
* Keep generated or protocol-specific types in a dedicated module.
* Expose route-neutral stream messages or normalized video ingress to upper layers.
* Set gRPC receive limits large enough for video frames.
* Unknown or unexpected `ReplyMessage` shape should fail with diagnostics rather than guessing.

## Acceptance Criteria

* [ ] Rust workspace builds with required gRPC/protobuf dependencies.
* [ ] Adapter can construct a client for the forwarded local TCP endpoint.
* [ ] Adapter exposes start, stop, request-IDR, and receive-stream behavior behind route-neutral types.
* [ ] Unit tests or adapter tests cover message conversion for known payload fields and unknown shape failure.
* [ ] CLI/render modules do not import generated protobuf/gRPC types.

## Dependencies

* Depends on `04-24-hos-route-abstraction-cli`.
* Uses forwarding produced by `04-24-hos-uitest-hdc-payload-lifecycle`.

## Out of Scope

* Device payload launch.
* Renderer integration.
* Non-H.264 official stream variants.

## Technical Notes

* Observed service:
  * `onStart(Empty) returns (stream ReplyMessage)`
  * `onEnd(Empty) returns (ReplyEndMessage)`
  * `onRequestIDRFrame(Empty) returns (ReplyEndMessage)`
* Observed message fields:
  * `ReplyMessage.data`
  * `ReplyMessage.reply_type`
  * `ReplyMessage.payload: map<string, ParamValue>`
  * `ParamValue.val_bytes` is a likely H.264 payload candidate but must be validated with real stream data.
