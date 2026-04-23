# Implement Device Session Listener Runtime

## Goal

Start a real device-side session listener/runtime when the companion is launched so the desktop host can connect directly without requiring the user to manually drive the UI shell.

## Requirements

* Stay within `sources/hscrcpy_server/**`.
* Build on the existing native transport/session state machine rather than replacing the protocol model.
* Ensure the launch path actually starts a listener for the negotiated `session` channel port used by the host MVP.
* The first runtime target is minimal but real:
  * accept one host session connection
  * read `host_hello`
  * produce `device_hello` or `session_error`
  * remain compatible with the existing `session_config` / `session_ready` path
* Keep ArkTS/UI responsibility limited to lifecycle, permissions, and optional bootstrap hooks.

## Acceptance Criteria

* [ ] Launching the companion starts a real device-side session listener instead of only loading preview UI state.
* [ ] The runtime listener is able to accept the first host connection on the `session` channel.
* [ ] Existing H264/JPEG negotiation and control baseline remain intact.
* [ ] If DevEco IDE validation is needed, the exact manual steps are reported.

## Write Scope

* `sources/hscrcpy_server/**` only
