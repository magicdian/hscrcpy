# Logging Guidelines

> What to log from non-UI runtime code.

---

## Overview

Current logging in the repo exists only in the HarmonyOS ArkTS shell and uses `hilog`.

As native code and a desktop host are added, logging should stay structured and phase-oriented. Logs must help debug startup, capability negotiation, stream health, and control injection without leaking sensitive data.

---

## Log Levels

Host Rust logs use `crates/hscrcpy-host/src/host_log.rs` for structured stdout logs. The current development default is `debug`; set `HSCRCPY_LOG=trace|debug|info|warn|error|off` to control verbosity.

* `trace`: exact command shapes and highly detailed retry internals useful during bringup
* `debug`: retry attempts, selected payload names, forward specs, and other development diagnostics
* `info`: lifecycle milestones, companion startup, capability negotiation, session start/stop
* `warn`: recoverable fallback, degraded codec path, retryable transport issues
* `error`: startup failure, incompatible versions, unrecoverable API errors, bridge failures

Do not spam frame-by-frame success logs in hot paths. During official `uitest` bringup, logging each lifecycle retry is acceptable because it is startup-path evidence, not steady-state media logging.

---

## Structured Logging

Include these fields whenever practical:

* subsystem
* operation
* device identifier or target alias
* selected codec or fallback mode
* error code or API name when a failure occurs

Host Rust log line shape:

```text
log level=<level> subsystem=<subsystem> operation=<operation> key=value ...
```

Use public-safe formatting in ArkTS logs.

---

## What to Log

* companion install/update decisions
* first-run authorization status
* codec capability detection
* session handshake and teardown
* fallback from `H.264` to `JPEG`

---

## What NOT to Log

* secrets, tokens, certificates, or raw private keys
* full user file paths unless they are necessary for debugging and safe to expose
* raw clipboard or input payloads
* per-frame debug logs in steady-state streaming

---

## Examples

* [`sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets`](../../sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets): uses `hilog.info()` for lifecycle milestones and `hilog.error()` for initialization failure.
* [`sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets`](../../sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets): demonstrates public-format logging from a UI event, which is acceptable for a template but should not become the only session telemetry path.
* [`sources/hscrcpy_server/code-linter.json5`](../../sources/hscrcpy_server/code-linter.json5): the repo already enforces security-oriented static rules on ArkTS files, which should be matched by equally conservative logging choices.

---

## Common Mistakes

* Treating logs as a substitute for error propagation.
* Logging raw payloads from device control or media streams.
* Using inconsistent subsystem names across ArkTS, native, and host code.

## Scenario: Native + Host Stream Diagnostics

### 1. Scope / Trigger

Use this contract when touching:

* `sources/hscrcpy_server/entry/src/main/cpp/core/native_diag_log.h`
* `sources/hscrcpy_server/entry/src/main/cpp/capture/h264_screen_capture_source.*`
* `sources/hscrcpy_server/entry/src/main/cpp/transport/video_channel.cpp`
* `crates/hscrcpy-host/src/render/bringup.rs`
* `apps/hscrcpy-host-cli/src/main.rs`

This applies whenever a change affects steady-state media timing, queue depth, packet send latency, or live-preview backpressure.

### 2. Signatures

* Native structured log entry:
  * tag: `hscrcpyDiag`
  * fields: `subsystem=<...> operation=<...> ...`
* Native summary operations:
  * `capture/h264_screen_capture_source -> encoder_output_summary|encoder_output_anomaly`
  * `capture/h264_screen_capture_source -> start_config|capture_state|encoder_stream_changed|encoder_codec_config|encoder_access_unit_trace`
  * `transport/video_channel -> send_summary|send_anomaly|runtime_error|stream_error`
* Host summary line:
  * CLI stdout prefix: `diag window=...`
  * render event log path: `<session_dir>/events.log`

### 3. Contracts

* Hot-path logs must emit interval summaries, not per-frame success spam.
* Anomaly logs must explain the condition with operation-specific fields.
* Summary lines must be keyed by subsystem/operation and include enough data to compare device PTS cadence against host receive/present cadence.
* Host summaries must survive in `events.log` even when stdout is not captured.

### 4. Validation & Error Matrix

