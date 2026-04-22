# Error Handling

> How errors should move across native, ArkTS, and host boundaries.

---

## Overview

This project crosses multiple runtime boundaries:

* desktop host to device companion
* ArkTS shell to native code
* native code to OS media or input APIs

Because of that, error handling must be explicit. Silent failure is worse than a visible failure during early development.

---

## Error Types

Current baseline:

* ArkTS lifecycle code logs structured failures with `hilog`.
* Native template code currently performs almost no real validation.

Required direction:

* Validate inputs at every boundary.
* Convert low-level failures into domain errors before crossing to the next layer.
* Preserve enough context to answer: what operation failed, on which device, during which phase.

---

## Error Handling Patterns

* Catch only when you can add context, clean up resources, or map the error into a boundary-safe form.
* For native bridge functions, check all N-API calls and argument types before using values.
* For lifecycle code, log the failure with stable tags and return early instead of continuing in a half-valid state.
* For device/session operations, prefer fail-fast startup over half-connected sessions.

---

## Boundary Rules

* ArkTS -> native: validate argument count and type before using values.
* Native -> OS APIs: wrap return codes and attach operation names.
* Host -> device companion: handshake failures must report version, capability, and transport phase.

---

## Examples

* [`sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets`](../../sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets): `onCreate()` catches `setColorMode()` failure, logs it, and avoids silent continuation.
* [`sources/hscrcpy_server/entry/src/main/cpp/napi_init.cpp`](../../sources/hscrcpy_server/entry/src/main/cpp/napi_init.cpp): the scaffold reads argument types but does not check N-API return values; treat this as a template anti-pattern to remove before real logic lands.
* [`sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets`](../../sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets): current UI click handling has no error branch, which is acceptable for template code but not for future session startup flows.

---

## Common Mistakes

* Logging an error and then continuing as if startup succeeded.
* Returning generic bridge failures with no operation name.
* Letting template code patterns survive into production paths.
