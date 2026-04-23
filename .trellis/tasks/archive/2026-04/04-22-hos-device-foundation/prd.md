# Build Device Companion Foundation

## Goal

Refactor the generated DevEco project into a thin companion shell with a clearer native-core boundary, so later media and control work can land without rewriting the project structure again.

## Requirements

* Keep the HarmonyOS project rooted at `sources/hscrcpy_server`.
* Move the template toward this structure:
  * thin ArkTS shell for lifecycle, permissions, and configuration
  * explicit N-API boundary
  * native-side foundation ready for later capture/transport/control modules
* Avoid embedding streaming or transport business logic into page files.
* If UI changes are needed, first align with relevant official HarmonyOS sample patterns.
* Preserve buildability assumptions for DevEco even if we cannot compile locally.

## Acceptance Criteria

* [ ] The DevEco scaffold no longer reads like an untouched template.
* [ ] ArkTS responsibility is limited to shell/lifecycle duties.
* [ ] Native bridge API shape is explicit enough for later session startup work.
* [ ] File organization points toward future native modules instead of one monolithic source file.
* [ ] Any build or module metadata changes are consistent with the new structure.

## Dependencies

* Should follow the shared architecture/contracts task.
* May use official sample research in parallel.

## Write Scope

* `sources/hscrcpy_server/**`
* Do not create or modify Rust host workspace files.
* Do not implement JPEG or H.264 transport yet beyond interface scaffolding.
