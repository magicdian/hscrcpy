# Journal - magicdian (Part 1)

> AI development session journal
> Started: 2026-04-22

---



## Session 1: Bootstrap guidelines and DevEco scaffold baseline

**Date**: 2026-04-22
**Task**: Bootstrap guidelines and DevEco scaffold baseline
**Branch**: `dev`

### Summary

(Add summary)

### Main Changes

| Area | Description |
|------|-------------|
| Trellis specs | Replaced bootstrap templates in `.trellis/spec/backend/` and `.trellis/spec/frontend/` with project-specific baseline guidance grounded in the current repo state. |
| Ignore rules | Added repo-level `.DS_Store` ignores while keeping DevEco-generated local/build artifacts excluded by project-local `.gitignore` files. |
| HarmonyOS scaffold | Initialized `sources/hscrcpy_server` as the first DevEco Native C++ companion project baseline for later implementation. |
| Task tracking | Archived `00-bootstrap-guidelines` after the spec bootstrap work was completed. |

**Notes**:
- This session established the first executable code-spec baseline for future AI-assisted work.
- The repository is clean after commit `75a791e`.


### Git Commits

| Hash | Message |
|------|---------|
| `75a791e` | (see git log) |
| `fcb2184` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 2: MVP transport runtime bringup

**Date**: 2026-04-23
**Task**: MVP transport runtime bringup
**Branch**: `dev`

### Summary

(Add summary)

### Main Changes

| Area | Outcome |
|------|---------|
| Host runtime | Implemented HDC target resolution, companion install/status checks, session/video channel orchestration, and artifact-based render bringup for JPEG/H264. |
| Device runtime | Implemented companion bootstrap, session listener on 27182, video listener on 27183, session/video packet flow, and required INTERNET permission for listener startup. |
| Protocol/spec | Promoted `docs/architecture/host-device-mvp-contract.md` to active runtime contract with concrete HDC commands, ports, packet header fields, error matrix, and validation cases; updated `.trellis/spec/backend/index.md`. |
| Manual validation | DevEco build/run succeeded, hilog showed `Session listener bootstrap state=listening port=27182 bound=true`, JPEG preview generated `preview.html` + `.jpg` frames, H264 path generated `.h264` frame artifacts via host CLI. |

**Completed Tasks Archived**:
- `04-22-hos-arch-contracts`
- `04-22-hos-device-foundation`
- `04-22-hos-host-foundation`
- `04-22-hos-device-jpeg-path`
- `04-22-hos-host-jpeg-path`
- `04-22-hos-jpeg-baseline`
- `04-23-hos-device-input-path`
- `04-23-hos-host-input-path`
- `04-22-hos-input-control`
- `04-23-hos-device-h264-path`
- `04-23-hos-host-h264-path`
- `04-22-hos-h264-mainline`
- `04-23-hos-host-hdc-runtime`
- `04-23-hos-host-companion-deploy`
- `04-23-hos-device-session-transport`
- `04-23-hos-host-session-transport`
- `04-23-hos-device-network-permission-fix`
- `04-23-hos-device-session-listener-runtime`
- `04-23-hos-device-video-channel-runtime`
- `04-23-hos-session-transport-runtime`
- `04-23-hos-host-render-bringup`

**Verification**:
- `cargo test --workspace`
- `cargo check --workspace`
- `git diff --check`
- DevEco manual build/run
- Host CLI manual JPEG/H264 bringup

**Open Follow-up**:
- `04-23-hos-host-launch-readiness` remains active for bounded retry/readiness polish on companion launch.
- Root task `04-22-hos-scrcpy-brainstorm` remains active as the umbrella planning task.


### Git Commits

| Hash | Message |
|------|---------|
| `e06a6ad` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 3: Record official hosScrcpy investigation and uitest POC findings

**Date**: 2026-04-24
**Task**: Record official hosScrcpy investigation and uitest POC findings
**Branch**: `dev`

### Summary

(Add summary)

