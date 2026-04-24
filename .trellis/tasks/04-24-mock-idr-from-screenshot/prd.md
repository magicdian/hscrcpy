# brainstorm: mock initial IDR from screenshot

## Goal

Explore whether `hscrcpy-host-cli` can eliminate the static-screen startup delay in the uitest scrcpy pipeline by taking a screenshot before starting the stream, converting it into an initial H.264 intra frame or equivalent bootstrap stream, and feeding that to `ffplay` when the incoming uitest stream starts without an IDR frame.

## What I already know

* Commit `5c3388232734f4e2b01529cb2ca5e8b3f7aac864` optimized the `hscrcpy_server` projection path so `ffplay` shows output almost immediately after startup.
* The suspected reason is that the AVScreenCapture + encoder path emits an initial I/IDR frame.
* The uitest scrcpy path appears to emit no IDR when the screen is static, so `ffplay` cannot decode visible output until enough screen change triggers a keyframe or recoverable frame sequence.
* Proposed idea: before starting the uitest scrcpy stream, capture a still screenshot matching device screen dimensions, then use it to provide a synthetic initial visual frame if the real stream lacks IDR.
* The architecture contract already reserves a future `--uitest-startup-snapshot-idr` fallback and states that a snapshot-derived IDR may be used only as a visual placeholder.
* Current `BringupRenderSurface` spawns `ffplay` and `H264LivePreview` waits for the first keyframe before writing official H.264 units to ffplay.
* Current live preview caches SPS/PPS from pre-IDR official units and writes cached config before the first official IDR.

## Assumptions (temporary)

* The startup delay is primarily a decoder bootstrap problem: `ffplay` needs SPS/PPS plus an IDR or equivalent random access point before it can display decoded frames.
* A still screenshot can be captured reliably before the uitest stream starts and can match the stream resolution or be scaled/padded to match.
* `hscrcpy-host-cli` owns enough of the host-side pipeline to insert data before forwarding to `ffplay`.

## Open Questions

* Confirm CLI shape for the unsafe passthrough experiment.

## Requirements (evolving)

* Analyze whether a screenshot-derived mock IDR is technically valid for the current uitest scrcpy stream.
* Identify feasible host-side approaches and trade-offs before implementation.
* Preserve existing fast path when the uitest stream already includes a usable IDR.
* Do not feed official non-IDR units after a synthetic screenshot IDR before the official stream has emitted its own IDR.
* Implement Approach B: optional screenshot-derived synthetic H.264 SPS/PPS/IDR bootstrap for ffplay.
* Add an explicit experimental switch to allow feeding official pre-IDR non-IDR units after the synthetic IDR, for real-device observation only.
* Log the unsafe passthrough mode clearly so corrupted/stalled output is attributable to the experiment.

## Acceptance Criteria (evolving)

* [x] Design identifies whether a standalone screenshot-encoded IDR can be safely prepended to the uitest H.264 stream.
* [x] Design covers resolution, SPS/PPS/profile, timestamp/order, and decoder state compatibility risks.
* [x] MVP scope is explicitly chosen before implementation.
* [x] Design chooses whether the screenshot appears in ffplay, preview.html, or both.
* [x] CLI can enable synthetic snapshot IDR fallback for `--route uitest --codec h264`.
* [x] CLI can choose safe pre-IDR drop mode or unsafe pre-IDR passthrough mode.
* [x] Events log distinguishes `synthetic_snapshot_idr`, `official_pre_idr_dropped`, and `official_pre_idr_passthrough`.

## Definition of Done (team quality bar)

* Tests added/updated where appropriate.
* Lint / typecheck / CI green.
* Docs/notes updated if stream startup behavior changes.
* Rollout/rollback considered if risky.

## Out of Scope (explicit)

* Replacing the entire uitest scrcpy pipeline.
* Changing device-side official `uitest` behavior unless repo inspection shows a narrow viable hook.

## Technical Notes

* Inspected commit `5c3388232734f4e2b01529cb2ca5e8b3f7aac864`; it touched host ffplay startup, H.264 config parsing, uitest IDR diagnostics, and HAP AVScreenCapture encoder behavior.
* Relevant files:
  * `apps/hscrcpy-host-cli/src/main.rs`: CLI options, `--uitest-request-idr-on-start`, render loop.
  * `crates/hscrcpy-host/src/render/bringup.rs`: ffplay process, first-keyframe gating, cached SPS/PPS behavior.
  * `crates/hscrcpy-host/src/video/h264.rs`: Annex-B NAL inspection and decoder config extraction.
  * `crates/hscrcpy-host/src/official_scrcpy.rs`: official gRPC stream normalization.
  * `crates/hscrcpy-host/src/hdc/mod.rs`: host-side shell bridge where screenshot capture/pull support would likely be added.
  * `docs/architecture/host-device-mvp-contract.md`: existing static startup fallback contract.

