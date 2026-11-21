#!/usr/bin/env bash
# In-game check for Pocket Storage slice 1: one `pocketstorage:psu_1` carries its
# own inventory with the jar's capacity (8 kinds, 255 each), fills from a
# container and empties into one by sneak-clicking it, keeps its contents
# through a drop and a relaunch, and refuses more when a slot is full.
#
# Every mod command is typed with `ctl chat "/cmd"` because `ctl command` runs
# the console path and bypasses the native `perform_command` hook.
#
# The End, in the void at x=200 (away from the central island and its dragon
# egg). The arena floor is stone, the air above it is cleared and every
# non-player entity is removed before a phase needs a clean level.
#
# FORMAT (2026-11-18): this port is one flat SDK package, so the code and the content
# travel together. `GAME` must be the game the **launcher** built from this package
# (`ports/tools/launcher-game.sh run pocketstorage` supplies it, with `CTL`,
# `CONTROL_GAME` and `RES`); there is no `--dev-mod` any more.
#
# CONTROL=1 (the launcher run sets `CONTROL_GAME`) first launches the same Nightly
# **without** the package, where the id must be unknown.
#
# Phases:
#  A  the unit item exists and is held;
#  B  a new unit is empty and reports the jar's numbers (8 kinds, 255 each);
#  C  `/psu put` fills one slot to exactly 255 and refuses the rest (the other
#     45 diamonds stay in the inventory);
#  D  a second kind takes the next slot;
#  E  sneak + right-click a chest deposits all but one of each kind: the chest
#     gets 254 diamonds and 9 cobblestone, the unit keeps one of each;
#  F  sneak + left-click the chest takes back exactly the kinds the unit holds
#     (the chest's dirt stays), and a plain item never deposits;
#  G  the contents survive a drop and a pickup;
#  H  the contents survive a relaunch;
#  I  a locked chest refuses both transfers;
#  J  the carried tier-1 recipe and the three `pocketstorage:upgrade` recipes
#     craft in a crafting table, each upgrade keeping the unit's contents, and a
#     tier-2 unit in the tier-2 pattern makes nothing.
#
# SELFTEST=1 flips F's "the chest is empty" assertion (it must FAIL), proving the
# scenario can fail. NEGATIVE=1 flips J's contents assertion (it must FAIL too),
# so the upgrade family has its own counterfactual.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${PS_DEBUG:-}" ] || set -x

CONTROL=${CONTROL:-0}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}

CLONE=${CLONE:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/pocketstorage
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-}
GAME=${GAME:-}
CONTROL_GAME=${CONTROL_GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/pocketstorage-e2e.sock}
WORLD=PocketStorageE2E
CONTROL_WORLD=PocketStorageControl
UNIT=pocketstorage:psu_1
# The arena: the End's void, far from the central island.
X0=200
CHEST_X=$((X0 + 2))
CAPACITY=255
SLOTS=8  # the tier-1 slots, for the readout assertions

command -v jq >/dev/null || { echo "missing jq (every assertion parses a ctl reply)" >&2; exit 1; }
[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/pocketstorage-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }
on_error() { echo "FAIL: line $1: '$2' exited $?" >&2; ctl logs --lines 40 >&2 || true; }
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # launch [WORLD] [GAME]
    local world=${1:-$WORLD}
    local game=${2:-$GAME}
    rm -f "$SOCK"
    cd "$SCRATCH"
    # `--offscreen`, not `--headless`: the recipe phase opens a crafting-table menu
    # and clicks its slots, and a headless client never opens a screen (the same
    # reason chisel and additional_lights launch offscreen for their craft phases).
    "$CTL" --socket "$SOCK" launch --game "$game" --world "$world" --seed 20261105 \
        --creative --offscreen --wait --resources "$RES" >/dev/null
    ctl wait-ready 120 >/dev/null
    ctl wait 40 >/dev/null
}

block_name() { ctl block "$1" "$2" "$3" | jq -r '.name // .block // empty'; }

# The End's void at X0: a stone floor and clear air above it. The player is
# teleported first, so the chunks the fills touch are loaded; the floor is laid
# before anything is placed, because a player over the void falls.
arena() {
    ctl command "execute in minecraft:the_end run tp @s $X0.5 104 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$X0" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the End arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((X0 - 2)) 100 -14 $((X0 + 4)) 100 14 minecraft:stone" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run fill $((X0 - 2)) 101 -14 $((X0 + 4)) 108 14 minecraft:air" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $X0.5 101 0.5" >/dev/null
    ctl wait 20 >/dev/null
    [ "$(block_name "$X0" 100 0)" = "minecraft:stone" ] || fail "the arena floor is not stone"
    [ "$(block_name "$X0" 101 0)" = "minecraft:air" ] || fail "the arena air above the floor is not air"
}

