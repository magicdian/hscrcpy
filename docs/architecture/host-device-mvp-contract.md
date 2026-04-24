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
* `H.264` is the preferred video path, `JPEG` is the required fallback, and `H.265` is reserved as experimental.
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
* launch the companion into session mode
* open the `session` channel first
* negotiate features and select the video path
* open and monitor the `video` channel after negotiation succeeds
* surface authorization and compatibility failures clearly

### Device Responsibilities

* expose a thin HarmonyOS app shell for lifecycle and permission prompts
* hand off runtime work to a native core
* report authorization state instead of guessing host intent
* advertise capture, codec, and control capabilities at runtime
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

Rules:

* `--device auto` is a host-side convenience only. The host must resolve it once to a concrete serial before issuing `hdc -t ...` commands.
* The host currently launches `EntryAbility` first, and the device-side `EntryAbility.onCreate()` is responsible for bootstrapping native session listener startup.
* Companion install/update decisions are based on the top-level `versionCode` / `versionName` fields from `bm dump`, not nested quick-fix or module fields.

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
| `0` | `1` | `codec` | `1=h264`, `2=jpeg`, `3=h265` |
| `1` | `8` | `pts_us` | big-endian unsigned integer |
| `9` | `1` | `is_keyframe` | `0` or `1` |
| `10` | `4` | `payload_length` | big-endian unsigned integer |
| `14` | `payload_length` | payload bytes | negotiated codec payload |

Rules:

* Host must reject packets whose `codec` tag does not match the negotiated codec.
* `payload_length` must be positive and must match the number of bytes following the header.
* For `codec=h264`, payload bytes must be one Annex-B access unit (3-byte or 4-byte start-code delimiters) with at least one valid NAL unit.
* For `codec=h264` packets marked `is_keyframe=1`, the access unit must include an IDR NAL (`nal_unit_type=5`) so host-side keyframe assumptions stay aligned with decoder expectations.

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
| `supported_video_codecs` | string[] | yes | Subset of `h264`, `jpeg`, `h265`. |
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
| `selected_video_codec` | string | yes | `h264`, `jpeg`, or later `h265`. |
| `video.max_width` | integer | yes | Host-selected width cap. |
| `video.max_height` | integer | yes | Host-selected height cap. |
| `video.max_fps` | integer | yes | Host-selected FPS cap. |
| `video.bitrate_kbps` | integer | no | Used only for bitrate-driven codecs. |
| `video.iframe_interval_ms` | integer | no | Mainly relevant for `h264`/`h265`. |
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

Codec negotiation is host-driven after the device advertises actual runtime capabilities.

Stable selection rules:

* `preferred_video_codecs` from `host_hello` is ordered by host preference.
* `available_video_codecs` from `device_hello` is the device source of truth.
* Host chooses the first codec that is supported by both sides and permitted by the session request.
* If `h264` is unavailable or unusable, host falls back to `jpeg`.
* `h265` must not displace `h264` in MVP unless explicitly requested by a future experimental mode.

Each `available_video_codecs` entry must provide:

| Field | Type | Required | Notes |
|---|---|---:|---|
| `codec` | string | yes | `h264`, `jpeg`, or `h265`. |
| `encoder_kind` | string | yes | `hardware` or `software`. |
| `max_width` | integer | yes | Device codec ceiling. |
| `max_height` | integer | yes | Device codec ceiling. |
| `max_fps` | integer | yes | Device codec ceiling. |
| `bitrate_control` | string | no | Optional hint such as `cbr` or `vbr`. |

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

## 7. Good / Base / Bad Cases

### Good

* Host detects the device, sees `ready`, opens `session`, negotiates `h264`, receives `session_ready`, then opens `video`.
* `video_capture` and `input_injection` are both `granted`, so the session starts without user interaction.
* Device bootstrap logs `state=listening port=27182 bound=true`, and later the host CLI captures JPEG or H.264 artifacts without a reset/EOF before the first frame.

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
* Host-side unit tests for codec selection order: `h264` preferred over `jpeg`, `jpeg` selected when `h264` absent, no silent `h265` promotion in MVP.
* Protocol parse/serialize tests for every `session` message type.
* Device-side tests that `session_config` is rejected when required authorization is not `granted`.
* Integration tests that `session_ready` is emitted before `video` activation and never after a prior `session_error`.
* Logging assertions for fallback and protocol mismatch paths so failures carry `subsystem`, `operation`, and `session_id`.
* Device-side bootstrap validation that `EntryAbility.onCreate()` logs `state=listening port=27182 bound=true` after install/run.
* Manual bringup checks:
  * `cargo run -p hscrcpy-host-cli -- --device auto --codec jpeg --max-frames 30 --output-dir /tmp/hscrcpy-preview`
  * `cargo run -p hscrcpy-host-cli -- --device auto --codec h264 --max-frames 30 --output-dir /tmp/hscrcpy-preview`
  * assert JPEG artifacts (`preview.html`, `latest.jpg`, `frames/*.jpg`) and H.264 artifacts (`frames/*.h264`, `stream.h264` where applicable) are produced

## 9. Wrong vs Correct

### Wrong

* Host and device both independently decide the fallback codec in opaque local logic.
* Device starts streaming on the first available codec before receiving `session_config`.
* Control messages are multiplexed into the hot video path because it looks simpler during bring-up.
* Host treats `auto` like a real HDC serial and issues `hdc -t auto ...`.
* Device tries to open TCP listeners without declaring `ohos.permission.INTERNET`.

### Correct

* Device reports runtime capabilities in `device_hello`.
* Host performs the final codec selection and sends it back in `session_config`.
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
