# Host-Device MVP Contract

Status: active runtime contract for task `04-22-hos-arch-contracts`

This document is the current source of truth for the HarmonyOS scrcpy-like MVP boundary between the future Rust host and the HarmonyOS companion. Downstream host, device, JPEG, H.264, and input-control tasks should use the names and state machines here unless this document is revised first.

## 1. Scope / Trigger

* Trigger: the project needs a concrete host-device contract before host and device scaffolding work proceeds in parallel.
* In scope:
  * companion install/update ownership
  * first-run authorization states
  * startup handshake
  * capability negotiation
  * session/channel split
* Out of scope:
  * actual Rust workspace layout
  * `sources/hscrcpy_server` implementation details
  * audio, file transfer, and non-MVP tooling
  * final binary framing bytes for video payloads

## 2. Stable vs Provisional

### Stable Now

* The desktop host owns companion install, update, launch, session start, and session stop.
* The HarmonyOS side stays split into a thin HAP shell plus native core.
* MVP device connectivity is HDC-based.
* MVP uses two logical channels:
  * `session` for handshake and control
  * `video` for media payloads
* Codec selection is negotiated from both sides' runtime capability:
  * device/route encoder support
  * host decoder/render support
  * user codec preference
* The startup contract must use an extensible codec capability registry rather than a closed enum. `H.264` is the required first playback target; H.265, H.266/VVC, VP9, AV1, JPEG, and other codecs are future probeable capabilities when route and host support exist.
* ArkTS stays responsible for lifecycle, permissions, and user-facing authorization prompts. Native code owns capture, encode, transport, and control handling.
* Protocol compatibility is governed by `protocol_major` and `protocol_minor`.

### Provisional

* Exact HarmonyOS permission/API sequence for capture and input injection.
* Exact binary frame header layout for the `video` channel.
* Exact device bundle name, ability name, and service naming.
* Whether later versions use direct sockets, reverse tunnels, or another HDC-compatible binding under the same logical channel model.

## 3. Runtime Architecture

### Host Responsibilities

* detect the target device over HDC
* compare installed companion metadata against the bundled manifest
* install or upgrade the companion when needed
* select the launch route, such as the installed HAP companion or the official `uitest` scrcpy route
* probe host decoder/render support before selecting a codec
* launch the selected route into session/stream mode
* open the `session` channel first
* negotiate features and select the video codec from route/device encoder support plus host decoder support
* open and monitor the `video` channel after negotiation succeeds
* surface authorization and compatibility failures clearly

### Device Responsibilities

* expose a thin HarmonyOS app shell for lifecycle and permission prompts
* hand off runtime work to a native core
* report authorization state instead of guessing host intent
* advertise capture, codec, and control capabilities at runtime
* expose route-specific encoder capabilities without forcing the host to know route internals
* start video only after session configuration is accepted
* keep control handling off the video path

## 4. Signatures

### 4.1 Host-Packaged Companion Manifest

The host foundation should treat the bundled companion description as the install/update contract, even if the exact file name is introduced later.

| Field | Type | Required | Notes |
|---|---|---:|---|
| `companion_id` | string | yes | Stable logical identity for the HarmonyOS companion package. |
| `artifact_path` | string | yes | Host-relative path to the packaged HAP artifact. |
| `version_name` | string | yes | Human-readable companion version. |
| `version_code` | integer | yes | Monotonic install/update comparison key. |
| `protocol_major` | integer | yes | Must match host `protocol_major`. |
| `protocol_minor` | integer | yes | Minor compatibility floor/ceiling for the shipped companion. |
| `sha256` | string | yes | Integrity check before install. |
| `supported_abis` | string[] | no | Optional until ABI packaging is finalized. |

Current MVP runtime manifest values in code:

| Field | Current Value | Source |
|---|---|---|
| `companion_id` | `cn.magicdian.hscrcpy.server` | `crates/hscrcpy-host/src/companion/mod.rs` |
| `artifact_path` | `assets/companion/hscrcpy_server.hap` | `crates/hscrcpy-host/src/companion/mod.rs` |
| `version_name` | `1.0.7` | `crates/hscrcpy-host/src/companion/mod.rs` and `sources/hscrcpy_server/AppScope/app.json5` |
| `version_code` | `1000007` | `crates/hscrcpy-host/src/companion/mod.rs` and `sources/hscrcpy_server/AppScope/app.json5` |
| `launch_ability` | `EntryAbility` | `crates/hscrcpy-host/src/companion/mod.rs` |

### 4.1.1 Current Host Runtime Commands

These are the current concrete command/API shapes used by the Rust host runtime:

