# UITest Real gRPC Transport E2E

## Goal

Make `--route uitest --codec h264` a real end-to-end projection path: the host launches the official `libscrcpy_server*.z.so`, connects to `scrcpy_grpc_socket`, calls official `ScrcpyService/onStart`, converts streamed H.264 messages into existing route-neutral ingress, and feeds the current renderer / ffplay preview until interrupted or `--max-frames` is reached.

## Requirements

* Replace the deferred official scrcpy transport with a real transport for `ScrcpyService/onStart`, `onEnd`, and `onRequestIDRFrame`.
* Keep generated or protocol-specific gRPC/protobuf details isolated inside the host official adapter layer; CLI and renderer must stay route-neutral.
* Support the current official proto shape documented in `official_scrcpy::protocol::SCRCPY_PROTO_SCHEMA`.
* Preserve the current UITest lifecycle: payload selection, push, stale process kill, `uitest start-daemon`, `localabstract:scrcpy_grpc_socket` forwarding, and cleanup.
* Feed real streamed H.264 access units into `OfficialScrcpyIngressAdapter` and then the existing `BringupRenderSurface` / ffplay path.
* Maintain fail-fast route behavior: `--route uitest` must not silently fall back to `hscrcpy-server`.
* Update diagnostics and docs so manual validation distinguishes real stream success from startup, protocol, frame-shape, and render failures.

## Acceptance Criteria

* [x] `cargo run -p hscrcpy-host-cli -- --device auto --route uitest --codec h264 --ffplay-bin /opt/homebrew/bin/ffplay` opens the official stream and ffplay displays real device frames on a supported device.
* [x] `--max-frames <n>` exits after receiving and rendering `n` official H.264 access units.
* [x] `onEnd` is called during normal shutdown and best-effort cleanup runs on error.
* [x] Unit tests cover protobuf/gRPC message encoding/decoding or the selected real transport boundary.
* [x] Integration-style host tests cover a mocked official stream feeding the existing renderer path without CLI/render importing generated protocol types.
* [x] Failure messages include route, phase, target, method, selected payload, selected codec, and underlying transport/protocol context.
* [x] `cargo fmt --check`, `cargo test --workspace -- --nocapture`, and `git diff --check` pass.
* [x] Docs update the previous deferred-gRPC blocker into concrete manual validation steps and known residual limitations.

## Technical Notes

* Current implementation uses a standard `tonic` + `prost` generated gRPC client and a repo-local `scrcpy.proto` reconstructed from the generated descriptor in the DevEco Testing / Hypium wheel.
* The data flow must remain: `UITest lifecycle -> official gRPC client -> OfficialScrcpyVideoMessage -> OfficialScrcpyIngressAdapter -> PreparedVideoIngress::H264 -> BringupRenderSurface -> ffplay`.
* Existing HAP route behavior must remain intact for `--route hscrcpy-server`.
* Manual validation on 2026-04-24 confirmed `--route uitest --codec h264` starts a session, opens ffplay, captures H.264 frames, and emits full diagnostic windows. UITest route startup is slower than the HAP route; track that as a follow-up performance issue rather than an E2E blocker.
