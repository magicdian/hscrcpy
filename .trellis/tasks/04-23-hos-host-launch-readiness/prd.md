# Implement Host Launch Readiness and UITest Launch Strategy

## Goal

Make host startup resilient and prepare it for the new `uitest`-loaded screen streaming route. The host should stop assuming that launch always means `aa start` of the installed `hscrcpy_server` HAP, should wait for the selected device-side endpoint to become ready, and should default future H.264 bringup toward the `uitest` route while keeping the existing `hscrcpy_server` route selectable by parameter.

## What I Already Know

* Current host startup uses `HdcCompanionManager` and launches the HAP shell with `aa start -b cn.magicdian.hscrcpy.server -a EntryAbility`.
* Current session/video transport assumes two fixed TCP endpoints:
  * session: `127.0.0.1:27182`
  * video: `127.0.0.1:27183`
* Current HDC forwarding supports only `tcp:<local> -> tcp:<remote>`.
* The existing host CLI has codec selection but no launch-route selection.
* `docs/harmony-hos-scrcpy-debug-guide.md` records the official route:
  * push a scrcpy server `.so`
  * run `uitest start-daemon singleness --extension-name <scrcpy so> ...`
  * wait for `xdevice_scrcpy` / `scrcpy_grpc_socket`
  * stream H.264 through gRPC over a Unix abstract socket.
* Official scrcpy server `.so` artifacts are archived under `third_party/hypium/hosScrcpy/6.1.0.210/libscrcpy`.
* Official xdevice recorder `.so` artifacts are archived under `third_party/hypium/xdevice-devicetest/6.1.0.210/recorder`.
* The `unix` scrcpy server variants correspond to `localabstract:*` socket mode.
* `docs/uitest-extension-poc.md` shows self-built `uitest` extension `.so` loading is currently blocked by XPM/signing or trusted extension checks, so "write our own `uitest --extension-name` payload" is not a safe mainline assumption yet.

## Requirements

* Stay outside `sources/hscrcpy_server/**` for this host-side task.
* Build on the existing host HDC runtime, companion launch abstraction, and session startup flow.
* Add an explicit host launch route concept.
* Default the host-facing screen streaming direction toward the `uitest` route.
* Keep the current installed `hscrcpy_server` HAP route selectable by CLI/config parameter.
* Add bounded readiness probing after launch instead of attempting exactly one immediate connect.
* Surface phase-specific failure context:
  * device discovery
  * payload deployment or HAP launch
  * HDC forward setup
  * endpoint readiness
  * session/gRPC connect
* Add HDC forwarding support for Unix abstract sockets for the `uitest` route.
* Implement the official scrcpy gRPC client path end-to-end for H.264 preview.
* Keep official gRPC/protobuf details behind a route adapter so the host render layer does not depend on the official wire protocol.
* Preserve a route-neutral video ingress abstraction so a future HAP path can feed the same renderer without rework.
* Use official `.so` artifacts from `third_party/hypium/**` by default for the `uitest` route.
* Keep a CLI/config override for selecting a specific official scrcpy server `.so` path when testing versions or ABIs.
* Fail fast when the selected route fails. Do not automatically fall back from `uitest` to `hscrcpy_server`; users must explicitly select `--route hscrcpy-server` for fallback/debug.
* Add startup-time codec capability matching:
  * detect route/device encoder support through an extensible codec descriptor model
  * detect host-side decoder/render support
  * choose the best mutually supported codec before starting the video stream
  * keep current end-to-end implementation focused on `H.264`
* Apply the same codec-selection abstraction to both `uitest` and `hscrcpy_server` routes.
* Do not introduce non-official third-party payloads into this repository.
* Do not assume self-built `uitest` extension payloads are loadable until the XPM/signing blocker is solved.

## Route Model

### Route A: `uitest` scrcpy route (preferred/default direction)

Host behavior:

* select or accept a scrcpy server `.so` path
* push it to `/data/local/tmp/scrcpy_server.so`
* run `uitest start-daemon singleness --extension-name scrcpy_server.so ...`
* forward host TCP to `localabstract:scrcpy_grpc_socket`
* probe readiness by checking socket/process/log evidence and/or attempting the client connection with bounded retry
* probe or infer official route codec support before opening the video stream
* map the official gRPC H.264 stream into the host render path

Known risk:

* The official route uses gRPC framing, not the current hscrcpy JSON session channel plus custom 14-byte video packet header. This task must adapt the official stream inside the `uitest` route adapter instead of leaking the official protocol into the renderer.

### Route B: `hscrcpy_server` HAP route (fallback/debug)

Host behavior:

* install/update the HAP companion
* launch `EntryAbility` with `aa start`
* forward host TCP to the current device TCP ports
* run the existing hscrcpy session negotiation
* use the same route-neutral codec selection result before starting the video channel

Known risk:

* This path may still require screen-capture authorization and does not provide the no-confirmation behavior observed from the official `uitest` route.

### Route C: self-built `uitest` extension route (not MVP)

Host behavior:

* would push and launch our own `UiTestExtension_OnInit` / `UiTestExtension_OnRun` payload

Known blocker:

* Current POC fails at `uitest` load time with XPM/signing-related permission denial, so this route stays out of scope for the launch-readiness MVP.

## Acceptance Criteria

* [ ] Host startup no longer attempts exactly one immediate session connect after launch.
* [ ] Retry/readiness behavior is bounded and surfaces clear failure context if the selected route never becomes ready.
* [ ] CLI/config can select the existing `hscrcpy_server` route even if the default route is `uitest`.
* [ ] The selected route is visible in host diagnostics/errors.
* [ ] `uitest` route implements the required socket target shape: `localabstract:scrcpy_grpc_socket`.
* [ ] `uitest` route can start the official scrcpy gRPC stream with `onStart`.
* [ ] `uitest` route can receive H.264 frame payloads from the official gRPC stream and feed the existing host preview/render path.
* [ ] `uitest` route can stop the official scrcpy server with `onEnd` during host shutdown.
* [ ] Official route protocol code is isolated behind an adapter; renderer and CLI do not import generated protobuf/gRPC types directly.
* [ ] HAP fallback route remains selectable without changing renderer-facing code.
* [ ] `uitest` route failure fails fast with route, phase, command/socket, and diagnostic context; it does not silently start the HAP route.
* [ ] Host startup computes a codec decision from device/route encode capability and host decode capability.
* [ ] Codec selection can represent an open set of codec names, including `h264`, `h265`, `h266`, `vp9`, `av1`, and `jpeg`, even though the first `uitest` playback milestone lands on H.264.
* [ ] The selected codec is visible in host diagnostics before the stream starts.
* [ ] The default `uitest` payload source resolves from `third_party/hypium/**` without requiring a user-provided path.
* [ ] Rust verification passes for the touched host modules.

## Dependencies

* Best applied after or alongside `04-23-hos-device-session-listener-runtime`
* Official-route stream ingestion depends on a Rust gRPC/protobuf mapping for `scrcpy.proto`.
* Self-built `uitest` payload development depends on resolving the XPM/signing/trusted-extension loading blocker documented in `docs/uitest-extension-poc.md`.

## Write Scope

* Host Rust/project files such as `crates/**`, `apps/**`, `docs/**` if needed
* Do not modify `sources/hscrcpy_server/**`

## Research Notes

### What similar tools do

* Huawei's official `hosScrcpy` route starts a scrcpy server `.so` through `/system/bin/uitest start-daemon singleness --extension-name ...`, creates a virtual screen, uses platform AVC encoding, and exposes `scrcpy_grpc_socket`.
* Official UITest/Hypium artifacts separate the H.264 scrcpy server path from the UITest agent path used for UI tree, JPEG capture, and input/control.
* The official route avoids the normal foreground `AVScreenCapture` user-confirmation model.

### Constraints from this repo

* The current host session protocol is hscrcpy-specific and not wire-compatible with official gRPC scrcpy server output.
* HDC forwarding currently needs to grow beyond TCP-to-TCP if it must connect to `localabstract:scrcpy_grpc_socket`.
* The existing CLI and orchestrator need launch-route selection before defaulting to `uitest`.
* Official `.so` artifacts already exist under `third_party/hypium/**`; route code should use that archive rather than requiring ad hoc local paths.
* Codec capability matching must be route-neutral so `uitest` and HAP startup do not diverge.

### Feasible approaches

**Approach A: Launch strategy + readiness first**

* How it works:
  * Introduce route selection, route-specific launch commands, HDC forward target modeling, bounded readiness, and diagnostics.
  * Keep actual official gRPC stream ingestion as a follow-up task.
* Pros:
  * Small enough to fit the current launch-readiness task.
  * Preserves the current working host render path.
  * Reduces risk by isolating launch mechanics from protocol adaptation.
* Cons:
  * Does not fully deliver `uitest` H.264 playback by itself.

**Approach B: End-to-end official scrcpy route in this task** (Selected)

* How it works:
  * Add route selection, payload push, `uitest` launch, abstract-socket forwarding, gRPC client, official protobuf mapping, and H.264 handoff to the renderer.
* Pros:
  * Directly targets the desired no-confirmation streaming path.
  * Produces a stronger user-visible milestone.