| Purpose | Current Command Shape | Source |
|---|---|---|
| list visible devices | `hdc list targets` | `crates/hscrcpy-host/src/hdc/mod.rs` |
| inspect installed companion | `hdc -t <resolved-target> shell bm dump -n <bundle>` | `crates/hscrcpy-host/src/companion/mod.rs` |
| launch companion shell | `hdc -t <resolved-target> shell aa start -b <bundle> -a <ability>` | `crates/hscrcpy-host/src/companion/mod.rs` |
| forward `session` channel | `hdc -t <resolved-target> fport tcp:27182 tcp:27182` | `crates/hscrcpy-host/src/session/startup.rs` |
| forward `video` channel | `hdc -t <resolved-target> fport tcp:27183 tcp:27183` | `crates/hscrcpy-host/src/session/startup.rs` |
| push official `uitest` scrcpy payload | `hdc -t <resolved-target> file send <selected-libscrcpy-server.so> /data/local/tmp/scrcpy_server.so` | `crates/hscrcpy-host/src/uitest.rs` |
| kill stale official `uitest` scrcpy processes | `hdc -t <resolved-target> shell kill -9 <xdevice_scrcpy-or-uitest-pid>` | `crates/hscrcpy-host/src/uitest.rs` |
| launch official `uitest` scrcpy server | `hdc -t <resolved-target> shell uitest start-daemon singleness --extension-name scrcpy_server.so ...` | `crates/hscrcpy-host/src/uitest.rs` |
| wait for official `uitest` scrcpy socket | `hdc -t <resolved-target> shell cat /proc/net/unix | grep -F scrcpy_grpc_socket` | `crates/hscrcpy-host/src/uitest.rs` |
| forward official `uitest` scrcpy socket | `hdc -t <resolved-target> fport tcp:<dynamic-local-port> localabstract:scrcpy_grpc_socket` | `crates/hscrcpy-host/src/uitest.rs` |
| launch official recorder-mode `uitest` server | `hdc -t <resolved-target> shell uitest start-daemon singleness --extension-name libscreen_recorder.z.so -p <port> -m 1 -screenId <id>` | `crates/hscrcpy-host/src/uitest.rs` |
| forward official recorder-mode port | `hdc -t <resolved-target> fport tcp:<dynamic-local-port> tcp:5001` | `crates/hscrcpy-host/src/uitest.rs` |
| remove temporary official `uitest` payload | `hdc -t <resolved-target> shell rm -f /data/local/tmp/scrcpy_server.so` | `crates/hscrcpy-host/src/uitest.rs` |

Rules:

* `--device auto` is a host-side convenience only. The host must resolve it once to a concrete serial before issuing `hdc -t ...` commands.
* The host currently launches `EntryAbility` first, and the device-side `EntryAbility.onCreate()` is responsible for bootstrapping native session listener startup.
* Companion install/update decisions are based on the top-level `versionCode` / `versionName` fields from `bm dump`, not nested quick-fix or module fields.
* Route selection must be explicit in host diagnostics. The HAP route and the official `uitest` route must both feed a route-neutral video ingress layer.
* The selected route must fail fast. If `--route uitest` fails during payload selection, stale process cleanup, payload push, daemon launch, socket forwarding, gRPC connect/start/status, first-frame ingestion, renderer handoff, or shutdown, the host must not automatically launch the `hscrcpy-server` HAP route.
* The official `uitest` route should use a fresh dynamic local TCP port for `scrcpy_grpc_socket` forwarding so stale fixed-port listeners cannot block startup. Startup still best-effort removes the legacy `tcp:27184 -> localabstract:scrcpy_grpc_socket` forward.
* The host supports two official `uitest` flavors for investigation: `scrcpy` maps to `hosScrcpy/libscrcpy_server_unix_*`, while `recorder` maps to `xdevice-devicetest/recorder/libscrcpy_server*.z.so`, the remote file name `libscreen_recorder.z.so`, and device TCP port `5001`.
* Recorder flavor must try bundled `libscrcpy_server*.z.so` payloads in descending filename order, try both `tcp:<local> -> tcp:5001` and `tcp:<local> -> localabstract:screen_record_grpc_socket` with fport before daemon launch, verify that the `libscreen_recorder` process remains alive after launch, and clean up before trying the next payload. This mirrors the official `record_agent.py` fallback path for device-specific recorder libraries.
* Scrcpy flavor must not start gRPC until the device-side `scrcpy_grpc_socket` appears in `/proc/net/unix` and `hdc fport` has returned successfully. A missing device socket is reported as `grpc-socket-ready`; a later local connection failure is reported as `grpc-connect`. Recorder flavor currently skips localabstract readiness and probes `tcp:5001` because `screen_record_grpc_socket` was not observed on device during bringup.
* The official `uitest` route must clean stale `xdevice_scrcpy` state plus the active dynamic `tcp:<port> -> localabstract:scrcpy_grpc_socket` or `tcp:<port> -> tcp:5001` forward during startup compensation and graceful shutdown. Ctrl+C/SIGINT during capture is a graceful shutdown request, not a process-abort path.
* The official `uitest` H.264 stream may begin with non-IDR access units. Host live preview must wait for the first IDR/keyframe before writing to ffplay.
* While waiting for the first IDR/keyframe, host live preview must cache H.264 decoder configuration NALs (`SPS` type 7 and `PPS` type 8) from pre-IDR access units and write them before the first IDR. Dropping those units causes ffplay startup failures such as `non-existing PPS`.
* The HAP `hscrcpy-server` H.264 encoder must preserve `OH_MD_KEY_CODEC_CONFIG` from `OnEncoderStreamChanged` and codec-data output buffers, normalize avcC decoder config into Annex-B SPS/PPS, and prepend it before IDR access units that do not already carry SPS/PPS. Initialize the native callback runtime before `OH_VideoEncoder_Prepare`/`Start`; some encoders emit stream config during prepare/start, before screen capture itself is started.
* The HAP `hscrcpy-server` H.264 encoder must configure `OH_MD_KEY_VIDEO_ENCODER_REPEAT_PREVIOUS_FRAME_AFTER` and `OH_MD_KEY_VIDEO_ENCODER_REPEAT_PREVIOUS_MAX_COUNT` so static screens still produce repeated access units. Real-device diagnostics on 2026-04-24 showed the tested encoder producing 33-us timestamp deltas when configured with value `33`, so this route currently uses `33000` as the repeat-after value and logs the raw configured value plus `repeat_previous_after_set` / `repeat_previous_max_set`. Host diagnostics use 120-unit windows, but live preview must not depend on a full diagnostic window before feeding ffplay.
* Host ffplay preview must favor a visible first frame over aggressive frame dropping. The default live preview command uses raw H.264 input with `-probesize 32`, `-analyzeduration 0`, `-fflags +genpts`, `-flags low_delay`, and `-sync ext`; avoid `-framedrop` / `-fflags nobuffer` until startup visibility is proven on the target platform.
* H.264 bringup diagnostics must let a single run prove where the first decodable access unit is lost. Native `hscrcpyDiag` must emit startup/config events plus `encoder_access_unit_trace` for the first few access units and keyframes with input/emitted SPS/PPS/IDR counts. Host `events.log` must record `payload_bytes`, `h264_sps`, `h264_pps`, `h264_idr`, and `h264_types` for units handed to ffplay.
* Real-device checks against the official Java `hosScrcpy` API on 2026-04-24 showed that automatic startup calls to `ScrcpyService/onRequestIDRFrame` can close the active `onStart` stream. Keep `onRequestIDRFrame` available as an opt-in diagnostic/control method (`--uitest-request-idr-on-start`) for static official streams, but do not call it by default while waiting for the first decodable frame.
* The current `uitest` route uses a standard host-side `tonic` + `prost` generated gRPC client for official `ScrcpyService/onStart`, `onEnd`, and `onRequestIDRFrame`. Protocol-specific generated types stay inside `crates/hscrcpy-host/src/official_scrcpy.rs`; the stream is normalized through `OfficialScrcpyIngressAdapter` into route-neutral H.264 ingress before reaching the bringup renderer / ffplay path.
* Scrcpy flavor startup should send best-effort `power-shell wakeup` after `onStart`, matching the official Java API startup behavior without mutating screen content.
* The repo-local schema lives at `crates/hscrcpy-host/proto/scrcpy.proto`. It was reconstructed from the generated descriptor embedded in `xdevice_devicetest-6.1.0.210-py3-none-any.whl/devicetest/controllers/tools/recorder/proto/scrcpy_pb2.py`; it is not a public Huawei `.proto` source file and was not recovered by reverse engineering the device-side `.so`.

