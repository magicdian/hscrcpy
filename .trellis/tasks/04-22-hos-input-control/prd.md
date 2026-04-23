# Implement Input Control Channel

## Goal

Add the first scrcpy-like control channel so the desktop host can send input commands and the device side can inject them safely.

## Requirements

* Define control message types from the shared contract.
* Implement host-side command emission and device-side handling.
* Keep control logic separate from video transport code.
* Respect HarmonyOS-side constraints around input injection APIs.

## Acceptance Criteria

* [ ] A host-device control path exists in code structure.
* [ ] Control message ownership and parsing boundaries are explicit.
* [ ] Device-side injection points are isolated enough for later permission/error handling work.

## Dependencies

* Depends on shared architecture/contracts.
* Depends on device foundation and host foundation.

## Write Scope

* May touch both host and device implementation after foundations are in place.
