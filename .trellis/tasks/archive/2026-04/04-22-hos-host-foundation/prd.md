# Build Desktop Host Foundation

## Goal

Bootstrap the Rust host workspace and orchestration baseline so the desktop side can eventually install/update the companion, manage HDC interactions, and own session startup.

## Requirements

* Introduce a Rust workspace or equivalent host layout at the repo root.
* Establish module boundaries for:
  * HDC interaction
  * companion install/update logic
  * session startup/orchestration
  * future rendering/client entry points
* Keep the first pass focused on foundation, not full functionality.
* Use the shared contracts task as the interface source of truth where available.
* Keep file ownership away from `sources/hscrcpy_server/**`.

## Acceptance Criteria

* [ ] A Rust host workspace or initial package layout exists.
* [ ] Host responsibilities are separated into clear modules instead of a single binary file.
* [ ] Install/update and session orchestration concepts are represented in code structure.
* [ ] The resulting layout is ready for later JPEG/H.264 and control tasks.

## Dependencies

* Should align with the shared architecture/contracts task.
* Can start before device streaming implementation exists.

## Write Scope

* Root Rust/project files such as `Cargo.toml`, `Cargo.lock`, `crates/**`, `apps/**`, `src/**`, or equivalent new host paths.
* Do not modify `sources/hscrcpy_server/**`.
