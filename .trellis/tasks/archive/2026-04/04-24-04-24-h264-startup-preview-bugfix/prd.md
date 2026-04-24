# Fix hscrcpy H.264 Startup Preview and Static Frame Handling

## Context

The `hscrcpy-server` route failed or appeared blank during startup in several ways:

- duplicate `requestPermissions` keys in the HarmonyOS module manifest dropped `ohos.permission.INTERNET`, causing native listener socket creation to fail;
- HAP H.264 output could reach host without visible ffplay startup because decoder config, IDR, ffplay startup flags, and static-screen repeat behavior were not observable enough;
- static screens depended on motion unless encoder repeat-previous-frame behavior was configured and verified;
- official `uitest` streams may begin with SPS/PPS only and not emit IDR until screen content changes.

## Scope

- Fix HAP manifest permission/version packaging so host reinstalls corrected companion builds.
- Preserve and prepend H.264 SPS/PPS decoder config before IDR when needed.
- Normalize HAP encoder timestamps to protocol microseconds.
- Configure and verify static-screen repeat output.
- Tune host ffplay startup parameters so the first decodable H.264 frame displays immediately.
- Add diagnostics that prove whether SPS/PPS/IDR were produced, forwarded, and handed to ffplay.
- Add opt-in `uitest` IDR request diagnostics without changing the default official route behavior.

## Acceptance

- `hscrcpy-server` starts without listener socket permission failure.
- First HAP H.264 frame includes SPS/PPS/IDR at host and displays immediately in ffplay.
- Static-screen HAP sessions continue to emit repeated H.264 units without requiring user screen interaction.
- Host timing summaries show fast ffplay writes and no startup dependency on a 120-unit diagnostic window.
- Official `uitest` H.264 path keeps waiting for a real official IDR, with optional `--uitest-request-idr-on-start` available for investigation.

## Verification

- `cargo fmt --check`
- `cargo check --workspace --offline`
- `cargo test --workspace --offline -- --nocapture`
- OpenHarmony native syntax check for `h264_screen_capture_source.cpp`
- `git diff --check`
- Human real-device validation on 2026-04-24:
  - `hscrcpy-server` route ffplay displayed immediately;
  - static-screen repeat produced full diagnostic windows;
  - `uitest` route became visibly faster after host ffplay tuning but still requires official IDR for static screens.
