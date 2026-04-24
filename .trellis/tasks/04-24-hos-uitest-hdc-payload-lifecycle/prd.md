# HDC Abstract Socket and Payload Lifecycle

## Goal

Implement the HDC and device-lifecycle pieces needed by the official `uitest` route: abstract-socket forwarding, official payload selection from `third_party/hypium`, stale process handling, and temporary `.so` cleanup.

## Requirements

* Support HDC forwarding from local TCP to `localabstract:scrcpy_grpc_socket`.
* Select a default official scrcpy server `.so` from `third_party/hypium/**`.
* Prefer `libscrcpy_server_unix_*.z.so` for abstract socket mode.
* Keep a CLI/config override for a specific `.so` path.
* Push the selected payload to `/data/local/tmp/scrcpy_server.so`.
* Detect stale `xdevice_scrcpy` or matching `uitest start-daemon` before launch.
* Try to kill stale route-owned processes; fail if kill is denied or the process remains alive.
* Remove `/data/local/tmp/scrcpy_server.so` after launch success or failure.
* Fail startup if `.so` cleanup fails after a failed launch attempt.

## Acceptance Criteria

* [ ] HDC abstraction can model TCP-to-TCP and TCP-to-localabstract forwarding.
* [ ] Payload selector resolves a default archived official `.so`.
* [ ] Missing payload fails before device launch with a payload-selection error.
* [ ] Stale process detection and kill behavior is covered by tests or mock HDC command assertions.
* [ ] Cleanup runs on both success and failure paths.
* [ ] Errors include route, phase, command target, and relevant device output.

## Dependencies

* Depends on `04-24-hos-route-abstraction-cli`.

## Out of Scope

* gRPC client implementation.
* H.264 payload parsing.
* Automatic fallback to HAP route.

## Technical Notes

* Likely files: `crates/hscrcpy-host/src/hdc/mod.rs`, new route module under `crates/hscrcpy-host/src/**`, and CLI option plumbing if override path is exposed here.
* Use `hdc -t <target> fport tcp:<local> localabstract:scrcpy_grpc_socket`.
