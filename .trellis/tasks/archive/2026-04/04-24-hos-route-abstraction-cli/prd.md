# Route Abstraction and CLI Selection

## Goal

Introduce route-neutral host startup abstractions and explicit CLI route selection so `uitest` and `hscrcpy_server` can coexist without coupling CLI/render code to either route's wire protocol.

## Requirements

* Add a route selection model for at least `uitest` and `hscrcpy-server`.
* Default route is `uitest`.
* `hscrcpy-server` remains explicitly selectable for fallback/debug.
* Route selection must be visible in diagnostics and errors.
* Keep renderer-facing code route-neutral.
* Do not implement official gRPC stream parsing in this task.

## Acceptance Criteria

* [ ] CLI accepts an explicit route option.
* [ ] Invalid route values fail with clear usage text.
* [ ] Host startup passes a route enum/config instead of hard-coding `HdcCompanionManager`.
* [ ] Route-neutral traits or structs exist for launch, stream session, and video ingress ownership.
* [ ] Existing HAP route can still be selected.
* [ ] Unit tests cover route parsing and default route behavior.

## Dependencies

* Parent: `04-23-hos-host-launch-readiness`

## Out of Scope

* Abstract socket forwarding.
* Official gRPC/protobuf adapter.
* H.264 frame ingestion.
* Device-side changes under `sources/hscrcpy_server/**`.

## Technical Notes

* Likely files: `apps/hscrcpy-host-cli/src/main.rs`, `crates/hscrcpy-host/src/session/**`, `crates/hscrcpy-host/src/companion/**`.
* Preserve current HAP startup behavior behind the `hscrcpy-server` route.
