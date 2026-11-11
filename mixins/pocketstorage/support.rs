// Game-side glue for Pocket Storage slice 1 (the tier-1 storage unit).
//
// Injected into `crate::game` as `pocketstorage_state`; it needs the game, so
// it is not part of the standalone library (only `runtime.rs` is).
//
// A unit's contents ride the stack's own component (`pocketstorage:contents`:
// one compound per filled slot, `{slot, id, count}`), so the stack is the
// container — nothing here needs a block entity or a world file, and a stack
// that is saved, thrown or picked up keeps what it holds. Sneak + right-click a
// container deposits all but one of each held kind into it (upstream's
// `giveItems` rule); sneak + left-click a container withdraws the kinds the
// unit already holds (upstream's `takeItems` rule). `/psu put|list` stands in
// for upstream's screen, which this slice does not carry.

use std::sync::Mutex;

use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_core::geometry::Direction;
use horizon_world::nbt::{JavaString, Tag};

use crate::game::Game;
use crate::game::native_lifecycle::ServerTickPhase;
use crate::game::native_server::{NativeCommand, NativeServerPlayer};
use crate::game::pocketstorage::{
    Action, COMPONENT, Contents, Entry, ROOT, Tier, is_unit, parse, tier_of,
};

/// The keys of one saved slot.
const KEY_SLOT: &str = "slot";
const KEY_ITEM: &str = "id";
const KEY_COUNT: &str = "count";

/// The players whose held stack changed, to resend at the end of the tick.
#[derive(Default)]
struct State {
    sync: Vec<u128>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn state() -> std::sync::MutexGuard<'static, Option<State>> {
    STATE.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// Registers the unit's component id, before content loads.
pub(crate) fn register_component_types(types: &mut Vec<String>) {
    types.push(COMPONENT.to_owned());
}

/// Sneak + right-click a container: deposit all but one of each held kind.
#[expect(
    private_interfaces,
    reason = "the module is injected inside crate::game and only its helpers call it"
)]
pub fn on_right_click_block(
    game: &mut Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    canceled: &mut bool,
) {
    if !sneaking(game) {
        return;
    }
    let Some(uuid) = game.native_acting_player().map(|player| player.uuid) else {
        return;
    };
    let Some(stack) = game.held().cloned() else {
        return;
    };
    let Some(tier) = tier_of(&stack.item) else {
        return;
    };
    let side = direction_of(face);
    let dimension = game.native_dimension().to_owned();
    if game
        .native_container_slots(&dimension, pos, side)
        .is_none()
    {
        return;
    }
    // The click is ours from here: a chest does not open on a deposit.
    *canceled = true;
    let before = read_contents(game, &stack, tier);
    if before.is_empty() {
        game.native_action_bar(uuid, Text::literal("The storage unit is empty"));
        return;
    }
    let mut contents = before.clone();
    let mut moved = 0_u64;
    for (slot, entry) in before.deposit_plan() {
        let chunk = max_stack(game, &entry.item);
        let mut left = entry.count;
        while left > 0 {
            let send = left.min(chunk);
            let rest = game.native_insert_item(pos, side, ItemStack::new(&entry.item, send));
            let put = send.saturating_sub(rest.map_or(0, |rest| rest.count));
            moved += u64::from(put);
            left -= put;
            if put == 0 {
                break;
            }
        }
        // Upstream keeps one of each kind behind, so the slot keeps the kind.
        contents.set(slot, Some(Entry::new(&entry.item, left + 1)));
    }
    if contents != before {
        write_contents(game, &stack, &contents);
    }
    let line = if moved == 0 {
        format!(
            "The container took nothing; the unit still holds {}",
            before.total()
        )
    } else {
        format!("Deposited {moved} items, keeping one of each kind")
    };
    game.native_action_bar(uuid, Text::literal(line));
}

