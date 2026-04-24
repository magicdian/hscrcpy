# Directory Structure

> How non-UI runtime code is organized in this project.

---

## Overview

This repository currently contains one concrete runtime package: the HarmonyOS companion app under `sources/hscrcpy_server`. Within that package, backend code means the native implementation and the build metadata that shapes how the companion is produced.

The project direction already decided in product planning is:

* desktop host owns orchestration
* device companion is a thin HAP shell
* native core owns capture, encode, transport, and control

That direction should be reflected in file placement as code is added.

---

## Directory Layout

```text
sources/hscrcpy_server/
├── AppScope/
│   └── app.json5
├── build-profile.json5
├── hvigorfile.ts
├── entry/
│   ├── build-profile.json5
│   ├── hvigorfile.ts
│   ├── oh-package.json5
│   └── src/
│       ├── main/
│       │   ├── cpp/
│       │   │   ├── CMakeLists.txt
│       │   │   ├── napi_init.cpp
│       │   │   └── types/libentry/Index.d.ts
│       │   ├── ets/
│       │   ├── module.json5
│       │   └── resources/
│       ├── test/
│       └── ohosTest/
└── oh-package.json5
```

---

## Module Organization

Current baseline:

* Put device native code in `entry/src/main/cpp/`.
* Keep ArkTS UI and lifecycle code in `entry/src/main/ets/`.
* Keep generated or bridge-facing TypeScript declarations in `entry/src/main/cpp/types/`.
* Keep packaging and build concerns in `build-profile.json5`, `module.json5`, `AppScope/app.json5`, and hvigor files.

Direction for new code:

* Do not mix streaming logic into ArkTS pages.
* Split native code by responsibility as soon as the template is replaced, for example:
  * `capture/`
  * `codec/`
  * `transport/`
  * `control/`
  * `bridge/`
* Keep the N-API boundary thin. The bridge should call core services, not contain business logic.

### Current Native Runtime Layout

The current device runtime now uses concrete native module slices under `entry/src/main/cpp/`:

* `capture/`
  * `screen_capture_authorization_probe.*` probes `OH_AVScreenCapture` authorization state for startup and POC flows.
  * `h264_screen_capture_source.*` owns the `AVScreenCapture + VideoEncoder(surface)` runtime and access-unit queueing.
* `codec/`
  * `h264_encoder_capability.*` owns `video/avc` encoder capability resolution and width/height/fps/bitrate clamping.
* `transport/`
  * `video_channel.cpp` owns runtime activation, packet framing, send diagnostics, and codec-specific streaming loops.
* `core/`
  * `native_diag_log.h` is the shared native structured logging seam for high-volume runtime diagnostics.
* `poc/`
  * `uitest_extension_poc.cpp` is an experiment-only entry point for `UiTestExtension_OnInit/OnRun`.
  * Treat this directory as disposable bringup code until the extension loading path is proven viable on real devices.

### Current Rust Host Runtime Layout

The Rust host workspace keeps route-specific protocols behind host adapter modules:

* `crates/hscrcpy-host/src/route.rs` owns route names and CLI-facing parsing.
* `crates/hscrcpy-host/src/uitest.rs` owns official `uitest` payload lifecycle, HDC push, stale process cleanup, daemon launch, abstract-socket forwarding, and payload cleanup.
* `crates/hscrcpy-host/src/official_scrcpy.rs` owns generated official scrcpy gRPC/protobuf types and converts them into route-neutral host messages.
* `crates/hscrcpy-host/proto/scrcpy.proto` is the repo-local schema reconstructed from the generated descriptor embedded in `xdevice_devicetest-6.1.0.210-py3-none-any.whl/devicetest/controllers/tools/recorder/proto/scrcpy_pb2.py`; it is not a public Huawei `.proto` source file and was not recovered by reverse engineering the device-side `.so`.
* `crates/hscrcpy-host/src/video/mod.rs` owns route-neutral video ingress adapters consumed by `render/bringup.rs`.

### Scenario: Official UITest gRPC Route Adapter

#### 1. Scope / Trigger

Use this contract when changing the official `uitest` projection route, generated scrcpy protobuf bindings, or the handoff from `ScrcpyService/onStart` into host H.264 rendering.

#### 2. Signatures

* CLI command: `hscrcpy-host-cli --device <id|auto> --route uitest --codec h264 [--ffplay-bin <path>] [--max-frames <n>] [--uitest-flavor <scrcpy|recorder>] [--uitest-payload <path>] [--record-h264]`
* Build script: `crates/hscrcpy-host/build.rs`
* Proto source: `crates/hscrcpy-host/proto/scrcpy.proto`
* Generated include: `include!(concat!(env!("OUT_DIR"), "/scrcpy.rs"))` inside `crates/hscrcpy-host/src/official_scrcpy.rs`
* Runtime path: `OfficialScrcpyClient::for_forwarded_local_tcp(port).start()`
* Service methods:
  * `/ScrcpyService/onStart`
  * `/ScrcpyService/onEnd`
  * `/ScrcpyService/onRequestIDRFrame`

