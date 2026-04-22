# Directory Structure

> How non-UI runtime code is organized in this project.

---

## Overview

This repository currently contains one concrete runtime package: the HarmonyOS companion app under `sources/hscrcpy_server`. Within that package, backend code means the native implementation and the build metadata that shapes how the companion is produced.

The project direction already decided in product planning is:

* desktop host owns orchestration
* device companion is a thin HAP shell
* native core owns capture, encode, transport, and control

That direction should be reflected in file placement as code is added.

---

## Directory Layout

```text
sources/hscrcpy_server/
├── AppScope/
│   └── app.json5
├── build-profile.json5
├── hvigorfile.ts
├── entry/
│   ├── build-profile.json5
│   ├── hvigorfile.ts
│   ├── oh-package.json5
│   └── src/
│       ├── main/
│       │   ├── cpp/
│       │   │   ├── CMakeLists.txt
│       │   │   ├── napi_init.cpp
│       │   │   └── types/libentry/Index.d.ts
│       │   ├── ets/
│       │   ├── module.json5
│       │   └── resources/
│       ├── test/
│       └── ohosTest/
└── oh-package.json5
```

---

## Module Organization

Current baseline:

* Put device native code in `entry/src/main/cpp/`.
* Keep ArkTS UI and lifecycle code in `entry/src/main/ets/`.
* Keep generated or bridge-facing TypeScript declarations in `entry/src/main/cpp/types/`.
* Keep packaging and build concerns in `build-profile.json5`, `module.json5`, `AppScope/app.json5`, and hvigor files.

Direction for new code:

* Do not mix streaming logic into ArkTS pages.
* Split native code by responsibility as soon as the template is replaced, for example:
  * `capture/`
  * `codec/`
  * `transport/`
  * `control/`
  * `bridge/`
* Keep the N-API boundary thin. The bridge should call core services, not contain business logic.

---

## Naming Conventions

* C++ source files use lowercase snake_case where possible, following the current `napi_init.cpp`.
* CMake entry files stay named `CMakeLists.txt`.
* ArkTS files follow HarmonyOS conventions with `EntryAbility.ets`, page names, and module metadata names kept explicit.
* Avoid placeholder names like `entry` for long-lived domain modules once code is no longer scaffold-only.

---

## Examples

* [`sources/hscrcpy_server/entry/src/main/cpp/CMakeLists.txt`](../../sources/hscrcpy_server/entry/src/main/cpp/CMakeLists.txt): native build entry point belongs under the module's `cpp` directory.
* [`sources/hscrcpy_server/entry/src/main/cpp/napi_init.cpp`](../../sources/hscrcpy_server/entry/src/main/cpp/napi_init.cpp): the current native bridge entry point lives beside the native build file.
* [`sources/hscrcpy_server/entry/build-profile.json5`](../../sources/hscrcpy_server/entry/build-profile.json5): native ABI and CMake configuration live in module build metadata, not in ad hoc scripts.

---

## Common Mistakes

* Putting device-agent runtime logic in ArkTS page files because the template starts there.
* Treating generated names like `entry` as stable architecture instead of template residue.
* Hiding cross-layer behavior in build scripts instead of explicit runtime modules.