### 4.1.2 Official `uitest` Static Startup Fallback Contract

#### Scope / Trigger

Use this contract when improving static-screen startup visibility for `--route uitest`.

#### Signatures

* Existing official H.264 stream:
  * gRPC method: `/ScrcpyService/onStart`
  * host adapter: `OfficialScrcpyIngressAdapter::ingest_message(...)`
  * renderer handoff: `PreparedVideoIngress::H264`
* Optional official IDR request:
  * CLI flag: `--uitest-request-idr-on-start`
  * gRPC method: `/ScrcpyService/onRequestIDRFrame`
  * host log operations: `official_scrcpy_request_idr`, `official_scrcpy_request_idr_failed`
* Future startup snapshot fallback:
  * proposed CLI flag: `--uitest-startup-snapshot-idr`
  * artifact: `<session_dir>/latest.jpg`
  * optional synthetic H.264 payload: one Annex-B SPS/PPS/IDR access unit encoded from the snapshot

#### Contracts

* Official `uitest` H.264 may start with SPS/PPS only and must not be considered decodable until an official IDR NAL arrives.
* A snapshot-derived synthetic IDR may be used only as a visual placeholder before the official stream becomes decodable.
* A snapshot-derived synthetic IDR must never be used as the reference frame for subsequent official non-IDR units. Host must keep dropping/caching official non-IDR units until an official IDR arrives.
* After the official IDR arrives, ffplay/live-preview ownership switches to the official stream; subsequent units must be from the same official encoder sequence.

#### Validation & Error Matrix

| Condition | Host behavior | Failure signal |
|---|---|---|
| First official unit has SPS/PPS but no IDR | Cache decoder config and wait | `live_preview status=waiting_for_keyframe` |
| Synthetic snapshot IDR is emitted | Display as placeholder only | event note should mark synthetic source |
| Official non-IDR arrives before official IDR | Do not feed it after synthetic IDR | host continues waiting for official keyframe |
| Official IDR arrives | Feed official SPS/PPS/IDR and switch to official stream | `h264_idr=true` in `events.log` |
| Synthetic encoder dimensions differ from official stream | Prefer preview.html/JPEG fallback or expect decoder reconfigure | visible flash/reconfigure; do not treat as transport bug |

#### Good/Base/Bad Cases

