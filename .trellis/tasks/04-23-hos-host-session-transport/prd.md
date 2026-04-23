# Implement Host Session Transport Runtime

## Goal

Add the Rust host-side runtime `session` / `video` transport path so negotiated channels can connect to the device companion and feed later render work.

## Requirements

* Stay outside `sources/hscrcpy_server/**`.
* Build on the architecture contract plus the completed HDC runtime and companion deploy work.
* Own host-side channel connection, handshake progression, and runtime message/payload movement.
* Preserve the split between:
  * session/control plane
  * video payload plane
* Keep the result compatible with the existing control baseline and negotiated H.264/JPEG paths.

## Acceptance Criteria

* [ ] Host code can advance from startup planning into runtime transport/channel setup.
* [ ] Session-plane runtime behavior matches the contract ordering.
* [ ] Video payload ingress is represented clearly enough for the later render task to consume.
* [ ] Rust verification passes for the touched host modules.

## Dependencies

* `04-23-hos-host-hdc-runtime`
* `04-23-hos-host-companion-deploy`
* Should be coordinated with `04-23-hos-device-session-transport`

## Write Scope

* Host Rust/project files such as `crates/**`, `apps/**`, `docs/**` if needed
* Do not modify `sources/hscrcpy_server/**`
