# Backend Development Guidelines

> Baseline rules for non-UI code in this repository.

---

## Overview

This project is still greenfield. Today, "backend" means the parts that are not user-facing UI:

* device-side native code under `sources/hscrcpy_server/entry/src/main/cpp`
* build and packaging metadata that controls the device companion
* future desktop host code (expected to be Rust)

These guidelines are intentionally grounded in the current repo, not an imagined final architecture.

---

## Current Scope

* Current implemented backend code spans:
  * device-side native/runtime code under `sources/hscrcpy_server/entry/src/main/cpp`
  * a Rust host workspace under `crates/**` and `apps/**`
* The native companion is built through [`sources/hscrcpy_server/entry/src/main/cpp/CMakeLists.txt`](../../sources/hscrcpy_server/entry/src/main/cpp/CMakeLists.txt) and packaged by the DevEco module config in [`sources/hscrcpy_server/entry/build-profile.json5`](../../sources/hscrcpy_server/entry/build-profile.json5).
* The primary cross-layer runtime contract is now the living doc at [`docs/architecture/host-device-mvp-contract.md`](../../docs/architecture/host-device-mvp-contract.md), not a future placeholder.

---

## Pre-Development Checklist

Read these before changing backend code:

1. [Directory Structure](./directory-structure.md)
2. [Error Handling](./error-handling.md)
3. [Logging Guidelines](./logging-guidelines.md)
4. [Quality Guidelines](./quality-guidelines.md)
5. [Host-Device MVP Contract](../../docs/architecture/host-device-mvp-contract.md) when touching companion startup, install/update, session negotiation, or channel boundaries
6. [Database Guidelines](./database-guidelines.md) if any persistence is being introduced
7. [Code Reuse Thinking Guide](../guides/code-reuse-thinking-guide.md) when adding helpers or shared constants
8. [Cross-Layer Thinking Guide](../guides/cross-layer-thinking-guide.md) when touching ArkTS <-> N-API <-> native boundaries

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | Module boundaries and file placement | Baseline defined |
| [Database Guidelines](./database-guidelines.md) | Current no-database policy and escalation rules | Baseline defined |
| [Error Handling](./error-handling.md) | Error propagation across ArkTS and native boundaries | Baseline defined |
| [Host-Device MVP Contract](../../docs/architecture/host-device-mvp-contract.md) | Install/update, startup handshake, runtime ports, wire framing, and channel contracts | Runtime bringup defined |
| [Quality Guidelines](./quality-guidelines.md) | Review and testing expectations for early-stage code | Baseline defined |
| [Logging Guidelines](./logging-guidelines.md) | Logging behavior and redaction rules | Baseline defined |

---

**Language**: Keep documentation and code comments in English unless an external API or upstream material requires another language.