### Main Changes

| Area | Finding |
|------|---------|
| Official source | Confirmed `scrcpy_server.so` comes from Huawei `DevecoTesting-Hypium 6.1.0.210` package and matches `hosScrcpy` unix `6.5-20260313` binary. |
| Official assets | Archived official `hosScrcpy` and `xdevice-devicetest` so bundles under `third_party/hypium/` with versioned layout for future ABI and version compatibility work. |
| Formal docs | Added forward-looking docs for official `hosScrcpy / uitest_agent / xdevice-devicetest` usage and a separate `uitest` extension POC record. |
| UITest POC | Built custom `hscrcpy_uitest_poc.so`, verified exported `UiTestExtension_OnInit/OnRun`, packaged it into signed HAP, and compared stripped vs packaged outputs. |
| Load result | `uitest` rejects the custom so at load time with `Xpm check failed` and `Permission denied`, while official `uitest_agent_1.2.3.so` launches successfully from `/data/local/tmp`. |
| Current conclusion | Self-written extension so is not currently a viable mainline path; blocker is extension trust / code-signing enablement rather than ABI, symbols, or chmod bits. |

**Artifacts**:
- `docs/harmony-hos-scrcpy-debug-guide.md`
- `docs/uitest-extension-poc.md`
- `docs/README.md`
- `third_party/hypium/README.md`
- `third_party/hypium/hosScrcpy/6.1.0.210/...`
- `third_party/hypium/xdevice-devicetest/6.1.0.210/...`

**Manual verification performed**:
- Compared official and live `scrcpy_server.so` hashes and package contents.
- Ran `uitest` load test for custom `hscrcpy_uitest_poc.so`.
- Ran `uitest` control test for official `uitest_agent_1.2.3.so`.
- Confirmed packaged HAP contains `hscrcpy_uitest_poc.so`, but packaged/stripped output still lacks evidence of extension-level trust metadata.


### Git Commits

| Hash | Message |
|------|---------|
| `3439861` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 4: Real H.264 bringup and uitest extension findings

**Date**: 2026-04-24
**Task**: Real H.264 bringup and uitest extension findings
**Branch**: `dev`

### Summary

(Add summary)

### Main Changes

| Area | Result |
|------|--------|
| Device H.264 mainline | Replaced placeholder H.264 bytes with `AVScreenCapture + VideoEncoder(surface)` and real access-unit streaming on the existing video channel. |
| Host H.264 validation | Tightened host ingest to require Annex-B framing and IDR presence for keyframes, then validated against real device output. |
| Live preview bringup | Upgraded host bringup from artifact dumping only to continuous `ffplay` live preview while preserving per-frame artifacts and `stream.h264` output. |
| Stream diagnostics | Added interval-based host/device timing summaries so queueing, backpressure, and preview lag can be diagnosed from `events.log`, CLI `diag ...` lines, and native `hscrcpyDiag` hilog entries. |
| UiTest extension POC | Built `hscrcpy_uitest_poc.so` with valid `UiTestExtension_OnInit/OnRun` exports, but real-device `uitest` loading failed at `Xpm check`, so self-authored extension `.so` is not a viable mainline path right now. |

**Completed Tasks Archived**:
- `04-23-hos-device-real-h264-pipeline`
- `04-23-hos-host-real-h264-validation`
- `04-23-hos-host-realtime-decode-display`
- `04-23-hos-real-h264-stream-bringup`

**Verification**:
- `cargo test -p hscrcpy-host`
- `cargo test -p hscrcpy-host-cli`
- `cargo fmt --check`
- Human verification on HarmonyOS device: real H.264 access units captured, authorization prompt observed, ffplay preview working, and `uitest` custom extension load rejected with `Xpm check failed`.

**Spec Sync**:
- Updated backend code-specs for native module layout, official hosScrcpy debug references, structured media diagnostics, and experiment-vs-production isolation.
- Updated `docs/architecture/host-device-mvp-contract.md` so H.264 packets require Annex-B access units and keyframes require an IDR NAL.


