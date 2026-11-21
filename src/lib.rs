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
    self, Command, CommandArgumentValue, CommandInvocation, CommandSource, Component, CraftingGrid,
    CraftingRecipeCodec, CraftingRecipeKind, Direction, Flow, Hand, Hooks, HostSide,
    HostTransaction, ITEMS, Identifier, ItemKey, ItemPlace, ItemResource, ItemStack,
    LeftClickBlockEvent, LevelRef, PlayerRef, RegisterCommandsEvent, Registrar,
    RegistrationContext, RightClickBlockEvent, RunModCommandEvent, ServerHost, Tag,
    TagsUpdatedEvent, Text,
};

use crate::runtime::{
    Action, COMPONENT, Contents, Entry, Tier, container_allows_transfer, is_unit, tier_of,
};

/// What the unit says when the clicked container is locked against it.
const LOCKED_MESSAGE: &str = "That container is locked";

/// Whether the container at `pos` carries the vanilla lock (its item predicate, the rule the
/// container's own open path asks). A block with no block entity, or an unloaded one, carries
/// no lock here.
fn container_locked(host: &dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> bool {
    host.block_entity(level, pos)
        .is_some_and(|entity| entity.lock(host).is_some())
}

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
    /// Before anything loads: register the upgrade crafting kind and resolve the item
    /// ids it may name, so the three `pocketstorage:upgrade` recipe files are read by
    /// this mod rather than refused as an unknown recipe type.
    fn register(&self, registrar: &mut dyn Registrar, _context: &RegistrationContext) {
        let keys = UPGRADE_ITEMS
            .iter()
            .map(|id| (*id, registrar.item_key(id)))
            .collect();
        registrar.register_crafting_recipe_serializer(UPGRADE_KIND, UpgradeKind { keys });
    }

    /// The merged tags were (re)published: the upgrade recipes' tag cells ask them at
    /// craft time, when no host is available, so keep their members here.
    fn tags_updated(&self, host: HostSide, _event: &mut TagsUpdatedEvent) {
        match host {
            HostSide::Server(host) => refresh_upgrade_tags(host),
            HostSide::Client(host) => refresh_upgrade_tags(host),
        }
    }

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
    if !container_allows_transfer(container_locked(&*host, level, click.pos)) {
        let _ = player.action_bar(host, Text::literal(LOCKED_MESSAGE));
        return true;
    }
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
    if !container_allows_transfer(container_locked(&*host, level, pos)) {
        let _ = player.action_bar(host, Text::literal(LOCKED_MESSAGE));
        return true;
    }
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

/// The upgrade family: the three `pocketstorage:upgrade` recipes, registered as a
/// crafting kind because the pin has no fixed recipe type for them
/// (`CraftingRecipeKind`, the same registration vanilla's own special recipes take).
///
/// The recipe is upstream's: a shaped 3×3 whose centre is the previous tier's unit,
/// which rides its `pocketstorage:contents` onto the next tier's unit. Matching is
/// pure and host-free, so the item keys an upgrade can name are resolved once at
/// registration and the tags are cached whenever the game publishes them.
const UPGRADE_KIND: &str = "upgrade";

/// The item ids an upgrade recipe may name, resolved to keys at registration. A
/// cell naming anything else refuses the recipe at load: the mod's three recipes
/// are these and nothing else.
const UPGRADE_ITEMS: [&str; 5] = [
    "pocketstorage:psu_1",
    "pocketstorage:psu_2",
    "pocketstorage:psu_3",
    "pocketstorage:psu_4",
    "minecraft:piston",
];

/// The item tags the three upgrade recipes name, cached from `tags_updated`.
const UPGRADE_TAGS: [&str; 4] = [
    "c:ingots/gold",
    "c:gems/diamond",
    "c:chests",
    "c:nether_stars",
];

/// The members of [`UPGRADE_TAGS`], as the last tag load published them. A craft-time
/// match takes no host, so this is the only way a tag cell can be tested there.
static UPGRADE_TAG_MEMBERS: std::sync::RwLock<Vec<(&'static str, Vec<ItemKey>)>> =
    std::sync::RwLock::new(Vec::new());

/// Whether `key` is in `tag` as the last tag load published it.
fn in_upgrade_tag(tag: &str, key: ItemKey) -> bool {
    let members = UPGRADE_TAG_MEMBERS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    members
        .iter()
        .any(|(name, keys)| *name == tag && keys.contains(&key))
}

/// Re-reads the upgrade recipes' tags from the game's merged tags.
fn refresh_upgrade_tags(host: &dyn mod_api::Host) {
    let mut loaded = Vec::with_capacity(UPGRADE_TAGS.len());
    for tag in UPGRADE_TAGS {
        let Some(id) = Identifier::parse(tag).ok() else {
            continue;
        };
        let Some(key) = host.tags().key(&id) else {
            continue;
        };
        let Some(members) = host.tags().items(key) else {
            continue;
        };
        loaded.push((tag, members));
    }
    *UPGRADE_TAG_MEMBERS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = loaded;
}

/// The upgrade recipe as it crosses the API boundary: plain data, so the encoding
/// is a `Wire` value and not a game type.
#[derive(Clone, Debug, PartialEq, Eq, mod_api::Wire)]
struct UpgradeValue {
    /// The nine cells, row-major.
    cells: Vec<UpgradeCell>,
    /// The item the recipe makes.
    result: String,
}

/// One cell of [`UpgradeValue`].
#[derive(Clone, Debug, PartialEq, Eq, mod_api::Wire)]
enum UpgradeCell {
    /// Nothing in the cell.
    Empty,
    /// An item id.
    Stack(String),
    /// An item tag, without its `#`.
    Tagged(String),
}

impl UpgradeValue {
    /// The wire value of a decoded recipe.
    fn of(upgrade: &runtime::Upgrade) -> Self {
        let cells = upgrade
            .cells()
            .iter()
            .map(|cell| match cell {
                runtime::PatternCell::Empty => UpgradeCell::Empty,
                runtime::PatternCell::Stack(id) => UpgradeCell::Stack(id.clone()),
                runtime::PatternCell::Tagged(tag) => UpgradeCell::Tagged(tag.clone()),
            })
            .collect();
        Self {
            cells,
            result: upgrade.result().to_owned(),
        }
    }

    /// The pure recipe this value carries, or `None` when it is malformed (which the
    /// decoder refuses, so this only happens to a hand-made value).
    fn upgrade(&self) -> Option<runtime::Upgrade> {
        let mut cells = Vec::with_capacity(self.cells.len());
        for cell in &self.cells {
            cells.push(match cell {
                UpgradeCell::Empty => runtime::PatternCell::Empty,
                UpgradeCell::Stack(id) => runtime::PatternCell::Stack(id.clone()),
                UpgradeCell::Tagged(tag) => runtime::PatternCell::Tagged(tag.clone()),
            });
        }
        let cells: [runtime::PatternCell; 9] = cells.try_into().ok()?;
        runtime::Upgrade::new(cells, &self.result).ok()
    }
}

/// The registered `pocketstorage:upgrade` kind, with the item keys it may name.
struct UpgradeKind {
    /// Every id in [`UPGRADE_ITEMS`], in order.
    keys: Vec<(&'static str, ItemKey)>,
}

impl UpgradeKind {
    /// The key of one of the item ids an upgrade may name.
    fn key(&self, id: &str) -> Option<ItemKey> {
        self.keys
            .iter()
            .find(|(name, _)| *name == id)
            .map(|(_, key)| *key)
    }
}

impl CraftingRecipeKind for UpgradeKind {
    type Value = UpgradeValue;

    fn codec(&self) -> Option<CraftingRecipeCodec<Self::Value>> {
        Some(CraftingRecipeCodec::wire())
    }

    fn decode(&self, json: &str) -> Result<Self::Value, String> {
        decode_upgrade(json).map(|upgrade| UpgradeValue::of(&upgrade))
    }

    fn write(&self, value: &Self::Value) -> Result<String, String> {
        let Some(upgrade) = value.upgrade() else {
            return Err("the upgrade value is not a recipe".to_owned());
        };
        Ok(write_upgrade(&upgrade))
    }

    fn matches(&self, value: &Self::Value, grid: CraftingGrid) -> bool {
        if (grid.width, grid.height) != (3, 3) || grid.cells.len() != 9 {
            return false;
        }
        let Some(upgrade) = value.upgrade() else {
            return false;
        };
        let mut keys: [Option<ItemKey>; 9] = [None; 9];
        for (slot, cell) in grid.cells.iter().enumerate() {
            keys[slot] = cell.as_ref().map(ItemStack::item);
        }
        upgrade.matches(&keys, |id| self.key(id), in_upgrade_tag)
    }

    fn assemble(&self, value: &Self::Value, grid: CraftingGrid) -> Option<ItemStack> {
        if (grid.width, grid.height) != (3, 3) {
            return None;
        }
        let upgrade = value.upgrade()?;
        let key = self.key(upgrade.result())?;
        let unit = grid.cells.get(upgrade.unit_cell())?.as_ref()?;
        let mut out = ItemStack::new(key, 1)?;
        // Upstream's `UpgradeRecipe` builds the next tier's unit and copies the old
        // unit's own data onto it: what the unit held rides along, nothing else does.
        out.copy_component_from(unit, COMPONENT);
        Some(out)
    }
}

/// Reads a `pocketstorage:upgrade` file: the jar's shaped pattern, with 26.3's
/// ingredients. The engine hands this the object with its `type` removed, so an
/// absent type is accepted and a wrong one refused.
pub(crate) fn decode_upgrade(json: &str) -> Result<runtime::Upgrade, String> {
    use mod_api::serde_json::Value;

    let value: Value = mod_api::serde_json::from_str(json).map_err(|error| error.to_string())?;
    if let Some(kind) = value.get("type").and_then(Value::as_str)
        && kind != format!("pocketstorage:{UPGRADE_KIND}")
    {
        return Err(format!("{kind} is not a pocketstorage upgrade"));
    }
    let rows = value
        .get("pattern")
        .and_then(Value::as_array)
        .ok_or("an upgrade needs a pattern")?;
    if rows.len() != 3 {
        return Err(format!("an upgrade pattern is 3 rows, not {}", rows.len()));
    }
    let ingredients = value
        .get("key")
        .and_then(Value::as_object)
        .ok_or("an upgrade needs a key")?;
    let mut cells = Vec::with_capacity(9);
    for row in rows {
        let row = row.as_str().ok_or("a pattern row is a string")?;
        if row.chars().count() != 3 {
            return Err(format!("a pattern row is 3 letters, not {row:?}"));
        }
        for letter in row.chars() {
            if letter == ' ' {
                cells.push(runtime::PatternCell::Empty);
                continue;
            }
            let written = ingredients
                .get(&letter.to_string())
                .and_then(Value::as_str)
                .ok_or_else(|| format!("the pattern names '{letter}', the key does not"))?;
            cells.push(runtime::PatternCell::from_ingredient(written)?);
        }
    }
    let cells: [runtime::PatternCell; 9] = cells
        .try_into()
        .map_err(|_| "an upgrade pattern is 3×3".to_owned())?;
    let result = value
        .get("result")
        .and_then(|result| {
            result
                .get("id")
                .and_then(Value::as_str)
                .or_else(|| result.as_str())
        })
        .ok_or("an upgrade needs a result item")?;
    runtime::Upgrade::new(cells, result)
}

/// Writes an upgrade back as the recipe file it came from, for client sync.
fn write_upgrade(upgrade: &runtime::Upgrade) -> String {
    use mod_api::serde_json::{Map, Value};

    let mut letters: Vec<(runtime::PatternCell, char)> = Vec::new();
    let mut rows: Vec<Value> = Vec::with_capacity(3);
    for row in 0..3 {
        let mut line = String::with_capacity(3);
        for column in 0..3 {
            let cell = &upgrade.cells()[row * 3 + column];
            if matches!(cell, runtime::PatternCell::Empty) {
                line.push(' ');
                continue;
            }
            let letter = match letters.iter().find(|(known, _)| known == cell) {
                Some((_, letter)) => *letter,
                None => {
                    let letter = char::from(b'A' + u8::try_from(letters.len()).unwrap_or(0));
                    letters.push((cell.clone(), letter));
                    letter
                }
            };
            line.push(letter);
        }
        rows.push(Value::String(line));
    }
    let mut key = Map::new();
    for (cell, letter) in &letters {
        if let Some(ingredient) = cell.ingredient() {
            key.insert(letter.to_string(), Value::String(ingredient));
        }
    }
    let mut result = Map::new();
    result.insert("id".to_owned(), Value::String(upgrade.result().to_owned()));
    result.insert("count".to_owned(), Value::from(1));
    let object = Map::from_iter([
        (
            "type".to_owned(),
            Value::String(format!("pocketstorage:{UPGRADE_KIND}")),
        ),
        ("pattern".to_owned(), Value::Array(rows)),
        ("key".to_owned(), Value::Object(key)),
        ("result".to_owned(), Value::Object(result)),
    ]);
    Value::Object(object).to_string()
}

#[cfg(test)]
mod tests {
    use super::{decode_upgrade, take_place_of, write_upgrade};

    #[test]
    fn an_upgrade_value_round_trips_through_the_wire_form() {
        // The kind's value crosses the API boundary as `UpgradeValue`, so the write
        // and decode halves must agree; a recipe the client cannot read back would
        // be a silent gap in the sync.
        let file = include_str!("../data/pocketstorage/recipe/tier2.json");
        let upgrade = decode_upgrade(file).expect("the shipped tier-2 recipe");
        let value = super::UpgradeValue::of(&upgrade);
        let written = write_upgrade(&value.upgrade().expect("a runtime recipe"));
        let again = decode_upgrade(&written).expect("the written recipe decodes");
        assert_eq!(again, upgrade);
    }

    #[test]
    fn a_recipe_naming_something_the_kind_cannot_resolve_is_refused() {
        // Upstream's tier-2 recipe with a different corner item: the pattern still
        // decodes, but the item is not one the kind resolved, so nothing may match
        // it. This is the data-side guard behind `UPGRADE_ITEMS`.
        let file = include_str!("../data/pocketstorage/recipe/tier2.json");
        assert!(decode_upgrade(file).is_ok());
        let wrong = file.replace("#c:ingots/gold", "minecraft:emerald");
        assert!(
            decode_upgrade(&wrong).is_ok(),
            "an emerald pattern is still a well-formed upgrade recipe"
        );
        // ... but its corners resolve to no key, which the match path treats as a
        // refusal (the unit tests of `Upgrade::matches` cover that).
    }

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