# Hold a fresh unit in the first hotbar slot.
hold_unit() {
    ctl command "item replace entity @s hotbar.0 with $UNIT 1" >/dev/null
    ctl wait 4 >/dev/null
    ctl select 1 >/dev/null
    ctl wait 2 >/dev/null
}

# The held stack's own data, as shell assignments: HELD, SLOTS, S<n>_ITEM,
# S<n>_COUNT, TOTAL.
read_unit() {
    HELD='' KINDS=0 TOTAL=0 COMPONENT='' S0_ITEM='' S0_COUNT='' S1_ITEM='' S1_COUNT=''
    eval "$(ctl inventory | python3 "$PORT/tools/unit_state.py")"
}

# The last lines a mod command answered, for the transcript.
replies() { ctl messages 16 | jq -r '.chat[]' | tail -4 | sed 's/^/  | /' >&2; }

expect_unit() { # expect_unit COUNT_0 [COUNT_1]
    read_unit
    [ "$HELD" = "$UNIT" ] || fail "the held item is '$HELD', expected $UNIT"
    [ "$KINDS" = "${3:-1}" ] || fail "the unit holds $KINDS kinds, expected ${3:-1}"
    [ "${S0_ITEM:-}" = "minecraft:diamond" ] || fail "slot 0 holds '${S0_ITEM:-}', expected diamonds"
    [ "$S0_COUNT" = "$1" ] || fail "slot 0 holds $S0_COUNT diamonds, expected $1"
    if [ -n "${2:-}" ]; then
        [ "${S1_ITEM:-}" = "minecraft:cobblestone" ] || fail "slot 1 holds '${S1_ITEM:-}', expected cobblestone"
        [ "${S1_COUNT:-}" = "$2" ] || fail "slot 1 holds ${S1_COUNT:-} cobblestone, expected $2"
    fi
    echo "  unit: $KINDS kinds, $TOTAL items (diamonds ${S0_COUNT}${2:+, cobblestone ${S1_COUNT:-}})"
}

# `/psu list`: the human-readable readout, and the numbers in it.
list_line() {
    say "psu list"
    ctl wait 12 >/dev/null
    ctl messages 16 | jq -r '.chat[]' | grep -m1 '^pocketstorage:psu_1: ' || true
}

# The container at X Y Z: how many of ITEM it holds.
container_count() { # container_count X Y Z ITEM
    ctl container "$1" "$2" "$3" | jq -r --arg item "$4" \
        '[.slots[] | select(.item == $item) | .count] | add // 0'
}

container_total() { ctl container "$1" "$2" "$3" | jq -r '[.slots[] | select(.item != null) | .count] | add // 0'; }

# Stand two blocks west of the chest and click one of its faces.
aim_chest() { # aim_chest FACE [X]
    local x=${2:-$CHEST_X}
    ctl command "execute in minecraft:the_end run tp @s $((x - 2)).5 101 0.5" >/dev/null
    ctl wait 6 >/dev/null
    ctl look-at-block "$x" 101 0 "$1" >/dev/null
    ctl wait 2 >/dev/null
}

sneak_click() { # sneak_click right|left FACE [X]
    local x=${3:-$CHEST_X}
    aim_chest "$2" "$x"
    ctl key shift down >/dev/null
    ctl wait 4 >/dev/null
    if [ "$1" = right ]; then
        ctl interact "$x" 101 0 "$2" >/dev/null
    else
        ctl mouse left click >/dev/null
    fi
    ctl wait 8 >/dev/null
    ctl key shift up >/dev/null
    ctl wait 6 >/dev/null
}

# The control: the same Nightly, unmodified (the launcher exports it as
# `CONTROL_GAME`), so the only difference is the package. The id must be unknown.
control_leg() {
    echo "== control: the same Nightly without this package =="
    launch "$CONTROL_WORLD" "$CONTROL_GAME"
    arena
    ctl command "item replace entity @s hotbar.0 with $UNIT 1" >/dev/null 2>&1 || true
    ctl wait 4 >/dev/null
    ctl inventory | jq -e "[.hotbar[], .main[] | select(.item == \"$UNIT\")] | length == 0" >/dev/null \
        || fail "the control game has the unit item, so the package was in it"
    ctl command "execute in minecraft:the_end run setblock $CHEST_X 101 0 $UNIT" >/dev/null 2>&1 || true
    ctl wait 4 >/dev/null
    local name
    name=$(block_name "$CHEST_X" 101 0)
    [ "$name" = "minecraft:air" ] || fail "the control game holds $name, so the item id was known"
    echo "  without the package: no unit item, the id is unknown"
    ctl quit >/dev/null 2>&1 || true
}

