//! Pocket Storage for Horizon: a tiered item that carries its own inventory.
//!
//! The rules — the four tiers' numbers, insert, extract, the deposit's all-but-one
//! rule, the withdraw's held-only rule and `/psu`'s parse — live in [`runtime`],
//! which knows nothing about the game. This file is the glue: the one `impl Hooks`
//! reads the held unit's `pocketstorage:contents` component, calls the runtime and
//! writes the result back through the host.
//!
//! A unit's contents ride the stack's own component (one compound per filled slot,
//! `{slot, id, count}`), so the stack is the container — nothing here needs a block
//! entity or a world file, and a stack that is saved, thrown or picked up keeps
//! what it holds. Sneak + right-click a container deposits all but one of each held
//! kind into it (upstream's `giveItems` rule); sneak + left-click a container
//! withdraws the kinds the unit already holds (upstream's `takeItems` rule).
//! `/psu put|list` stands in for upstream's screen, which this slice does not carry.
//! `notes.md` records how each retired `.hnmod` helper maps onto this file.

pub mod runtime;

use mod_api::{
    self, Command, CommandArgumentValue, CommandInvocation, CommandSource, Component, Direction,
    Flow, Hand, Hooks, HostSide, HostTransaction, ITEMS, Identifier, ItemKey, ItemPlace,
    ItemResource, ItemStack, LeftClickBlockEvent, PlayerRef, RegisterCommandsEvent,
    RightClickBlockEvent, RunModCommandEvent, ServerHost, Tag, Text,
};

use crate::runtime::{Action, COMPONENT, Contents, Entry, Tier, is_unit, tier_of};

/// The key of one saved slot's slot number.
const KEY_SLOT: &str = "slot";
/// The key of one saved slot's item id.
const KEY_ITEM: &str = "id";
/// The key of one saved slot's count.
const KEY_COUNT: &str = "count";

/// The usage text.
const HELP: [&str; 3] = [
    "/psu list            the held unit's slots and counts",
    "/psu put <item> [n]  move n (1) plain items from your inventory in",
    "/psu help            this text",
];

/// The stack component a unit's contents ride: one compound per filled slot, the
/// shape the `.hnmod` slice saved, so a unit carried across the migration keeps
/// everything it held.
struct ContentsComponent;

impl Component for ContentsComponent {
    /// One tag per filled slot.
    type Value = Vec<Tag>;

    fn id(&self) -> &str {
        COMPONENT
    }

    fn decode(&self, value: &Tag) -> Option<Self::Value> {
        match value {
            Tag::List(entries) => Some(entries.clone()),
            _ => None,
        }
    }

    fn encode(&self, value: Self::Value) -> Tag {
        Tag::List(value)
    }
}

/// The mod has no settings and keeps no state between hooks: everything a call
/// needs is in the held stack and the container it talks to.
#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Before content loads: register the unit's component id, so a saved stack's
    /// `pocketstorage:contents` is a known, networked component.
    fn component_types(
        &self,
        _context: &mod_api::RegistrationContext,
        event: &mut mod_api::ComponentTypesEvent<'_>,
    ) {
        if !event.types.iter().any(|known| known == COMPONENT) {
            event.types.push(COMPONENT.to_owned());
        }
    }

    /// A sneak right click on a container empties the held unit into it, all but
    /// one of each kind. Cancelled with `Fail` so a deposit never opens the chest,
    /// and so the unit's own (absent) use does not run instead.
    fn right_click_block(&self, host: HostSide, event: &mut RightClickBlockEvent<'_>) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        if !deposit(host, event.click) {
            return Flow::Continue;
        }
        *event.cancellation_result = mod_api::InteractionResult::Fail;
        Flow::Cancel
    }

    /// A sneak left click on a container takes back the kinds the held unit
    /// already holds, and cancels the dig so a withdraw never breaks the container.
    fn left_click_block(&self, host: &mut dyn ServerHost, event: &mut LeftClickBlockEvent) -> Flow {
        if withdraw(host, event.pos, event.face) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    /// Declares `/psu list|put|help`: the screen's insert/read half, for the slice.
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        for usage in [
            "psu list",
            "psu put <item:item> [count:integer]",
            "psu help",
        ] {
            event.commands.push(Command::new(usage));
        }
    }

    /// Runs a `/psu` usage for the player who typed it, whose held stack is in
    /// place. The dispatcher has already parsed and resolved the arguments, so
    /// this reads them typed instead of re-parsing a line.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let CommandSource::Player { player, .. } = *event.source else {
            return Flow::Continue;
        };
        let Some(action) = action_of(event.command, &*host) else {
            return Flow::Continue;
        };
        let lines = match action {
            Action::Help => HELP.iter().map(|line| (*line).to_owned()).collect(),
            Action::List => list_lines(host, player),
            Action::Put { item, count } => vec![put(host, player, &item, count)],
            Action::Unknown => return Flow::Continue,
        };
        for line in lines {
            let _ = player.tell(host, Text::literal(line));
        }
        Flow::Cancel
    }
}

