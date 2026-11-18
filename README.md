# Pocket Storage — Horizon port (SDK package)

A native Horizon port of **Pocket Storage 1.2.5** (`Flanks255/PocketStorage`, **MIT**): a
storage unit that carries its own inventory in the item. This slice ships
`pocketstorage:psu_1` — 8 kinds of item, up to 255 of each — which fills from and empties
into a container by sneak-clicking it, and keeps its contents through a save, a drop and a
pickup.

**LOCAL-ONLY** until the maintainer approves publishing (ARCHITECTURE.md §8). MIT is
licence rank 0 (permissive); approval is still required.

The behaviour reading is in `FINDINGS.md`; the ladder, the migration record, deviations and
gaps are in `notes.md`; the content decisions are in `CONTENT.md`. Those files are repository
documentation and are not part of the built `.hmod`.

## In game

| Gesture | What happens |
|---|---|
| sneak + **right-click** a container | the unit empties into it, **all but one** of each kind (so the slot keeps the kind) |
| sneak + **left-click** a container | the unit takes back the kinds it already holds, up to each slot's room; the dig is cancelled |
| `/psu list` | the held unit's slots and counts |
| `/psu put <item> [n]` | move n (1) plain items from your inventory into the held unit — the screen's insert half, for this slice |
| `/psu help` | the usage text |

Only simple items go in: not another unit, no custom components, no damage.

## Numbers (upstream's `PSUTier`)

| Tier | Slots | Per slot | Shipped |
|---|---:|---:|---|
| 1 | 8 | 255 | **yes** |
| 2 | 16 | 4,095 | gap |
| 3 | 32 | 65,535 | gap |
| 4 | 64 | 1,048,575 | gap |

## Package

This is one flat SDK package (`horizon-mod.toml` + `Cargo.toml` + `src/` + `assets/` +
`data/`), built with the `cargo-horizon-mod` tool against the nightly pinned in
`horizon-mod.toml`:

```sh
cargo test --locked --offline                                 # 19 runtime tests + 1 glue test
cargo horizon-mod build                                       # -> target/horizon-mod/pocketstorage.hmod
cargo horizon-mod check pocketstorage.hmod                    # the registry's own check
cargo horizon-mod fits pocketstorage.hmod --build <nightly>    # does it still fit another build
```

`src/runtime.rs` is the pure logic (the tiers, slot arithmetic, the deposit and withdraw
rules, `/psu`); `src/lib.rs` is the one `impl Hooks` that reads the stack component and
writes the result back through the host.