echo "== launch (The End arena, content + native bundle) =="
if [ "$CONTROL" = 1 ]; then
    [ -n "$CONTROL_GAME" ] || { echo "CONTROL=1 needs CONTROL_GAME" >&2; exit 2; }
    control_leg
fi
launch
arena

echo "== A: the unit item registers and is held =="
ctl command "item replace entity @s hotbar.0 with $UNIT 1" >/dev/null
ctl wait 4 >/dev/null
ctl select 1 >/dev/null
ctl inventory | jq -e "[.hotbar[], .main[] | select(.item == \"$UNIT\")] | length >= 1" >/dev/null \
    || fail "the unit item did not register: $(ctl inventory | jq -c '{hotbar,main}')"
read_unit
[ "$HELD" = "$UNIT" ] || fail "the held item is '$HELD', expected $UNIT"
echo "  held: $HELD, no contents component: $([ -z "$COMPONENT" ] && echo yes || echo no)"

echo "== B: a new unit is empty and reports the jar's numbers =="
LINE=$(list_line)
echo "  $LINE"
case "$LINE" in
    "$UNIT: 0 of $SLOTS kinds, 0 items, $CAPACITY each at most") ;;
    *) fail "a new unit does not report 0 of $SLOTS kinds of $CAPACITY: $LINE" ;;
esac
ctl messages 16 | jq -r '.chat[]' | grep -qx -- "- empty" || fail "a new unit does not list as empty"
[ -z "$COMPONENT" ] || fail "an empty unit carries a component: $COMPONENT"

echo "== C: one slot fills to exactly 255 and refuses the rest =="
ctl command "give @s minecraft:diamond 300" >/dev/null
ctl wait 6 >/dev/null
say "psu put minecraft:diamond 300"
ctl wait 12 >/dev/null
replies
read_unit
echo "  held: $HELD, $KINDS kinds, component: ${COMPONENT:-<none>}"
[ "$S0_COUNT" = "$CAPACITY" ] || fail "the unit took '${S0_COUNT:-nothing}' diamonds, expected $CAPACITY"
LEFT=$(ctl inventory | jq -r '.counts["minecraft:diamond"] // 0')
[ "$LEFT" = 45 ] || fail "the inventory holds $LEFT diamonds, expected the 45 the unit refused"
echo "  the unit took $CAPACITY of 300; $LEFT stayed in the inventory"

echo "== D: a second kind takes the next slot =="
ctl command "give @s minecraft:cobblestone 10" >/dev/null
ctl wait 6 >/dev/null
say "psu put minecraft:cobblestone 10"
ctl wait 12 >/dev/null
expect_unit "$CAPACITY" 10 2

echo "== E: sneak right-click a chest deposits all but one of each kind =="
ctl command "execute in minecraft:the_end run setblock $CHEST_X 101 0 minecraft:chest" >/dev/null
ctl wait 6 >/dev/null
[ "$(block_name "$CHEST_X" 101 0)" = "minecraft:chest" ] || fail "the chest did not place"
sneak_click right west
DIAMONDS=$(container_count "$CHEST_X" 101 0 minecraft:diamond)
COBBLE=$(container_count "$CHEST_X" 101 0 minecraft:cobblestone)
[ "$DIAMONDS" = 254 ] || fail "the chest holds $DIAMONDS diamonds, expected 254 (all but one)"
[ "$COBBLE" = 9 ] || fail "the chest holds $COBBLE cobblestone, expected 9 (all but one)"
[ "$(block_name "$CHEST_X" 101 0)" = "minecraft:chest" ] || fail "the chest is gone"
expect_unit 1 1 2

echo "== F: sneak left-click the chest takes back the kinds the unit holds =="
sneak_click left west
DIAMONDS=$(container_count "$CHEST_X" 101 0 minecraft:diamond)
COBBLE=$(container_count "$CHEST_X" 101 0 minecraft:cobblestone)
if [ "$SELFTEST" = 1 ]; then
    echo "(selftest: asserting the chest still holds 254 diamonds)"
    [ "$DIAMONDS" = 254 ] || fail "selftest: the chest holds $DIAMONDS diamonds, the flipped expectation fails as it must"
