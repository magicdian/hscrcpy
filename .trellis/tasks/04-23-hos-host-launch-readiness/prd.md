# Implement Host Launch Readiness Retry

## Goal

Add a bounded readiness/retry path after companion launch so the host waits briefly for the device-side session listener to become connectable instead of failing immediately.

## Requirements

* Stay outside `sources/hscrcpy_server/**`.
* Build on the existing host HDC runtime, companion launch abstraction, and session startup flow.
* Keep this task narrow:
  * bounded retry/backoff or readiness probing
  * phase-specific error reporting around launch/session connect
* Do not redesign protocol framing or device launch ownership.

## Acceptance Criteria

* [ ] Host startup no longer attempts exactly one immediate session connect after launch.
* [ ] Retry/readiness behavior is bounded and surfaces clear failure context if the listener never becomes ready.
* [ ] Rust verification passes for the touched host modules.

## Dependencies

* Best applied after or alongside `04-23-hos-device-session-listener-runtime`

## Write Scope

* Host Rust/project files such as `crates/**`, `apps/**`, `docs/**` if needed
* Do not modify `sources/hscrcpy_server/**`
