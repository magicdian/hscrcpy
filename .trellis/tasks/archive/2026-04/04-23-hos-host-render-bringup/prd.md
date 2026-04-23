# Implement Host Render Bringup

## Goal

Bring up an initial desktop-side render path for negotiated video units once the runtime transport begins delivering frames to the host.

## Requirements

* Stay outside `sources/hscrcpy_server/**`.
* Build on the negotiated video-unit semantics and host-side transport output.
* Own the `crates/hscrcpy-host/src/render/**` boundary and any host-only entry-point glue needed to exercise it.
* Keep the first pass focused on bring-up, not a polished final UI.
* Avoid rewriting transport/session logic in this task.

## Acceptance Criteria

* [ ] Host render code advances beyond `NotImplemented`.
* [ ] There is a concrete path from negotiated video units into a render/decode sink or bring-up surface.
* [ ] The implementation remains compatible with both `H264Mainline` and `JpegFallback`.
* [ ] Rust verification passes for the touched host modules.

## Dependencies

* `04-23-hos-host-session-transport`

## Write Scope

* `crates/hscrcpy-host/src/render/**`
* Host-only entry-point or test/demo files if needed
* Do not modify `sources/hscrcpy_server/**`