* Cons:
  * Much larger task.
  * Requires protocol work and likely generated gRPC/protobuf bindings.
  * More likely to conflict with the current hscrcpy session abstraction.

Selection notes:

* The project should pursue this approach now because the `uitest` path is intended to become the primary host streaming route.
* The implementation must avoid coupling upper layers to the official gRPC protocol, so future HAP streaming does not require renderer or CLI rewrites.

**Approach C: Replace device path with self-built `uitest` extension**

* How it works:
  * Continue building our own extension `.so` and try to load it with `uitest --extension-name`.
* Pros:
  * Best long-term open implementation if the loading blocker is solved.
* Cons:
  * Currently blocked by `uitest` load checks.
  * Not suitable as the default implementation path today.

## Expansion Sweep

Future evolution:

* The `uitest` agent route can later support UI tree analysis, JPEG capture, gestures, key injection, text input, and rotation handling.
* The launch abstraction should leave room for multiple device payloads: scrcpy H.264 server, UITest/Hypium agent, and existing HAP companion.

Related scenarios:

* HDC forwarding should consistently model both TCP ports and Unix abstract sockets so launch routes do not hard-code command strings.
* Diagnostics should include route, process/socket evidence, and exact HDC command phase to make device bringup failures actionable.

Failure and edge cases:

* `uitest` command succeeds but `scrcpy_grpc_socket` never appears.
* Device has no trusted/compatible scrcpy server `.so` for its OS/ABI.
* Forwarding `localabstract:scrcpy_grpc_socket` fails or collides with stale fport state.
* Payload starts but the gRPC service is not protocol-compatible with our host adapter.
* Official gRPC `ReplyMessage` stream does not expose H.264 bytes in the expected field; adapter should log message shape and fail with diagnostics rather than guessing.
* Host decode capability probe says H.264 is unavailable; selected `uitest` route should fail fast for this task rather than auto-switch route.
* Stale `xdevice_scrcpy`, stale fport entries, or stale `/data/local/tmp/scrcpy_server.so` may exist from a prior failed run.

## Decision (ADR-lite)

**Context**: The current HAP route can stream to the host, but the newly confirmed official `uitest` route can avoid user screen-capture confirmation and is likely the better foundation for future UI analysis.

**Decision**: Implement the end-to-end official `uitest` scrcpy route in this task, including launch, readiness, abstract-socket forwarding, gRPC start/stop, H.264 stream ingestion, and renderer handoff. Treat the `uitest` scrcpy route as the preferred/default host streaming route. Preserve the installed `hscrcpy_server` HAP route behind an explicit parameter for fallback and debugging. Use official `.so` artifacts from `third_party/hypium/**` by default. Do not treat self-built `uitest` extension loading as viable until the documented XPM/signing blocker is resolved.

**Consequences**:

* Host launch code needs route selection rather than a single `CompanionManager` assumption.
* Readiness checks must become route-specific.
* The official route requires abstract-socket forwarding and a gRPC stream adapter now.
* Existing hscrcpy session negotiation remains useful for the HAP fallback route but should not constrain the official route's wire protocol.
* Codec negotiation must become a route-neutral startup phase driven by device/route encode support and host decode support.
* Route fallback is explicit only; automatic fallback would hide primary-route failures and is out of scope for this task.

## Technical Approach

### Route-neutral host abstractions

The host should split the startup/render path into route-neutral interfaces before wiring official gRPC:

* `LaunchRoute` or equivalent selects `uitest` vs `hscrcpy_server`.
* `RouteLauncher` or equivalent owns route-specific deployment, command execution, forwarding, and readiness.
* `CodecCapabilityProbe` or equivalent combines route/device encoder support and host decoder support.
* `CodecSelection` or equivalent records the selected codec plus fallback reason.
* `VideoIngressSource` or equivalent yields normalized `PreparedVideoIngress` / encoded access units to the existing render path.
* `StreamSession` or equivalent owns route-specific stop/cleanup.

Rule:

* Generated protobuf/gRPC types may exist in a dedicated official-scrcpy adapter module only.
* CLI and render code should depend on route-neutral host types, not on official `ScrcpyService` types.
* HAP fallback must be able to implement the same route-neutral video ingress source later or continue through a compatibility adapter.
* Codec probing and selection must live above individual stream adapters so fallback is consistent across routes and can evolve beyond the current H.264 implementation.

### Codec negotiation direction

Startup should evaluate codec support before opening the video stream:

* Device/route side reports or infers encoder support:
  * `h264`
  * future examples: `h265`, `h266`, `vp9`, `av1`, `jpeg`
  * max width/height/fps/bitrate when known
* Host side reports decoder/render support:
  * `h264`
  * future examples: `h265`, `h266`, `vp9`, `av1`, `jpeg`
