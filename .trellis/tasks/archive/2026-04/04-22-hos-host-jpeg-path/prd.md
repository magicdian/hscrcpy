# Implement Host JPEG Video Path

## Goal

Add the Rust host-side JPEG baseline path on top of the host foundation so the host can negotiate, receive, and route the fallback video path cleanly.

## Requirements

* Stay outside `sources/hscrcpy_server/**`.
* Use the architecture contract as the source of truth for session/video semantics.
* Build on the Rust workspace and host/session/contracts crates from the host foundation task.
* Implement only the JPEG baseline path for the host side.
* Do not add `H.264` in this task.

## Acceptance Criteria

* [ ] Host-side code has an explicit JPEG path in contracts/session/render or adjacent modules.
* [ ] Session startup can select or preserve the JPEG fallback path explicitly.
* [ ] The host-side path is separated enough that later H.264 work can layer on top cleanly.
* [ ] The result compiles with cargo checks.

## Write Scope

* Root Rust/project files such as `Cargo.toml`, `Cargo.lock`, `apps/**`, `crates/**`, `docs/**` if needed
* Do not modify `sources/hscrcpy_server/**`
