# Implement Host H264 Mainline

## Goal

Add the Rust host-side `H.264` mainline path and explicit JPEG fallback negotiation on top of the host baseline so the host prefers `H.264` while keeping the fallback path intact.

## Requirements

* Stay outside `sources/hscrcpy_server/**`.
* Use the architecture contract as the source of truth for capability negotiation and session/video semantics.
* Build on the current host foundation, JPEG baseline, and input-control baseline.
* Add an explicit `H.264` mainline path without removing the JPEG fallback behavior.
* Keep the result layered so later real media transport/decoding work can extend it cleanly.

## Acceptance Criteria

* [ ] Host-side code has an explicit H.264 mainline path or module boundary.
* [ ] Negotiation logic prefers `H.264` while retaining a clear JPEG fallback.
* [ ] The result compiles and tests cleanly with cargo checks.
* [ ] Control/session behavior remains compatible with the existing baseline.

## Write Scope

* Root Rust/project files such as `Cargo.toml`, `Cargo.lock`, `apps/**`, `crates/**`, `docs/**` if needed
* Do not modify `sources/hscrcpy_server/**`
