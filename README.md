# Pocket Storage — a Horizon port

**Pocket Storage** is a Minecraft mod. This repository is a **port of it to [Horizon](https://github.com/horizon-rust/horizon)**, the Rust engine for Minecraft Java 26.3. It carries the complete source of the port plus the content files it ships, and installs as a single `.hmod` package.

Upstream: [Flanks255/pocketstorage](https://github.com/Flanks255/pocketstorage) — licence **MIT**.

| | |
|---|---|
| Upstream mod | Pocket Storage ([source](https://github.com/Flanks255/pocketstorage)) |
| Ported to | Horizon 0b14f8a, Minecraft Java 26.3, mod API `0.2` |
| Package | `pocketstorage.hmod` (install it with the Horizon launcher) |
| Licence | **MIT** — the upstream mod's licence; see [Licence and credit](#licence-and-credit) |

## What this port implements

- **`pocketstorage:psu_1`** (`data/pocketstorage/horizon/item/psu_1.json`, the
- **8 slots of 255** — upstream's `PSUTier.TIER1` (`slots = 8`,
- **The gestures**, upstream's own: **sneak + right-click** a container
- **The contents are the stack's** — the `pocketstorage:contents` component
- **Only simple items** go in: not another unit, no custom component, no damage
- **`/psu list|put <item> [n]|help`** — a documented extension standing in for

## Not yet ported

- **Tiers 2–4** (`psu_2` 16 × 4,095, `psu_3` 32 × 65,535, `psu_4` 64 ×
- **The upgrade recipe type** (`pocketstorage:upgrade`, a shaped recipe whose
- **The screen** (`PSUGui`/`PSUContainer`): no menu, no 8-slot container view;
- **The tooltip** (`appendHoverText`: the capacity line, the deposit/withdraw
- **Pickup absorption** (`ItemEntityPickupEvent.Pre` → into a matching slot,
- **The unit's UUID, `StorageManager` cache and `/psu open|list|recover`**: not
- **Sounds** (`SoundEvents.ITEM_PICKUP` on deposit/withdraw), the screen's
- **Config**: upstream ships none.

## Installing

1. Install the [Horizon launcher](https://github.com/horizon-rust/horizon/releases) (0.2.70 or newer).
2. Add `pocketstorage.hmod` to the instance's mods.
3. The package is pinned to a specific Horizon build; the launcher checks that pin.

## Licence and credit

Pocket Storage was created by its upstream authors; the code, models and assets in the original mod
remain theirs and are used here under **MIT**. The Rust in this repository is a clean-room
rewrite for Horizon, not a translation of the original source.

- `LICENSE` — the upstream licence text.
- `LICENSE-NOTICE.md` — what this port carries and where it came from.
- The upstream project: https://github.com/Flanks255/pocketstorage

## Technical details

```
horizon-mod.toml   package manifest (id, version, api, [native] pin)
Cargo.toml         the crate; depends on `horizon-mod-api` from the fetched SDK
src/               lib.rs (the one `impl Hooks`), runtime.rs (pure logic + tests), glue
assets/ data/      the content the package ships
tools/             content generators and the e2e scenario
```

- Build: `cargo horizon-mod build` (packages; it does not compile against the game).
- Test: `cargo test`, `cargo clippy --all-targets -- -D warnings`.
- In-game: `ports/tools/launcher-game.sh run pocketstorage` in the campaign tree, or install the
  `.hmod` and play; the harness scenario is `tools/e2e.sh`.

This repository is generated from the campaign's port at `ports/pocketstorage/`; the port's own
`notes.md` is the full record (decisions, traps, evidence).