### Git Commits

| Hash | Message |
|------|---------|
| `63b0a4a` | (see git log) |

### Testing

- [OK] (Add test results)

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 5: UITest scrcpy projection route

**Date**: 2026-04-24
**Task**: UITest scrcpy projection route
**Branch**: `dev`

### Summary

Implemented and archived the host launch readiness work: route abstraction, codec registry, UITest HDC lifecycle, standard tonic/prost official ScrcpyService client, H.264 ingress to renderer/ffplay, diagnostics, docs, and executable backend spec updates. Manual validation confirmed --route uitest --codec h264 displays through ffplay; follow-up performance work remains for UITest startup latency and artifact overhead.

### Main Changes

- Added explicit host route selection with `uitest` as the default route and `hscrcpy-server` as the HAP fallback route.
- Added a route-neutral codec capability registry and selected H.264 as the current UITest end-to-end target.
- Implemented official UITest lifecycle support: payload selection, push, stale process cleanup, `uitest start-daemon`, abstract socket forwarding, and cleanup.
- Replaced the temporary handwritten official gRPC transport with a standard `tonic` + `prost` generated `ScrcpyService` client.
- Added route-neutral H.264 ingress from official gRPC stream messages into the existing bringup renderer and ffplay stdin preview path.
- Updated architecture/debug docs, Trellis task records, and backend code-spec guidance for the official UITest gRPC route.

### Git Commits

| Hash | Message |
|------|---------|
| `c7a735a` | (see git log) |
| `aff535f` | (see git log) |

### Testing

- [OK] `cargo fmt --all --check`
- [OK] `cargo check --workspace --offline`
- [OK] `cargo test --workspace --offline -- --nocapture`
- [OK] `git diff --check`
- [OK] Manual `--route uitest --codec h264` run displayed H.264 frames through ffplay and emitted full diagnostic windows.

### Status

[OK] **Completed**

### Next Steps

- None - task complete


## Session 6: Stabilize uitest H.264 preview pipeline

**Date**: 2026-04-24
**Task**: Stabilize uitest H.264 preview pipeline
**Branch**: `dev`

### Summary

Hardened the official uitest H.264 preview route: made H.264 artifacts opt-in, passed selected FPS to ffplay, preserved SPS/PPS before first IDR, added startup/timing diagnostics and graceful cleanup, documented official hosScrcpy/recorder limitations, and kept the brainstorm task active for remaining scope.

### Main Changes

- Made H.264 diagnostic artifacts opt-in with `--record-h264` so latency runs no longer write every access unit to disk by default.
- Passed the selected stream FPS into ffplay's raw H.264 input `-framerate` option and kept the host default at 120 FPS.
- Preserved SPS/PPS decoder configuration before the first IDR so ffplay can start reliably after official `uitest` sends pre-keyframe units.
- Added startup phase timing, first-frame timing, keyframe visibility, live-preview diagnostics, SIGINT graceful shutdown, and stale official `xdevice_scrcpy` / fport cleanup.
- Propagated HAP video configuration into the device encoder path for frame rate, bitrate, and I-frame interval.
- Compared official Huawei `hosScrcpy` and recorder behavior on device; documented that static-screen first-frame weakness and recorder failure are also official-side issues for this package/device combination.

### Git Commits

| Hash | Message |
|------|---------|
| `0804bda` | (see git log) |

### Testing

- [OK] `cargo fmt -- --check`
- [OK] `cargo check --workspace --offline`
- [OK] `cargo test --workspace --offline -- --nocapture`
- [OK] `git diff --check`
- [OK] Real-device manual check: `--route uitest --codec h264 --ffplay-bin /opt/homebrew/bin/ffplay` displayed smoothly after SPS/PPS preview fix.

### Status

[OK] **Completed**

### Next Steps

- Keep `04-22-hos-scrcpy-brainstorm` active for remaining MVP planning and future HAP/control-route work.
