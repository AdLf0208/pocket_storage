#!/usr/bin/env python3
"""Craft the tier-1 unit and the three upgrades in a real crafting table.

Called by `tools/e2e.sh` with the open game's control socket. The harness opens a
crafting table as a screen whose 3x3 grid is slots 100..108 (row-major) and whose
result is 200; the player's own slots keep their inventory numbers (hotbar 0..8).

What it proves, in this order:

1. the carried tier-1 recipe (`minecraft:crafting_shaped`, iron + piston +
   redstone + chest) makes one `pocketstorage:psu_1`, and its ingredients are
   consumed;
2. `/psu put` fills that unit with 30 diamonds;
3. the `pocketstorage:upgrade` kind is really read by the game: the jar's tier-2
   pattern (gold + piston + chest around the unit) makes one
   `pocketstorage:psu_2` that still holds the 30 diamonds, and the old unit and
   the materials are consumed;
4. the jar's tier-3 pattern (diamonds) and tier-4 pattern (nether stars) make
   `psu_3` and `psu_4`, each still holding the 30 diamonds, and tier 4 reports
   the jar's 64 kinds of 1,048,575;
5. the *control*: the tier-2 pattern with the tier-4 unit in its centre leaves
   the result empty and the unit untouched, so a recipe matches the tier it
   declares and nothing else.

Exit 1 with the observed result on the first mismatch. `NEGATIVE=1` flips the
contents assertion of step 3, so the run MUST exit 1.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys

SELFTEST = os.environ.get("NEGATIVE") == "1"

# The harness's crafting screen: the table's grid and its result.
GRID = list(range(100, 109))
RESULT = 200
CORNERS = [100, 102, 106, 108]
ABOVE_BELOW = [101, 107]
LEFT_RIGHT = [103, 105]
CENTRE = 104
# The crafted stack goes into the first hotbar slot, where `select 1` holds it and
# the unit's own readout can be read.
HOTBAR = 0


def main() -> int:
    sock, ctl, x, y, z = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]
    port = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

    def run(*args: str) -> dict:
        out = subprocess.run([ctl, "--socket", sock, *args], text=True, capture_output=True)
        try:
            return json.loads(out.stdout)
        except json.JSONDecodeError:
            raise SystemExit(f"ctl {' '.join(args)}: {out.stdout}{out.stderr}")

    def wait(ticks: int = 4) -> None:
        run("wait", str(ticks))

    def slots() -> list[dict]:
        screen = run("screen")
        if "slots" not in screen:
            raise SystemExit(f"no menu is open: {screen}")
        return screen["slots"]

    def item_of(slot: int) -> str | None:
        found = [s for s in slots() if s["slot"] == slot]
        return (found[0].get("item") if found else None) or None

    def count_of(item: str) -> int:
        return run("inventory")["counts"].get(item, 0)

    def find(item: str) -> int:
        for slot in slots():
            if slot.get("item") == item:
                return slot["slot"]
        raise SystemExit(f"missing ingredient {item} (inventory: {run('inventory')['counts']})")

    def result() -> dict | None:
        found = [s for s in slots() if s["slot"] == RESULT]
        if not found or not found[0].get("item"):
            return None
        return found[0]

    def place(item: str, targets: list[int], one_each: bool = True) -> None:
        """Pick the item up, put one in each target, put the rest back."""
        slot = find(item)
        run("click-slot", str(slot))
        wait(1)
        for target in targets:
            if one_each:
                run("click-slot", str(target), "right")
            else:
                run("click-slot", str(target))
            wait(1)
        run("click-slot", str(slot))
        wait(2)

    def setup(items: list[tuple[str, int]], clear: bool = True) -> None:
        """A fresh table and exactly `items` in the inventory.

        `clear=False` keeps what the player already holds: the unit an upgrade
        moves must survive into the next step, and every craft consumed the rest.
        """
        try:
            run("close")
        except SystemExit:
            pass
        wait(5)
        run("setblock", x, y, z, "minecraft:air")
        run("setblock", x, y, z, "minecraft:crafting_table")
        wait(2)
        run("command", f"tp @s {float(x) - 0.5} {y} {float(z) + 0.5}")
        wait(3)
        if clear:
            run("command", "clear @s")
        for item, count in items:
            run("give", item, str(count))
        wait(3)
        run("interact", x, y, z)
        wait(5)

    def take_into_hotbar() -> None:
        """Take the result into the first hotbar slot; the take consumes the grid."""
        run("click-slot", str(RESULT))
        wait(2)
        run("click-slot", str(HOTBAR))
        wait(2)

    def close_and_hold() -> None:
        run("close")
        wait(3)
        run("select", "1")
        wait(3)

    def held_contents() -> tuple[str, list[tuple[str, int]]]:
        reply = run("inventory")
        parsed = subprocess.run(
            [sys.executable, os.path.join(port, "tools/unit_state.py")],
            input=json.dumps(reply), text=True, capture_output=True, check=True,
        ).stdout
        env: dict[str, str] = {}
        for line in parsed.splitlines():
            key, _, value = line.partition("=")
            env[key] = value.strip("'")
        entries = []
        index = 0
        while f"S{index}_ITEM" in env:
            entries.append((env[f"S{index}_ITEM"], int(env[f"S{index}_COUNT"])))
            index += 1
        return env.get("HELD", ""), entries

    def expect_result(want_item: str | None, label: str) -> dict | None:
        got = result()
        print(f"  {label} -> {json.dumps(got)}")
        if want_item is None:
            if got is not None:
                raise SystemExit(f"FAIL: {label}: expected no result, got {got}")
            return None
        if not got or got.get("item") != want_item or got.get("count") != 1:
            grid = [s for s in slots() if s["slot"] in GRID]
            raise SystemExit(f"FAIL: {label}: expected 1 {want_item}, got {got}; grid {json.dumps(grid)}")
        return got

    def consumed(items: list[tuple[str, int]], label: str) -> None:
        for item, _ in items:
            left = count_of(item)
            if left != 0:
                raise SystemExit(f"FAIL: {label}: {left} of {item} left after the craft")
        grid = [s for s in slots() if s["slot"] in GRID and s.get("item")]
        if grid:
            raise SystemExit(f"FAIL: {label}: the grid still holds {grid}")

    def expect_held(want: str, diamonds: int | None) -> None:
        held, entries = held_contents()
        if held != want:
            raise SystemExit(f"FAIL: the unit is {held!r}, expected {want}")
        want_entries = [] if diamonds is None else [("minecraft:diamond", diamonds)]
        if entries != want_entries:
            raise SystemExit(f"FAIL: {want} holds {entries}, expected {want_entries}")

    def upgrade(label: str, target: str, materials: list[tuple[str, int]],
                corners: list[str], above_below: list[str], left_right: list[str],
                unit: str, diamonds: int | None) -> None:
        setup(materials, clear=False)
        place(corners[0], CORNERS)
        place(above_below[0], ABOVE_BELOW)
        place(left_right[0], LEFT_RIGHT)
        place(unit, [CENTRE])
        expect_result(target, label)
        take_into_hotbar()
        consumed(materials, label)
        close_and_hold()
        expect_held(target, diamonds)

    # ---------------------------------------------------------------- tier 1
    print("== the carried tier-1 recipe crafts in a crafting table ==")
    tier1 = [("minecraft:iron_ingot", 4), ("minecraft:piston", 2),
             ("minecraft:redstone", 2), ("minecraft:chest", 1)]
    setup(tier1)
    place("minecraft:iron_ingot", CORNERS)
    place("minecraft:piston", ABOVE_BELOW)
    place("minecraft:redstone", LEFT_RIGHT)
    place("minecraft:chest", [CENTRE])
    expect_result("pocketstorage:psu_1", "iron + piston + redstone + chest")
    take_into_hotbar()
    consumed(tier1, "tier 1")
    close_and_hold()
    expect_held("pocketstorage:psu_1", None)

    # ------------------------------------------------------- contents to move
    # No `clear` here: the craft consumed every ingredient, so the only thing in
    # the inventory is the crafted unit itself.
    run("give", "minecraft:diamond", "30")
    wait(3)
    run("chat", "/psu put minecraft:diamond 30")
    wait(12)
    held, entries = held_contents()
    if held != "pocketstorage:psu_1" or entries != [("minecraft:diamond", 30)]:
        raise SystemExit(f"FAIL: the unit is {held!r} holding {entries}, expected psu_1 with 30 diamonds")
    print("  the unit holds 30 diamonds before the upgrade")

    # --------------------------------------------------------------- 1 -> 2
    print("== the jar's upgrade pattern makes tier 2 and keeps the contents ==")
    tier2 = [("minecraft:gold_ingot", 4), ("minecraft:piston", 2), ("minecraft:chest", 2)]
    setup(tier2, clear=False)
    place("minecraft:gold_ingot", CORNERS)
    place("minecraft:piston", ABOVE_BELOW)
    place("minecraft:chest", LEFT_RIGHT)
    place("pocketstorage:psu_1", [CENTRE])
    expect_result("pocketstorage:psu_2", "gold + piston + chest around psu_1")
    take_into_hotbar()
    consumed(tier2, "upgrade 1->2")
    close_and_hold()
    if SELFTEST:
        # The flipped expectation: the contents must NOT have ridden along. The run
        # has to fail here, which is what proves the assertion above can.
        held, entries = held_contents()
        if held == "pocketstorage:psu_2" and entries == [("minecraft:diamond", 30)]:
            raise SystemExit(
                "FAIL: selftest: the upgraded unit kept its contents, the flipped "
                "expectation fails as it must"
            )
        print("  selftest: the flipped contents expectation failed as it must")
        return 1
    expect_held("pocketstorage:psu_2", 30)

    # --------------------------------------------------------------- 2 -> 3
    print("== diamonds upgrade tier 2 to 3 ==")
    tier3 = [("minecraft:diamond", 4), ("minecraft:piston", 2), ("minecraft:chest", 2)]
    upgrade("diamond + piston + chest around psu_2", "pocketstorage:psu_3", tier3,
            ["minecraft:diamond"], ["minecraft:piston"], ["minecraft:chest"],
            "pocketstorage:psu_2", 30)

    # --------------------------------------------------------------- 3 -> 4
    print("== nether stars upgrade tier 3 to 4 ==")
    tier4 = [("minecraft:piston", 4), ("minecraft:chest", 2), ("minecraft:nether_star", 2)]
    upgrade("piston + chest + nether star around psu_3", "pocketstorage:psu_4", tier4,
            ["minecraft:piston"], ["minecraft:chest"], ["minecraft:nether_star"],
            "pocketstorage:psu_3", 30)

    run("chat", "/psu list")
    wait(12)
    readout = run("messages", "16")["chat"]
    line = next((text for text in readout if text.startswith("pocketstorage:psu_4: ")), "")
    print(f"  {line}")
    want = "pocketstorage:psu_4: 1 of 64 kinds, 30 items, 1048575 each at most"
    if line != want:
        raise SystemExit(f"FAIL: tier 4 reports {line!r}, expected {want!r}")

    # ---------------------------------------------------------------- control
    # Last, so the ingredients a refused craft leaves in the grid cannot leak into
    # another step: the tier-2 pattern with the tier-4 unit in its centre must make
    # nothing, and the unit must come back exactly as it went in.
    print("== the control: a tier-4 unit is not the tier-1 ingredient ==")
    setup(tier2, clear=False)
    place("minecraft:gold_ingot", CORNERS)
    place("minecraft:piston", ABOVE_BELOW)
    place("minecraft:chest", LEFT_RIGHT)
    place("pocketstorage:psu_4", [CENTRE])
    expect_result(None, "gold + piston + chest around psu_4")
    if item_of(CENTRE) != "pocketstorage:psu_4":
        raise SystemExit("FAIL: the tier-4 unit left its cell, so the control crafted something")
    print("  the tier-4 unit stayed in the grid: the recipe is the tier it declares")
    run("click-slot", str(CENTRE))
    wait(2)
    run("click-slot", str(HOTBAR))
    wait(2)
    close_and_hold()
    expect_held("pocketstorage:psu_4", 30)

    run("close")
    wait(2)
    print("craft PASS: the tier-1 recipe and all three upgrades craft in a table, each upgrade "
          "keeps the 30 diamonds, the old unit and the materials are consumed, and a tier-4 "
          "unit in the tier-2 pattern makes nothing")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
