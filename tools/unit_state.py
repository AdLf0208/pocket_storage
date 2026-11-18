#!/usr/bin/env python3
"""Read a `pocketstorage:contents` component out of a `ctl inventory` reply.

Takes a `ctl inventory` reply (or one item entity of `ctl entities`) on stdin and prints shell assignments, so an e2e can assert on
the stack's own data rather than on a message:

    eval "$(ctl inventory | python3 tools/unit_state.py)"
    [ "$HELD" = pocketstorage:psu_1 ]
    [ "$KINDS" = 2 ]
    [ "$S0_ITEM" = minecraft:diamond ] && [ "$S0_COUNT" = 255 ]

`HELD` is the held item id (empty when nothing is held) and `KINDS` counts the
component's entries (0 when the stack carries none). `S<n>_ITEM`/`S<n>_COUNT`
describe entry `n` in the order the component lists them; `TOTAL` sums their
counts and `COMPONENT` is the raw SNBT. The command's exit status is 0 either
way: the caller asserts.
"""

from __future__ import annotations

import json
import re
import sys

# The SNBT is a compound of one list of slot compounds. NBT compounds are
# sorted by key (`{count:255,id:"minecraft:diamond",slot:0}`), so the fields
# are matched by name, not by order.
OBJECT = re.compile(r"\{([^{}]*)\}")
PAIR = re.compile(r'(slot|id|count):("[^"]*"|[^,}]+)')


def entries(components: str) -> list[tuple[int, str, int]]:
    found = []
    for body in OBJECT.findall(components):
        fields = {key: value for key, value in PAIR.findall(body)}
        if set(fields) != {"slot", "id", "count"}:
            continue
        item = fields["id"].strip()
        if item.startswith('"'):
            item = item[1:-1]
        try:
            found.append((int(fields["slot"]), item, int(fields["count"])))
        except ValueError:
            continue
    return found


def shell(value: str) -> str:
    """A single-quoted shell word for `value`."""
    return "'" + value.replace("'", "'\\''") + "'"


def main() -> None:
    reply = json.load(sys.stdin)
    # A `ctl inventory` reply names the held stack `held`; a dropped item
    # entity's stack is `item`, so either shape reads the same way.
    stack = reply.get("held") or reply.get("item") or {}
    held = stack.get("item") or ""
    components = stack.get("components") or ""
    slots = entries(components)
    print(f"HELD={shell(held)}")
    print(f"KINDS={len(slots)}")
    total = 0
    for index, (slot, item, count) in enumerate(slots):
        total += count
        print(f"S{index}_SLOT={slot}")
        print(f"S{index}_ITEM={shell(item)}")
        print(f"S{index}_COUNT={count}")
    print(f"TOTAL={total}")
    print(f"COMPONENT={shell(components)}")


if __name__ == "__main__":
    main()
