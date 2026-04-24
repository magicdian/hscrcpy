# UITest H.264 Ingress to Renderer

## Goal

Convert official `uitest` scrcpy gRPC stream output into normalized H.264 video ingress and feed the existing host preview/render path.

## Requirements

* Consume route-neutral output from the official gRPC adapter.
* Extract H.264 access units from official stream messages.
* Normalize payloads into existing `PreparedVideoIngress` or equivalent renderer-facing types.
* Preserve route-neutral renderer APIs.
* Validate H.264 access units before presenting when practical.
* Keep live preview based on existing H.264 render path.

## Acceptance Criteria

* [ ] `uitest` route can feed at least one H.264 unit into the existing render surface.
* [ ] Existing `.h264` artifact and live preview behavior still works for H.264.
* [ ] Unexpected codec or malformed payload fails with diagnostics.
* [ ] Renderer code does not know whether frames came from HAP TCP video or official gRPC.
* [ ] Manual bringup command can capture bounded H.264 frames through `--route uitest`.

## Dependencies

* Depends on `04-24-hos-official-scrcpy-grpc-adapter`.
* Depends on `04-24-hos-codec-capability-registry` for selected codec plumbing.

## Out of Scope

* H.265/H.266/VP9/AV1 playback.
* UITest JPEG agent path.
* Audio.
* UI tree or input-control RPC.

## Technical Notes

* Likely files: `crates/hscrcpy-host/src/render/**`, `crates/hscrcpy-host/src/video/h264.rs`, route adapter modules, `apps/hscrcpy-host-cli/src/main.rs`.
* Current H.264 validation expects Annex-B style access units; official stream shape must be confirmed.
