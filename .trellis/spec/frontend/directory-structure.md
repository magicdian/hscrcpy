# Directory Structure

> How the HarmonyOS shell is organized.

---

## Overview

The current frontend is a thin ArkTS shell inside the `entry` module of the DevEco project. That shell should remain small and explicit as the native core grows.

---

## Directory Layout

```text
sources/hscrcpy_server/entry/src/main/
├── cpp/
│   └── types/libentry/
├── ets/
│   ├── entryability/
│   │   └── EntryAbility.ets
│   ├── entrybackupability/
│   └── pages/
│       └── Index.ets
├── module.json5
└── resources/
    ├── base/element/
    ├── base/media/
    └── base/profile/
```

---

## Module Organization

* Put ability lifecycle code in `ets/entryability/`.
* Put visible pages in `ets/pages/`.
* Keep generated bridge typings under `cpp/types/`.
* Keep strings, numbers, icons, and page registration in `resources/`.
* Keep business logic out of page files unless it is directly about UI flow.

---

## Naming Conventions

* Abilities use HarmonyOS naming like `EntryAbility.ets`.
* Pages use clear page names rather than generic helper names.
* Resource keys stay descriptive and scoped by concern, for example `page_text_font_size`.
* Avoid embedding strings or sizing constants directly in component code when a resource file already exists.

---

## Examples

* [`sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets`](../../sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets): lifecycle code is isolated from pages.
* [`sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets`](../../sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets): page implementation lives under `pages/`.
* [`sources/hscrcpy_server/entry/src/main/resources/base/profile/main_pages.json`](../../sources/hscrcpy_server/entry/src/main/resources/base/profile/main_pages.json): page registration is resource-driven, not hard-coded in random files.

---

## Common Mistakes

* Growing `Index.ets` into a dumping ground for app logic.
* Duplicating resource values in code.
* Mixing permission, configuration, and streaming orchestration in the same page component.
