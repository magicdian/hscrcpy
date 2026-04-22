# brainstorm: HarmonyOS scrcpy-like tool

## Goal

Build an open-source, free, cross-platform HarmonyOS screen mirroring tool inspired by scrcpy, with a strong focus on low latency, smooth playback, and a Rust-first implementation strategy.

## What I already know

* The repository is currently a greenfield project with only a minimal `README.md`.
* The intended product is a cross-platform HarmonyOS device mirroring tool similar to scrcpy.
* Rust is the preferred implementation language.
* Candidate video transport or encoding paths mentioned so far are H.265, H.264, and JPEG.
* Target product direction is "the smoothest" open-source and free mirroring experience, with a stretch goal around 120 FPS.
* Comparable projects mentioned by the user:
  * `HOScrcpy`
  * `ohscrcpy`
  * `HoKit`
* Initial repo inspection of comparable tools shows a strong shared pattern:
  * desktop host app
  * HDC-based device connectivity
  * device-side screen capture or stream service
  * separate video transport path from control path
* The user has created a formal HarmonyOS Native C++ project at `sources/hscrcpy_server`.
* The generated project is a standard DevEco app template:
  * `entry` module with `UIAbility`
  * ArkTS page shell
  * N-API bridge in `entry/src/main/cpp`
  * shared library target `libentry.so`
* The user prefers the runtime model to be desktop-driven:
  * the desktop `hscrcpy` process should launch the device-side logic
  * the Native C++ framework is primarily to make HarmonyOS-native build output practical
  * any UI should be secondary and mainly used for permissions and feature configuration

## Assumptions (temporary)

* The first release should prioritize screen mirroring before broader device-control features.
* Cross-platform means at least macOS, Windows, and Linux desktop hosts.
* HarmonyOS device connectivity will likely rely on HDC or an equivalent official device bridge.
* Achieving 120 FPS depends more on device capture support, encoder availability, transport efficiency, and decode/render pipeline design than on host language choice alone.
* The first shipping architecture should separate:
  * device agent / capture
  * host transport / session control
  * host decode / render
  * optional input and audio services

## Open Questions

* What should the MVP include beyond screen mirroring: audio, input control, recording, file transfer, or only video first?
* Which codec strategy should be first-class in MVP given legal/licensing, device support, and latency trade-offs?
* Should the architecture optimize first for lowest latency, widest device compatibility, or easiest open-source distribution?
* For the first formal milestone, should the desktop host assume the device-side HAP or service shell is already installed, or should the desktop host be responsible for installing and updating it automatically?

## Requirements (evolving)

* Provide desktop-hosted mirroring for HarmonyOS devices.
* Keep the project open-source and free to use.
* Design for cross-platform host support.
* Prefer a Rust-centric codebase.
* Evaluate multiple streaming paths instead of hard-coding a single codec strategy.
* MVP follows the "core scrcpy route":
  * video mirroring
  * input control
  * automatic resolution / rotation adaptation
  * automatic downgrade between `H.264` and `JPEG`
* The desktop host must automatically install or update the device-side companion instead of requiring manual setup for normal use.
* First use may require a one-time authorization or configuration step on the HarmonyOS device, but subsequent launches should be as desktop-driven and hands-off as possible.

## Acceptance Criteria (evolving)

* [ ] A clear MVP scope is defined.
* [ ] A recommended transport and codec strategy is selected for MVP.
* [ ] A host/device architecture is defined at a high level.
* [ ] MVP interaction model is defined for both screen streaming and input injection.
* [ ] Device-side installation and startup ownership is defined.
* [ ] First-run authorization expectations are explicit.
* [ ] Non-goals for the first release are explicit.
* [ ] Implementation can proceed with concrete module boundaries and technical constraints.

## Definition of Done (team quality bar)

* Tests added or updated where implementation exists.
* Lint, formatting, and type checks pass.
* Docs are updated for user-visible behavior and architecture decisions.
* Risky protocol or codec choices are documented with trade-offs.

## Out of Scope (explicit)

* Commercial or paywalled codec features.
* Final implementation details for every subsystem before MVP scope is locked.
* Audio sync in the first milestone unless it is explicitly pulled into MVP later.
* File management, performance monitor, and UI tree tooling in the first milestone.

## Technical Notes

* Local repo state:
  * `README.md`: only contains project name and short description.
  * `sources/hscrcpy_server` exists and currently contains generated DevEco files.
* Initial external references provided by the user:
  * `https://gitcode.com/OpenHarmonyToolkitsPlaza/HOScrcpy`
  * `https://gitee.com/cleefun/ohscrcpy`
  * `https://github.com/yabi-zzh/HoKit`
* Comparable project observations:
  * `ohscrcpy` is a lightweight demo that pushes a `scrcpy_server`-style binary to device over HDC, forwards a TCP port, and sends JPEG frames to a desktop client.
  * `HOScrcpy` packages a Java SDK around HarmonyOS remote capture, exposes H.264 stream mode and image mode, and documents configurable bitrate, port, I-frame interval, and frame rate with a default of 120 FPS.
  * `HoKit` positions H.264 as the high-performance path and JPEG as compatibility fallback, and treats audio sync plus input tooling as add-on services.