| Area | Required fields | Failure / anomaly signal |
|------|-----------------|--------------------------|
| Native encoder callback | `session_id`, `callbacks`, `access_units`, `payload_bytes_avg`, `pts_step_avg_us`, `callback_step_avg_us`, `dropped_units`, `first_keyframe_idx`, `idr_with_config`, `idr_with_prepended_config` | `encoder_output_anomaly` |
| Native H.264 startup trace | `session_id`, `frame_idx`, `payload_bytes`, `emitted_bytes`, `input_sps`, `input_pps`, `input_idr`, `emitted_sps`, `emitted_pps`, `emitted_idr`, `prepended_config_bytes` | missing first `encoder_access_unit_trace` keyframe or IDR without SPS/PPS |
| Native transport send | `session_id`, `packets`, `send_duration_avg_us`, `send_duration_max_us`, `timeline_lag_max_us`, `backpressure` | `send_anomaly`, `runtime_error`, `stream_error` |
| Host preview timing | `units`, `pts_span_us`, `host_rx_span_us`, `host_present_span_us`, `host_present_minus_pts_us`, `ffplay_write_*` | `diag window=...` flush with large skew or slow writes |
| Host H.264 frame profile | `frame`, `payload_bytes`, `h264_nals`, `h264_sps`, `h264_pps`, `h264_idr`, `h264_types` in `<session_dir>/events.log` | `h264_parse_error`, keyframe without `h264_idr=true`, first decodable frame without SPS/PPS |

### 5. Good/Base/Bad Cases

* Good: emit one summary every 120 H.264 units and separate anomaly logs when send/write cadence crosses a threshold.
* Base: log startup decisions once, then only emit teardown summary if the stream ends before a full window.
* Bad: logging every frame write or dumping raw payload bytes to stdout/hilog.

### 6. Tests Required

* Host unit tests that confirm H.264 timing summaries flush into the bringup renderer.
* Manual real-device check:
  * `hdc shell hilog -x | grep -E 'hscrcpyDiag|capture/h264_screen_capture_source|transport/video_channel'`
  * host CLI with `tee` so `diag window=...` lines are captured.
* Any new anomaly threshold or field rename must be reflected in both native log text and host summary formatting.

### 7. Wrong vs Correct

#### Wrong

* One `hilog.info()` per access unit with no subsystem/operation field.
* Host preview timing only printed to stdout and lost after process exit.

#### Correct

* Native logs use `hscrcpyDiag` with `subsystem` and `operation`, interval summaries, and anomaly-specific fields.
* Host timing summaries are printed to stdout and appended to `<session_dir>/events.log` for post-run analysis.
* H.264 startup investigation compares native `encoder_access_unit_trace` against host `frame=... h264_*` lines to prove whether IDR/SPS/PPS was produced by the encoder, altered by server packaging, or lost before ffplay.

## Scenario: Stream Latency and FPS Regression Triage

### 1. Scope / Trigger

Use this checklist when investigating slow startup, visible stream delay, frame cadence below the target, or uncertainty about whether ffplay or project code is causing latency.

### 2. Contracts

