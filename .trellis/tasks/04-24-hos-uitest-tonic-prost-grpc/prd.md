# UITest Tonic Prost gRPC

## Goal

Replace the handwritten `h2` + manual protobuf official scrcpy transport with a standard Rust gRPC implementation based on `tonic` and `prost`, while preserving the existing route-neutral host API and documenting the actual proto provenance.

## Requirements

* Add a concrete `scrcpy.proto` file to the repo for the observed official `ScrcpyService` schema.
* Generate Rust protobuf/gRPC bindings with `tonic-build` / `tonic-prost-build` or the current tonic-recommended build path.
* Use a `tonic` client for `onStart`, `onEnd`, and `onRequestIDRFrame`.
* Remove the handwritten protobuf encoder/decoder and direct `h2` gRPC framing from production code.
* Keep generated/protocol-specific types inside the official scrcpy adapter layer; CLI, renderer, and video ingress must stay route-neutral.
* Preserve current `--route uitest` behavior and diagnostics, including no automatic fallback to `hscrcpy-server`.
* Document proto provenance accurately: reconstructed from the generated descriptor in `xdevice_devicetest-6.1.0.210-py3-none-any.whl/devicetest/controllers/tools/recorder/proto/scrcpy_pb2.py`, not from public `.proto` source and not from reverse engineering the `.so`.
* Keep the existing real E2E validation boundary honest: automated tests can pass locally, but ffplay real-device success requires visible HDC target hardware.

## Acceptance Criteria

* [x] `crates/hscrcpy-host/proto/scrcpy.proto` exists and matches the descriptor fields used by the adapter.
* [x] Production `official_scrcpy` transport uses `tonic` generated client code, not handcrafted gRPC frame parsing.
* [x] `cargo fmt --check`, `cargo test --workspace -- --nocapture`, and `git diff --check` pass after dependencies are available.
* [x] If dependencies cannot be fetched in the sandbox, the task records the exact command the user must run, and code is not falsely marked fully verified.
* [x] Docs clearly state proto provenance and avoid claiming a public Huawei `.proto` source.
* [x] Existing HAP route remains unaffected.
* [x] Real-device manual validation command remains:

```bash
cargo run -p hscrcpy-host-cli -- \
  --device auto \
  --route uitest \
  --codec h264 \
  --ffplay-bin /opt/homebrew/bin/ffplay
```

## Technical Notes

Latest checked crate docs show `tonic` and `tonic-build` at `0.14.5`. Prefer the compatible `prost` / `tonic-prost-build` versions used by that tonic release.

The current environment can build the standard `tonic` / `prost` path offline from local Cargo cache after lockfile resolution. If a fresh environment lacks these dependencies, run `cargo fetch` before validation.

## Implementation Notes

* Added the repo-local package-less `crates/hscrcpy-host/proto/scrcpy.proto` and `crates/hscrcpy-host/build.rs` generation path using `tonic-prost-build`.
* Replaced production `official_scrcpy` transport with `tonic` generated client calls for `onStart`, `onEnd`, and `onRequestIDRFrame`.
* Removed production handwritten gRPC frame parsing and manual protobuf encode/decode helpers.
* Kept generated `ReplyMessage` / `ParamValue` conversion inside `crates/hscrcpy-host/src/official_scrcpy.rs`; session/render/CLI continue to use route-neutral video ingress.

## Verification Status

Validated in this workspace:

```bash
cargo check -p hscrcpy-host --offline
cargo fmt --all --check
cargo test --workspace --offline -- --nocapture
git diff --check
```

Fresh machines should run `cargo fetch` first. If `prost-build` cannot find `protoc`, install Protocol Buffers or set `PROTOC` to a local `protoc` executable, then rerun the same commands.
