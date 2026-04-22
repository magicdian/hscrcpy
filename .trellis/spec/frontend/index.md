# Frontend Development Guidelines

> Baseline rules for user-facing HarmonyOS shell code.

---

## Overview

Today, "frontend" in this repository means the ArkTS and resource layer inside `sources/hscrcpy_server/entry/src/main`.

This layer is intentionally thin:

* UIAbility lifecycle
* permission and configuration surfaces
* page composition
* the bridge into native code

It is not where streaming, transport, or device-control business logic should live.

---

## Current Scope

* Current UI is the default DevEco template page in [`sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets`](../../sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets).
* App lifecycle is handled in [`sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets`](../../sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets).
* Static resources and page registration live under `entry/src/main/resources/`.
* There are no custom hooks, no global store, and no network/server-state framework yet.

---

## Pre-Development Checklist

Read these before changing frontend code:

1. [Directory Structure](./directory-structure.md)
2. [Component Guidelines](./component-guidelines.md)
3. [State Management](./state-management.md)
4. [Type Safety](./type-safety.md)
5. [Quality Guidelines](./quality-guidelines.md)
6. [Hook Guidelines](./hook-guidelines.md) if introducing reusable stateful helpers
7. [Cross-Layer Thinking Guide](../guides/cross-layer-thinking-guide.md) when adding or changing native bridge calls

---

## Guidelines Index

| Guide | Description | Status |
|-------|-------------|--------|
| [Directory Structure](./directory-structure.md) | ArkTS and resource placement | Baseline defined |
| [Component Guidelines](./component-guidelines.md) | ArkTS page/component structure | Baseline defined |
| [Hook Guidelines](./hook-guidelines.md) | Current no-hook policy for Harmony shell | Baseline defined |
| [State Management](./state-management.md) | Local-only state strategy for current shell | Baseline defined |
| [Quality Guidelines](./quality-guidelines.md) | Review, lint, and test expectations | Baseline defined |
| [Type Safety](./type-safety.md) | ArkTS and native-bridge typing rules | Baseline defined |

---

**Language**: Write UI copy in the product language you choose later, but keep code comments, docs, and identifiers in English.
