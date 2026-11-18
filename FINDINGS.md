# Pocket Storage — findings (jar and upstream source)

Ported from **Pocket Storage 1.2.5** (`pocketstorage-1.2.5+1.21.1-b3.jar`,
77 entries, kept at `ports/_shared/licence-cache/`), Minecraft Java 1.21.1 /
NeoForge 21.1.89. Analysis done 2026-11-11.

## Licence — read from the artefact

`META-INF/neoforge.mods.toml`:

```toml
modLoader="javafml"
loaderVersion="[4,)"
license="MIT"
[[mods]]
modId="pocketstorage"
version="1.2.5"
displayName="Pocket Storage"
authors="Flanks255"
```

**MIT, rank 0.** No `ASSETS_LICENSE*` file, no separate asset terms: the jar's
textures/models/lang carry the same MIT grant. Local-only until the maintainer
approves publishing (ARCHITECTURE.md §8).

## Jar inventory (`unzip -l`, 77 entries)

| Family | Entries | What it is |
|---|---|---|
| classes | 33 | `PocketStorageUnit` (the item), `PSUTier`, `PSUItemHandler`, `PSUData`, `PSUSlot`, `StorageManager`, `PSUContainer`/`PSUGui` (menu + screen), `PSUCommands`/`Open`/`List`/`Recover`, `UpgradeRecipe`, network packets, datagen |
| `assets/pocketstorage/models/item/psu_1..4.json` | 4 | one `minecraft:item/handheld` model each, all `pocketstorage:item/psu` |
| `assets/pocketstorage/textures/item/psu.png` | 1 | **one shared 16×16 item texture**, 301 B |
| `assets/pocketstorage/textures/gui/{psugui,oldpsugui,buttons}.png` | 3 | the menu screen's textures |
| `assets/pocketstorage/lang/en_us.json` | 1 | 4 item names, 4 × 3 info lines, 20 utility keys |
| `data/pocketstorage/recipe/tier1..4.json` | 4 | tier 1 is `minecraft:crafting_shaped`; tiers 2–4 are a **custom `pocketstorage:upgrade` type** (a shaped recipe whose `D` is the previous unit) |
| `pack.mcmeta`, `logo.png`, `META-INF/accesstransformer.cfg`, `.cache/*` | 7 | packaging; the AT exposes `ShapedRecipe.pattern` for the upgrade type |
| **blocks** | **0** | the mod adds **no blocks** — the storage unit is an item, the "inventory" is the stack |

So the families are: **4 items, 4 recipes (1 vanilla shaped + 3 custom), 1 lang
file, 1 item texture, 3 menu textures, no blocks, no loot tables, no tags.**
The brief's line ("77 entries, 4 item models `psu_1`…`psu_4`, 4 recipes, 1 lang
file, no blocks") matches the jar exactly.

The jar's lang (the numbers the tooltip shows):

```
"item.pocketstorage.psu_1.info": "Holds %s different items, up to %s of each.",
"item.pocketstorage.psu_1.info2": "Automatically absorbs like items on pickup, voids excess.",
"pocketstorage.util.deposit": "%s a chest to deposit contents.",
"pocketstorage.util.withdraw": "%s a chest to withdraw matching contents.",
```

## Upstream source (analysis aid, not a code source)

- **Source map row is wrong.** `ports/_shared/source-map/atm10-sources.csv` lists
  `Gerolmed/PocketStorage` for this mod. That repository is an unrelated Bukkit
  plugin by `endrealm` (`pocket-storage-{api,core,plugin}`); the jar's package is
  `com/flanks255/pocketstorage` and its `issueTrackerURL` is
  `Flanks255/simplylight`. The artefact wins.
- **Repo actually used:** <https://github.com/Flanks255/PocketStorage>,
  `master` = `7257f80fea887b83b74b1730996d170a546fb126`, `gradle.properties`
  `mod_version=1.2.5`, `minecraft_version=1.21.1`, `neo_version=21.1.89` — the
  exact version the pack ships. Clone deleted after these notes (gitignored
  scratch); the jar stays as evidence.
