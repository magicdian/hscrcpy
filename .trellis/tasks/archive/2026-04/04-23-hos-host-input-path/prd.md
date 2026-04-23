# Implement Host Input Control Path

## Goal

Add the Rust host-side control message emission and session-control flow on top of the host foundation so the host can prepare and route scrcpy-like control events cleanly.

## Requirements

* Stay outside `sources/hscrcpy_server/**`.
* Use the architecture contract as the source of truth for `control_event` shape, sequencing, and normalized coordinates.
* Build on the Rust host foundation and current session startup flow.
* Move host-side control support from planning to a real baseline path.
* Do not add H.264 logic in this task.

## Acceptance Criteria

* [ ] Host-side code has an explicit control/input module or boundary.
* [ ] Session startup/control flow can support `enable_control=true` intentionally rather than as a placeholder.
* [ ] Control event types and sequencing are represented in host-side types or modules clearly enough for later integration.
* [ ] The result compiles with cargo checks.

## Write Scope

* Root Rust/project files such as `Cargo.toml`, `Cargo.lock`, `apps/**`, `crates/**`, `docs/**` if needed
* Do not modify `sources/hscrcpy_server/**`
