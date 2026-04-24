# Host-side real H.264 stream validation

## Goal

Validate that the host startup and H.264 ingest path behaves correctly once the device emits real H.264 packets, and tighten host/runtime checks or docs as needed.

## Requirements

- Stay outside `sources/hscrcpy_server/**`.
- Build on the existing Rust session startup, packet parsing, and H.264 ingest path.
- Own host-side contract or documentation updates if real-stream behavior changes host assumptions.
- Keep this task focused on validation, parsing, startup sequencing, and tests instead of redesigning the device protocol.

## Acceptance Criteria

- [ ] Host-side H.264 ingest assumptions are explicitly validated against real access-unit behavior.
- [ ] Any necessary host/runtime changes remain bounded to startup, packet validation, render bringup, tests, or docs.
- [ ] Rust verification passes for touched host modules.
- [ ] Device-side ownership is preserved; this task does not modify `sources/hscrcpy_server/**`.

## Write Scope

- `crates/**`
- `apps/**`
- `docs/**`

## Technical Notes

- Initial focus files are `crates/hscrcpy-host/src/session/startup.rs` and `crates/hscrcpy-host/src/video/h264.rs`.
- The current runtime already accepts Annex-B-style H.264 payload bytes structurally; this task should verify whether additional validation or readiness handling is warranted once real packets arrive.
