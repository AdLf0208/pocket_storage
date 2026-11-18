# Pocket Storage — slice 1 (`psu_1`) implementation plan

> **For agentic workers:** this is the plan of record for the slice the brief
> `ports/_shared/queue/brief-pocketstorage.md` asks for. Steps are checkboxes;
> the runtime is written test-first (red then green).

**Goal:** one `pocketstorage:psu_1` item that carries its own inventory with the
jar's exact capacity (8 slots × 255), fills from / empties into a container by
sneak-click, keeps its contents through a save and a drop/pickup, and refuses
more when full.

**Architecture:** a content `.hmod` inside the one `.hnmod` (AE2 pattern) for
the item, its texture/model/lang/recipe/tab, and a native module for behaviour:
the contents live in a registered item component (`pocketstorage:contents`), so
the stack itself is the container and nothing needs a block entity. `runtime.rs`
is the pure slot arithmetic (upstream's `PSUItemHandler` rules) with no game
types; `support.rs` bridges to the game (`native_set_component`, the block
container helpers, the acting player) and one helper file per injection.

**Tech Stack:** Rust 1.95 (pin `1d63e0a5`, mixins `5a64af93`), horizon-mixins
schema 6, `cargo-horizon-mod` for the `.hmod`, `horizon-ctl` for the in-game
oracle.

**Spec:** `ports/_shared/queue/brief-pocketstorage.md` +
`FINDINGS.md` (the behaviour extracted from the jar and the upstream repo).

## Global constraints

- Every number comes from the artefact: `8/255, 16/4095, 32/65535, 64/1048575`
  (`PSUTier.java`, `README.MD`, and the jar's lang `Holds %s … up to %s of each`).
- Stamped hooks only: `component_types`, `right_click_block`, `left_click_block`,
  `perform_command`, `register_commands`, `server_tick`, plus the raw
  `content_root` target the AE2 pattern uses. Nothing else.
- One deliverable: `ports/pocketstorage/pocketstorage.hnmod` (never `target/`),
  carrying `mixins/pocketstorage/data/pocketstorage-content.hmod`.
- No bare `cargo`: every build/test/e2e through
  `ports/tools/build-slot.sh …` with `CARGO_INCREMENTAL=0`, no `--target-dir`.
- Licence MIT read from the jar (`META-INF/neoforge.mods.toml`), rank 0; the
  port stays local-only until the maintainer approves publishing (ARCH §8).

## File structure

| File | Responsibility |
|---|---|
| `mixins/pocketstorage/runtime.rs` | pure tier table, `Contents` slot list, insert/extract/deposit/withdraw arithmetic; the crate lib and the injected `crate::game::pocketstorage` |
| `mixins/pocketstorage/support.rs` | glue: component read/write, container interop, held-stack writes and the client sync, commands |
| `mixins/pocketstorage/component_types.rs` | `component_types` HEAD: register `pocketstorage:contents` |
| `mixins/pocketstorage/right_click_block.rs` | `right_click_block` HEAD: sneak + right-click a container deposits |
| `mixins/pocketstorage/left_click_block.rs` | `left_click_block` HEAD: sneak + left-click a container withdraws |
| `mixins/pocketstorage/register_commands.rs` | `register_commands` HEAD: `/psu list`, `/psu put`, `/psu help` |
| `mixins/pocketstorage/perform_command.rs` | `perform_command` HEAD: the command fallback for the missing GUI |
| `mixins/pocketstorage/server_tick.rs` | `server_tick` HEAD: resend the inventory after a held-stack write |
| `mixins/pocketstorage/embedded_content.rs` | raw `content_root` HEAD: write the embedded `.hmod`, append it to `dev` |
| `content/` | the item declaration, model, texture, lang, tier-1 recipe, `c:` tags, creative tab |
| `tools/assemble_content.py` | rebuild `content/` from the jar (carry + the 26.3 re-spellings) |
| `tools/e2e.sh` | the in-game scenario and its control / selftest legs |

## Task 1 — the pure runtime (TDD)

**Files:** create `mixins/pocketstorage/runtime.rs`, `Cargo.toml`,
`rust-toolchain.toml`.

**Interfaces (produced):**
`Tier { item: &'static str, slots: usize, capacity: u32 }`, `TIER1..TIER4`,
`TIERS`, `tier_of(&str) -> Option<Tier>`, `Entry { item: String, count: u32 }`,
`Contents::{new, from_entries, slots, get, is_empty, total, insert, has,
matching_slot, extract, deposit_plan, withdraw_amount}`.

- [x] Step 1: write the failing tests (the numbers and rules above).
- [x] Step 2: `cargo test` in the port → red (types missing).
- [x] Step 3: implement the smallest code that satisfies them.
- [x] Step 4: `cargo test` → green; `cargo fmt`, `cargo clippy -D warnings`.

## Task 2 — the glue and the plan

**Files:** create `support.rs`, the six helper files, `horizon-mixin.toml`,
`compatibility.json`.

**Interfaces (consumes):** `runtime::*`; the game's `native_component`,
`native_set_component`, `native_dimension`, `native_container_slots`,
`native_extract_item`, `native_insert_item`, `native_acting_player`,
`native_take_items`, `native_give_items`, `native_feedback`, `held`/`held_mut`.

- [x] Step 1: `component_types` + `support::register_component_types`.
- [x] Step 2: `right_click_block` deposit (all but one per slot, upstream's rule).
- [x] Step 3: `left_click_block` withdraw (only items the unit already holds).
- [x] Step 4: `/psu list|put|help` + the `server_tick` inventory resend.
- [x] Step 5: `embedded_content`.

## Task 3 — the content half

- [x] Step 1: `tools/assemble_content.py` writes `content/` from the jar.
- [x] Step 2: `cargo horizon-mod build` + `check` inside `content/`.
- [x] Step 3: copy the built `.hmod` into `mixins/pocketstorage/data/`.

## Task 4 — ladder and evidence

- [x] Step 1: `horizon.py stage → check → test → build` in one lease each.
- [x] Step 2: `tools/package.sh` → `.hnmod` in the port root, `check --kind native-mod`.
- [x] Step 3: `tools/e2e.sh` default (dev-mod), `EMBEDDED=1` (no `--dev-mod`,
      with the unusable-`.horizon-cache` control) and `SELFTEST=1` (must exit 1).
- [x] Step 4: `check-e2e-logs.py pocketstorage --strict`, `check-skills.py --notes`,
      `upstream_ready.py`.
- [x] Step 5: `notes.md` with the class/tier, licence, boundary, gaps, `Skills:`.