/// Sneak + left-click a container: withdraw the kinds the unit already holds.
#[expect(
    private_interfaces,
    reason = "the module is injected inside crate::game and only its helpers call it"
)]
pub fn on_left_click_block(
    game: &mut Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    canceled: &mut bool,
) {
    if !sneaking(game) {
        return;
    }
    let Some(uuid) = game.native_acting_player().map(|player| player.uuid) else {
        return;
    };
    let Some(stack) = game.held().cloned() else {
        return;
    };
    let Some(tier) = tier_of(&stack.item) else {
        return;
    };
    let side = direction_of(face);
    let dimension = game.native_dimension().to_owned();
    let Some(slots) = game.native_container_slots(&dimension, pos, side) else {
        return;
    };
    // The click is ours: a withdraw never starts a dig.
    *canceled = true;
    let before = read_contents(game, &stack, tier);
    let mut contents = before.clone();
    let mut moved = 0_u64;
    for slot in slots {
        let (Some(here), true) = (slot.stack, slot.can_take) else {
            continue;
        };
        let want = contents.withdraw_amount(&here.item, here.count);
        if want == 0 {
            continue;
        }
        let Some(taken) = game.native_extract_item(&dimension, pos, side, slot.slot, want) else {
            continue;
        };
        let leftover = contents.insert(&taken.item, taken.count);
        moved += u64::from(taken.count - leftover);
        if leftover > 0 {
            let _ = game.native_insert_item(pos, side, ItemStack::new(&taken.item, leftover));
        }
    }
    if contents != before {
        write_contents(game, &stack, &contents);
    }
    let line = if moved == 0 {
        "Nothing here is in the unit".to_owned()
    } else {
        format!(
            "Withdrew {moved} items; the unit holds {} of {} kinds",
            contents.filled(),
            contents.len()
        )
    };
    game.native_action_bar(uuid, Text::literal(line));
}

const HELP: [&str; 3] = [
    "/psu list            the held unit's slots and counts",
    "/psu put <item> [n]  move n (1) plain items from your inventory in",
    "/psu help            this text",
];

/// Declares the command fallback for the screen this slice does not carry.
pub(crate) fn on_register_commands(commands: &mut Vec<NativeCommand>) {
    for usage in [
        format!("{ROOT} list"),
        format!("{ROOT} put <item:word> [count:integer]"),
        format!("{ROOT} help"),
    ] {
        commands.push(NativeCommand::new(&usage));
    }
}

/// Runs `/psu …` for the acting player, whose held stack is in place.
#[expect(
    private_interfaces,
    reason = "the module is injected inside crate::game and only its helpers call it"
)]
pub fn on_perform_command(
    game: &mut Game<'_>,
    _player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    let action = parse(command);
    if action == Action::Unknown {
        return;
    }
    *ran = true;
    match action {
        Action::Help => {
            for line in HELP {
                game.native_feedback(Text::literal(line));
            }
        }
        Action::List => {
            for line in list_lines(game) {
                game.native_feedback(Text::literal(line));
            }
        }
        Action::Put { item, count } => {
            let line = put(game, &item, count);
            game.native_feedback(Text::literal(line));
        }
        Action::Unknown => {}
    }
}

/// End of a server tick: resend the inventory of a player whose held stack we
/// wrote inside a guest turn (akashictome's finding: a held-stack write there
/// needs an explicit `send_inventory`).
#[expect(
    private_interfaces,
    reason = "the module is injected inside crate::game and only its helpers call it"
)]
pub fn on_server_tick(game: &mut Game<'_>, phase: ServerTickPhase) {
    if !matches!(phase, ServerTickPhase::ServerTickEnd { .. }) {
        return;
    }
    let sync = {
        let mut guard = state();
        std::mem::take(&mut guard.get_or_insert_with(State::default).sync)
    };
    for uuid in sync {
        let Some(host) = game.lan.as_mut() else {
            break;
        };
        if let Some(index) = host
            .hub
            .guests
            .iter()
            .position(|guest| guest.placed && !guest.dead && guest.uuid == uuid)
        {
            host.hub.send_inventory(index);
        }
    }
}

/// The held unit's contents as chat lines.
fn list_lines(game: &Game<'_>) -> Vec<String> {
    let Some(stack) = game.held() else {
        return vec!["Hold a Pocket Storage Unit first.".to_owned()];
    };
    let Some(tier) = tier_of(&stack.item) else {
        return vec!["The held item is not a Pocket Storage Unit.".to_owned()];
    };
    let contents = read_contents(game, stack, tier);
    let mut lines = vec![format!(
        "{}: {} of {} kinds, {} items, {} each at most",
        tier.item,
        contents.filled(),
        contents.len(),
        contents.total(),
        tier.capacity
    )];
    if contents.is_empty() {
        lines.push("- empty".to_owned());
        return lines;
    }
    for (slot, entry) in contents.slots() {
        lines.push(format!("- slot {slot}: {} x{}", entry.item, entry.count));
    }
    lines
}

