# Pocket Storage — port notes (slice 1: the tier-1 storage unit)

Ported from **Pocket Storage 1.2.5** (`pocketstorage-1.2.5+1.21.1-b3.jar`,
SHA-256 `4c6e052210d74cf6a7b50ad556233d86e4af5e52f0c5f50b19d96d82c70c8303`,
77 entries, kept at `ports/_shared/licence-cache/`) for Minecraft Java 1.21.1 /
NeoForge 21.1.89. **MIT → licence rank 0**; local-only until the maintainer
approves publishing (ARCHITECTURE.md §8). Pin **`1d63e0a5`** (`game.rs`
`64813cf4…`), mixins **`5a64af93…`** (schema 6).

Status: **ported-partial** — slice 1 (the tier-1 unit's own inventory, the two
container gestures and the `/psu` command fallback) is implemented and
verified in-game. The gap list at the end is the rest of the mod.
**Format:** migrated to the flat SDK package (`.hmod`, `api = "0.2"`) on
2026-11-18 — see *Migration to the SDK format* at the end; the behaviour is
unchanged and `runtime.rs` is byte-identical.

**Why this mod now:** it is the rank-0 `content`/`machines` core mod
`precheck-mod.py pocketstorage` offers (jar licence MIT, rank 0, no unported
dependency, no worldgen). `ports/_shared/queue/brief-pocketstorage.md` is the
brief.

## Class + tier and the approach

- **Class:** `content`; **tier:** `machines` (`ports/ledger.csv`,
  `ports/_shared/source-map/atm10-sources.csv`, `horizon/docs/modpacks.md`).