fi
[ "$DIAMONDS" = 0 ] || fail "the chest still holds $DIAMONDS diamonds after the withdraw"
[ "$COBBLE" = 0 ] || fail "the chest still holds $COBBLE cobblestone after the withdraw"
expect_unit "$CAPACITY" 10 2
[ "$(block_name "$CHEST_X" 101 0)" = "minecraft:chest" ] || fail "the withdraw broke the chest"

echo "== F2: a kind the unit does not hold stays, and a plain item deposits nothing =="
ctl container "$CHEST_X" 101 0 minecraft:dirt 16 >/dev/null
ctl wait 6 >/dev/null
sneak_click left west
[ "$(container_count "$CHEST_X" 101 0 minecraft:dirt)" = 16 ] || fail "the unit took dirt it does not hold"
expect_unit "$CAPACITY" 10 2
ctl command "give @s minecraft:diamond 1" >/dev/null
ctl wait 4 >/dev/null
ctl command "item replace entity @s hotbar.1 with minecraft:diamond 1" >/dev/null
ctl wait 4 >/dev/null
ctl select 2 >/dev/null
ctl wait 2 >/dev/null
sneak_click right west
[ "$(container_count "$CHEST_X" 101 0 minecraft:dirt)" = 16 ] || fail "a plain item changed the chest"
[ "$(container_total "$CHEST_X" 101 0)" = 16 ] || fail "a plain item deposited into the chest"
ctl select 1 >/dev/null
ctl wait 2 >/dev/null
echo "  the chest still holds its 16 dirt; a plain diamond deposited nothing"

echo "== G: the contents survive a drop and a pickup =="
read_unit
BEFORE_TOTAL=$TOTAL
[ "$BEFORE_TOTAL" = 265 ] || fail "the unit holds $BEFORE_TOTAL items before the drop, expected 265"
ctl command "execute in minecraft:the_end run tp @s $X0.5 101 4.5" >/dev/null
ctl wait 6 >/dev/null
ctl drop all >/dev/null
ctl wait 20 >/dev/null
DROP=$(ctl entities 8 | jq -c --arg unit "$UNIT" \
    '[.entities[] | select(.kind == "item" and .item.item == $unit)] | first')
[ -n "$DROP" ] && [ "$DROP" != null ] || fail "the dropped unit is not on the ground: $(ctl inventory | jq -c .held)"
DROPPED=$(printf '%s' "$DROP" | python3 "$PORT/tools/unit_state.py")
eval "$DROPPED"
[ "$HELD" = "$UNIT" ] || fail "the dropped stack is '$HELD', expected $UNIT"
[ "$TOTAL" = "$BEFORE_TOTAL" ] || fail "the dropped stack holds $TOTAL items, expected $BEFORE_TOTAL"
echo "  the dropped stack still holds $TOTAL items in $KINDS kinds: $COMPONENT"
POS=$(printf '%s' "$DROP" | jq -r '"\(.pos[0]) \(.pos[1]) \(.pos[2])"')
ctl tp $POS >/dev/null
ctl wait 60 >/dev/null
ctl select "$UNIT" >/dev/null
ctl wait 4 >/dev/null
read_unit
[ "$HELD" = "$UNIT" ] || fail "the dropped unit was not picked back up (held '$HELD')"
[ "$TOTAL" = "$BEFORE_TOTAL" ] || fail "the unit holds $TOTAL items after the pickup, expected $BEFORE_TOTAL"
expect_unit "$CAPACITY" 10 2

echo "== H: the contents survive a relaunch =="
ctl quit >/dev/null 2>&1 || true
sleep 2
launch
ctl wait 20 >/dev/null
ctl select 1 >/dev/null
ctl wait 4 >/dev/null
expect_unit "$CAPACITY" 10 2
LINE=$(list_line)
echo "  after the relaunch: $LINE"
case "$LINE" in
    "$UNIT: 2 of $SLOTS kinds, 265 items, $CAPACITY each at most") ;;
    *) fail "the relaunched unit does not report 2 kinds of 265 items: $LINE" ;;
esac
echo "  the chest still holds its 16 dirt: $(container_count "$CHEST_X" 101 0 minecraft:dirt)"