- **Files read:** `items/PSUTier.java`, `items/PocketStorageUnit.java`,
  `inventory/PSUItemHandler.java`, `inventory/PSUData.java`,
  `inventory/PSUSlot.java`, `inventory/StorageManager.java`, `README.MD`.
  Java was read for behaviour only; nothing was copied or translated.

### Behaviour extracted

**Tiers** (`PSUTier`): `TIER1(Tier 1, COMMON, 8 slots, 255)`, `TIER2(UNCOMMON,
16, 4095)`, `TIER3(RARE, 32, 65535)`, `TIER4(EPIC, 64, 1048575)`; `0xFF`,
`0xFFF`, `0xFFFF`, `0xFFFFF` are the same numbers. `README.MD` repeats them.
Each unit is `stacksTo(1)` with the tier's rarity.

**A unit's contents** (`PSUItemHandler`): a fixed list of `slots` entries, each
an item plus a count up to `capacity`; serialised as `Slots: [{Item, Count}…]`.
An empty slot serialises as `Item: ""`/`Count: 0`.

**Insert** (`insertItemSlotless`, the pickup/deposit path): find the **first
slot holding the same item and components** and set it to `min(count + n,
capacity)`, returning the overflow as the remainder; with no match, take the
**first empty slot** and put the whole stack there; if neither, the whole stack
is refused. `insertItem(slot, …)` is the capability path: an empty slot takes
the stack, a matching slot tops up to capacity, anything else is refused.

**Extract** (`extractItem`): `min(amount, min(slotCount, item.maxStackSize))`;
the slot is cleared when it runs out.

**What a unit accepts** (`isItemValid`): not an item with `custom_data`, not
another Pocket Storage Unit, and an empty component patch — the lang's "Only
simple items(no data) are allowed inside".

**Gestures** (`PocketStorageUnit`):

- plain **use** (right-click in air) → open the unit's menu (`PSUGui`);
- **sneak + right-click a block that has a block entity** → **deposit**: each
  filled slot sends `count − 1` to the clicked container through
  `ItemHandlerHelper.insertItemStacked`, keeping **one of each item behind** so
  the slot keeps its filter; the remainder stays in the unit;
- **sneak + left-click a block that has a block entity** → **withdraw**: for
  every container slot whose stack the unit **already holds** (`hasItem`), the
  whole stack moves in (`insertItemSlotless`), and the part that does not fit
  goes back into the same container slot;
- **pickup**: an item entity whose stack matches a slot the unit already holds
  is absorbed into that slot and the ground entity is cancelled; the overflow is
  **voided** ("voids excess") — this is why a unit never fills a *new* item type
  on its own;
- `appendHoverText`: shift shows `Holds <slots> different items, up to
  <capacity> of each.`, the deposit/withdraw hints, and (advanced) the unit's
  UUID.

**Storage registry** (`PSUData`/`StorageManager`): the unit carries its own
contents in its stack in 1.2.5; `StorageManager` keeps a per-unit handler cache
and a UUID for the `/psu recover|list|open` commands and for the upgrade
recipe. The 1.21.1 unit's contents are item data, not a world file.

## What this port keeps, and where it differs

Kept: the tier numbers, the one-of-each deposit rule, the withdraw-only-held
rule, "simple items only", `stacksTo(1)`, the carried texture/model/lang/recipe.

Different (all recorded in `notes.md` as gaps):

- No menu/screen (upstream's `PSUGui`): the slice's gestures are the two
  sneak-clicks, and `/psu put|list` stands in for the GUI's insert/read half so
  the slice is complete and testable headlessly.
- Contents live in a registered component `pocketstorage:contents` instead of
  upstream's `Slots` NBT inside `minecraft:custom_data` (both are item data; the
  component is the pin's supported shape and keeps the stack comparable).
- Pickup absorption (`item_picked_up` + voiding) is deferred, so a unit fills
  only through `/psu put` and the withdraw gesture in this slice.
- Only `psu_1` is carried; tiers 2–4 need the `pocketstorage:upgrade` recipe
  type, which the pin's recipe registry has no path for.