* Selection chooses the first codec allowed by user preference and supported by both sides.
* Current implementation should keep `h264` as the only required end-to-end stream target.
* The capability model must not hard-code the universe to H.264/H.265/JPEG; H.266/VVC, VP9, AV1, and other codecs should be addable without redesigning route startup.
* If live preview still depends on `ffplay`, the host capability probe may initially treat `ffplay` availability and codec support as the host decoder capability source.
* The selection result must be logged before launch/start:
  * route
  * selected codec
  * device-supported codecs
  * host-supported codecs
  * fallback reason when the first preference is not selected

### Official scrcpy gRPC contract observed locally

The local official sample exposes `scrcpy.proto` with:

* `service ScrcpyService`
* `rpc onStart(Empty) returns (stream ReplyMessage)`
* `rpc onEnd(Empty) returns (ReplyEndMessage)`
* `rpc onRequestIDRFrame(Empty) returns (ReplyEndMessage)`
* `ReplyMessage`
  * `data: string`
  * `reply_type: int32`
  * `payload: map<string, ParamValue>`
* `ParamValue`
  * `val_int`
  * `val_double`
  * `val_string`
  * `val_bool`
  * `val_bytes`
  * `val_float`
* `ReplyEndMessage`
  * `result: int32`

Expected adapter behavior:

* call `onStart` after the abstract socket forward is ready
* interpret each streamed `ReplyMessage` into normalized H.264 video ingress
* request IDR through `onRequestIDRFrame` when the host renderer/diagnostics needs it
* call `onEnd` during stop

### CLI direction

The host CLI should expose a launch route parameter:

* default: `--route uitest`
* fallback/debug: `--route hscrcpy-server`

The exact flag names may change during implementation, but route selection must remain explicit and visible in diagnostics.

### Payload source policy

The implementation should resolve the default `uitest` scrcpy server payload from the official archive under `third_party/hypium/hosScrcpy/6.1.0.210/libscrcpy`, preferring a `libscrcpy_server_unix_*.z.so` variant for `localabstract:scrcpy_grpc_socket` mode. A CLI/config override should remain available for testing another official version or ABI.

Selection rules:

* Prefer a `unix` scrcpy server variant when targeting `localabstract:scrcpy_grpc_socket`.
* Prefer the newest archived version compatible with the connected device/ABI when this can be determined.
* If ABI/version cannot be determined, use the current known-good archived `unix` variant and print the selected path.
* If no compatible official payload is found, fail before device launch with a payload-selection error.

### Lifecycle and cleanup

Startup should be idempotent across failed prior runs:

* clean or replace the target `/data/local/tmp/scrcpy_server.so` before launch
* remove or overwrite stale fport entries for the selected local port when possible
* detect stale `xdevice_scrcpy` / matching `uitest start-daemon` process before launch
* try to kill stale `xdevice_scrcpy` / matching `uitest start-daemon` process before launch
* if killing a stale process fails because shell permission is insufficient or the process remains alive, fail with route/phase/process diagnostics
* remove `/data/local/tmp/scrcpy_server.so` after the launch attempt, regardless of whether startup succeeds or fails
* treat device-side `.so` cleanup as best-effort after a successful launch because the process should already have loaded the library
* fail startup if `.so` cleanup fails after a failed launch attempt, because leaving the file behind consumes device storage and makes the next run ambiguous
* on normal shutdown, call `onEnd` and then release route resources

### Explicit out of scope

* Automatic fallback from `uitest` to HAP route.
* UI tree analysis and UITest agent RPC integration.
* JPEG continuous capture through the UITest agent.
* Input injection through the UITest agent.
* Audio streaming.
* End-to-end H.265, H.266/VVC, VP9, or AV1 playback.
* Multi-device, multi-display, or multiple simultaneous sessions on one device.

## Subtask Plan

1. `04-24-hos-route-abstraction-cli`
   * Introduce route-neutral host abstractions and explicit CLI route selection.
   * Keep existing HAP route selectable.
2. `04-24-hos-uitest-hdc-payload-lifecycle`
   * Add abstract-socket fport support, official payload selection, stale process handling, and temporary `.so` cleanup.
3. `04-24-hos-codec-capability-registry`
   * Add extensible codec descriptors and route-neutral selection logic while keeping H.264 as the current implementation target.
4. `04-24-hos-official-scrcpy-grpc-adapter`
   * Add isolated official ScrcpyService client and message conversion seam.
5. `04-24-hos-uitest-h264-render-ingress`
   * Feed official H.264 stream data into the existing render/preview path.
6. `04-24-hos-launch-diagnostics-e2e-validation`
   * Add fail-fast diagnostics, cleanup validation, and end-to-end bringup checks.