echo "== I: a locked chest refuses both transfers =="
# A container's vanilla lock (`BlockEntity.lock`, set here with `data merge block`) is the
# access rule its open path asks; the unit's transfers go through the sided capability,
# which bypasses it, so a locked chest must take nothing and give nothing. A fresh unit
# is used so both directions have room, and the main unit's 265 items are not touched.
LOCKED_X=$((X0 + 3))
ctl command "item replace entity @s hotbar.0 with $UNIT 1" >/dev/null
ctl wait 6 >/dev/null
ctl select 1 >/dev/null
ctl wait 4 >/dev/null
ctl command "give @s minecraft:diamond 4" >/dev/null
ctl wait 6 >/dev/null
say "psu put minecraft:diamond 4"
ctl wait 12 >/dev/null
read_unit
[ "${S0_COUNT:-}" = 4 ] || fail "the fresh unit took ${S0_COUNT:-none} diamonds, expected 4"
ctl command "execute in minecraft:the_end run setblock $LOCKED_X 101 0 minecraft:chest" >/dev/null
ctl wait 6 >/dev/null
LOCK_REPLY=$(ctl command "data merge block $LOCKED_X 101 0 {lock:{items:\"minecraft:barrier\"}}" 2>&1 || true)
echo "  lock: $LOCK_REPLY"
ctl wait 6 >/dev/null
ctl container "$LOCKED_X" 101 0 minecraft:diamond 6 >/dev/null
ctl wait 6 >/dev/null
[ "$(container_count "$LOCKED_X" 101 0 minecraft:diamond)" = 6 ] || fail "the locked chest did not take its 6 diamonds"
ctl command "execute in minecraft:the_end run tp @s $((LOCKED_X - 2)).5 101 0.5" >/dev/null
ctl wait 6 >/dev/null
ctl key shift down >/dev/null
ctl wait 4 >/dev/null
ctl interact "$LOCKED_X" 101 0 west >/dev/null 2>&1 || true
ctl wait 8 >/dev/null
ctl key shift up >/dev/null
ctl wait 6 >/dev/null
LOCKED_DEPOSIT=$(container_count "$LOCKED_X" 101 0 minecraft:diamond)
[ "$LOCKED_DEPOSIT" = 6 ] || fail "the unit deposited into a locked chest ($LOCKED_DEPOSIT diamonds, expected 6)"
ctl key shift down >/dev/null
ctl wait 4 >/dev/null
ctl mouse left click >/dev/null 2>&1 || true
ctl wait 8 >/dev/null
ctl key shift up >/dev/null
ctl wait 6 >/dev/null
LOCKED_WITHDRAW=$(container_count "$LOCKED_X" 101 0 minecraft:diamond)
[ "$LOCKED_WITHDRAW" = 6 ] || fail "the unit withdrew from a locked chest ($LOCKED_WITHDRAW diamonds, expected 6)"
read_unit
[ "${S0_COUNT:-}" = 4 ] || fail "the unit's diamonds changed through a locked chest (${S0_COUNT:-none}, expected 4)"
echo "  the locked chest still holds 6 diamonds; the unit still holds 4 (deposit and withdraw both refused)"

echo "== J: the carried recipes craft, and the three upgrades keep the contents =="
# The tier-1 recipe and the three `pocketstorage:upgrade` recipes, crafted the way
# they declare in a real crafting table, with the ingredient consumption and the
# contents-riding assert. A tier-2 unit in the tier-2 pattern must make nothing.
# `NEGATIVE=1` flips the contents assertion, so this leg must exit 1 (a second,
# family-specific counterfactual beside SELFTEST's phase-F flip).
ctl command "gamemode survival @s" >/dev/null
ctl wait 6 >/dev/null
ctl command "execute in minecraft:the_end run setblock $((X0 + 4)) 101 0 minecraft:crafting_table" >/dev/null
ctl wait 6 >/dev/null
python3 "$PORT/tools/craft_upgrade.py" "$SOCK" "$CTL" $((X0 + 4)) 101 0 || fail "the recipe/upgrade check failed"
echo "recipes PASS"

echo "PASS: the unit registers and holds nothing, reports 8 kinds of 255, fills one slot to exactly 255 and refuses the other 45, takes a second kind in the next slot, deposits all but one of each kind into a chest (254 + 9), withdraws exactly the kinds it holds (the chest's dirt stays), a plain diamond deposits nothing, a locked chest refuses both transfers, both the drop/pickup and the relaunch keep 255 diamonds and 10 cobblestone, the carried tier-1 recipe crafts in a table, and the jar's gold/diamond/nether-star upgrades make tiers 2, 3 and 4 each keeping the unit's 30 diamonds while a tier-4 unit in the tier-2 pattern makes nothing"
