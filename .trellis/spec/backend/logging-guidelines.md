# Logging Guidelines

> What to log from non-UI runtime code.

---

## Overview

Current logging in the repo exists only in the HarmonyOS ArkTS shell and uses `hilog`.

As native code and a desktop host are added, logging should stay structured and phase-oriented. Logs must help debug startup, capability negotiation, stream health, and control injection without leaking sensitive data.

---

## Log Levels

* `info`: lifecycle milestones, companion startup, capability negotiation, session start/stop
* `warn`: recoverable fallback, degraded codec path, retryable transport issues
* `error`: startup failure, incompatible versions, unrecoverable API errors, bridge failures

Do not spam frame-by-frame success logs in hot paths.

---

## Structured Logging

Include these fields whenever practical:

* subsystem
* operation
* device identifier or target alias
* selected codec or fallback mode
* error code or API name when a failure occurs

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
| Native encoder callback | `session_id`, `callbacks`, `access_units`, `payload_bytes_avg`, `pts_step_avg_us`, `callback_step_avg_us`, `dropped_units` | `encoder_output_anomaly` |
| Native transport send | `session_id`, `packets`, `send_duration_avg_us`, `send_duration_max_us`, `timeline_lag_max_us`, `backpressure` | `send_anomaly`, `runtime_error`, `stream_error` |
| Host preview timing | `units`, `pts_span_us`, `host_rx_span_us`, `host_present_span_us`, `host_present_minus_pts_us`, `ffplay_write_*` | `diag window=...` flush with large skew or slow writes |

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
