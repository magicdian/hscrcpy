# Implement H264 Mainline and Fallback Negotiation

## Goal

Add `H.264` as the main streaming path and make runtime fallback behavior explicit so the project can pursue low latency without losing compatibility.

## Requirements

* Use runtime capability negotiation rather than hard-coded assumptions.
* Keep `H.264` as the preferred main path and `JPEG` as fallback.
* Define how fallback is selected, surfaced, and logged.
* Keep room for later `H.265` experimentation without blocking the MVP path.

## Acceptance Criteria

* [ ] Negotiation logic can prefer `H.264` when available.
* [ ] Fallback behavior toward `JPEG` is explicit in code and docs.
* [ ] Host and device assumptions around the mainline codec are aligned.

## Dependencies

* Depends on shared architecture/contracts.
* Depends on device foundation and host foundation.
* Depends on the JPEG baseline or at least a compatible fallback path.

## Write Scope

* May touch both host and device implementation after foundations are in place.