* OpenHarmony media capability findings:
  * AVCodec capability APIs allow runtime querying of supported hardware/software codecs and limits per device.
  * OpenHarmony AVCodec documentation explicitly documents hardware encode/decode around H.264 and H.265.
  * OpenHarmony Media Kit includes `AVScreenCapture` for screen capture and can produce stream data in native code paths.
  * OpenHarmony documents temporal scalability features for video encoding, which is relevant for adaptive frame dropping under constrained transport or host decode conditions.
* Architecture implications:
  * JPEG is the easiest fully open fallback but is unlikely to be the best path for a "silky" experience at high FPS because bandwidth and CPU cost scale poorly.
  * H.264 is the practical default for MVP because ecosystem support and decode support are strongest across desktop hosts.
  * H.265 is valuable as an optional high-efficiency path, but should not be the only path due to broader licensing and compatibility concerns.
  * Runtime codec capability probing on the device is mandatory; codec choice should be negotiated per session instead of hard-coded.
* A Rust host can own session orchestration, transport, decode integration, rendering, and cross-platform packaging even if the device-side agent starts as Harmony native C/C++.
* Local project-structure implications:
  * The current template should be treated as a shell, not as the final architecture.
  * The native core should own capture, encode, transport, and control logic.
  * ArkTS should remain responsible only for lifecycle, permissions, and narrow host/native bridge duties unless HarmonyOS APIs force more logic into ArkTS.
  * The desired operational model is desktop-orchestrated rather than user-launch-first on the device.

## Research Notes

### What similar tools do

* `ohscrcpy` uses HDC to deploy a device-side executable, forwards a local TCP port, and streams JPEG frames to a native desktop client.
* `HOScrcpy` exposes both H.264 video stream mode and image-stream mode through a Java SDK, plus control injection and layout inspection.
* `HoKit` markets H.264 for low-latency high-FPS mirroring and keeps JPEG as fallback; audio sync is treated as a separate optional service.
* Upstream `scrcpy` itself now supports H.264, H.265, and AV1 for video, and OPUS, AAC, and RAW for audio, but that does not imply HarmonyOS device-side support for all of them.

### Constraints from platform and project

* This repo is greenfield, so we can choose a clean architecture.
* Cross-platform host support strongly favors Rust for the core session engine.
* HarmonyOS support must be grounded in HDC tooling and actual device codec capabilities.
* Open-source distribution should avoid making patented codecs the only viable path.
* 120 FPS is a performance target, not an assumption; it depends on capture, encode, transport, decode, render, and device support.
* The user selected the MVP scope closest to scrcpy core behavior rather than a video-only MVP.

### Feasible approaches here

**Approach A: Hybrid codec ladder** (Recommended)

* How it works:
  * Device agent negotiates codec support at runtime.
  * MVP ships with `H.264` as primary, `JPEG` as compatibility fallback, and reserves `H.265` as an opt-in experimental path.
* Pros:
  * Strongest chance of a usable MVP quickly.
  * Matches what current HarmonyOS mirroring tools appear to do.
  * Keeps a fully open fallback path.
* Cons:
  * Still depends on H.264 availability for best experience.
  * Requires maintaining two or three paths.

**Approach B: JPEG-first universal path**

* How it works:
  * Start with image streaming only, optimize transport and rendering later.
* Pros:
  * Lowest codec/legal complexity.
  * Simplest bootstrap path.
* Cons:
  * Poor fit for "smoothest possible" goal.
  * 120 FPS is unlikely to be realistic outside constrained scenarios.

**Approach C: H.26x-first performance path**

* How it works:
  * Prioritize H.264 and H.265, treat JPEG as debug-only or emergency fallback.
* Pros:
  * Best chance at low latency and high frame rate.
  * Cleaner rendering pipeline.
* Cons:
  * Weaker open-distribution story.
  * More exposure to codec support and licensing constraints.

## Decision (ADR-lite)

**Context**: The product aims to be the smoothest open-source and free HarmonyOS mirroring tool, while still being practical to build and distribute as a greenfield project.

**Decision**: Use a scrcpy-like host/device split for MVP, with `H.264` as the primary video path, `JPEG` as compatibility fallback, and `H.265` reserved as an experimental or later-stage path. MVP scope includes screen mirroring, input control, and automatic display adaptation. The desktop host is responsible for installing, updating, and starting the device-side companion.

**Consequences**:

* We need a device-side agent instead of a host-only architecture.
* We can keep the host mostly Rust-native and cross-platform.
* We should avoid overcommitting to a single device-agent packaging model before confirming HarmonyOS permission and runtime constraints for screen capture and input injection.
* The first formal device-side implementation will be based on the existing DevEco project rather than on a disposable proof-of-concept binary.
* The preferred user experience is that the desktop host orchestrates startup; device UI is optional and should exist mainly for permission granting and configuration, not as the primary control surface.
* The desktop host now also owns companion version management and installation workflow, which improves product ergonomics but adds packaging and upgrade logic to the host MVP.
* MVP assumes first-run authorization on device is acceptable if that unlocks a smoother steady-state desktop workflow afterward.