/// `/psu put <item> <count>`: the screen's insert path, for the slice. Only
/// what the unit can still hold is taken out of the inventory, so nothing is
/// ever dropped.
fn put(game: &mut Game<'_>, item: &str, count: u32) -> String {
    if is_unit(item) {
        return "A storage unit cannot hold another storage unit.".to_owned();
    }
    if game.native_max_stack_size(item).is_none() {
        return format!("{item} is not an item this game has.");
    }
    let Some(stack) = game.held().cloned() else {
        return "Hold a Pocket Storage Unit first.".to_owned();
    };
    let Some(tier) = tier_of(&stack.item) else {
        return "The held item is not a Pocket Storage Unit.".to_owned();
    };
    let mut contents = read_contents(game, &stack, tier);
    let space = contents.space_for(item).min(count);
    if space == 0 {
        return format!(
            "The unit is full: {} of {} kinds holding {} items.",
            contents.filled(),
            contents.len(),
            contents.total()
        );
    }
    let taken = game.native_take_items(item, space);
    if taken == 0 {
        return format!("You carry no plain {item}.");
    }
    let refused = contents.insert(item, taken);
    write_contents(game, &stack, &contents);
    let put = taken.saturating_sub(refused);
    format!(
        "The unit holds {} items of {} kinds; {put} of {item} went in.",
        contents.total(),
        contents.filled()
    )
}

/// The unit's contents in its stack component, an empty unit of `tier` when it
/// carries none.
fn read_contents(game: &Game<'_>, stack: &ItemStack, tier: Tier) -> Contents {
    let entries = match game.native_component(stack, COMPONENT) {
        Ok(Some(Tag::List(items))) => items.iter().filter_map(entry_from_tag).collect(),
        _ => Vec::new(),
    };
    Contents::from_entries(tier, entries)
}

/// Writes the contents into the held stack and hands the change to the client;
/// an empty unit carries no component at all (so it stacks as a new one does).
fn write_contents(game: &mut Game<'_>, stack: &ItemStack, contents: &Contents) {
    let mut stack = stack.clone();
    let written = if contents.is_empty() {
        game.native_remove_component(&mut stack, COMPONENT).is_ok()
    } else {
        game.native_set_component(&mut stack, COMPONENT, contents_tag(contents))
            .is_ok()
    };
    if !written {
        log::warn!("Horizon: Pocket Storage's contents could not be written");
        return;
    }
    replace_held(game, stack);
}

/// One tag per filled slot, in slot order.
fn contents_tag(contents: &Contents) -> Tag {
    Tag::List(
        contents
            .slots()
            .map(|(slot, entry)| {
                Tag::compound([
                    (
                        KEY_SLOT,
                        Tag::Int(i32::try_from(slot).unwrap_or(i32::MAX)),
                    ),
                    (KEY_ITEM, Tag::String(JavaString::from(entry.item.as_str()))),
                    (
                        KEY_COUNT,
                        Tag::Int(i32::try_from(entry.count).unwrap_or(i32::MAX)),
                    ),
                ])
            })
            .collect(),
    )
}

/// One saved slot, or `None` when it is not a complete entry.
fn entry_from_tag(tag: &Tag) -> Option<(usize, Entry)> {
    let Tag::Compound(entry) = tag else {
        return None;
    };
    let slot = match entry.get(&JavaString::from(KEY_SLOT))? {
        Tag::Int(slot) => usize::try_from(*slot).ok()?,
        _ => return None,
    };
    let item = match entry.get(&JavaString::from(KEY_ITEM))? {
        Tag::String(id) => id.to_string_lossy(),
        _ => return None,
    };
    let count = match entry.get(&JavaString::from(KEY_COUNT))? {
        Tag::Int(count) => u32::try_from(*count).ok()?,
        _ => return None,
    };
    Some((slot, Entry::new(&item, count)))
}

/// Replaces the acting player's held stack and queues the client sync.
fn replace_held(game: &mut Game<'_>, stack: ItemStack) {
    let slot = game.held_slot();
    let uuid = game.native_acting_player().map(|player| player.uuid);
    *game.held_mut() = Some(stack);
    if game.lan_acting.is_none() {
        game.hand_stack_replaced(slot);
    }
    if let Some(uuid) = uuid {
        let mut guard = state();
        let state = guard.get_or_insert_with(State::default);
        if !state.sync.contains(&uuid) {
            state.sync.push(uuid);
        }
    }
}

/// Whether the acting player is sneaking, as vanilla's `isSecondaryUseActive`.
fn sneaking(game: &Game<'_>) -> bool {
    game.player
        .as_ref()
        .is_some_and(|player| player.shift_key_down)
}

/// The most of `item` one container slot takes.
fn max_stack(game: &Game<'_>, item: &str) -> u32 {
    game.native_max_stack_size(item).unwrap_or(64).max(1)
}

/// A clicked face (`native_right_click_block`'s direction vector) as vanilla's
/// `Direction`: `Direction::ALL` is `[down, up, north, south, west, east]`.
fn direction_of(face: [i32; 3]) -> Direction {
    match face {
        [0, -1, 0] => Direction::Down,
        [0, 1, 0] => Direction::Up,
        [0, 0, -1] => Direction::North,
        [0, 0, 1] => Direction::South,
        [-1, 0, 0] => Direction::West,
        _ => Direction::East,
    }
}
