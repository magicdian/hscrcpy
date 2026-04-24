# Device-side real H.264 capture and encode pipeline

## Goal

Wire a HarmonyOS-official screen capture and H.264 encode path into the device native runtime so the existing video channel can stream real access units.

## Requirements

- Stay primarily inside `sources/hscrcpy_server/**`.
- Keep the N-API and ArkTS shell thin; do not move streaming logic into ArkTS.
- Prefer official HarmonyOS media APIs and sample patterns, especially `AVScreenCapture` and `AVCodec`.
- Preserve the current packet header and session/video channel activation model unless a hard blocker is found.
- Keep write scope focused on device native/runtime files and module config needed for media/runtime permissions.

## Acceptance Criteria

- [ ] The device runtime no longer depends on `GetPlaceholderH264AccessUnitBytes()` for the H.264 mainline path.
- [ ] The video channel sends real H.264 access units produced from a screen-capture + encode pipeline, or clearly reports why the path cannot start.
- [ ] Native logging and error propagation include operation names and phase context.
- [ ] Any new runtime modules follow the existing backend directory and cross-layer guidelines.

## Write Scope

- `sources/hscrcpy_server/**`
- Shared contract docs only if absolutely required by the implementation and no host-owned doc task is blocked

## Technical Notes

- Primary integration points are `entry/src/main/cpp/transport/video_channel.cpp`, `entry/src/main/cpp/video/h264_video_path.*`, and any new `capture/` or `codec/` modules.
- Review official sample structure before introducing new helpers to avoid inventing the wrong abstraction.
