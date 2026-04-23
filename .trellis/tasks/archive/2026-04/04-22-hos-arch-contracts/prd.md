# Define Host-Device Architecture and Contracts

## Goal

Define the first executable architecture contract for `hscrcpy` so later host and device subagents can work in parallel without inventing incompatible startup, packaging, or session interfaces.

## Requirements

* Produce a concrete architecture note for the current MVP direction:
  * desktop-driven startup
  * companion HAP plus native core on HarmonyOS
  * Rust host as orchestration entry point
* Define initial contracts for:
  * device companion installation/update flow
  * first-run authorization flow
  * startup handshake
  * capability negotiation
  * session/channel split
* Identify which contracts are stable now and which remain provisional.
* Reflect the output in project docs or task artifacts that later tasks can cite.
* Include a pointer to relevant HarmonyOS sample patterns if they materially affect the design.

## Acceptance Criteria

* [ ] A concrete architecture artifact exists in the repo.
* [ ] Startup and install/update ownership are explicit.
* [ ] Capability negotiation fields are named explicitly enough for host and device scaffolding to depend on them.
* [ ] Video and control channel responsibilities are defined at a high level.
* [ ] Open questions are listed instead of left implicit.

## Dependencies

* May use existing brainstorm PRD and official sample research.
* Should complete before feature-level streaming implementation tasks.

## Write Scope

* Preferred: docs or architecture notes under new repo-owned paths.
* May update task artifacts related to `hos-scrcpy-brainstorm`.
* Do not modify `sources/hscrcpy_server` implementation files.
* Do not bootstrap the Rust host workspace here.