#### 3. Contracts

* Keep the proto package-less so generated tonic paths remain `/ScrcpyService/<method>`.
* Generated protobuf/gRPC types may be referenced inside `official_scrcpy.rs` only; CLI, renderer, and video modules consume route-neutral host types.
* `ReplyMessage.payload` may contain mixed `ParamValue` entries. Exactly one `val_bytes` entry is treated as the H.264 access unit candidate; zero, empty, or multiple byte entries fail before renderer handoff.
* `--route uitest` must never fall back to the HAP `hscrcpy-server` route after a selected-route failure.
* `--uitest-flavor scrcpy` uses the `hosScrcpy` payload, `scrcpy_grpc_socket`, and HoKit-observed scrcpy launch arguments. `--uitest-flavor recorder` uses the `xdevice-devicetest/recorder` payload line, device TCP port `5001`, and the official recorder launch shape `-p 5001 -m 1 -screenId <id>`.
* Recorder flavor must follow the official `record_agent.py` fallback behavior: try bundled `libscrcpy_server*.z.so` payloads in descending filename order, try both `tcp:<local> -> tcp:5001` and `tcp:<local> -> localabstract:screen_record_grpc_socket` with fport before daemon launch, verify that the `libscreen_recorder` process remains alive after launch, and clean up before trying the next payload.
* Live H.264 preview writes access units to ffplay stdin (`-f h264 -i pipe:0`). Disk artifacts under `frames/` and `stream.h264` are opt-in diagnostics, not the playback source, and must not be enabled by default during latency testing.
* Official `uitest` H.264 may start with non-IDR access units. Live preview must not feed ffplay until the first IDR/keyframe is observed.
* While waiting for the first IDR/keyframe, live preview must cache H.264 decoder configuration NALs (`SPS` type 7 and `PPS` type 8) from pre-IDR access units and write them before the first IDR. Dropping all pre-IDR bytes can make ffplay fail with `non-existing PPS` even after later IDR frames arrive.
* Do not automatically call `ScrcpyService/onRequestIDRFrame` during startup by default. Real-device checks against the official Java `hosScrcpy` API on 2026-04-24 showed startup IDR requests can close the active `onStart` stream. Keep the method available as a manual diagnostic/control hook.
* Scrcpy flavor startup should send best-effort `power-shell wakeup` after `onStart`, matching the official Java API startup behavior without mutating screen content.
* Host shutdown paths, including Ctrl+C/SIGINT and `grpc-start` startup failure after `uitest` launch, must attempt `ScrcpyService/onEnd` when a runtime exists and clean up stale official `xdevice_scrcpy` processes plus the dynamically forwarded scrcpy localabstract socket or recorder TCP port.
* Use a fresh dynamic local TCP port for official `uitest` gRPC forwarding. Do not assume legacy `tcp:27184` is free; startup may best-effort remove it, but active sessions should not depend on it.
* Wait for device-side `scrcpy_grpc_socket` readiness before creating the scrcpy-flavor HDC forward, then start gRPC only after `hdc fport` succeeds. Keep `grpc-socket-ready` distinct from `grpc-connect` in errors. Recorder flavor currently skips localabstract readiness and probes `tcp:5001` because `screen_record_grpc_socket` was not observed on device during bringup.

#### 4. Validation & Error Matrix

| Boundary | Required validation | Failure signal |
|----------|---------------------|----------------|
| Proto generation | `cargo check -p hscrcpy-host --offline` builds generated client | build script or generated include failure |
| Route startup | selected route is `uitest` and selected codec is `h264` | route/phase error with no HAP fallback |
| Device socket readiness | scrcpy flavor: `scrcpy_grpc_socket` appears in `/proc/net/unix` after `uitest start-daemon`; recorder flavor: `tcp:5001` is forwarded directly | `grpc-socket-ready` or `grpc-connect` context |
| gRPC connect/start | target is forwarded local TCP, method is named in errors | `grpc-connect`, `grpc-start`, or `grpc-status` context |
| Stream message conversion | exactly one non-empty `val_bytes` payload | `ContractViolation` before `OfficialScrcpyIngressAdapter` |
| Renderer handoff | payload is Annex-B H.264 | H.264 ingress validation error |
| Preview | ffplay consumes stdin pipe | `live_preview status=disabled` and `ffplay_write_*` diagnostics |
| Shutdown / interrupted capture | receive loop can leave idle waits, then `onEnd` and route cleanup run | no stale `xdevice_scrcpy` process or lingering active dynamic forward |

#### 5. Good/Base/Bad Cases

* Good: `official_scrcpy.rs` converts generated `ReplyMessage` into `OfficialScrcpyVideoMessage`, then `video/mod.rs` normalizes it into `PreparedVideoIngress::H264`.
* Base: unit tests use a mock `OfficialScrcpyTransport` and generated message structs to verify conversion without opening a real gRPC socket.
* Bad: CLI imports generated protobuf types, renderer knows about `ScrcpyService`, or a second hand-written protobuf decoder is kept as a production fallback.