/// Sneak + right-click a container: deposit all but one of each held kind.
/// Returns whether the gesture was the unit's, so the caller cancels the click.
fn deposit(host: &mut dyn ServerHost, click: &mod_api::BlockClick<'_>) -> bool {
    let Some(player) = host.player(click.player) else {
        return false;
    };
    if !player.is_secondary_use_active(&*host).unwrap_or(false) {
        return false;
    }
    let Some(stack) = click.stack else {
        return false;
    };
    let Some(tier) = tier_of(&stack.item().id(&*host)) else {
        return false;
    };
    let Some(level) = player.level(&*host) else {
        return false;
    };
    let side = direction_of(click.hit.face);
    // The clicked face's item handler, as upstream's `CapHelper.getItemHandler(world, pos, face)`.
    let Some(handler) = level.capability(host, &ITEMS, click.pos, Some(side)) else {
        return false;
    };
    // The click is ours from here: a chest does not open on a deposit.
    let before = read_contents(stack, tier);
    if before.is_empty() {
        let _ = player.action_bar(host, Text::literal("The storage unit is empty"));
        return true;
    }
    let mut contents = before.clone();
    let moved = host
        .transaction(|host, tx| {
            let mut moved = 0_u32;
            for (slot, entry) in before.deposit_plan() {
                let Some(key) = item_key(&*host, &entry.item) else {
                    continue;
                };
                let resource = ItemResource::of(&ItemStack::of(key));
                let put = handler.insert_any(host, tx, &resource, entry.count);
                moved += put;
                // Upstream keeps one of each kind behind, so the slot keeps the kind.
                contents.set(slot, Some(Entry::new(&entry.item, entry.count - put + 1)));
            }
            Ok::<u32, ()>(moved)
        })
        .unwrap_or(0);
    let written = contents == before || write_contents(host, player, click.hand, stack, &contents);
    let line = if moved == 0 {
        format!(
            "The container took nothing; the unit still holds {}",
            before.total()
        )
    } else {
        format!("Deposited {moved} items, keeping one of each kind")
    };
    let _ = player.action_bar(host, Text::literal(saving(line, written)));
    true
}

/// Sneak + left-click a container: withdraw the kinds the unit already holds.
/// Returns whether the gesture was the unit's, so the caller cancels the dig.
fn withdraw(host: &mut dyn ServerHost, pos: [i32; 3], face: [i32; 3]) -> bool {
    let Some(player) = host.players().acting_player() else {
        return false;
    };
    if !player.is_secondary_use_active(&*host).unwrap_or(false) {
        return false;
    }
    let Some(living) = host.living(player.entity()) else {
        return false;
    };
    let Some(stack) = living.hand_item(&*host, Hand::Main) else {
        return false;
    };
    let Some(tier) = tier_of(&stack.item().id(&*host)) else {
        return false;
    };
    let Some(level) = player.level(&*host) else {
        return false;
    };
    let side = direction_of(face);
    let Some(handler) = level.capability(host, &ITEMS, pos, Some(side)) else {
        return false;
    };
    // The click is ours: a withdraw never starts a dig.
    let before = read_contents(&stack, tier);
    let mut contents = before.clone();
    let mut refused = 0_u64;
    let size = handler.size(&mut *host);
    let moved = host
        .transaction(|host, tx| {
            let mut moved = 0_u64;
            for index in 0..size {
                let Some(resource) = handler.resource(host, index) else {
                    continue;
                };
                let here = resource.as_stack();
                let id = here.item().id(&*host);
                // Only a plain stack can be stored: an entry is `{id, count}`, and rebuilding a
                // damaged or component-bearing stack from it would hand back a plain one and
                // destroy the custom state for good (the reviewer's P1).
                let want = contents.withdrawable(
                    &id,
                    handler.amount(host, index),
                    here.damage(),
                    here.components().is_some(),
                );
                if want == 0 {
                    if here.damage() != 0 || here.components().is_some() {
                        refused += u64::from(handler.amount(host, index));
                    }
                    continue;
                }
                let got = handler.extract(host, tx, index, &resource, want);
                if got == 0 {
                    continue;
                }
                let leftover = contents.insert(&id, got);
                moved += u64::from(got - leftover);
                if leftover > 0 {
                    // Unreachable: `withdrawable` never asks for more than the slot's room. Kept
                    // so a container answering differently never loses an item.
                    handler.insert(host, tx, index, &resource, leftover);
                }
            }
            Ok::<u64, ()>(moved)
        })
        .unwrap_or(0);
    let written = contents == before || write_contents(host, player, Hand::Main, &stack, &contents);
    let line = if moved == 0 {
        if refused > 0 {
            format!(
                "Nothing here is in the unit; {refused} item(s) with damage or components stay in the container"
            )
        } else {
            "Nothing here is in the unit".to_owned()
        }
    } else if refused > 0 {
        format!(
            "Withdrew {moved} items; the unit holds {} of {} kinds ({refused} item(s) with damage or components stay)",
            contents.filled(),
            contents.len()
        )
    } else {
        format!(
            "Withdrew {moved} items; the unit holds {} of {} kinds",
            contents.filled(),
            contents.len()
        )
    };
    let _ = player.action_bar(host, Text::literal(saving(line, written)));
    true
}

