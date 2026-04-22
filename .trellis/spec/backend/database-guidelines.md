# Database Guidelines

> Persistence rules for this project.

---

## Overview

There is currently no database in this repository.

That is the real project state today:

* no ORM
* no SQL driver
* no migration framework
* no schema directory

Do not invent a database layer casually. The initial mirroring product should prefer explicit config files, runtime memory, or platform-managed preferences until a real data model exists.

---

## Current Policy

* Device companion work should avoid introducing a database for session state.
* Desktop host work should avoid introducing a database for transient mirroring state.
* If persistence is needed before a database exists, prefer:
  * versioned config files
  * platform preference storage
  * explicit exported artifacts

---

## Query Patterns

No query conventions exist yet because no database exists.

If a database is introduced later, the change must define all of the following in the same task:

* storage engine
* migration tool
* schema location
* ownership boundaries
* backup and compatibility expectations

---

## Migrations

There are no migrations today.

If a migration system is introduced:

* keep migration files in-repo
* make migration ordering explicit
* document rollback behavior before landing the change
* update this guideline and the relevant package specs in the same task

---

## Naming Conventions

No schema naming convention exists yet because no schema exists.

If database storage becomes necessary, use explicit, boring names:

* snake_case table names
* snake_case column names
* descriptive index names

Do not add one-off embedded storage with undocumented keys.

---

## Examples

* [`sources/hscrcpy_server/oh-package.json5`](../../sources/hscrcpy_server/oh-package.json5): no database dependencies are declared at the workspace level.
* [`sources/hscrcpy_server/entry/oh-package.json5`](../../sources/hscrcpy_server/entry/oh-package.json5): the module depends only on the generated native bridge package, not on storage libraries.
* [`sources/hscrcpy_server/entry/src/main/resources/base/profile/main_pages.json`](../../sources/hscrcpy_server/entry/src/main/resources/base/profile/main_pages.json): current structured data in the repo is app/profile metadata, not application database state.

---

## Common Mistakes

* Introducing hidden persistence before the data model is understood.
* Using a database for cache-like runtime session state.
* Adding storage dependencies without documenting migration and compatibility strategy.