#### 6. Tests Required

* `cargo check -p hscrcpy-host --offline`
* `cargo test --workspace --offline -- --nocapture`
* Unit tests asserting:
  * generated single `val_bytes` payload maps to `VideoCodec::H264`
  * missing/ambiguous/empty bytes payloads fail
  * mock official stream feeds `BringupRenderSurface`
  * unsupported route codec does not enter HAP session/video channels
* Manual real-device check:
  * `cargo run -p hscrcpy-host-cli -- --device auto --route uitest --codec h264 --ffplay-bin /opt/homebrew/bin/ffplay`
  * Expected: ffplay displays frames, `captured frame 1 codec=h264` appears, and `diag window=full` summaries continue.

#### 7. Wrong vs Correct

##### Wrong

* Reintroducing a production `h2` frame parser or manual protobuf decoder beside generated tonic/prost bindings.
* Writing H.264 to disk by default or pointing ffplay at `stream.h264` for live playback.

##### Correct

* Use `tonic`/`prost` generated bindings for the gRPC boundary, isolate them in `official_scrcpy.rs`, and feed ffplay directly through stdin while keeping file artifacts optional diagnostics behind `--record-h264`.

### Scenario: Introducing New Native Runtime Modules

#### Scope / Trigger

Use this layout rule when adding a device-side native module that is consumed by the streaming runtime, host/device contract, or `uitest` extension experiments.

#### Signatures

* Device shared runtime build: `sources/hscrcpy_server/entry/src/main/cpp/CMakeLists.txt`
* Production native output: `add_library(entry SHARED ...)`
* Experiment extension output: `add_library(hscrcpy_uitest_poc SHARED ...)`

#### Contracts

* Files under `capture/`, `codec/`, `transport/`, and `core/` may be linked into `entry`.
* Files under `poc/` may be linked into `hscrcpy_uitest_poc`, but must not become a hidden dependency of `entry`.
* Shared helpers used by multiple runtime modules belong in `core/` only if they are runtime-safe and product-agnostic.

#### Validation & Error Matrix

| Change | Required validation | Failure signal |
|--------|---------------------|----------------|
| Add source to `entry` | DevEco native build or HAP build succeeds | native link/compile failure |
| Add source to `hscrcpy_uitest_poc` | exported `UiTestExtension_OnInit/OnRun` still present | `nm -D` missing symbol or `uitest` load failure |
| Move logic from bridge to native module | N-API call sites stay thin | bridge starts owning transport/capture logic |

#### Good/Base/Bad Cases

* Good: `capture/h264_screen_capture_source.cpp` owns screen capture state and is called by `transport/video_channel.cpp`.
* Base: `codec/h264_encoder_capability.cpp` exposes pure configuration helpers without transport state.
* Bad: adding screen-capture start/stop logic directly into `bridge/companion_napi.cpp` or ArkTS page files.

#### Tests Required

* Rust-side or host-side tests for any contract change exposed over transport.
* DevEco native build for any new C++ file added to `entry` or `hscrcpy_uitest_poc`.
* Real-device `hilog` verification when module changes touch capture, transport, or extension loading.

#### Wrong vs Correct

##### Wrong

* `bridge/*.cpp` starts owning codec capability selection or screen-capture state machines.
* `poc/` helpers are silently reused by production runtime because they already exist.

##### Correct

* Runtime modules live under `capture/`, `codec/`, `transport/`, and `core/`, with `bridge/` only forwarding explicit calls.
* `poc/` remains explicitly isolated, documented, and safe to delete if the extension path is abandoned.

---

## Naming Conventions

* C++ source files use lowercase snake_case where possible, following the current `napi_init.cpp`.
* CMake entry files stay named `CMakeLists.txt`.
* ArkTS files follow HarmonyOS conventions with `EntryAbility.ets`, page names, and module metadata names kept explicit.
* Avoid placeholder names like `entry` for long-lived domain modules once code is no longer scaffold-only.

---

## Examples

* [`sources/hscrcpy_server/entry/src/main/cpp/CMakeLists.txt`](../../sources/hscrcpy_server/entry/src/main/cpp/CMakeLists.txt): native build entry point belongs under the module's `cpp` directory.
* [`sources/hscrcpy_server/entry/src/main/cpp/napi_init.cpp`](../../sources/hscrcpy_server/entry/src/main/cpp/napi_init.cpp): the current native bridge entry point lives beside the native build file.
* [`sources/hscrcpy_server/entry/build-profile.json5`](../../sources/hscrcpy_server/entry/build-profile.json5): native ABI and CMake configuration live in module build metadata, not in ad hoc scripts.

---

## Common Mistakes

* Putting device-agent runtime logic in ArkTS page files because the template starts there.
* Treating generated names like `entry` as stable architecture instead of template residue.
* Hiding cross-layer behavior in build scripts instead of explicit runtime modules.
