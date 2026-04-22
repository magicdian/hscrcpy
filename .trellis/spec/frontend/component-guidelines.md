# Component Guidelines

> How ArkTS components and pages should be written in this repo.

---

## Overview

The current frontend is small, but the intended direction is clear: pages and components are a shell around native capabilities, not the primary home of product logic.

---

## Component Structure

* Use `@Entry` only for true entry pages.
* Keep component-local state near the UI that owns it.
* Call native functionality through a narrow, typed bridge.
* Extract reusable UI pieces before a page becomes a mixed layout/control file.

---

## Props Conventions

Current repo state:

* There are no reusable ArkTS component props yet.

When reusable components appear:

* keep props explicit and typed
* prefer small, descriptive inputs over large mutable objects
* do not pass native handles or opaque session blobs through the visual tree

---

## Styling Patterns

* Prefer resource-backed values for common strings, colors, and sizes.
* Keep layout code readable and close to the component that uses it.
* Avoid magic numbers unless they are one-off layout values and clearly local.

---

## Accessibility

This repo does not yet contain meaningful accessibility behavior, but that should not be interpreted as permission to ignore it.

Baseline rule:

* if a page becomes more than a debug screen, give interactive elements meaningful labels and deterministic structure

---

## Examples

* [`sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets`](../../sources/hscrcpy_server/entry/src/main/ets/pages/Index.ets): current page uses `@State` for local UI state and references a resource-backed font size.
* [`sources/hscrcpy_server/entry/src/main/resources/base/element/float.json`](../../sources/hscrcpy_server/entry/src/main/resources/base/element/float.json): visual constants already have a resource home instead of being hard-coded everywhere.
* [`sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets`](../../sources/hscrcpy_server/entry/src/main/ets/entryability/EntryAbility.ets): lifecycle behavior stays in the ability, not inside the page component.

---

## Common Mistakes

* Letting pages directly own session orchestration.
* Packing permissions, transport state, and layout into one component.
* Treating template code as a finished component pattern.
