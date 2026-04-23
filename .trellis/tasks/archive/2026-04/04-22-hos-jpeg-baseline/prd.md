# Implement JPEG Streaming Baseline

## Goal

Create the first end-to-end video path using JPEG so the project has a bring-up path and a long-term compatibility fallback.

## Requirements

* Use the host/device foundations and shared contracts as the base.
* Establish a minimal session that can move image frames from device to host.
* Keep the implementation aligned with the long-term fallback role of JPEG.
* Avoid over-optimizing before the basic path works reliably.

## Acceptance Criteria

* [ ] Device side can produce JPEG frame output through the agreed session path.
* [ ] Host side can receive and process the JPEG baseline path.
* [ ] The path is documented as baseline/fallback, not the final performance solution.

## Dependencies

* Depends on shared architecture/contracts.
* Depends on device foundation and host foundation.

## Write Scope

* May touch both host and device implementation after foundations are in place.