* Good: snapshot placeholder displays immediately, official non-IDR units are withheld, and official IDR switches the stream cleanly.
* Base: no snapshot fallback is enabled; host waits for official IDR as it does today.
* Bad: host encodes a screenshot into IDR and then feeds official P frames before any official IDR. The decoder reference state does not match the official encoder's DPB and may produce corruption or stalls.

#### Tests Required

* Unit test: official SPS/PPS-only message remains non-keyframe and live preview waits.
* Unit test for future snapshot fallback: synthetic IDR does not set the official stream as ready for official P frames.
* Manual real-device test:
  * `--route uitest --codec h264` on a static screen waits for official IDR.
  * `--uitest-request-idr-on-start` logs request result without failing startup.

#### Wrong vs Correct

Wrong:

```text
synthetic_snapshot_idr -> official_p_frame -> official_p_frame
```

Correct:

```text
synthetic_snapshot_idr_placeholder -> drop official non-IDR -> official_sps_pps_idr -> official_p_frame
```

### 4.2 Session Channel Messages

The `session` channel is the control-plane channel for MVP.

Stable wire rules:

* reliable, ordered transport
* UTF-8 JSON messages
* one message per frame/unit
* every message includes `type` and `session_id`
* unknown fields must be ignored
* unknown required message types within the same `protocol_major` are fatal

Supported MVP message types:

* `host_hello`
* `device_hello`
* `authorization_update`
* `session_config`
* `session_ready`
* `stop_session`
* `session_error`
* `control_event`

### 4.3 Video Channel

The `video` channel is a separate logical channel activated only after `session_ready`.

Stable semantic rules:

* one negotiated video codec per session
* binary payloads only
* each payload unit carries one encoded access unit or one JPEG frame
* per-unit semantics must include:
  * `codec`
  * `pts_us`
  * `is_keyframe`
  * `payload_length`

The exact byte packing for those video-unit fields remains provisional and should be finalized by the JPEG/H.264 implementation tasks without changing the channel split or handshake model.

### 4.4 Current Runtime Ports and Activation

The current MVP runtime uses fixed local/device loopback ports:

| Channel | Port | Activation Point | Device Runtime |
|---|---:|---|---|
| `session` | `27182` | started from `EntryAbility.onCreate()` | native listener accepts one TCP client and handles line-delimited JSON |
| `video` | `27183` | started after `session_config` is accepted and before `session_ready` is emitted | native listener accepts one TCP client and writes binary packet frames |

Rules:

* `session` must be listening before the host opens the control-plane socket.
* `video` must not be opened by the host before `session_ready`.
* Device teardown must close both listener and client sockets when the active session stops or disconnects.

### 4.5 Current Runtime Video Packet Header

The current host/device runtime now shares a concrete binary header:

| Offset | Size | Field | Encoding |
|---|---:|---|---|
| `0` | `1` | `codec` | codec registry tag; current tags are `1=h264`, `2=jpeg`, `3=h265` |
| `1` | `8` | `pts_us` | big-endian unsigned integer |
| `9` | `1` | `is_keyframe` | `0` or `1` |
| `10` | `4` | `payload_length` | big-endian unsigned integer |
| `14` | `payload_length` | payload bytes | negotiated codec payload |

Rules:

* Host must reject packets whose `codec` tag does not match the negotiated codec.
* `payload_length` must be positive and must match the number of bytes following the header.
* For `codec=h264`, payload bytes must be one Annex-B access unit (3-byte or 4-byte start-code delimiters) with at least one valid NAL unit.
* For `codec=h264` packets marked `is_keyframe=1`, the access unit must include an IDR NAL (`nal_unit_type=5`) so host-side keyframe assumptions stay aligned with decoder expectations.
* Device-side encoder timestamps must be normalized into protocol microseconds before writing `pts_us`. OpenHarmony H.264 encoder output `attr.pts` is nanosecond-scale on the tested device, so the HAP route divides by 1000 before sending.
* Future codec tags such as H.266/VVC, VP9, and AV1 must be added through the shared codec registry before either route emits packets for them.

## 5. Contracts

### 5.1 Install / Update Contract

Ownership:

* Host is the only side that installs or updates the companion in MVP.
* Device companion never self-updates.

Decision states:

| State | Meaning | Host Action |
|---|---|---|
| `missing` | No compatible companion is installed. | Install from bundled manifest. |
| `ready` | Installed companion matches manifest and protocol. | Skip install. |
| `upgrade_required` | Installed companion is older than bundled manifest. | Upgrade before launch. |
| `protocol_incompatible` | Installed companion major protocol differs from host. | Reinstall if bundled companion is compatible, else fail. |
| `companion_too_new` | Device has a newer companion than the current host knows how to manage. | Fail fast; no auto-downgrade in MVP. |

Launch ownership:

* After install/update, the host launches the companion into session mode.
* The current HDC command shape is concrete, but later readiness/backoff may refine it:
  * the companion becomes reachable on the `session` channel
  * the companion can surface authorization prompts through the device shell if required

### 5.2 First-Run Authorization Contract

Authorization is reported per feature. MVP reserves these feature keys:

* `video_capture`
* `input_injection`

Allowed authorization states:

* `granted`
* `needs_user_action`
* `denied`
* `unsupported`

Rules:

* `device_hello` must include the current authorization state for every requested feature.
* `authorization_update` is the only message that may change authorization state after `device_hello`.
* The host must not send `session_config` until all required features for the requested session are `granted`.
* `needs_user_action` is not a fatal error by itself. It means the host should wait, prompt the user, or retry after the device-side flow completes.
* `denied` is fatal for the feature requested by the current session.
* `unsupported` is fatal unless the host can drop that feature and restart negotiation explicitly.
* Device-side TCP listeners require `ohos.permission.INTERNET` in `sources/hscrcpy_server/entry/src/main/module.json5`; without it, session bootstrap may fail with `Operation not permitted` before `bind()`.
* HarmonyOS module permissions must live in one `requestPermissions` array. Adding a second same-named key can cause the packaged manifest to keep only the later array, dropping `ohos.permission.INTERNET` and breaking the HAP route before the host reaches authorization negotiation.

### 5.3 Startup Handshake Contract

The startup sequence is:

1. Host detects device and resolves install/update state.
2. Host ensures the companion is running.
3. Host opens `session`.
4. Host sends `host_hello`.
5. Device replies with `device_hello`.
6. If needed, device later sends `authorization_update` until required features are `granted` or fail.
7. Host sends `session_config`.
8. Device replies with either `session_ready` or `session_error`.
9. Host opens `video` and steady-state streaming begins.

`host_hello` fields:

| Field | Type | Required | Notes |
|---|---|---:|---|
| `type` | string | yes | Must be `host_hello`. |
| `session_id` | string | yes | Stable ID for both channels. |
| `protocol_major` | integer | yes | Major compatibility gate. |
| `protocol_minor` | integer | yes | Minor capability marker. |
| `host_version` | string | yes | Host build version. |
| `requested_features` | string[] | yes | MVP values: `video`, `control`. |
| `supported_video_codecs` | string[] | yes | Codec names supported by the host decoder/render path. Current required value is `h264`; future examples include `h265`, `h266`, `vp9`, `av1`, and `jpeg`. |
| `preferred_video_codecs` | string[] | yes | Ordered preference list. |
| `video_limits.max_width` | integer | yes | Host render/decode ceiling. |
| `video_limits.max_height` | integer | yes | Host render/decode ceiling. |
| `video_limits.max_fps` | integer | yes | Host steady-state target ceiling. |
| `video_limits.bitrate_kbps` | integer | no | Host hint, not a guarantee. |

`device_hello` fields:

| Field | Type | Required | Notes |
|---|---|---:|---|
| `type` | string | yes | Must be `device_hello`. |
| `session_id` | string | yes | Mirrors the host session. |
| `protocol_major` | integer | yes | Device major compatibility. |
| `protocol_minor` | integer | yes | Device minor capability level. |
| `companion_version` | string | yes | Device companion build version. |
| `device_name` | string | yes | Human-readable target name. |
| `authorization` | object | yes | Per-feature authorization map. |
| `available_features` | string[] | yes | MVP values: `video`, `control`. |
| `available_video_codecs` | object[] | yes | Codec descriptors listed below. |
| `display.width` | integer | yes | Current capture width hint. |
| `display.height` | integer | yes | Current capture height hint. |
| `display.rotation` | integer | yes | Current rotation in degrees. |

`authorization_update` fields:

| Field | Type | Required | Notes |
|---|---|---:|---|
| `type` | string | yes | Must be `authorization_update`. |
| `session_id` | string | yes | Session being updated. |
| `authorization` | object | yes | Updated per-feature authorization map. |
| `reason` | string | no | Optional human-readable explanation for logs/UI. |

`session_config` fields:

| Field | Type | Required | Notes |
|---|---|---:|---|
| `type` | string | yes | Must be `session_config`. |
| `session_id` | string | yes | Session being configured. |
| `selected_video_codec` | string | yes | Selected codec name from the shared registry. Current required implementation target is `h264`. |
| `video.max_width` | integer | yes | Host-selected width cap. |
| `video.max_height` | integer | yes | Host-selected height cap. |
| `video.max_fps` | integer | yes | Host-selected FPS cap. |
| `video.bitrate_kbps` | integer | no | Used only for bitrate-driven codecs. |
| `video.iframe_interval_ms` | integer | no | Mainly relevant for inter-frame video codecs such as H.264/H.265 and future codecs with keyframe control. |
| `control.enabled` | boolean | yes | Whether control messages will be sent. |
| `rotation.locked` | boolean | no | Reserved for later display policy. |

`session_ready` fields:

| Field | Type | Required | Notes |
|---|---|---:|---|
| `type` | string | yes | Must be `session_ready`. |
| `session_id` | string | yes | Session now active. |
| `selected_video_codec` | string | yes | Final device-confirmed codec. |
| `channel_layout` | object | yes | Must confirm `session` and `video`. |
| `display.width` | integer | yes | Active stream width. |
| `display.height` | integer | yes | Active stream height. |
| `display.rotation` | integer | yes | Active stream rotation. |

`session_error` fields:

| Field | Type | Required | Notes |
|---|---|---:|---|
| `type` | string | yes | Must be `session_error`. |
| `session_id` | string | yes | Session that failed. |
| `code` | string | yes | Stable error code from the matrix below. |
| `message` | string | yes | Human-readable description. |
| `retryable` | boolean | yes | Whether the host may retry automatically. |

