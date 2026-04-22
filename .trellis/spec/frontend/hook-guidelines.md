# Hook Guidelines

> Reusable stateful logic rules for frontend code.

---

## Overview

This HarmonyOS shell does not currently use React-style hooks, and ArkTS in this repo does not yet define any custom hook abstraction.

That absence is part of the current convention. Do not invent a pseudo-hook layer unless there is clear repeated stateful logic that benefits from it.

---

## Current Policy

* Use `UIAbility` lifecycle methods for app-level lifecycle behavior.
* Use `@State` for small page-local state.
* Use plain helper modules for stateless shared logic.
* Introduce a custom stateful abstraction only after the pattern repeats and the ownership boundary is clear.

---

## Data Fetching

There is no frontend data-fetching framework in the repo today.

For the current HarmonyOS shell:

* do not turn page files into polling or transport managers
* keep runtime/device communication behind a service or bridge layer

---

## Naming Conventions

If a reusable stateful helper appears later:

* name it after the behavior it owns
* colocate it with the layer that owns the state
* document why a helper is better than a plain function or a service object

---

## Examples

* [`sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets`](../../sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets): lifecycle logic is expressed through ability methods rather than a custom hook abstraction.
* [`sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets`](../../sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets): current page state is a simple `@State` property, which is enough for the existing UI.
* [`sources/hscrcpy_server/entry/src/mock/Libentry.mock.ets`](../../sources/hscrcpy_server/entry/src/mock/Libentry.mock.ets): mocks live separately from production UI code instead of being hidden behind ad hoc helper state.

---

## Common Mistakes

* Creating a hook-like abstraction before there are repeated consumers.
* Letting stateful helpers hide native bridge side effects.
* Mixing UI state and session-control state in the same helper.
