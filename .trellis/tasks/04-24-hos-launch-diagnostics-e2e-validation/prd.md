# Launch Diagnostics and End-to-End Validation

## Goal

Add route-aware diagnostics and validation coverage for successful and failed `uitest` startup so failures are actionable and do not silently fall back to the HAP route.

## Requirements

* Fail fast on selected-route failure.
* Never automatically fall back from `uitest` to `hscrcpy-server`.
* Include route, phase, command/socket target, selected payload, selected codec, and device output in errors where relevant.
* Validate stale process, payload cleanup, socket readiness, gRPC start, first frame, and shutdown paths.
* Document manual bringup commands.

## Acceptance Criteria

* [x] Failure during payload selection, push, stale kill, launch, cleanup, fport, readiness, gRPC start, or frame ingestion reports the correct phase.
* [x] `uitest` startup failure does not start HAP route.
* [x] Successful shutdown calls `onEnd` and releases route resources as far as current mock official adapter and lifecycle abstractions allow.
* [x] Manual validation steps are documented in the parent or task notes.
* [x] Rust verification passes for touched host modules.

## Final Status

Implemented route-aware diagnostics and validation coverage through the current host abstractions. The `uitest` route now fails fast at deferred official `ScrcpyService/onStart` startup with route, phase, target, selected payload, selected codec, method, and receive-limit context instead of entering the HAP session protocol.

Real-device `--route uitest` H.264 capture remains blocked because real Rust HTTP/2 gRPC/protobuf transport is not implemented in the offline dependency environment. Covered scope is lifecycle, adapter protocol conversion, mock official H.264 ingress, shutdown delegation, diagnostics, and manual validation docs.

## Dependencies

* Depends on all implementation subtasks:
  * `04-24-hos-route-abstraction-cli`
  * `04-24-hos-uitest-hdc-payload-lifecycle`
  * `04-24-hos-codec-capability-registry`
  * `04-24-hos-official-scrcpy-grpc-adapter`
  * `04-24-hos-uitest-h264-render-ingress`

## Out of Scope

* New product UI.
* Automatic recovery/fallback policy.
* Multi-device or multi-session support.

## Technical Notes

* This task should be the final integration/validation slice.
* Include both mockable unit tests and real-device manual verification notes where automated coverage is not practical.
