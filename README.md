# Pocket Storage — Horizon port (SDK package)

A native Horizon port of **Pocket Storage 1.2.5** (`Flanks255/PocketStorage`, **MIT**): a
storage unit that carries its own inventory in the item. This package ships all four tiers —
`pocketstorage:psu_1` to `psu_4`, 8×255 up to 64×1,048,575 — the jar's tier-1 recipe and its
three upgrade recipes, which carry the old unit's contents onto the next tier.

**LOCAL-ONLY** until the maintainer approves publishing (ARCHITECTURE.md §8). MIT is
licence rank 0 (permissive); approval is still required.

The behaviour reading is in `FINDINGS.md`; the ladder, deviations and gaps are in `notes.md`;
the content decisions are in `CONTENT.md`. Those files are repository documentation and are not
part of the built `.hmod`.

## In game

| Gesture / recipe | What happens |
|---|---|
| sneak + **right-click** a container | the unit empties into it, **all but one** of each kind (so the slot keeps the kind) |
| sneak + **left-click** a container | the unit takes back the kinds it already holds, up to each slot's room; the dig is cancelled |
| `/psu list` | the held unit's slots and counts |
| `/psu put <item> [n]` | move n (1) plain items from your inventory into the held unit — the screen's insert half, for this slice |
| `/psu help` | the usage text |
| **craft** iron + piston + redstone + chest | one tier-1 unit (the jar's own 3×3) |
| **craft** a unit in a tier's upgrade pattern | the next tier, holding everything the old unit held (gold → tier 2, diamond → tier 3, nether star → tier 4) |

Only simple items go in: not another unit, no custom components, no damage.

## Numbers (upstream's `PSUTier`)

| Tier | Slots | Per slot | Upgrade from the previous tier |
|---|---:|---:|---|
| 1 | 8 | 255 | crafted: 4 iron ingots, 2 pistons, 2 redstone, 1 chest |
| 2 | 16 | 4,095 | 4 gold ingots, 2 pistons, 2 chests around the tier-1 unit |
| 3 | 32 | 65,535 | 4 diamonds, 2 pistons, 2 chests around the tier-2 unit |
| 4 | 64 | 1,048,575 | 4 pistons, 2 chests, 2 nether stars around the tier-3 unit |

## Package

This is one flat SDK package (`horizon-mod.toml` + `Cargo.toml` + `src/` + `assets/` +
`data/`), built with the `cargo-horizon-mod` tool against the release pinned in
`horizon-mod.toml`:

```sh
cargo test --offline                                         # 29 unit tests
cargo horizon-mod build                                      # -> target/horizon-mod/pocketstorage.hmod
cargo horizon-mod check pocketstorage.hmod                   # the registry's own check
cargo horizon-mod fits pocketstorage.hmod --build <nightly>   # does it still fit another build
```

`src/runtime.rs` is the pure logic (the tiers, slot arithmetic, the deposit and withdraw
rules, the upgrade pattern, `/psu`); `src/lib.rs` is the one `impl Hooks` that reads the stack
component, registers the upgrade crafting kind and writes results back through the host.
