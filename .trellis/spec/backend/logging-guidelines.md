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
