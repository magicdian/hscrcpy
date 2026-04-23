# Fix Device Session Listener Network Permission

## Goal

Add the required HarmonyOS module permissions or related configuration so the companion can create and bind the MVP session listener socket on device startup.

## Requirements

* Stay within `sources/hscrcpy_server/**`.
* Keep scope narrow to manifest/config/permission fixes and the smallest related code adjustment if required.
* Use the current device startup logs as the trigger:
  * `Session listener bootstrap state=error port=27182 bound=false`
  * `failed to create session listener socket: Operation not permitted`
* If HarmonyOS requires reinstall or rebuild after the permission change, state that explicitly.

## Acceptance Criteria

* [ ] The companion module declares the required permission/config for creating the session listener socket.
* [ ] The fix is minimal and does not broaden unrelated runtime logic.
* [ ] Manual DevEco rebuild/reinstall steps are reported clearly.

## Write Scope

* `sources/hscrcpy_server/**` only