* The default H.264 live path must avoid diagnostic disk writes in the per-frame hot path.
* `.h264` frame files and `stream.h264` are opt-in diagnostics and must require an explicit host flag.
* A 120 Hz target means host requests, route launch arguments, route capability metadata, and device encoder configuration must all carry `max_fps=120` or a measured lower device capability.
* Host `diag window=full units=120` lines are diagnostic aggregation only. They must not be interpreted as a live-preview startup gate; ffplay should receive H.264 as soon as the first decodable keyframe path is available.
* If `<session-dir>/live-preview.log` reports `non-existing PPS` or `no frame`, inspect H.264 decoder config handling before blaming ffplay timing. The HAP route must cache encoder `OH_MD_KEY_CODEC_CONFIG` and prepend Annex-B SPS/PPS before an IDR that does not already include them.
* For the HAP `hscrcpy-server` surface-encoder path, configure `OH_MD_KEY_VIDEO_ENCODER_REPEAT_PREVIOUS_FRAME_AFTER` and `OH_MD_KEY_VIDEO_ENCODER_REPEAT_PREVIOUS_MAX_COUNT` so static screens still emit repeated H.264 access units. Without them, startup and visible refresh can appear dependent on screen changes even when host pipe writes are fast. Compare `raw_pts` deltas in `encoder_access_unit_trace` after static-screen repeat begins; if a configured value of `33` produces 33-us deltas, use a `33000` repeat-after value for that device stack and log the raw configured value.
* HAP H.264 encoder timestamps must be normalized to protocol microseconds before transport. If `pts_span_us` is roughly 1000x larger than `host_rx_span_us`, the device is likely forwarding nanosecond-scale encoder PTS as microseconds.
* Official `uitest` launch parameters must keep HoKit-verified values unless real-device diagnostics prove otherwise; in particular `-frameRate 120` is compatible with `-repeatInterval 33`, because repeat interval maps to encoder repeat-previous-frame behavior rather than the nominal stream FPS.
* ffplay raw H.264 preview must set the input `-framerate` to the selected stream FPS; the raw H.264 demuxer default is not a valid latency/fps assumption.
* When ffplay writes are fast but the window stays blank, inspect `<session_dir>/live-preview.log` before blaming transport. `non-existing PPS` means the preview path dropped SPS/PPS decoder config before the first IDR.
* Do not blame ffplay until `ffplay_write_*` diagnostics show slow writes or backpressure while device/host cadence remains healthy.
* When ffplay writes are fast, the first host frame contains `h264_sps=true h264_pps=true h264_idr=true`, and the window still does not show a frame, inspect the live-preview command before returning to encoder debugging. Aggressive `-fflags nobuffer` / `-framedrop` settings can discard startup frames on short raw H.264 bursts; prefer `-fflags +genpts`, explicit `-probesize 32`, `-analyzeduration 0`, and `-sync ext` while bringing up visibility.
* Official `uitest` streams may start with SPS/PPS only and never emit IDR until the screen changes. Keep the default path passive, but allow explicit `--uitest-request-idr-on-start` runs to test whether `ScrcpyService/onRequestIDRFrame` can make static screens visible on a given device. Log `official_scrcpy_request_idr` / `official_scrcpy_request_idr_failed` so stream-close regressions are attributable to the opt-in request.
* If adding a future `uitest` startup snapshot fallback, distinguish synthetic placeholder H.264 from official H.264 in logs. A snapshot-derived IDR may make the window visible, but it must not be logged or treated as the official first keyframe. Keep logging official `h264_idr=true` separately so corruption from feeding official P frames after a synthetic IDR is detectable.
* For `uitest` startup snapshot fallback, a single synthetic raw-H.264 access unit may be parsed by ffplay but still not visibly present. Feed a short repeated synthetic-IDR burst and log `repeat_units`, `payload_bytes`, and `total_bytes` so real-device runs can distinguish encoder failure from ffplay presentation/probing behavior.
* When the snapshot fallback appears blank, inspect `<session_dir>/live-preview.log` before changing the H.264 encoder. If ffplay logs an `Input #0, h264` stream and increments `fd`/`vq`, the pipe and bitstream are valid enough to parse; the next suspect is presentation timing or insufficient buffered samples, not screenshot capture.

### 3. Required Evidence

| Layer | Evidence | Interpretation |
|-------|----------|----------------|
| Device encoder callback | `pts_step_avg_us`, `callback_step_avg_us`, `dropped_units`, `queue_depth_max` | Proves whether capture/encode cadence is already slow before transport |
| Device transport send | `send_duration_avg_us`, `send_duration_max_us`, `timeline_lag_max_us`, `backpressure` | Proves whether socket/HDC writes are blocking |
| Host ingress/render | `ingress_wait_avg_us`, `host_rx_span_us`, `host_present_span_us`, `present_avg_us` | Proves whether host code is adding delay before preview |
| ffplay pipe | `ffplay_write_avg_us`, `ffplay_write_max_us`, `ffplay_slow_writes` | Proves whether ffplay stdin is exerting backpressure |

### 4. Wrong vs Correct

#### Wrong

* Running a latency test while recording every H.264 access unit to disk by default.
* Feeding raw H.264 to ffplay without `-framerate <selected_fps>`.
* Waiting for the first H.264 IDR by dropping all earlier access units, including SPS/PPS decoder config.
* Inferring "ffplay is slow" only from visible delay without checking `ffplay_write_*` metrics.
* Setting `--fps 120` on the host while route launch args or device encoder configuration still clamp to 30/60 fps.

#### Correct

* Run the default live-preview path without H.264 disk recording first.
* Add `--record-h264` only for stream artifact capture, and treat the result as a diagnostic run with extra I/O.
* Compare all four timing layers before assigning root cause.
