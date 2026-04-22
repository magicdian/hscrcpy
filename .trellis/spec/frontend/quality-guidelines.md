# Quality Guidelines

> Quality expectations for ArkTS shell code.

---

## Overview

The frontend in this repo is not the product core, but it is the first layer users and permissions flow will touch. Keep it disciplined, typed, and small.

---

## Forbidden Patterns

* Embedding transport or codec logic in pages
* Bypassing the typed native bridge
* Copying resource values directly into components
* Growing template screens into a permanent control center
* Ignoring lint or test scaffolding because the UI is "just a shell"

---

## Required Patterns

* Keep lifecycle work in `UIAbility`
* Keep page state local unless a stronger boundary is needed
* Use resources for stable strings and constants
* Keep permission/configuration UX explicit
* Keep user-facing shell code readable enough that native integration points are obvious

---

## Testing Requirements

Current repo baseline already includes:

* local unit test scaffolding
* `ohosTest` ability-test scaffolding

Expectation:

* keep scaffolding valid
* add tests for bridge-facing UI behavior once real behavior exists
* add permission/configuration flow tests when those screens are introduced

---

## Code Review Checklist

* Is this UI code still a shell, or is it absorbing runtime logic?
* Are bridge calls typed and narrowly scoped?
* Are strings and constants placed in resources where appropriate?
* Does the change preserve a clean split between lifecycle, page UI, and native core?
* Were tests or mocks updated if the UI contract changed?

---

## Examples

* [`sources/hscrcpy_server/code-linter.json5`](../../sources/hscrcpy_server/code-linter.json5): ArkTS linting is already enabled with performance and TypeScript-oriented rules.
* [`sources/hscrcpy_server/entry/src/test/LocalUnit.test.ets`](../../sources/hscrcpy_server/entry/src/test/LocalUnit.test.ets): frontend unit-test scaffolding exists and should not be abandoned.
* [`sources/hscrcpy_server/entry/src/mock/Libentry.mock.ets`](../../sources/hscrcpy_server/entry/src/mock/Libentry.mock.ets): mocks belong in dedicated mock files rather than ad hoc page hacks.

---

## Common Mistakes

* Treating shell code as exempt from architecture discipline.
* Overusing mock-only code paths in production components.
* Forgetting that permission UX is part of product quality.
