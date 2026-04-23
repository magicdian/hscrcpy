# Implement Host HDC Runtime

## Goal

Replace the current host-side `HdcBridge` stubs with a real runtime adapter for device discovery, shell execution, and port forwarding so later session transport work can rely on concrete HDC behavior.

## Requirements

* Stay outside `sources/hscrcpy_server/**`.
* Own the `crates/hscrcpy-host/src/hdc/**` boundary and adjacent host-only tests/helpers.
* Do not take ownership of companion install/update policy or session startup orchestration in this task.
* Expose concrete behavior for:
  * ensuring a target device is visible
  * executing shell commands on the device
  * binding or forwarding local/runtime ports needed by later session work
* Keep the result testable behind the existing trait boundary.

## Acceptance Criteria

* [ ] `HdcBridge` is backed by a concrete runtime implementation instead of all-`NotImplemented` stubs.
* [ ] Device discovery, shell execution, and port-forwarding behavior are represented in code and error handling.
* [ ] The implementation is usable by later session startup code without forcing transport-specific logic into this task.
* [ ] Rust verification passes for the touched host modules.

## Write Scope

* `crates/hscrcpy-host/src/hdc/**`
* Host-side tests/helpers adjacent to the HDC module
* Minimal host-only support files if necessary
* Do not modify `sources/hscrcpy_server/**`
* Avoid editing `crates/hscrcpy-host/src/session/startup.rs` unless strictly required for compilation