/// Appends what a refused hand write means to the line the player sees: the
/// gesture moved items but the unit could not record them.
fn saving(line: String, written: bool) -> String {
    if written {
        line
    } else {
        format!("{line} (the unit could not save that)")
    }
}

/// The held unit's contents as chat lines.
fn list_lines(host: &mut dyn ServerHost, player: PlayerRef) -> Vec<String> {
    let Some(stack) = hand_unit(host, player) else {
        return vec!["Hold a Pocket Storage Unit first.".to_owned()];
    };
    let Some(tier) = tier_of(&stack.item().id(&*host)) else {
        return vec!["The held item is not a Pocket Storage Unit.".to_owned()];
    };
    let contents = read_contents(&stack, tier);
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
fn put(host: &mut dyn ServerHost, player: PlayerRef, item: &str, count: u32) -> String {
    if is_unit(item) {
        return "A storage unit cannot hold another storage unit.".to_owned();
    }
    let Some(key) = item_key(&*host, item) else {
        return format!("{item} is not an item this game has.");
    };
    let Some(stack) = hand_unit(host, player) else {
        return "Hold a Pocket Storage Unit first.".to_owned();
    };
    let Some(tier) = tier_of(&stack.item().id(&*host)) else {
        return "The held item is not a Pocket Storage Unit.".to_owned();
    };
    let mut contents = read_contents(&stack, tier);
    let space = contents.space_for(item).min(count);
    if space == 0 {
        return format!(
            "The unit is full: {} of {} kinds holding {} items.",
            contents.filled(),
            contents.len(),
            contents.total()
        );
    }
    let Some(wanted) = ItemStack::new(key, space) else {
        return format!("You carry no plain {item}.");
    };
    let resource = ItemResource::of(&wanted);
    // The player's own slots, as vanilla's `Inventory` numbers them; only a plain
    // stack of the named item can go in, which is what the command says.
    let slots = player.inventory_slots(&*host).unwrap_or_default();
    let taken = host
        .transaction(|host, tx| {
            let mut taken = 0_u32;
            for (slot, here) in &slots {
                if taken >= space {
                    break;
                }
                if here.item() != key || here.damage() != 0 || here.components().is_some() {
                    continue;
                }
                let Some(slot) = take_place_of(*slot) else {
                    continue;
                };
                let place = ItemPlace::PlayerSlot {
                    player: player.uuid(),
                    slot,
                };
                taken += place.extract(host, tx, &resource, space - taken);
            }
            Ok::<u32, ()>(taken)
        })
        .unwrap_or(0);
    if taken == 0 {
        return format!("You carry no plain {item}.");
    }
    let refused = contents.insert(item, taken);
    let written = write_contents(host, player, Hand::Main, &stack, &contents);
    let put = taken.saturating_sub(refused);
    saving(
        format!(
            "The unit holds {} items of {} kinds; {put} of {item} went in.",
            contents.total(),
            contents.filled()
        ),
        written,
    )
}

/// The stack in a player's main hand, or `None` when they hold none.
fn hand_unit(host: &mut dyn ServerHost, player: PlayerRef) -> Option<ItemStack> {
    host.living(player.entity())?.hand_item(&*host, Hand::Main)
}

/// Where `/psu put` may take from, as an [`ItemPlace::PlayerSlot`] number.
///
/// `PlayerRef::inventory_slots` and `ItemPlace::PlayerSlot` number a player's
/// inventory differently: the first counts the armour as 36..=39 and the off hand
/// as 40, while the second wants 100..=103 for the armour and `-106` for the off
/// hand. The main inventory (0..=35) has the same number in both. This command
/// takes from the main inventory and the off hand — what the retired
/// `native_take_items` reached (the 36 main slots and the off hand, never the
/// armour) — so the armour and body slots are not places it takes from.
fn take_place_of(slot: u32) -> Option<i32> {
    match slot {
        0..=35 => i32::try_from(slot).ok(),
        40 => Some(-106),
        _ => None,
    }
}

/// The unit's contents in its stack component, an empty unit of `tier` when it
/// carries none.
fn read_contents(stack: &ItemStack, tier: Tier) -> Contents {
    let entries: Vec<(usize, Entry)> = stack
        .get(&ContentsComponent)
        .unwrap_or_default()
        .iter()
        .filter_map(entry_from_tag)
        .collect();
    Contents::from_entries(tier, entries)
}

/// Writes the contents into the hand's stack; an empty unit carries no component
/// at all (so it stacks as a new one does). The hand write is the game's own, so
/// the player's client is told as it is for any equip. `false` means the game
/// refused the write, and the caller says so in its line: nothing was saved.
fn write_contents(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    hand: Hand,
    stack: &ItemStack,
    contents: &Contents,
) -> bool {
    let mut stack = stack.clone();
    if contents.is_empty() {
        stack.remove(&ContentsComponent);
    } else if stack
        .set(&ContentsComponent, contents_tag(contents))
        .is_err()
    {
        return false;
    }
    let Some(living) = host.living(player.entity()) else {
        return false;
    };
    matches!(living.set_hand_item(host, hand, Some(stack)), Ok(true))
}

/// One tag per filled slot, in slot order.
fn contents_tag(contents: &Contents) -> Vec<Tag> {
    contents
        .slots()
        .map(|(slot, entry)| {
            Tag::compound([
                (KEY_SLOT, Tag::Int(i32::try_from(slot).unwrap_or(i32::MAX))),
                (KEY_ITEM, Tag::String(entry.item.as_str().into())),
                (
                    KEY_COUNT,
                    Tag::Int(i32::try_from(entry.count).unwrap_or(i32::MAX)),
                ),
            ])
        })
        .collect()
}

/// One saved slot, or `None` when it is not a complete entry.
fn entry_from_tag(tag: &Tag) -> Option<(usize, Entry)> {
    let slot = match tag.get(KEY_SLOT)? {
        Tag::Int(slot) => usize::try_from(*slot).ok()?,
        _ => return None,
    };
    let item = match tag.get(KEY_ITEM)? {
        Tag::String(id) => id.to_string_lossy(),
        _ => return None,
    };
    let count = match tag.get(KEY_COUNT)? {
        Tag::Int(count) => u32::try_from(*count).ok()?,
        _ => return None,
    };
    Some((slot, Entry::new(&item, count)))
}

/// The runtime action a parsed `/psu` invocation asks for, or `None` when it is
/// not one of the port's usages.
fn action_of(invocation: &CommandInvocation, host: &dyn mod_api::Host) -> Option<Action> {
    match invocation.literals.get(1).map(String::as_str) {
        Some("list") => Some(Action::List),
        Some("help") | None => Some(Action::Help),
        Some("put") => {
            let item = match invocation.argument("item")? {
                CommandArgumentValue::Item(stack) => stack.item().id(host),
                _ => return None,
            };
            let count = match invocation.argument("count") {
                Some(CommandArgumentValue::Integer(count)) if *count > 0 => {
                    u32::try_from(*count).unwrap_or(u32::MAX)
                }
                Some(_) => return None,
                None => 1,
            };
            Some(Action::Put { item, count })
        }
        Some(_) => None,
    }
}

/// The process-local key of `id`, or `None` for an id this game does not have.
fn item_key(host: &dyn mod_api::Host, id: &str) -> Option<ItemKey> {
    let id = Identifier::parse(id).ok()?;
    host.item_key(&id)
}

/// A clicked face (the hook's direction vector) as vanilla's `Direction`:
/// `Direction::ALL` is `[down, up, north, south, west, east]`.
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

#[cfg(test)]
mod tests {
    use super::take_place_of;

    #[test]
    fn only_the_main_inventory_and_the_off_hand_are_places_put_takes_from() {
        assert_eq!(take_place_of(0), Some(0));
        assert_eq!(take_place_of(35), Some(35));
        assert_eq!(
            take_place_of(40),
            Some(-106),
            "the off hand is `-106` in `ItemPlace::PlayerSlot`"
        );
        assert_eq!(
            take_place_of(36),
            None,
            "feet: the armour slots are not places `/psu put` takes from"
        );
        assert_eq!(take_place_of(41), None, "body");
        assert_eq!(take_place_of(42), None, "saddle");
    }
}