- **Approach from the tier:** a **machines** shape — an item-carried inventory
  (the stack itself is the container) with the tiers' capacities and the
  container interop a machine needs. The pin has no item-container path that can
  hold 255–1,048,575 of one item in a slot (`minecraft:container` caps a slot at
  the item's own stack size, 64/99), so the contents ride a registered item
  component (`pocketstorage:contents`) and the port's own slot arithmetic — the
  same shape upstream uses (`Slots` NBT in the unit's item data).
- **Carry-over vs agent port:** the item model, the shared item texture, the
  lang entries and the tier-1 recipe are carried from the jar
  ([`CONTENT.md`](CONTENT.md), `tools/assemble_content.py`); the item
  declaration, the creative tab, the three `c:` tags, the component, the two
  click gestures and `/psu` are the agent port.

## What the slice does

- **`pocketstorage:psu_1`** (`data/pocketstorage/horizon/item/psu_1.json`, the
  `minecraft:iron_ingot` template with `max_stack_size: 1`), one of the four the
  jar ships. Its model and texture are the jar's own.
- **8 slots of 255** — upstream's `PSUTier.TIER1` (`slots = 8`,
  `capacity = 0xFF = 255`), repeated in its `README.MD` and in the jar's lang
  (`Holds %s different items, up to %s of each.`). Insert tops up the slot
  holding the same item, else takes the first empty slot, else refuses the
  stack; a deposit sends **all but one** of each kind (upstream's `giveItems`
  keeps the kind as the slot's filter); a withdraw only takes a container stack
  whose item the unit **already holds** (upstream's `takeItems`).
- **The gestures**, upstream's own: **sneak + right-click** a container
  deposits, **sneak + left-click** a container withdraws. Plain right-click is
  upstream's screen, which this slice does not carry (gap). The left click is
  cancelled so a withdraw never starts a dig or breaks the container.
- **The contents are the stack's** — the `pocketstorage:contents` component
  (one compound per filled slot, `{slot, id, count}`), so a saved, thrown or
  picked-up unit keeps everything, and an empty unit carries no component at all
  (it stacks like a new one).
- **Only simple items** go in: not another unit, no custom component, no damage
  (upstream's `isItemValid`/`isStandardItem`; the lang says "Only simple
  items(no data) are allowed").
- **`/psu list|put <item> [n]|help`** — a documented extension standing in for
  upstream's screen (`PSUGui`), the only way upstream inserts a *new* kind.
  `put` takes only what the unit can still hold, so nothing is ever dropped.
  Precedent: `akashictome`'s command fallback for its missing picker.

## Hook and helper map

| Hook (API) | Helper | What it does |
|---|---|---|
| `content_root` (raw target) | `embedded_content.rs` | writes the bundle's content half out and adds it to the dev-mod list (AE2 pattern) |
| `component_types` (65) | `component_types.rs` | registers `pocketstorage:contents` before content loads |
| `right_click_block` (21) | `right_click_block.rs` | sneak right click on a container → deposit |
| `left_click_block` (30) | `left_click_block.rs` | sneak left click on a container → withdraw, cancelling the dig |
| `register_commands` (8) | `register_commands.rs` | `/psu list`, `/psu put`, `/psu help` (level 0) |
| `perform_command` (8) | `perform_command.rs` | runs them with the player's state swapped in |
| `server_tick` (44) | `server_tick.rs` | resends the inventory after a held-stack write |

Helpers used: `native_component` / `native_set_component` /
`native_remove_component` (API 65), `native_container_slots` /
`native_extract_item` (API 48), `native_insert_item` (API 20),
`native_max_stack_size` (API 97), `native_acting_player` / `native_acting_id`
(API 21), `native_dimension` (API 48), `native_take_items` / `native_give_items`
(API 9), `native_action_bar` (API 21), `native_feedback` (API 8), `held` /
`held_mut` / `held_slot` / `hand_stack_replaced`, `hub.send_inventory`.
No unstamped hook or draft fragment is named.

## Verification (evidence in `ports/_shared/evidence/pocketstorage/`)

| Step | Log | Result |
|---|---|---|
| TDD red | `tdd-red.log` | `cargo test` **exit 101**, 55 errors: `Contents`, `Tier`, `tier_of`… do not exist yet |
| runtime tests | `evidence/pocketstorage/ladder/horizon-test.log` (`horizon.py test`) | **18 passed**, 0 failed, 1240 filtered out (embedded, `game::pocketstorage::tests`) |
| fmt + clippy | local run | `cargo fmt --check` clean; `cargo clippy --all-targets --locked --offline -- -D warnings` clean; 0 warnings in the staged `check` |
| staged check | `evidence/pocketstorage/ladder/horizon-check.log` | `cargo_exit: 0`, **0 warnings** |
| e2e, content as a dev mod | `e2e.log` | **PASS**, `EXIT:0` — all phases A–H |
| e2e, one file only (`EMBEDDED=1`, no `--dev-mod`) | `e2e-embedded.log` | **PASS**, `EXIT:0`; the bundle wrote its content (3,207 B, byte-identical to `content/pocketstorage.hmod`), and the control leg (same binary, `.horizon-cache` unusable) knows neither id |
| embed fail-capability | `e2e-selftest.log` | `SELFTEST=1` → **FAIL**, `EXIT:1` (the chest was empty as the withdraw promises, the flipped assertion demanded 254) |
| checker | `check-e2e-logs.py pocketstorage --strict` | exit 0, 3 logs |

The e2e's phases (quoted from `e2e.log`):

```
A  the unit item registers and is held
B  a new unit is empty: "pocketstorage:psu_1: 0 of 8 kinds, 0 items, 255 each at most"
C  "/psu put minecraft:diamond 300" → the unit holds 255, 45 stay in the inventory
D  a second kind takes the next slot: 2 kinds, 265 items
E  sneak right-click a chest → the chest holds 254 diamonds + 9 cobblestone,
   the unit keeps one of each
F  sneak left-click the chest → the unit holds 255 + 10 again, the chest is empty,
   the chest still exists (the dig was cancelled)
F2 a chest stack the unit does not hold (16 dirt) stays; a plain diamond deposits nothing
G  the dropped stack still holds {"pocketstorage:contents":[{count:255,…},{count:10,…}]},
   and after the pickup the unit holds 265 items again
H  after a relaunch ("/psu list") the unit still holds 2 kinds of 265 items
```

## Deliverables

| File | Size | SHA-256 | Entries |
|---|---:|---|---:|
| `content/pocketstorage.hmod` | 3,207 B | `36467b325cb4cee40f12a83943231bb3750ecc57087ff9b842e5615debd6b47a` | 11 |
| `pocketstorage.hnmod` (in the port root) | 16,885 B | `78f4226ae1078376d01e7d7d5366df449fd58d1c4bb447b82f43ce21b9ddb38e` | 12 |

The `.hnmod` carries the content half at
`mixins/pocketstorage/data/pocketstorage-content.hmod` (byte-identical to
`content/pocketstorage.hmod`), and `tools/package.py` refuses a stale copy.
Both are reproducible: `tools/package.py` run twice gave the same bytes
(fixed timestamps, sorted entries).

## Gaps (the rest of the mod)

- **Tiers 2–4** (`psu_2` 16 × 4,095, `psu_3` 32 × 65,535, `psu_4` 64 ×
  1,048,575): only the tier-1 item is carried; the runtime already holds all
  four tiers' numbers, but the items, models/tabs and the upgrade path are not
  shipped.
- **The upgrade recipe type** (`pocketstorage:upgrade`, a shaped recipe whose
  `D` is the previous unit, with its `ShapedRecipe.pattern` access transformer):
  the pin has no custom recipe type a port can register, so a tier-2..4 recipe
  cannot be authored yet. The three upgrade recipes are a named gap.
- **The screen** (`PSUGui`/`PSUContainer`): no menu, no 8-slot container view;
  `/psu put|list` is the substitute. The jar's three GUI textures are not
  carried.
- **The tooltip** (`appendHoverText`: the capacity line, the deposit/withdraw
  hints, the advanced-tooltip UUID): not carried, so the item shows only its
  name.
- **Pickup absorption** (`ItemEntityPickupEvent.Pre` → into a matching slot,
  excess **voided**): the pin's `item_picked_up` is a cancellable hook that
  could do it; not in this slice, so a unit fills only by `/psu put` and the
  withdraw gesture.
- **The unit's UUID, `StorageManager` cache and `/psu open|list|recover`**: not
  carried. They are upstream's bookkeeping for the screen and the upgrade
  recipe, both gaps above.
- **Sounds** (`SoundEvents.ITEM_PICKUP` on deposit/withdraw), the screen's
  scrolling/buttons, the block-placement feature the README lists as planned.
- **Config**: upstream ships none.
- **The tier-1 recipe and the creative tab are carried but not exercised in
  game**: a malformed recipe would refuse the content root (the item loads, so
  they parse), but the headless harness has no renderer for `ctl click-slot` on
  a crafting grid, and the tab needs a screen. Their correctness is therefore
  from the `.hmod` check and the content load, not from a craft in the e2e.

## Deviations (deliberate, all recorded above)

1. The contents live in `pocketstorage:contents` rather than in
   `minecraft:custom_data.Slots`; same durability, but the pin's supported
   component path (and it keeps the stack comparable for stacking).
2. `/psu put|list|help` is a documented extension for the missing screen; it
   only ever moves what fits, so nothing is dropped.
3. A withdraw cancels the dig (`left_click_block`'s cancel), so sneak-clicking a
   chest never breaks it. Upstream's left-click listener does not cancel the
   event (it acts on `LeftClickBlock`), so in creative upstream would break the
   container on a withdraw — the port does not.
4. The deposit targets a container through the **clicked face**, as upstream
   does (`CapHelper.getItemHandler(world, pos, face)`); a withdraw puts the part
   that does not fit back through the same face rather than into the exact slot
   upstream reinserts into.
5. `command` ties the readout to the port's own formatting, so the e2e also
   asserts on the stack's component (`tools/unit_state.py`) — the artefact, not
   just the message.

## Upstream source analysis

- Repo `https://github.com/Flanks255/PocketStorage`, `master` =
  `7257f80fea887b83b74b1730996d170a546fb126` (`gradle.properties`
  `mod_version=1.2.5`, `minecraft_version=1.21.1`, `neo_version=21.1.89` — the
  pack's exact version). The clone was deleted after the notes; the jar stays as
  the licence evidence.
- **The source-map row is wrong**: it names `Gerolmed/PocketStorage`, an
  unrelated Bukkit plugin by `endrealm`. The jar's packages
  (`com/flanks255/pocketstorage`) and its `issueTrackerURL` identify the real
  repository. Recorded in [`FINDINGS.md`](FINDINGS.md).
- Files read for behaviour only (no copy, no translation): `items/PSUTier.java`,
  `items/PocketStorageUnit.java`, `inventory/PSUItemHandler.java`,
  `inventory/PSUData.java`, `inventory/PSUSlot.java`,
  `inventory/StorageManager.java`, `README.MD`. The behaviour extracted is in
  `FINDINGS.md`.

## Commands run (the ladder)

```sh
# runtime (TDD red, then green), fmt and clippy — local slots
ports/tools/build-slot.sh run --job ps-test --port ports/pocketstorage --step test -- cargo test --locked --offline
ports/tools/build-slot.sh run --job ps-fmt  --port ports/pocketstorage --step fmt  -- bash -c 'cargo fmt --all -- --check && cargo clippy --all-targets --locked --offline -- -D warnings'
# staged game (forwards to the second machine)
ports/tools/build-slot.sh run --job ps-stage --port ports/pocketstorage --step stage -- python3 tools/horizon.py stage
ports/tools/build-slot.sh run --job ps-check --port ports/pocketstorage --step check -- python3 tools/horizon.py check
ports/tools/build-slot.sh run --job ps-test  --port ports/pocketstorage --step test  -- python3 tools/horizon.py test
# packaging (content .hmod + one .hnmod)
ports/tools/build-slot.sh run --job ps-pack --port ports/pocketstorage --step package -- env CARGO_INCREMENTAL=0 tools/package.sh
# e2e in one lease (rebuild + scenario)
ports/tools/build-slot.sh e2e --job ps-e2e --port ports/pocketstorage -- tools/e2e.sh
ports/tools/build-slot.sh e2e --job ps-embedded --port ports/pocketstorage -- env EMBEDDED=1 tools/e2e.sh
ports/tools/build-slot.sh e2e --job ps-selftest --port ports/pocketstorage -- env SELFTEST=1 tools/e2e.sh
```

## Review repairs (PR #663, 2026-11-11)

Review: `ports/_shared/horizon-mods/reviews/pr-663-pocketstorage.md` (CHANGES_REQUESTED, 5 items).

| # | Reviewer's item | This job |
|---|---|---|
| 1 | original link is relative `(Flanks255/PocketStorage)` | already absolute in `registry.toml` (orchestrator's earlier pass); re-checked |
| 2 | description truncated ("slots of 255", "upstream's giveItems", ",048,575"), repeated bullets, unmatched bold | **rewritten** from this file: complete tier/slot/capacity text, the deposit/withdraw rules, and the tier 2-4 + upgrade-recipe gaps |
| 3 | artifact not in the registry release/staging path | **orchestrator** — no write access (horizon-mods#686) |
| 4 | **[P1] a withdraw destroys item components** | **fixed** (see below) |
| 5 | **[P2] the distributed archives omit the MIT notice** | **fixed**: `tools/package.py` now writes `mixins/pocketstorage/LICENSE` into the `.hnmod`, and `tools/notice.py` writes `LICENSE` into the embedded `.hmod` |

### [P1] withdraw preserved the stack, not just its id

`support.rs::on_left_click_block` computed the withdraw amount from the item id and
count alone and rebuilt the stored stack from `{id, count}`, so withdrawing an
enchanted, renamed or damaged stack from a chest handed back a **plain** one and the
custom state was gone for good.

`runtime.rs` now has `Contents::withdrawable(item, count, damage, has_extra)`: a stack
that is more than its item id is refused whole and stays in the container, because a
saved slot is one `{id, count}` entry. The glue calls it, counts the refusals and says
so in the action bar. Tests: `a_withdraw_refuses_a_stack_that_is_more_than_its_item_id`
(red before the change: the guard did not exist) plus the existing withdraw tests.

### Packaging

`tools/package.py` adds `LICENSE` (and `LICENSE-NOTICE.md`/`COPYING` when present) at
`mixins/<id>/`, which the bundle reader accepts as a crate text file
(`horizon_mods::index::native::text`) and never compiles; `tools/notice.py` does the
same for the embedded content `.hmod`, whose reader ignores a root `LICENSE*`. Both
runs are byte-identical (two runs, same SHA-256) and `cargo horizon-mod check` passes
on the new bundle.

Artifact: `pocketstorage.hnmod` **19,357 B** `5d9b3502eb8da1b5b2e54274411b296b9831e4ecc4a83458a58d28e90d659373`
(content `pocketstorage.hmod` **4,088 B** `eb24ffe60e2c29534510746b02442afdd9cc65db6507b38e7befddda6b45cb49`).

## Migration to the SDK format (2026-11-18)

Horizon's mod-API merge (#16941, launcher ≥ 0.2.70) retired the `.hnmod` container and the
registry closed this port's submission (#663) with *"republish this as a native `.hmod` with
the SDK from the next Nightly"*. This job migrated the port to the flat SDK package at
nightly **`0b14f8acfd97fa2c750e42795249dcc981178ca4`**, mixins
**`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`** (launcher 0.2.78), and did **not** change the
mod's behaviour.

**The rule of the migration held: `src/runtime.rs` is the old `runtime.rs`, unchanged.**
`cmp` against the retired copy is byte-identical (SHA-256
`5c62f8183819f1ae2550d670b98defb88a6573d2a5addac63dd349769424071b`), and its **19 tests**
passed before any glue was written (`evidence/pocketstorage/sdk-test.log`; the crate now runs
**20**: those 19 plus one for the inventory-slot mapping the new glue needs).

### Files created, carried and deleted

| Action | Path | Note |
|---|---|---|
| created | `horizon-mod.toml` | the one manifest: id/name/version/licence/api/side/capabilities + `[native]` |
| rewritten | `Cargo.toml` | depends on `horizon-mod-api` from `.horizon/sdk/`; `publish = false` |
| created | `src/runtime.rs` | the old `mixins/pocketstorage/runtime.rs`, byte-identical |
| created | `src/lib.rs` | the one `#[mod_api::horizon_mod] impl Hooks` (was `support.rs` + 7 hook helpers) |
| carried | `assets/pocketstorage/**`, `data/**` | the old `content/assets` + `content/data`, byte-identical (`evidence/pocketstorage/content-carried.log`) |
| created | `LICENSE-NOTICE.md` | licence provenance, inside the package |
| rewritten | `README.md` | it ships inside the `.hmod`; the old one described the retired layout |
| created | `.gitignore` | `/target`, `/.horizon`, `/Cargo.lock` (the SDK is fetched, never packed) |
| deleted | `mixins/pocketstorage/` | `horizon-mixin.toml` + `support.rs` + 7 helpers + the embedded content `.hmod` |
| deleted | `content/` | the content half is now `assets/` + `data/` at the package root |
| deleted | `compatibility.json` | the pin is in `horizon-mod.toml`'s `[native]` |
| deleted | `rust-toolchain.toml` | **refused by the build** (`unexpected package path`) |
| deleted | `pocketstorage.hnmod` | superseded |
| retired | `_shared/evidence/pocketstorage/retired-hnmod-0.1.0/` | `mixins/`, `content/`, `compatibility.json`, the `.hnmod` and `rust-toolchain.toml` kept for the record, not for the package |

`tools/` is kept as the previous slice's harness record; `horizon.py`, `package.py/.sh`,
`assemble_content.py`, `notice.py` and `e2e.sh` drive the **retired** staged pipeline and are
not run by this slice (see the gap list: there is no launcher build path on this box).

### Manifest mapping

| Old (`mixins/pocketstorage/horizon-mixin.toml`) | New (`horizon-mod.toml`) |
|---|---|
| `[mod] id = "pocketstorage"` | `id = "pocketstorage"` (also the resource namespace) |
| `[mod] version = "0.1.0"` | `version = "0.2.0"` (a resubmission is a new version) |
| `[mod] priority = 100` | gone (listener order is `#[listener(priority = …)]`, unused here) |
| `[compatibility] horizon_revision = "1d63e0a5…"` | `[native] horizon = "0b14f8ac…"` (exact 40-hex) |
| — | `[native] mixins = "e59da909…"` (was implicit in the mixins checkout) |
| — | `[native] game = "26.3"` |
| `[[module]] runtime` (injected as `crate::game::pocketstorage`) | `src/runtime.rs` + `pub mod runtime` |
| `[[module]] state` (`support.rs`) | the same code as methods/handlers in `src/lib.rs` |
| `[[injection]]` × 6 + `content_root` target | `impl Hooks` methods (one per moment), no plan file |
| content inside the bundle at `data/pocketstorage-content.hmod` | `assets/` + `data/` in the package (nothing embedded) |
| `api = "0.2"` in the content manifest | the one `api = "0.2"` |

### Hook mapping (every old injection, and what it became)

| Old hook (API) | Old helper | SDK equivalent |
|---|---|---|
| `content_root` (raw target) | `embedded_content.rs` | **removed, not mapped**: one flat `.hmod` carries `assets/` and `data/`, so there is no second half to write out, no `native_data!` and no `content_root` cancellation |
| `component_types` (65) | `component_types.rs` | `Hooks::component_types(&RegistrationContext, &mut ComponentTypesEvent)` → `event.types.push(COMPONENT)` |
| `right_click_block` (21) | `right_click_block.rs` | `Hooks::right_click_block(HostSide, &mut RightClickBlockEvent) -> Flow`; the click's player, hand, stack, pos and face come from `event.click` (`BlockClick`), so no `native_acting_player`/`held`/`native_dimension`; the cancel is `Flow::Cancel` + `cancellation_result = Fail` instead of `*canceled = true` |
| `left_click_block` (30) | `left_click_block.rs` | `Hooks::left_click_block(&mut dyn ServerHost, &mut LeftClickBlockEvent) -> Flow`; the acting player is `host.players().acting_player()` |
| `register_commands` (8) | `register_commands.rs` | `Hooks::register_commands(&mut dyn ServerHost, &mut RegisterCommandsEvent)` → `Command::new(usage)` |
| `perform_command` (8) | `perform_command.rs` | `Hooks::run_mod_command(&mut dyn ServerHost, &mut RunModCommandEvent) -> Flow`; the line arrives **parsed** (`CommandInvocation`: literals + typed arguments) instead of raw text |
| `server_tick` (44) + `hub.send_inventory` | `server_tick.rs` | **removed, not mapped**: the hand is written through `LivingRef::set_hand_item`, the game's own equip write, which syncs the inventory itself |

Helper mapping inside the glue:

| Old (`support.rs`) | SDK call |
|---|---|
| `native_component` / `native_set_component` / `native_remove_component` | `ItemStack::get` / `set` / `remove` with a `Component` descriptor (`ContentsComponent`) |
| `native_container_slots` / `native_extract_item` / `native_insert_item` | `LevelRef::capability(host, &ITEMS, pos, Some(side))` → `Capability<dyn ResourceHandler<ItemResource>>`: `size`, `resource`, `amount`, `insert_any`, `insert`, `extract` |
| `native_max_stack_size` | not needed: the handler's `insert` enforces the slot's own limit |
| `native_acting_player` / `native_acting_id` | `host.player(uuid)` (right click), `host.players().acting_player()` (left click), `CommandSource::Player` (command) |
| `native_dimension` | `PlayerRef::level(host)` → `LevelRef` |
| `held` / `held_mut` / `held_slot` / `hand_stack_replaced` | `LivingRef::hand_item` / `set_hand_item` |
| `native_take_items` | `ItemPlace::PlayerSlot { player, slot }` `.extract(host, tx, …)` inside `host.transaction` |
| `native_give_items`, `native_action_bar`, `native_feedback` | `PlayerRef::tell` / `PlayerRef::action_bar` |
| `game.lan` / `hub.send_inventory` | gone (the game syncs a hand write) |

### Gaps after the migration

- **No in-game run of the SDK package.** There is no `horizon-launcher` on this box, so the
  flat package cannot be compiled into a game binary (`horizon-launcher native-build …`) or
  driven by the retired `tools/e2e.sh`; the previous slice's fail-capable e2e
  (`e2e.log`/`e2e-embedded.log`/`e2e-selftest.log`) is what proves the *behaviour*, and this
  job's evidence is the byte-identical runtime with its tests, the byte-identical content,
  `cargo horizon-mod check` and `fits`. The first item of the next slice is a
  launcher-built e2e of this package (and of the hand-write sync, which is now the game's).
- The rest of the behaviour gaps are unchanged from slice 1: tiers 2–4 items and the upgrade
  recipe type, the screen/menu, the tooltip, pickup absorption, the unit's UUID and
  `StorageManager`, sounds. See "Gaps (the rest of the mod)" above.
- A deposit into a container whose handler refuses a kind leaves that kind in the unit (the
  old `native_insert_item` remainder behaved the same); it is not reported per kind, only in
  the moved total.

### Traps this migration hit (keep for the next one)

1. **The build refuses anything outside the package schema**, naming the path:
   `error: unexpected package path: rust-toolchain.toml`, then
   `…: mixins/pocketstorage/runtime.rs`. The retired layout must be *deleted*, not kept
   beside the new one.
2. **`cargo test` needs `.horizon/sdk/`**: the `horizon-mod-api` path dependency does not
   exist until a `cargo horizon-mod` command fetches it; `.horizon/` is gitignored and never
   packed.
3. **`host.items()` does not exist.** Item and living reads are on `Host` itself
   (`host.item_key`, `host.living`, `host.player`, `host.max_stack_size`), while
   `players()`, `commands()` and `levels()` are services. The container helpers
   (`container_size`, `game_extract`, …) are **backend-only**: the public route is the
   capability (`ITEMS` for blocks, `ItemPlace` for places, `ENTITY_ITEMS` for an entity's
   inventory).
4. **`RightClickBlockEvent.click.stack` is the held stack** (a borrow), so a deposit does not
   need a `held()` read; `click.hand` is what the modified stack goes back to.
5. **`RunModCommandEvent.source` is `&CommandSource`** and `PlayerRef` is `Copy`:
   `let CommandSource::Player { player, .. } = *event.source`.
6. **The build packs root files by whitelist**: `horizon-mod.toml`, `Cargo.toml`,
   `Cargo.lock`, `LICENSE`, `LICENSE-NOTICE.md`, `README.md`, `icon.png`, `src/`,
   `assets/`, `data/`, `uses.json`. `notes.md`, `FINDINGS.md`, `CONTENT.md`, `PLAN.md`,
   `VALIDATION.md` and `tools/` are **not** packed — which is why `README.md` had to be
   rewritten (it ships) and `notes.md` did not.
7. **The slot runner runs from the campaign root**: wrap the port in
   `bash -c 'cd ports/pocketstorage && …'`, or it answers
   `could not find Cargo.toml in …/all_the_mods_10_rust_rewrite`.
8. **`build-slot.sh run` forwards to the second box** and fails with
   `cd: /home/deepseek/atm10/pocketstorage: No such file or directory` until the port is
   synced there (`remote-slot.sh`) — `BUILD_LOCAL=1` keeps a first migration local.
9. **A native mod crate has no `log`**: `log::warn!` does not compile. A refused write is
   reported in the player's own line (`… (the unit could not save that)`) instead of a log.
10. **`cargo fmt --all` runs before the check** that matters: an SDK crate formats `src/`
    only, so `cargo fmt --all -- --check` + `cargo clippy --all-targets -- -D warnings` are
    the two gates (both green: `sdk-fmt.log`, `sdk-clippy.log`).
11. **Two different inventory numberings.** `PlayerRef::inventory_slots` counts the armour as
    36..=39 and the off hand as 40 (`Inventory.EQUIPMENT_SLOT_MAPPING`), while
    `ItemPlace::PlayerSlot` wants 100..=103 for the armour and **`-106` for the off hand**.
    Only the main inventory (0..=35) has the same number in both. `/psu put` therefore maps
    0..=35 through and 40 to `-106`, and takes from nothing else (`take_place_of`, with a test
    of its own) — which is exactly the retired `native_take_items`' reach (the 36 main slots
    and the off hand, never the armour). Passing the numbers through unmapped would have
    addressed the armour slots as main-inventory slots 36..=39, which do not exist.

### Verification of this migration

| Step | Command | Log | Result |
|---|---|---|---|
| runtime, byte-identical | `cmp src/runtime.rs retired/mixins/pocketstorage/runtime.rs` | — | identical, SHA-256 `5c62f818…424071b` |
| runtime tests | `cargo test --locked --offline` | `sdk-test.log` | **20 passed**, 0 failed, `EXIT:0` (the 19 runtime tests + 1 for the glue's inventory-slot mapping) |
| the glue test fails when the mapping is wrong | `take_place_of(40)` flipped to `Some(40)`, re-run | `sdk-glue-test-red.log` | **FAILED**, `EXIT:101` (`left: Some(40)`, `right: Some(-106)`); restored, green again |
| formatting | `cargo fmt --all -- --check` | `sdk-fmt.log` | `EXIT:0` |
| lints | `cargo clippy --all-targets -- -D warnings` | `sdk-clippy.log` | `EXIT:0`, 0 warnings |
| build | `cargo horizon-mod build` | `sdk-build.log` | `Built ./target/horizon-mod/pocketstorage.hmod`, `EXIT:0` |
| registry check | `cargo horizon-mod check pocketstorage.hmod` | `sdk-check.log`, `sdk-check-json.log`, `sdk-check-uses.log` | `ok:true`, 28,494 B, `EXIT:0` |
| fits | `cargo horizon-mod fits pocketstorage.hmod` | `sdk-fits.log` | `0b14f8a  made for`, `EXIT:0` |
| content carried | byte-compare against the retired content `.hmod` | `content-carried.log` | every entry `same`, `ALL SAME` |
| package hash | `sha256sum pocketstorage.hmod` | `sdk-sha256.txt` | `5416872e41c7f62d9d4a8fa3fd6da5b482e6c9dc82821c4f03bfda85d034995a` |

**Deliverable:** `pocketstorage.hmod` **28,494 B**
`5416872e41c7f62d9d4a8fa3fd6da5b482e6c9dc82821c4f03bfda85d034995a`, 19 entries
(`horizon-mod.toml`, `Cargo.toml`, `Cargo.lock`, `LICENSE`, `LICENSE-NOTICE.md`,
`README.md`, `src/lib.rs`, `src/runtime.rs`, `uses.json`, 4 `assets/`, 6 `data/`), with the
MIT notice and the provenance note inside.

**Changelog for the resubmission** (verbatim): *"Supersedes #663, closed because Horizon
retired `.hnmod` (#17992). Changes: flat SDK package with `api = "0.2"` and `[native]` pinned
to `0b14f8a`; hooks rewritten as `impl mod_api::Hooks` methods against typed hosts/events
(`right_click_block`, `left_click_block`, `register_commands`, `run_mod_command`,
`component_types`) with the host's `ITEMS` capability and `ItemPlace` for the container and
inventory moves; the embedded-content `content_root` injection and the `server_tick`
inventory resync are gone (one package carries its own assets/data; a hand write is the
game's own equip write); `runtime.rs` and its 19 tests unchanged."*

## Skills

Skills: superpowers/using-superpowers,brainstorming,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices

