# Implement Host Companion Deploy Runtime

## Goal

Replace the current host-side companion install/update stubs with a concrete deployment/runtime plan so the desktop host can reason about the bundled HarmonyOS companion artifact and apply install-or-update actions.

## Requirements

* Stay outside `sources/hscrcpy_server/**` for writes.
* Own the `crates/hscrcpy-host/src/companion/**` boundary plus host-packaged companion metadata/assets if introduced.
* Build on the architecture contract install/update states:
  * `missing`
  * `ready`
  * `upgrade_required`
  * `protocol_incompatible`
  * `companion_too_new`
* Introduce a real manifest/metadata representation for the bundled companion artifact.
* Do not take ownership of HDC runtime implementation details beyond consuming the existing abstraction.

## Acceptance Criteria

* [ ] Companion plan/apply logic is concrete instead of all-`NotImplemented`.
* [ ] Bundled companion metadata is represented in host code or assets.
* [ ] Deploy decisions align with the architecture contract states.
* [ ] Rust verification passes for the touched host modules.

## Write Scope

* `crates/hscrcpy-host/src/companion/**`
* Host-side metadata/assets such as `assets/**`, `docs/**`, or root Rust files if needed
* Minimal read-only use of `sources/hscrcpy_server/**` for package identity context is allowed, but no writes there
* Avoid editing `crates/hscrcpy-host/src/session/startup.rs` unless strictly required for compilation
