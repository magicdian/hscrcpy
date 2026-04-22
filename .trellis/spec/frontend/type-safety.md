# Type Safety

> Type rules for ArkTS and native bridge usage.

---

## Overview

This repo already has a typed ArkTS-to-native bridge surface, even though the functionality is still scaffold-level. Keep that typed boundary intact as the project grows.

---

## Type Organization

* Keep native bridge declaration files under `entry/src/main/cpp/types/`.
* Keep resource-backed values in resource files rather than retyping literal values throughout components.
* Keep component-local types close to the component until reuse becomes real.

---

## Validation

Type declarations are not enough at runtime.

Rules:

* validate inputs before crossing into native code
* validate native outputs before trusting them in UI code
* keep bridge APIs narrow and explicit so validation stays tractable

---

## Common Patterns

* Prefer generated or checked declaration files for native modules.
* Prefer typed imports over dynamic property access on bridge modules.
* Keep bridge methods small enough that each parameter has an obvious meaning.

---

## Forbidden Patterns

* untyped bridge access
* broad `any`-style escape hatches for native APIs
* type assertions used to suppress uncertainty instead of handling it
* duplicating the same native contract in multiple handwritten files

---

## Examples

* [`sources/hscrcpy_server/entry/src/main/cpp/types/libentry/Index.d.ts`](../../sources/hscrcpy_server/entry/src/main/cpp/types/libentry/Index.d.ts): the native bridge already has an explicit typed signature.
* [`sources/hscrcpy_server/entry/src/main/cpp/types/libentry/oh-package.json5`](../../sources/hscrcpy_server/entry/src/main/cpp/types/libentry/oh-package.json5): the type declaration package is explicitly wired into the native module metadata.
* [`sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets`](../../sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets): imports `libentry.so` as a typed module instead of reaching into an untyped global object.

---

## Common Mistakes

* Trusting declaration files without runtime validation.
* Expanding bridge APIs faster than their type model.
* Hiding native contract drift behind type assertions.
