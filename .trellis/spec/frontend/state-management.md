# State Management

> How UI state should be handled in the current HarmonyOS shell.

---

## Overview

The current repo uses only local UI state. There is no global store and no frontend server-state cache.

That is the right baseline for now because the shell should stay thin.

---

## State Categories

* Local UI state: allowed in page components through `@State`
* App lifecycle state: handled by `UIAbility`
* Native/runtime session state: belongs in native core or a dedicated bridge/service layer, not in page state
* Persisted config: not yet implemented; when added, keep it explicit and separate from ephemeral session state

---

## When to Use Global State

Do not introduce global frontend state until there are at least two real consumers that cannot be modeled by:

* page-local `@State`
* a dedicated service object
* native-owned session state

The first version of this product should bias toward fewer frontend state containers, not more.

---

## Server State

No frontend server-state library exists today.

When the desktop companion and device agent talk to each other, session/control state should live in the runtime/service layer rather than a UI cache.

---

## Examples

* [`sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets`](../../sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets): `message` is page-local `@State` and stays owned by the page.
* [`sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets`](../../sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets): lifecycle transitions are owned by the ability, not duplicated into page state.
* [`sources/hscrcpy_server/entry/src/main/cpp/napi_init.cpp`](../../sources/hscrcpy_server/entry/src/main/cpp/napi_init.cpp): native functions are called imperatively; this is a hint that runtime state should not be mirrored blindly into frontend component state.

---

## Common Mistakes

* Creating a global store before the shell has meaningful shared UI state.
* Mirroring transport/session internals into UI state objects.
* Persisting transient stream state as if it were user configuration.