`stop_session` fields:

| Field | Type | Required | Notes |
|---|---|---:|---|
| `type` | string | yes | Must be `stop_session`. |
| `session_id` | string | yes | Session being stopped. |
| `reason` | string | no | Optional shutdown reason for logs. |

`control_event` fields:

| Field | Type | Required | Notes |
|---|---|---:|---|
| `type` | string | yes | Must be `control_event`. |
| `session_id` | string | yes | Session carrying the input event. |
| `event_type` | string | yes | See allowed values below. |
| `sequence` | integer | yes | Monotonic per-session control sequence number. |
| `position_norm.x` | number | no | Normalized X in `[0.0, 1.0]` for pointer events. |
| `position_norm.y` | number | no | Normalized Y in `[0.0, 1.0]` for pointer events. |
| `pointer_id` | integer | no | Pointer contact ID for move/down/up. |
| `button` | string | no | `primary`, `secondary`, or `middle`. |
| `scroll_delta_x` | integer | no | Horizontal scroll delta. |
| `scroll_delta_y` | integer | no | Vertical scroll delta. |
| `key_code` | string | no | Host-normalized key identifier. |
| `text` | string | no | Reserved for later text input support. |
| `device_action` | string | no | `back`, `home`, or later system action names. |

Allowed MVP `control_event.event_type` values:

* `pointer_down`
* `pointer_move`
* `pointer_up`
* `scroll`
* `key_down`
* `key_up`
* `device_action`

Rules:

* Pointer coordinates are normalized against the active streamed display, not absolute host pixels.
* The device side owns mapping normalized coordinates into current device display coordinates.
* `sequence` is used for ordering/debugging only; MVP does not require explicit control acknowledgements.

### 5.4 Capability Negotiation Contract

Codec negotiation is host-driven after the selected route advertises or infers actual encoder capabilities and the host probes decoder/render capabilities. The codec set must be extensible; current implementation can require only H.264 end-to-end while still preserving future capability entries.

Stable selection rules:

* `preferred_video_codecs` from `host_hello` is ordered by host preference.
* `available_video_codecs` from `device_hello` is the device source of truth.
* `host_available_decoders` or an equivalent host-side capability set is the host source of truth.
* Host chooses the first codec that is supported by device/route encoding, host decoding, and the session request.
* Current route bringup should select H.264 unless explicitly testing another verified codec.
* Future codecs such as H.265, H.266/VVC, VP9, AV1, and JPEG may be selected only after both route/device encoder support and host decoder support are verified.
* If the preferred codec is unavailable or not allowed, host should fall back to the next mutually supported codec.
* The selected codec and fallback reason must be visible in host diagnostics before stream start.

Each `available_video_codecs` entry must provide:

| Field | Type | Required | Notes |
|---|---|---:|---|
| `codec` | string | yes | Codec registry name. Current required implementation target is `h264`; future examples include `h265`, `h266`, `vp9`, `av1`, and `jpeg`. |
| `encoder_kind` | string | yes | `hardware` or `software`. |
| `max_width` | integer | yes | Device codec ceiling. |
| `max_height` | integer | yes | Device codec ceiling. |
| `max_fps` | integer | yes | Device codec ceiling. |
| `bitrate_control` | string | no | Optional hint such as `cbr` or `vbr`. |

Each host decoder capability entry should provide:

| Field | Type | Required | Notes |
|---|---|---:|---|
| `codec` | string | yes | Codec registry name. Current required implementation target is `h264`; future examples include `h265`, `h266`, `vp9`, `av1`, and `jpeg`. |
| `decoder_kind` | string | yes | `hardware`, `software`, `external`, or `unknown`. |
| `max_width` | integer | no | Host decode/render ceiling when known. |
| `max_height` | integer | no | Host decode/render ceiling when known. |
| `max_fps` | integer | no | Host decode/render ceiling when known. |

Route rules:

* HAP route may obtain device encoder capability from the device `device_hello`.
* Official `uitest` route may infer device encoder capability from selected official payload, launch parameters, and verified stream behavior until the route exposes a richer capability API.
* The final codec decision must use the same selection function for both routes.
* Adding a new codec must not require redesigning route startup; it should add registry metadata, route capability mapping, host decoder probing, tests, and renderer handling.

### 5.5 Channel Split Contract

MVP channel responsibilities are:

| Channel | Purpose | Payload Type | Notes |
|---|---|---|---|
| `session` | handshake, authorization updates, control events, teardown | UTF-8 JSON | Open first and keep for full session lifetime. |
| `video` | encoded video units only | binary | Open only after `session_ready`. |

Rules:

* `control_event` messages always travel on `session`.
* Video payloads never travel on `session`.
* A `session_id` binds both channels into one session.
* If `session` dies, the whole session is invalid.
* If `video` dies while `session` remains alive, the host may stop and renegotiate, but must not silently keep sending controls to a dead stream.

## 6. Validation & Error Matrix

