# Host realtime decode display bringup

## Goal

Upgrade the host H.264 bringup path from artifact dumping only to live decode/display, while preserving the existing artifact outputs for debugging and regression analysis.

## Requirements

- Stay within host-side Rust code (`crates/**`, `apps/**`, optionally `docs/**`).
- Preserve the existing session startup and video ingest behavior.
- Keep H.264 artifact dumping available so bringup remains inspectable.
- Add a live preview path that works for the current H.264 mainline without introducing unnecessary architecture churn.
- Surface clear runtime failures when live preview cannot be started or fed.

## Acceptance Criteria

- [ ] Running the host CLI against a live H.264 session opens a realtime display path instead of only writing files.
- [ ] Existing artifact output remains available for debugging.
- [ ] The new preview path fails clearly and does not silently corrupt the existing bringup path.
- [ ] Host tests pass for touched modules.

## Technical Notes

- Current bringup renderer lives in `crates/hscrcpy-host/src/render/bringup.rs`.
- Current CLI entry point lives in `apps/hscrcpy-host-cli/src/main.rs`.
- The current pragmatic bringup direction is acceptable if it keeps the render module boundaries explicit and the data flow simple to evolve later into a built-in decoder.
