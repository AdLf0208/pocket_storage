#!/usr/bin/env python3
"""Assemble the slice-1 `content/` folder from the pack's Pocket Storage jar.

The jar is the ATM10 file
(`ports/_shared/licence-cache/pocketstorage-1.2.5+1.21.1-b3.jar`), kept as the
licence and provenance evidence. Nothing is invented: the tier-1 model, the
shared item texture and the lang entries come out of the jar byte for byte and
are re-spelled only where 26.3 differs (a 1.21.1 `models/item` file gains an
`assets/<ns>/items/` definition, a shaped recipe's `{"tag": T}` becomes `"#T"`).
The item declaration, the creative tab and the three `c:` tags the recipe names
are written here, because a 1.21.1 jar has no `horizon/item` files and 26.3
ships no convention tags.

Rerun it to rebuild `content/` (it deletes its own generated folders first):

    python3 tools/assemble_content.py
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import zipfile

ROOT = Path(__file__).resolve().parents[1]
JAR = ROOT.parent / "_shared/licence-cache/pocketstorage-1.2.5+1.21.1-b3.jar"
CONTENT = ROOT / "content"
MOD = "pocketstorage"
# The jar's own SHA-256, so a different file cannot masquerade as the source.
JAR_SHA256 = "4c6e052210d74cf6a7b50ad556233d86e4af5e52f0c5f50b19d96d82c70c8303"

# Carried byte-for-byte: the jar's own assets (only what the slice ships).
CARRIED = [
    "assets/pocketstorage/models/item/psu_1.json",
    "assets/pocketstorage/textures/item/psu.png",
]

MANIFEST = """\
# Port-owned content mod for Pocket Storage 1.2.5 (slice 1: the tier-1 unit).
# The item model, its texture and the lang entries are the upstream mod's own
# assets (MIT; see ../LICENSE), carried from the pack's jar. Local-only until
# the maintainer approves publishing (ARCHITECTURE.md §8).
id = "pocketstorage"
name = "Pocket Storage"
version = "0.1.0"
description = "A storage unit that carries its own inventory in the item"
authors = ["Flanks255"]
api = "0.2"
side = "both"
capabilities = ["registries"]
"""

ITEM_DEFINITION = {
    "model": {"type": "minecraft:model", "model": "pocketstorage:item/psu_1"}
}

# One kind of item, a stack of one: upstream's `new Item.Properties()
# .stacksTo(1).rarity(TIER1.rarity = COMMON)`. The template only lends its
# components; the model comes from the jar's own `models/item/psu_1.json`.
ITEM_DECLARATION = {
    "template": "minecraft:iron_ingot",
    "max_stack_size": 1,
}

# `data/pocketstorage/recipe/tier1.json`, re-spelled for 26.3: `{"tag": T}` is
# `"#T"` and a tag ingredient is a string (see dyenamics/actuallyadditions).
TIER1_RECIPE = {
    "type": "minecraft:crafting_shaped",
    "category": "misc",
    "key": {
        "A": "#c:ingots/iron",
        "B": "minecraft:piston",
        "C": "#c:dusts/redstone",
        "D": "#c:chests",
    },
    "pattern": ["ABA", "CDC", "ABA"],
    "result": {"count": 1, "id": "pocketstorage:psu_1"},
    "show_notification": False,
}

# 26.3 ships no `c:` convention tags; the recipe names three, so the port
# supplies them (a missing tag is an empty ingredient and the recipe could
# never match).
CONVENTION_TAGS = {
    "c:ingots/iron": ["minecraft:iron_ingot"],
    "c:dusts/redstone": ["minecraft:redstone"],
    "c:chests": ["minecraft:chest", "minecraft:trapped_chest", "minecraft:barrel"],
}

CREATIVE_TAB = {
    "title": "Pocket Storage",
    "icon": "pocketstorage:psu_1",
    "items": ["pocketstorage:psu_1"],
}


def jar_bytes(path: str) -> bytes:
    with zipfile.ZipFile(JAR) as jar:
        return jar.read(path)


def lang() -> dict[str, str]:
    """The jar's lang, limited to the tier-1 unit and the lines it shows."""
    carried = json.loads(jar_bytes("assets/pocketstorage/lang/en_us.json"))
    keys = [
        "item.pocketstorage.psu_1",
        "item.pocketstorage.psu_1.info",
        "item.pocketstorage.psu_1.info2",
        "item.pocketstorage.psu_1.info3",
        "pocketstorage.util.count",
        "pocketstorage.util.deposit",
        "pocketstorage.util.sneak_left",
        "pocketstorage.util.sneak_right",
        "pocketstorage.util.withdraw",
    ]
    return {key: carried[key] for key in keys}


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def build() -> None:
    digest = hashlib.sha256(JAR.read_bytes()).hexdigest()
    if JAR_SHA256 and digest != JAR_SHA256:
        raise SystemExit(f"{JAR} is not the recorded jar (sha256 {digest})")
    for folder in ("assets", "data"):
        shutil.rmtree(CONTENT / folder, ignore_errors=True)
    CONTENT.mkdir(parents=True, exist_ok=True)
    (CONTENT / "horizon-mod.toml").write_text(MANIFEST)

    for path in CARRIED:
        target = CONTENT / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(jar_bytes(path))

    write_json(CONTENT / f"assets/{MOD}/items/psu_1.json", ITEM_DEFINITION)
    write_json(CONTENT / f"assets/{MOD}/lang/en_us.json", lang())
    write_json(CONTENT / f"data/{MOD}/horizon/item/psu_1.json", ITEM_DECLARATION)
    write_json(CONTENT / f"data/{MOD}/recipe/tier1.json", TIER1_RECIPE)
    write_json(CONTENT / f"data/{MOD}/horizon/creative_tab/psu.json", CREATIVE_TAB)
    for tag, values in CONVENTION_TAGS.items():
        namespace, name = tag.split(":", 1)
        write_json(CONTENT / "data" / namespace / "tags/item" / f"{name}.json", {"values": values})

    files = sorted(p.relative_to(CONTENT).as_posix() for p in CONTENT.rglob("*") if p.is_file())
    print(json.dumps({"jar": str(JAR), "jar_sha256": digest, "files": len(files), "path": str(CONTENT)}, indent=2))


if __name__ == "__main__":
    build()