| Condition | Detected By | Expected Response | Error Code |
|---|---|---|---|
| Host and device `protocol_major` differ | host or device during hello | Abort immediately | `protocol_major_mismatch` |
| No shared video codec exists | host after `device_hello` | Abort before `session_config` | `no_shared_video_codec` |
| Host decoder does not support the device's preferred codec | host before stream start | Select the next mutually supported codec | `codec_fallback` |
| Selected route cannot provide the selected codec | host before stream start | Select next codec or abort if none remains | `route_codec_unavailable` |
| Stale `xdevice_scrcpy` process exists before launch | host before `uitest` launch | Try to kill it; fail if kill is denied or process remains alive | `stale_route_process` |
| Temporary scrcpy server `.so` remains after failed launch | host cleanup after failed launch | Try to remove it; fail if cleanup fails | `route_payload_cleanup_failed` |
| A requested feature is `unsupported` | device during hello/update | Abort or drop feature and restart explicitly | `feature_unsupported` |
| A required feature is `denied` | device during hello/update | Abort current session | `authorization_denied` |
| Authorization still pending | device during hello/update | Wait for user action; do not start video | `authorization_pending` |
| Installed companion is older than required | host before launch | Upgrade then retry | `companion_outdated` |
| Installed companion is newer than host contract | host before launch | Fail fast | `companion_too_new` |
| Device session listener fails to create/bind its socket | device during bootstrap | Abort startup and log device-side runtime error | `session_listener_bootstrap_failed` |
| Host uses unresolved `auto` target for `hdc -t ...` | host before shell/forward | Resolve to a concrete visible target before command execution | `invalid_auto_target` |
| Device rejects selected config | device on `session_config` | Return `session_error` | `session_config_rejected` |
| Video channel does not come up after `session_ready` | host after ready | Stop session and report failure | `video_channel_timeout` |
| Host opens `video` but device never writes a complete header | host after ready | Abort current capture and inspect device video runtime/logs | `video_packet_header_eof` |
| Official `uitest` payload selection fails | host before `uitest` lifecycle | Abort selected route before any HDC mutation | `route_payload_selection_failed` |
| Official `uitest` payload push, stale kill, daemon launch, cleanup, or fport fails | host during `uitest` lifecycle | Abort selected route, include route, phase, target, and HDC output | `route_lifecycle_failed` |
| Official `uitest` gRPC connect/start/status fails | host after lifecycle/fport | Abort selected route with route, phase, target, method, payload, codec, and transport/status context; do not fall back to HAP route | `route_grpc_transport_failed` |
| Official first H.264 frame is not Annex-B or is empty | host before renderer handoff | Abort frame ingestion with H.264/Annex-B context | `route_frame_ingress_invalid` |
| Official `uitest` shutdown is requested through adapter | host during teardown | Call `ScrcpyService/onEnd`; surface result/failure | `route_shutdown_failed` |

## 7. Good / Base / Bad Cases

### Good

* Host detects the device, sees `ready`, opens `session`, negotiates `h264`, receives `session_ready`, then opens `video`.
* `video_capture` and `input_injection` are both `granted`, so the session starts without user interaction.
* Device bootstrap logs `state=listening port=27182 bound=true`, and later the host CLI receives JPEG or H.264 units without a reset/EOF before the first frame.

### Base

* Host detects the device, sees `ready`, but `h264` is not present in `available_video_codecs`.
* Host selects `jpeg`, sends `session_config`, receives `session_ready`, and starts a lower-performance fallback session without changing channel roles.
* `jpeg` fallback succeeds end-to-end first while `h264` remains a decoder-ready bringup path that writes `.h264` artifacts instead of a polished live renderer.

### Bad

* `device_hello.authorization.video_capture` is `needs_user_action`, but the host tries to send `session_config` anyway.
* Device should reject that flow with `session_error.code = authorization_pending`.

* Host sees a device-side companion with a higher incompatible protocol and silently keeps going.
* Host should stop before launch with `companion_too_new`.

* Device bootstrap logs `failed to create session listener socket: Operation not permitted`, but the host still retries transport blindly.
* Correct response is to reinstall a HAP whose module manifest declares `ohos.permission.INTERNET`, then rerun startup.

* Host forwards and opens `video`, but the device never writes a binary header.
* Correct response is to treat that as a device video-runtime defect, not as a render bug.

## 8. Tests Required

* Host-side unit tests for install/update decision states: `missing`, `ready`, `upgrade_required`, `protocol_incompatible`, `companion_too_new`.
* Host-side unit tests that `--device auto` resolves once to a concrete visible target and is reused by later `shell`/`fport` calls.
* Host-side unit tests for codec selection:
  * `h264` selected for the current supported implementation path when route/device encoder and host decoder support it
  * future codec names such as `h265`, `h266`, `vp9`, `av1`, and `jpeg` can be represented without changing the selection data model
  * fallback selects the next mutually supported codec when a preferred codec is unavailable
  * no codec selected when host decode and route/device encode sets do not intersect
