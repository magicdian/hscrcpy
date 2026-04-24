# Implement real H.264 screen stream bringup

## Goal

Replace the current placeholder H.264 payload path with a real HarmonyOS screen-capture and H.264-encode pipeline while keeping the existing host-device session and video-channel contract intact.

## Requirements

- Preserve the current two-channel MVP contract in `docs/architecture/host-device-mvp-contract.md`.
- Prefer HarmonyOS official media APIs and sample patterns for screen capture and H.264 encode.
- Use `AVScreenCapture` and `AVCodec`-style native seams where the current SDK/runtime allows.
- Keep ArkTS thin; native code owns capture, encode, transport, and runtime state.
- Split work into device-side implementation and host-side validation tasks with disjoint write scopes.

## Acceptance Criteria

- [ ] A device-side runtime path emits real H.264 access units over the existing video channel instead of structural placeholders.
- [ ] Host-side runtime assumptions are validated against real H.264 packets and tightened where needed.
- [ ] Error handling and logging clearly expose capability, startup, and stream-failure context.
- [ ] Task outputs reference official HarmonyOS docs or samples that justify the chosen API path.

## Technical Notes

- Current placeholder seams live in `sources/hscrcpy_server/entry/src/main/cpp/video/h264_video_path.cpp` and `sources/hscrcpy_server/entry/src/main/cpp/transport/video_channel.cpp`.
- Current host ingest path lives in `crates/hscrcpy-host/src/video/h264.rs`.
- Official references under review include OpenHarmony `AVScreenCapture` docs and HarmonyOS sample projects such as `AVCodecVideo` and `AVCodecBufferMode`.
