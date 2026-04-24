# Quality Guidelines

> Quality expectations for backend and native code.

---

## Overview

This repo is early-stage, so discipline matters more than completeness. New backend code must improve structure as it adds behavior. Do not let scaffold code become permanent architecture.

---

## Forbidden Patterns

* Business logic inside ArkTS pages when it belongs in native core or host runtime
* Unchecked N-API calls in production bridge code
* Hidden global state for device sessions
* Introducing a database, service framework, or helper layer without documenting why it exists
* Copy-pasting protocol constants across ArkTS, native, and host layers

---

## Required Patterns

* Keep bridge functions thin and explicit
* Keep module boundaries aligned with runtime responsibilities
* Add comments only where control flow or OS constraints are non-obvious
* Prefer explicit capability negotiation over hard-coded assumptions
* Update specs when a new architectural pattern is introduced
* Keep experiment-only native paths explicitly isolated from production runtime targets
* When adding HarmonyOS module permissions, merge them into the existing `requestPermissions` array. Do not add a second same-named key; JSON/JSON5 parsers may keep only the later value, silently dropping required permissions such as `ohos.permission.INTERNET`.
* If a manifest fix changes packaged runtime behavior, bump the HAP `versionCode` and keep the host bundled companion manifest in sync so installed broken builds are upgraded instead of treated as ready.

---

## Testing Requirements

Current baseline:

* ArkTS unit and ability-test scaffolding exists and should be kept passing.
* No native or host test harness exists yet, so adding new subsystems should include the first relevant test baseline rather than deferring it forever.

Minimum expectation for new backend work:

* bridge code: argument and failure-path tests where practical
* protocol code: parse/serialize tests
* capability selection logic: deterministic unit tests

---

## Code Review Checklist

* Is logic in the correct layer?
* Are cross-layer contracts explicit?
* Are failures surfaced with enough context?
* Are names still template residue, or do they reflect real responsibilities?
* Did the change introduce reusable constants or helpers that should be centralized?
* Did manifest/config edits preserve existing keys and permissions instead of replacing them with duplicate object properties?
* If the fix requires reinstalling a companion, will host install/update planning actually detect a newer bundled version?

---

## Examples

* [`sources/hscrcpy_server/entry/src/test/LocalUnit.test.ets`](../../sources/hscrcpy_server/entry/src/test/LocalUnit.test.ets): test scaffolding already exists; do not bypass it just because the repo is new.
* [`sources/hscrcpy_server/entry/src/ohosTest/ets/test/Ability.test.ets`](../../sources/hscrcpy_server/entry/src/ohosTest/ets/test/Ability.test.ets): ability-level tests exist separately from local unit tests, which is the right split to preserve.
* [`sources/hscrcpy_server/entry/src/main/cpp/napi_init.cpp`](../../sources/hscrcpy_server/entry/src/main/cpp/napi_init.cpp): scaffold code is acceptable as a starting point, but unchecked return values make it unfit as the pattern for real bridge code.

---

## Common Mistakes

* Shipping template code unchanged.
* Growing the native bridge before defining the native core API.
* Delaying basic tests until after protocol complexity appears.
* Treating `uitest` extension experiments as production-ready before the device proves the load/trust path.
