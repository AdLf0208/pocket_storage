# pocketstorage (Horizon port)

A Horizon port of [pocketstorage](Flanks255/PocketStorage) — MIT.

**Status: first slices.** What is below is what the port actually does today.

## Implemented so far

- `pocketstorage:psu_1`** (`data/pocketstorage/horizon/item/psu_1.json`, the
- `minecraft:iron_ingot` template with `max_stack_size: 1`), one of the four the
- jar ships. Its model and texture are the jar's own.
- slots of 255** — upstream's `PSUTier.TIER1` (`slots = 8`,
- `capacity = 0xFF = 255`), repeated in its `README.MD` and in the jar's lang
- (`Holds %s different items, up to %s of each.`). Insert tops up the slot
- holding the same item, else takes the first empty slot, else refuses the
- stack; a deposit sends **all but one** of each kind (upstream's `giveItems`

## Not yet ported

- Tiers 2–4** (`psu_2` 16 × 4,095, `psu_3` 32 × 65,535, `psu_4` 64 ×
- ,048,575): only the tier-1 item is carried; the runtime already holds all
- four tiers' numbers, but the items, models/tabs and the upgrade path are not
- shipped.
- The upgrade recipe type** (`pocketstorage:upgrade`, a shaped recipe whose
- `D` is the previous unit, with its `ShapedRecipe.pattern` access transformer):
- the pin has no custom recipe type a port can register, so a tier-2..4 recipe
- cannot be authored yet. The three upgrade recipes are a named gap.

## Install

Build the bundle with a native-mod capable launcher, or add the file from the
registry once it is listed. Pinned to Horizon `1d63e0a` and horizon-mixins
`5a64af93` for game 26.3.

## Build from source

```sh
cargo test
python3 tools/horizon.py check --horizon <horizon checkout>
python3 tools/horizon.py build --horizon <horizon checkout>
python3 tools/package.py
```

## Credits and licence

Original mod: Flanks255/PocketStorage. Port: AdLf0208. Licence: MIT — see [LICENSE](LICENSE);
the original notices are preserved.