* Protocol parse/serialize tests for every `session` message type.
* Device-side tests that `session_config` is rejected when required authorization is not `granted`.
* Integration tests that `session_ready` is emitted before `video` activation and never after a prior `session_error`.
* Logging assertions for fallback and protocol mismatch paths so failures carry `subsystem`, `operation`, and `session_id`.
* Device-side bootstrap validation that `EntryAbility.onCreate()` logs `state=listening port=27182 bound=true` after install/run.
* Static manifest validation that `sources/hscrcpy_server/entry/src/main/module.json5` contains one `requestPermissions` key and includes `ohos.permission.INTERNET` alongside any other shell permissions.
* Manual bringup checks for the HAP route:
  * `cargo run -p hscrcpy-host-cli -- --device auto --route hscrcpy-server --codec jpeg --max-frames 30 --output-dir /tmp/hscrcpy-preview --no-live-preview`
  * `cargo run -p hscrcpy-host-cli -- --device auto --route hscrcpy-server --codec h264 --max-frames 30 --output-dir /tmp/hscrcpy-preview`
  * `cargo run -p hscrcpy-host-cli -- --device auto --route hscrcpy-server --codec h264 --max-frames 30 --output-dir /tmp/hscrcpy-preview --record-h264`
  * assert JPEG artifacts (`preview.html`, `latest.jpg`, `frames/*.jpg`) are produced on the JPEG route, H.264 live preview works without disk recording by default, and H.264 artifacts (`frames/*.h264`, `stream.h264`) are produced only when `--record-h264` is set
* Manual lifecycle and stream check for the official `uitest` route:
  * `cargo run -p hscrcpy-host-cli -- --device auto --route uitest --codec h264 --uitest-payload third_party/hypium/hosScrcpy/6.1.0.210/libscrcpy/libscrcpy_server_unix_6.5-20260313.z.so --max-frames 30 --output-dir /tmp/hscrcpy-uitest-preview --ffplay-bin /opt/homebrew/bin/ffplay`
  * expected success: lifecycle diagnostics run through stale process scan, payload push, `uitest start-daemon`, `scrcpy_grpc_socket` forwarding, and payload cleanup; `ScrcpyService/onStart` returns H.264 access units; ffplay displays real device frames; `ScrcpyService/onEnd` is called when `--max-frames` is reached
  * expected failure shape: errors name `route=uitest`, a concrete phase such as `local-port-selection`, `grpc-socket-ready`, `grpc-connect`, `grpc-start`, `grpc-status`, `grpc-stream`, frame ingress, or render, plus the dynamic `127.0.0.1:<port>` target, method, selected payload, selected codec, device runtime diagnostics, and the underlying transport/protocol context
* Device-side inspection during manual checks:
  * `DEVICE="$(hdc list targets | awk 'NR==1{print $1}')"`
  * `hdc -t "$DEVICE" shell 'ps -ef | grep -E "[x]device_scrcpy|[u]itest start-daemon|[h]scrcpy"'`
  * `hdc -t "$DEVICE" shell 'cat /proc/net/unix | grep -E "scrcpy_grpc_socket|screen_record_grpc_socket|uitest_socket"'`
  * `hdc fport ls`
  * `hdc -t "$DEVICE" shell 'hilog -x | grep -E "xdevice_scrcpy|scrcpy_grpc_socket|screen_record|uitest|hypium|UiTestKit_Addon|CreateVirtualScreen|video/avc|hscrcpyDiag"'`

## 9. Wrong vs Correct

### Wrong

* Host and device both independently decide the fallback codec in opaque local logic.
* Route-specific code hard-codes `h264` while host-side decoder probing says only `jpeg` is available.
* Device starts streaming on the first available codec before receiving `session_config`.
* Host leaves `/data/local/tmp/scrcpy_server.so` behind after a failed `uitest` launch.
* Control messages are multiplexed into the hot video path because it looks simpler during bring-up.
* Host treats `auto` like a real HDC serial and issues `hdc -t auto ...`.
* Device tries to open TCP listeners without declaring `ohos.permission.INTERNET`.

### Correct

* Device reports runtime capabilities in `device_hello`.
* Host performs the final codec selection and sends it back in `session_config`.
* Host codec selection combines route/device encoder capability with host decoder capability before stream start.
* Host removes temporary `uitest` payload files after launch success or failure.
* Host attempts to kill stale route-owned device processes before launch and fails clearly if shell permission is insufficient.
* Device starts streaming only after `session_ready`.
* `session` remains the single home for control events and handshake traffic.
* Host resolves `auto` to a concrete visible target before all later HDC commands.
* Device manifest declares `ohos.permission.INTERNET` before session/video listener bootstrap is attempted.

## 10. HarmonyOS Pattern Pointers

These patterns materially affect later device work and should be preserved:

* Keep the generated HarmonyOS app-shell pattern: `UIAbility` and ArkTS pages own lifecycle and user prompts, not the capture pipeline.
* Keep the N-API bridge thin between ArkTS and native code; do not move stream business logic into page handlers.
* Align capture bring-up with the OpenHarmony `AVScreenCapture` native usage pattern already noted in the brainstorm research.
* Align codec probing with the OpenHarmony `AVCodec` capability-query pattern so `device_hello.available_video_codecs` is runtime-derived, not hard-coded.

## 11. Open Questions

* Whether HarmonyOS input injection can reuse the same long-lived authorization model as screen capture or needs a separate re-prompt path.
* Whether the companion should keep a background-ready mode between sessions or exit fully after each stop in MVP.
* Whether the future `video` channel will use fixed HDC-forwarded ports or dynamically allocated bindings under the same two-channel model.
* Whether later display metadata needs a stable `display_id` field once multi-display support matters.
