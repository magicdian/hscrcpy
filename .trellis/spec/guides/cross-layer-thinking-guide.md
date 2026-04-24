# Cross-Layer Thinking Guide

> **Purpose**: Think through data flow across layers before implementing.

---

## The Problem

**Most bugs happen at layer boundaries**, not within layers.

Common cross-layer bugs:
- API returns format A, frontend expects format B
- Database stores X, service transforms to Y, but loses data
- Multiple layers implement the same logic differently

---

## Before Implementing Cross-Layer Features

### Step 1: Map the Data Flow

Draw out how data moves:

```
Source → Transform → Store → Retrieve → Transform → Display
```

For each arrow, ask:
- What format is the data in?
- What could go wrong?
- Who is responsible for validation?

### Step 2: Identify Boundaries

| Boundary | Common Issues |
|----------|---------------|
| API ↔ Service | Type mismatches, missing fields |
| Service ↔ Database | Format conversions, null handling |
| Backend ↔ Frontend | Serialization, date formats |
| Component ↔ Component | Props shape changes |
| Manifest/config ↔ Runtime APIs | Duplicate keys, dropped permissions, build-time config that changes OS capability at runtime |
| CLI signal ↔ Host startup/render/runtime | Ctrl+C only sets a process-level interrupt flag, but startup, stream reads, renderer writes, or cleanup keep blocking until their own I/O returns |

### Step 3: Define Contracts

For each boundary:
- What is the exact input format?
- What is the exact output format?
- What errors can occur?

---

## Common Cross-Layer Mistakes

### Mistake 1: Implicit Format Assumptions

**Bad**: Assuming date format without checking

**Good**: Explicit format conversion at boundaries

### Mistake 2: Scattered Validation

**Bad**: Validating the same thing in multiple layers

**Good**: Validate once at the entry point

### Mistake 3: Leaky Abstractions

**Bad**: Component knows about database schema

**Good**: Each layer only knows its neighbors

### Mistake 4: Duplicate Config Keys Hide Runtime Permission Changes

**Bad**: Adding a second `requestPermissions` key to HarmonyOS `module.json5` when a new permission is needed.

**Good**: Merge new permissions into the existing array and add a static guard for required runtime permissions. Duplicate JSON/JSON5 object keys can make the later key override the earlier one, so a native socket failure can be caused by a manifest edit rather than transport code.

### Mistake 5: Treating Ctrl+C as a CLI-Only Concern

**Bad**: Installing a SIGINT handler in the CLI and checking it only between video frames or after route startup completes.

**Good**: Model shutdown as a cross-layer cancellation contract. Every long-running host phase must either accept the shared cancellation token directly or use a bounded wait that returns control quickly enough to observe it. This includes route startup, HDC operations, gRPC stream polling, TCP packet reads, renderer writes, child-process management, and device cleanup.

Do not make the contract depend on Unix-only primitives such as raw `select(2)` or `epoll(7)` at the application boundary. Prefer a portable Rust abstraction first: a cancellation token plus bounded blocking operations, a crossbeam channel, or a Tokio runtime for async I/O. Platform-specific polling can exist inside a transport adapter, but the session/runtime API must expose portable cancellation semantics that still work on Linux, macOS, and Windows.

---

## Checklist for Cross-Layer Features

Before implementation:
- [ ] Mapped the complete data flow
- [ ] Identified all layer boundaries
- [ ] Defined format at each boundary
- [ ] Decided where validation happens
- [ ] Checked manifest/config changes for duplicate keys and preserved existing runtime permissions
- [ ] Defined how cancellation moves through every long-running phase, including startup before the first frame exists

After implementation:
- [ ] Tested with edge cases (null, empty, invalid)
- [ ] Verified error handling at each boundary
- [ ] Checked data survives round-trip
- [ ] Verified Ctrl+C or equivalent shutdown exits promptly when the device sends no video data, sends only pre-IDR H.264 units, or blocks during route startup

---

## When to Create Flow Documentation

Create detailed flow docs when:
- Feature spans 3+ layers
- Multiple teams are involved
- Data format is complex
- Feature has caused bugs before