## Research Notes

### Constraints from the current repo

* The official uitest stream is normalized to H.264 access units and handed to the same renderer as the HAP route.
* The renderer currently treats `access_unit.is_keyframe` as the moment ffplay may receive data.
* Current dependencies do not include image decoding or H.264 encoding crates/libraries; synthetic encoding would add a new tool or dependency.
* `HdcBridge` currently supports shell execution and push, but not file receive. Screenshot bootstrap would need either stdout capture from a shell command or a pull/recv method.

### Feasible approaches here

**Approach A: Snapshot as separate visual placeholder** (Recommended MVP)

* How it works: capture screenshot before starting official stream, write it to `latest.jpg` / preview surface immediately, keep ffplay waiting for official IDR exactly as today.
* Pros: lowest risk; no decoder-state corruption; validates screenshot timing and UX value first.
* Cons: does not make the ffplay window show immediately unless a separate image viewer/preview surface is used.

**Approach B: Snapshot-encoded IDR as ffplay-only placeholder**

* How it works: capture screenshot, encode it as a standalone Annex-B SPS/PPS/IDR, write it to ffplay before official IDR, but continue dropping official non-IDR units until official IDR arrives.
* Pros: ffplay can show a first visual quickly; matches the proposed future contract.
* Cons: needs host-side encoder/tooling; ffplay may reconfigure or flash when switching from synthetic encoder sequence to official stream; must carefully avoid treating synthetic IDR as official stream readiness.

**Approach C: Force/request official IDR**

* How it works: use or refine `/ScrcpyService/onRequestIDRFrame` after stream start.
* Pros: keeps one encoder sequence; best decoder correctness if it works.
* Cons: existing real-device diagnostics found startup calls can close the active `onStart` stream, so this remains opt-in/diagnostic rather than default.

## Current Recommendation

The idea is reasonable only if framed as a visual placeholder. It is not safe to prepend a screenshot-derived IDR and then feed official P-frames as if they reference that IDR. The decoder picture buffer and SPS/PPS/profile/level may not match the official encoder sequence. For MVP, first prove the screenshot capture and placeholder UX, then add ffplay synthetic IDR as a guarded opt-in.

## Decision (ADR-lite)

**Context**: Static official `uitest` streams may not emit IDR on startup, leaving `ffplay` black until the screen changes. A screenshot-derived IDR can make `ffplay` show a placeholder immediately, but official non-IDR frames before the official IDR may not reference the synthetic frame correctly.

**Decision**: Implement Approach B as an opt-in experimental fallback. Include two modes:

* Safe mode: synthetic snapshot IDR is written to ffplay, then official pre-IDR non-IDR units continue to be withheld until the official IDR arrives.
* Unsafe passthrough mode: synthetic snapshot IDR is written to ffplay, then official pre-IDR non-IDR units are also written for observation.

**Consequences**: Unsafe passthrough can show corruption, stalls, decoder warnings, or seemingly work on some devices by accident. It must be a deliberate switch and must not update the documented safe contract unless real-device evidence proves it reliable.

## Implementation Notes

* Added host-side startup snapshot capture through `snapshot_display -f /data/local/tmp/hscrcpy_startup_snapshot.jpg` plus `hdc file recv`.
* Added `ffmpeg`-based one-frame H.264 encoder path using `libx264`, scaled/padded to the negotiated display dimensions.
* Added CLI flags:
  * `--uitest-startup-snapshot-idr`
  * `--uitest-startup-snapshot-passthrough-pre-idr`
  * `--ffmpeg-bin <path>`
* Safe mode writes the synthetic IDR to ffplay and keeps official pre-IDR units withheld until official IDR.
* Unsafe mode writes official pre-IDR units to ffplay after the synthetic IDR for device observation.
* Real-device validation on 2026-04-24:
  * Single synthetic IDR was parsed by ffplay but did not visibly present.
  * Repeating the synthetic IDR 120 times made ffplay show the placeholder before screen changes.
  * With `--uitest-startup-snapshot-passthrough-pre-idr`, later screen movement connected to the official stream with only a mild stall and no observed corruption on the tested device/so combination.
* Validation run:
  * `cargo check --workspace --offline`
  * `cargo test --workspace --offline -- --nocapture`
