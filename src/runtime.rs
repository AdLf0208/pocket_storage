// Pocket Storage's pure rules: the tier table and one unit's slots.
//
// The numbers are the mod's own (`PSUTier.java` 1.2.5, repeated in its
// `README.MD`): tier 1 holds 8 kinds of item with up to 255 of each, tier 2
// 16 × 4095, tier 3 32 × 65535, tier 4 64 × 1048575. The insert, extract,
// deposit and withdraw arithmetic is upstream's `PSUItemHandler`: a matching
// slot tops up to the capacity, else the first empty slot takes the stack,
// else the stack is refused; a deposit sends all but one of each kind so the
// slot keeps its filter; a withdraw only moves a container's stack when the
// unit already holds that item.
//
// Nothing here touches the game: the glue reads the stack component, calls
// these rules and writes the result back.

/// The registered item component that holds a unit's contents.
pub const COMPONENT: &str = "pocketstorage:contents";

/// One of the four tiers the mod ships: its item, its slots and its capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tier {
    /// The tier's item id, `pocketstorage:psu_<n>`.
    pub item: &'static str,
    /// How many different items the unit holds.
    pub slots: usize,
    /// The most of one item one slot holds (`PSUTier.capacity`).
    pub capacity: u32,
}

/// Tier 1: 8 kinds of item, up to 255 of each.
pub const TIER1: Tier = Tier {
    item: "pocketstorage:psu_1",
    slots: 8,
    capacity: 255,
};

/// Tier 2: 16 kinds of item, up to 4,095 of each.
pub const TIER2: Tier = Tier {
    item: "pocketstorage:psu_2",
    slots: 16,
    capacity: 4095,
};

/// Tier 3: 32 kinds of item, up to 65,535 of each.
pub const TIER3: Tier = Tier {
    item: "pocketstorage:psu_3",
    slots: 32,
    capacity: 65535,
};

/// Tier 4: 64 kinds of item, up to 1,048,575 of each.
pub const TIER4: Tier = Tier {
    item: "pocketstorage:psu_4",
    slots: 64,
    capacity: 1048575,
};

/// The four tiers, lowest first.
pub const TIERS: [Tier; 4] = [TIER1, TIER2, TIER3, TIER4];

/// The tier an item id is, or `None` when it is not a storage unit.
#[must_use]
pub fn tier_of(item: &str) -> Option<Tier> {
    TIERS.iter().copied().find(|tier| tier.item == item)
}

/// Whether an item id is one of the mod's storage units.
#[must_use]
/// Whether a deposit or withdraw may touch a container. The unit's transfers go through the
/// block's automation capability, which bypasses the player-facing lock, so a container that
/// carries one is refused: the player would otherwise reach inside a chest they cannot open.
pub fn container_allows_transfer(locked: bool) -> bool {
    !locked
}

pub fn is_unit(item: &str) -> bool {
    tier_of(item).is_some()
}

/// One filled slot: an item id and how many of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The item id, `minecraft:diamond`.
    pub item: String,
    /// How many of it, at least 1 and at most the tier's capacity.
    pub count: u32,
}

impl Entry {
    /// An entry for `count` of `item`.
    #[must_use]
    pub fn new(item: &str, count: u32) -> Self {
        Self {
            item: item.to_owned(),
            count,
        }
    }
}

/// A unit's contents: exactly the tier's slots, in slot order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contents {
    tier: Tier,
    slots: Vec<Option<Entry>>,
}

impl Contents {
    /// An empty unit of `tier`.
    #[must_use]
    pub fn new(tier: Tier) -> Self {
        Self {
            tier,
            slots: vec![None; tier.slots],
        }
    }

    /// The contents a saved component describes: entries past the tier's slots
    /// and entries with no count are dropped, a saved count above the capacity
    /// is clamped, and the first entry for a slot wins.
    #[must_use]
    pub fn from_entries(tier: Tier, entries: impl IntoIterator<Item = (usize, Entry)>) -> Self {
        let mut contents = Self::new(tier);
        for (slot, entry) in entries {
            if entry.count == 0 || entry.item.is_empty() {
                continue;
            }
            let Some(target) = contents.slots.get_mut(slot) else {
                continue;
            };
            if target.is_some() {
                continue;
            }
            *target = Some(Entry {
                item: entry.item,
                count: entry.count.min(tier.capacity),
            });
        }
        contents
    }

    /// How many slots the tier has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether the unit holds nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }

    /// The entry in `slot`, if it is filled.
    #[allow(
        dead_code,
        reason = "the standalone lib and its tests use it; the injected module does not"
    )]
    #[must_use]
    pub fn get(&self, slot: usize) -> Option<&Entry> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    /// The filled slots, in slot order, with their numbers.
    pub fn slots(&self) -> impl Iterator<Item = (usize, &Entry)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(slot, entry)| entry.as_ref().map(|entry| (slot, entry)))
    }

    /// How many kinds the unit holds.
    #[must_use]
    pub fn filled(&self) -> usize {
        self.slots().count()
    }

    /// How many items the unit holds in all.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.slots().map(|(_, entry)| u64::from(entry.count)).sum()
    }

    /// Whether the unit holds `item` at all.
    #[must_use]
    pub fn has(&self, item: &str) -> bool {
        self.matching(item).is_some()
    }

    /// The slot holding `item`, if any.
    #[must_use]
    pub fn matching(&self, item: &str) -> Option<usize> {
        self.slots()
            .find(|(_, entry)| entry.item == item)
            .map(|(slot, _)| slot)
    }

    /// Puts `count` of `item` in, as upstream's `insertItemSlotless`: the first
    /// slot holding the item tops up to the capacity, else the first empty slot
    /// takes what fits, else the unit refuses it. Returns what did not fit.
    pub fn insert(&mut self, item: &str, count: u32) -> u32 {
        if count == 0 || item.is_empty() {
            return count;
        }
        let capacity = self.tier.capacity;
        if let Some(slot) = self.matching(item) {
            let at = self.slots[slot]
                .as_mut()
                .expect("a matching slot holds an entry");
            let taken = capacity.saturating_sub(at.count).min(count);
            at.count += taken;
            return count - taken;
        }
        let Some(slot) = self.slots.iter().position(Option::is_none) else {
            return count;
        };
        let taken = count.min(capacity);
        self.slots[slot] = Some(Entry {
            item: item.to_owned(),
            count: taken,
        });
        count - taken
    }

    /// The room left in `item`'s slot: 0 when the unit does not hold the item.
    #[must_use]
    pub fn room(&self, item: &str) -> u32 {
        let Some(slot) = self.matching(item) else {
            return 0;
        };
        let held = self.slots[slot].as_ref().map_or(0, |entry| entry.count);
        self.tier.capacity.saturating_sub(held)
    }

    /// Takes up to `max` of `slot` out; an emptied slot becomes empty again.
    #[allow(
        dead_code,
        reason = "the standalone lib and its tests use it; the injected module does not"
    )]
    pub fn extract(&mut self, slot: usize, max: u32) -> Option<Entry> {
        let entry = self.slots.get(slot)?.as_ref()?;
        let taken = entry.count.min(max);
        if taken == 0 {
            return None;
        }
        let item = entry.item.clone();
        let left = entry.count - taken;
        self.slots[slot] = (left > 0).then(|| Entry {
            item: item.clone(),
            count: left,
        });
        Some(Entry { item, count: taken })
    }

    /// What a deposit sends to a container: all but one of each held kind, so
    /// the slot keeps the kind as a filter (upstream's `giveItems`).
    #[must_use]
    pub fn deposit_plan(&self) -> Vec<(usize, Entry)> {
        self.slots()
            .filter(|(_, entry)| entry.count > 1)
            .map(|(slot, entry)| {
                (
                    slot,
                    Entry {
                        item: entry.item.clone(),
                        count: entry.count - 1,
                    },
                )
            })
            .collect()
    }

    /// How much of a container's stack of `item` a withdraw takes: only what
    /// the unit already holds, and only up to its slot's room.
    #[must_use]
    pub fn withdraw_amount(&self, item: &str, available: u32) -> u32 {
        self.room(item).min(available)
    }

    /// What a withdraw may take from a container slot whose stack is `item`,
    /// `count` strong, at `damage` and possibly carrying components.
    ///
    /// A saved slot is one `{id, count}` entry, so a stack that is more than its
    /// item id — a damaged tool, a renamed or enchanted item, anything with a
    /// component — cannot be stored: rebuilding it from the entry would hand the
    /// player a plain one and destroy the custom state for good. Those stacks are
    /// refused whole and stay where they are; the slice is plain-items-only, which
    /// is what the command path already says.
    #[must_use]
    pub fn withdrawable(&self, item: &str, count: u32, damage: u32, has_extra: bool) -> u32 {
        if damage != 0 || has_extra {
            return 0;
        }
        self.withdraw_amount(item, count)
    }

    /// The most of `item` the unit could still take: the room in its slot, the
    /// capacity of an empty slot for a kind it does not hold yet, else 0.
    #[must_use]
    pub fn space_for(&self, item: &str) -> u32 {
        if self.has(item) {
            return self.room(item);
        }
        if self.filled() < self.slots.len() {
            return self.tier.capacity;
        }
        0
    }

    /// Writes one slot back, or empties it: the glue's path for a deposit's
    /// remainder. A count past the capacity is clamped and a zero count empties.
    pub fn set(&mut self, slot: usize, entry: Option<Entry>) {
        let capacity = self.tier.capacity;
        let Some(target) = self.slots.get_mut(slot) else {
            return;
        };
        *target = entry.filter(|entry| entry.count > 0).map(|entry| Entry {
            item: entry.item,
            count: entry.count.min(capacity),
        });
    }
}

/// One cell of an upgrade recipe's 3×3 pattern: nothing, an item, or an item tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternCell {
    /// The cell must be empty.
    Empty,
    /// The cell must hold this item id.
    Stack(String),
    /// The cell must hold an item of this tag (a `#c:chests`-style name, no `#`).
    Tagged(String),
}

impl PatternCell {
    /// A cell from a recipe file's ingredient: `#c:chests` is a tag, anything else
    /// an item id. Both need a namespace, as 26.3's own data does.
    pub fn from_ingredient(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let (tag, name) = match text.strip_prefix('#') {
            Some(name) => (true, name),
            None => (false, text),
        };
        match name.split_once(':') {
            Some((namespace, path)) if !namespace.is_empty() && !path.is_empty() => Ok(if tag {
                Self::Tagged(name.to_owned())
            } else {
                Self::Stack(name.to_owned())
            }),
            _ => Err(format!(
                "{text} is not a namespaced {}",
                if tag { "tag" } else { "item id" }
            )),
        }
    }

    /// The ingredient as the recipe file writes it: `#` for a tag.
    #[must_use]
    pub fn ingredient(&self) -> Option<String> {
        match self {
            Self::Empty => None,
            Self::Stack(id) => Some(id.clone()),
            Self::Tagged(tag) => Some(format!("#{tag}")),
        }
    }
}

/// A `pocketstorage:upgrade` recipe: a 3×3 shaped pattern with exactly one cell
/// naming the unit it upgrades, whose result is that unit's next tier. The unit
/// keeps what it holds (the ingredient's `pocketstorage:contents` rides onto the
/// result); that part is the glue's, and the rule here is which grid it accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upgrade {
    cells: [PatternCell; 9],
    result: String,
    unit_cell: usize,
}

impl Upgrade {
    /// The recipe the pattern declares, or the reason it is not an upgrade.
    ///
    /// Upstream's `UpgradeRecipe` is a shaped recipe whose one unit ingredient is a
    /// tier and whose result is the next: exactly one cell must name a unit, that
    /// unit must have a next tier, and the result must be that tier's item.
    pub fn new(cells: [PatternCell; 9], result: &str) -> Result<Self, String> {
        let mut unit = None;
        for (index, cell) in cells.iter().enumerate() {
            let PatternCell::Stack(id) = cell else {
                continue;
            };
            if !is_unit(id) {
                continue;
            }
            if let Some(previous) = unit {
                return Err(format!("two units in one upgrade ({previous} and {index})"));
            }
            unit = Some(index);
        }
        let Some(unit_cell) = unit else {
            return Err("an upgrade recipe names no storage unit".to_owned());
        };
        let PatternCell::Stack(unit) = &cells[unit_cell] else {
            unreachable!("the unit cell was found as an item");
        };
        let tier = tier_of(unit).ok_or_else(|| format!("{unit} is not a storage unit"))?;
        let next = TIERS
            .iter()
            .position(|known| known.item == tier.item)
            .and_then(|index| TIERS.get(index + 1))
            .ok_or_else(|| format!("{unit} is the highest tier"))?;
        if next.item != result {
            return Err(format!(
                "an upgrade of {unit} makes {}, not {result}",
                next.item
            ));
        }
        Ok(Self {
            cells,
            result: result.to_owned(),
            unit_cell,
        })
    }

    /// The nine cells, row-major.
    #[must_use]
    pub fn cells(&self) -> &[PatternCell; 9] {
        &self.cells
    }

    /// The item the recipe makes.
    #[must_use]
    pub fn result(&self) -> &str {
        &self.result
    }

    /// The cell the unit ingredient is in.
    #[must_use]
    pub fn unit_cell(&self) -> usize {
        self.unit_cell
    }

    /// The unit item the recipe upgrades.
    #[must_use]
    pub fn unit(&self) -> &str {
        match &self.cells[self.unit_cell] {
            PatternCell::Stack(id) => id,
            _ => unreachable!("a recipe is built with a unit in its unit cell"),
        }
    }

    /// Whether `grid` is this recipe's pattern. The caller resolves an item id to
    /// the key the grid holds (`resolve`) and answers tag membership (`in_tag`),
    /// because a craft-time match has no host to ask the registries.
    #[must_use]
    pub fn matches<Key: Copy + Eq>(
        &self,
        grid: &[Option<Key>; 9],
        resolve: impl Fn(&str) -> Option<Key>,
        in_tag: impl Fn(&str, Key) -> bool,
    ) -> bool {
        self.cells
            .iter()
            .zip(grid)
            .all(|(cell, here)| match (cell, here) {
                (PatternCell::Empty, None) => true,
                (PatternCell::Empty, Some(_)) => false,
                (PatternCell::Stack(id), Some(key)) => {
                    resolve(id).is_some_and(|wanted| wanted == *key)
                }
                (PatternCell::Stack(_), None) => false,
                (PatternCell::Tagged(tag), Some(key)) => in_tag(tag, *key),
                (PatternCell::Tagged(_), None) => false,
            })
    }
}

/// The command a line asks for (`/psu …`), as the mod's command fallback for
/// the screen this slice does not carry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Put `count` of `item` from the player's inventory into the held unit.
    Put { item: String, count: u32 },
    /// List the held unit's contents.
    List,
    /// The usage text.
    Help,
    /// Another mod's command, or one this port does not have.
    Unknown,
}

/// The command's root word.
pub const ROOT: &str = "psu";

/// Reads one command line, with or without the leading slash the player types.
#[must_use]
pub fn parse(line: &str) -> Action {
    let line = line.trim().strip_prefix('/').unwrap_or(line.trim());
    let mut words = line.split_whitespace();
    if words.next() != Some(ROOT) {
        return Action::Unknown;
    }
    match words.next() {
        Some("list") => Action::List,
        Some("put") => {
            let Some(item) = words.next() else {
                return Action::Unknown;
            };
            let count = match words.next() {
                Some(count) => match count.parse::<u32>() {
                    Ok(count) if count > 0 => count,
                    _ => return Action::Unknown,
                },
                None => 1,
            };
            if words.next().is_some() {
                return Action::Unknown;
            }
            Action::Put {
                item: item.to_owned(),
                count,
            }
        }
        Some("help") | None => Action::Help,
        Some(_) => Action::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contents(tier: Tier, entries: &[(usize, &str, u32)]) -> Contents {
        Contents::from_entries(
            tier,
            entries
                .iter()
                .map(|(slot, item, count)| (*slot, Entry::new(item, *count))),
        )
    }

    #[test]
    fn a_locked_container_refuses_the_transfer_a_player_could_not_make() {
        // The deposit and the withdraw use the sided automation capability, which does not
        // ask the lock; a locked chest is therefore refused explicitly, so the unit cannot
        // take from or put into a container its owner locked.
        assert!(container_allows_transfer(false));
        assert!(!container_allows_transfer(true));
    }

    #[test]
    fn the_four_tiers_are_the_jars_numbers() {
        assert_eq!(
            (TIER1.slots, TIER1.capacity),
            (8, 255),
            "tier 1 is 8 slots of 255"
        );
        assert_eq!(
            (TIER2.slots, TIER2.capacity),
            (16, 4095),
            "tier 2 is 16 slots of 4095"
        );
        assert_eq!(
            (TIER3.slots, TIER3.capacity),
            (32, 65535),
            "tier 3 is 32 slots of 65535"
        );
        assert_eq!(
            (TIER4.slots, TIER4.capacity),
            (64, 1048575),
            "tier 4 is 64 slots of 1048575"
        );
    }

    #[test]
    fn a_tier_is_found_by_its_item_id_and_nothing_else() {
        assert_eq!(tier_of("pocketstorage:psu_1"), Some(TIER1));
        assert_eq!(tier_of("pocketstorage:psu_4"), Some(TIER4));
        assert_eq!(tier_of("pocketstorage:psu_5"), None);
        assert_eq!(tier_of("minecraft:chest"), None);
        assert!(is_unit("pocketstorage:psu_3"));
        assert!(!is_unit("pocketstorage:contents"));
    }

    #[test]
    fn a_slot_takes_its_capacity_and_then_refuses_more() {
        let mut unit = Contents::new(TIER1);
        assert_eq!(unit.insert("minecraft:cobblestone", 255), 0);
        assert_eq!(unit.get(0), Some(&Entry::new("minecraft:cobblestone", 255)));
        assert_eq!(
            unit.insert("minecraft:cobblestone", 1),
            1,
            "a full slot refuses the rest"
        );
        assert_eq!(unit.get(0), Some(&Entry::new("minecraft:cobblestone", 255)));
        assert_eq!(unit.len(), 8);
    }

    #[test]
    fn a_second_kind_takes_the_next_slot_and_the_first_slot_keeps_its_item() {
        let mut unit = Contents::new(TIER1);
        unit.insert("minecraft:cobblestone", 10);
        assert_eq!(unit.insert("minecraft:dirt", 5), 0);
        assert_eq!(unit.get(0), Some(&Entry::new("minecraft:cobblestone", 10)));
        assert_eq!(unit.get(1), Some(&Entry::new("minecraft:dirt", 5)));
    }

    #[test]
    fn a_repeat_of_a_held_item_tops_up_its_slot_instead_of_taking_a_new_one() {
        let mut unit = contents(TIER1, &[(0, "minecraft:cobblestone", 10)]);
        assert_eq!(unit.insert("minecraft:cobblestone", 20), 0);
        assert_eq!(unit.get(0), Some(&Entry::new("minecraft:cobblestone", 30)));
        assert_eq!(unit.get(1), None);
    }

    #[test]
    fn the_overflow_of_a_nearly_full_slot_is_returned() {
        let mut unit = contents(TIER1, &[(0, "minecraft:cobblestone", 250)]);
        assert_eq!(unit.insert("minecraft:cobblestone", 10), 5);
        assert_eq!(unit.get(0), Some(&Entry::new("minecraft:cobblestone", 255)));
    }

    #[test]
    fn eight_kinds_fill_the_unit_and_a_ninth_is_refused_whole() {
        let mut unit = Contents::new(TIER1);
        for index in 0..8 {
            assert_eq!(unit.insert(&format!("minecraft:item{index}"), 1), 0);
        }
        assert_eq!(
            unit.insert("minecraft:diamond", 64),
            64,
            "a full unit of eight kinds refuses the whole stack"
        );
        assert_eq!(unit.get(8), None, "there is no ninth slot");
    }

    #[test]
    fn inserting_nothing_changes_nothing() {
        let mut unit = Contents::new(TIER1);
        assert_eq!(unit.insert("minecraft:diamond", 0), 0);
        assert!(unit.is_empty());
    }

    #[test]
    fn extract_takes_at_most_the_count_and_clears_an_emptied_slot() {
        let mut unit = contents(TIER1, &[(2, "minecraft:diamond", 40)]);
        assert_eq!(
            unit.extract(2, 16),
            Some(Entry::new("minecraft:diamond", 16))
        );
        assert_eq!(unit.get(2), Some(&Entry::new("minecraft:diamond", 24)));
        assert_eq!(
            unit.extract(2, 64),
            Some(Entry::new("minecraft:diamond", 24))
        );
        assert_eq!(unit.get(2), None, "an emptied slot is empty again");
    }

    #[test]
    fn extract_from_an_empty_slot_or_past_the_end_is_nothing() {
        let mut unit = contents(TIER1, &[(0, "minecraft:diamond", 4)]);
        assert_eq!(unit.extract(1, 4), None);
        assert_eq!(unit.extract(8, 4), None, "tier 1 has slots 0..=7");
        assert_eq!(unit.get(0), Some(&Entry::new("minecraft:diamond", 4)));
    }

    #[test]
    fn a_deposit_sends_all_but_one_of_each_kind() {
        let unit = contents(
            TIER1,
            &[
                (0, "minecraft:cobblestone", 255),
                (3, "minecraft:diamond", 1),
                (5, "minecraft:dirt", 12),
            ],
        );
        assert_eq!(
            unit.deposit_plan(),
            vec![
                (0, Entry::new("minecraft:cobblestone", 254)),
                (5, Entry::new("minecraft:dirt", 11)),
            ],
            "one of each kind stays behind; a lone item sends nothing"
        );
    }

    #[test]
    fn a_withdraw_takes_only_what_the_unit_already_holds_and_has_room_for() {
        let unit = contents(TIER1, &[(0, "minecraft:cobblestone", 250)]);
        assert_eq!(unit.withdraw_amount("minecraft:cobblestone", 64), 5);
        assert_eq!(
            unit.withdraw_amount("minecraft:iron_ingot", 64),
            0,
            "a kind the unit does not hold stays in the container"
        );
        let empty = Contents::new(TIER1);
        assert_eq!(empty.withdraw_amount("minecraft:cobblestone", 64), 0);
        let full = contents(TIER1, &[(0, "minecraft:cobblestone", 255)]);
        assert_eq!(
            full.withdraw_amount("minecraft:cobblestone", 64),
            0,
            "a full slot has no room"
        );
    }

    #[test]
    fn saved_entries_that_do_not_fit_the_tier_are_dropped() {
        let unit = contents(
            TIER1,
            &[
                (0, "minecraft:diamond", 300),
                (8, "minecraft:dirt", 1),
                (1, "minecraft:stone", 0),
                (2, "minecraft:cobblestone", 7),
            ],
        );
        assert_eq!(
            unit.get(0),
            Some(&Entry::new("minecraft:diamond", 255)),
            "a saved count is clamped to the capacity"
        );
        assert_eq!(unit.get(8), None, "slot 8 is past tier 1");
        assert_eq!(unit.get(1), None, "a zero count is an empty slot");
        assert_eq!(unit.get(2), Some(&Entry::new("minecraft:cobblestone", 7)));
    }

    #[test]
    fn a_withdraw_refuses_a_stack_that_is_more_than_its_item_id() {
        let unit = contents(TIER1, &[(0, "minecraft:diamond_sword", 3)]);
        assert_eq!(
            unit.withdrawable("minecraft:diamond_sword", 1, 0, false),
            1,
            "a plain sword is withdrawn as before"
        );
        assert_eq!(
            unit.withdrawable("minecraft:diamond_sword", 1, 137, false),
            0,
            "a damaged sword stays in the container: a saved slot is id and count only"
        );
        assert_eq!(
            unit.withdrawable("minecraft:diamond_sword", 1, 0, true),
            0,
            "a renamed or enchanted sword stays too: its components cannot be rebuilt"
        );
        let full = contents(TIER1, &[(0, "minecraft:diamond_sword", 255)]);
        assert_eq!(
            full.withdrawable("minecraft:diamond_sword", 1, 0, false),
            0,
            "a full slot has no room either"
        );
        let empty = Contents::new(TIER1);
        assert_eq!(
            empty.withdrawable("minecraft:diamond_sword", 1, 0, false),
            0,
            "a kind the unit does not hold stays in the container"
        );
    }

    #[test]
    fn a_unit_totals_every_slot_and_knows_what_it_holds() {
        let unit = contents(
            TIER1,
            &[
                (0, "minecraft:cobblestone", 255),
                (1, "minecraft:diamond", 4),
            ],
        );
        assert_eq!(unit.total(), 259);
        assert!(unit.has("minecraft:diamond"));
        assert!(!unit.has("minecraft:dirt"));
        assert_eq!(unit.matching("minecraft:cobblestone"), Some(0));
        assert_eq!(unit.matching("minecraft:dirt"), None);
        assert_eq!(unit.filled(), 2);
        assert!(!unit.is_empty());
    }

    #[test]
    fn a_slot_can_be_written_back_and_an_empty_one_cleared() {
        let mut unit = contents(TIER1, &[(0, "minecraft:cobblestone", 5)]);
        unit.set(0, Some(Entry::new("minecraft:cobblestone", 12)));
        assert_eq!(unit.get(0), Some(&Entry::new("minecraft:cobblestone", 12)));
        unit.set(0, None);
        assert_eq!(unit.get(0), None);
        assert!(unit.is_empty());
        unit.set(0, Some(Entry::new("minecraft:dirt", 999)));
        assert_eq!(
            unit.get(0),
            Some(&Entry::new("minecraft:dirt", 255)),
            "a written count past the capacity is clamped"
        );
    }

    #[test]
    fn a_put_only_takes_what_the_unit_can_still_hold() {
        let unit = contents(TIER1, &[(0, "minecraft:cobblestone", 250)]);
        assert_eq!(unit.space_for("minecraft:cobblestone"), 5);
        assert_eq!(
            unit.space_for("minecraft:diamond"),
            255,
            "a new kind takes an empty slot's whole capacity"
        );
        let full = contents(TIER1, &[(0, "minecraft:cobblestone", 255)]);
        assert_eq!(full.space_for("minecraft:cobblestone"), 0);
        let eight = contents(
            TIER1,
            &[
                (0, "minecraft:a", 1),
                (1, "minecraft:b", 1),
                (2, "minecraft:c", 1),
                (3, "minecraft:d", 1),
                (4, "minecraft:e", 1),
                (5, "minecraft:f", 1),
                (6, "minecraft:g", 1),
                (7, "minecraft:h", 1),
            ],
        );
        assert_eq!(
            eight.space_for("minecraft:diamond"),
            0,
            "eight kinds leave no slot for a ninth"
        );
    }

    #[test]
    fn the_command_reads_the_held_units_actions() {
        assert_eq!(parse("psu list"), Action::List);
        assert_eq!(
            parse("psu put minecraft:diamond 5"),
            Action::Put {
                item: "minecraft:diamond".to_owned(),
                count: 5
            }
        );
        assert_eq!(
            parse("psu put minecraft:diamond"),
            Action::Put {
                item: "minecraft:diamond".to_owned(),
                count: 1
            },
            "a put without a count takes one"
        );
        assert_eq!(parse("/psu list"), Action::List, "a typed slash is ignored");
        assert_eq!(parse("psu"), Action::Help);
        assert_eq!(parse("psu help"), Action::Help);
        assert_eq!(parse("psu put"), Action::Unknown, "a put needs an item");
        assert_eq!(
            parse("psu put minecraft:diamond none"),
            Action::Unknown,
            "a count that is not a number is refused"
        );
        assert_eq!(parse("other list"), Action::Unknown);
        assert_eq!(parse("psu take minecraft:diamond 1"), Action::Unknown);
    }

    #[test]
    fn an_empty_unit_of_any_tier_reports_nothing() {
        for tier in TIERS {
            let unit = Contents::new(tier);
            assert!(unit.is_empty());
            assert_eq!(unit.total(), 0);
            assert_eq!(unit.filled(), 0);
            assert_eq!(unit.len(), tier.slots);
            assert_eq!(unit.slots().count(), 0);
        }
    }

    // The upgrade family: `pocketstorage:upgrade` is a shaped 3×3 recipe whose
    // central cell is the previous tier's unit and whose result is the next tier,
    // keeping what the unit holds. The rule is pure: who is in the grid is a key
    // the caller resolves, and a tag cell asks the caller whether a key is a member.

    /// The jar's tier-2 recipe as a pattern: gold in the corners, pistons above and
    /// below, chests left and right, the unit in the centre.
    fn tier2_pattern(unit: &str) -> [PatternCell; 9] {
        [
            PatternCell::Tagged("c:ingots/gold".to_owned()),
            PatternCell::Stack("minecraft:piston".to_owned()),
            PatternCell::Tagged("c:ingots/gold".to_owned()),
            PatternCell::Tagged("c:chests".to_owned()),
            PatternCell::Stack(unit.to_owned()),
            PatternCell::Tagged("c:chests".to_owned()),
            PatternCell::Tagged("c:ingots/gold".to_owned()),
            PatternCell::Stack("minecraft:piston".to_owned()),
            PatternCell::Tagged("c:ingots/gold".to_owned()),
        ]
    }

    /// Each ingredient as a key: 1 = gold, 2 = piston, 3 = chest, 10 = unit.
    fn tier2_grid(corner: u32, middle: u32, unit: u32) -> [Option<u32>; 9] {
        [
            Some(corner),
            Some(2),
            Some(corner),
            Some(middle),
            Some(unit),
            Some(middle),
            Some(corner),
            Some(2),
            Some(corner),
        ]
    }

    #[test]
    fn an_upgrade_recipe_is_the_next_tier_and_refuses_anything_else() {
        let recipe = Upgrade::new(tier2_pattern(TIER1.item), TIER2.item)
            .expect("the jar's tier-2 shape is an upgrade");
        assert_eq!(recipe.unit(), TIER1.item);
        assert_eq!(recipe.result(), TIER2.item);
        assert_eq!(recipe.unit_cell(), 4, "the unit is the centre cell");

        assert!(
            Upgrade::new(tier2_pattern(TIER1.item), TIER1.item).is_err(),
            "an upgrade from a tier to itself is not one"
        );
        assert!(
            Upgrade::new(tier2_pattern(TIER1.item), TIER3.item).is_err(),
            "an upgrade skips no tier"
        );
        assert!(
            Upgrade::new(tier2_pattern(TIER3.item), TIER2.item).is_err(),
            "an upgrade is never downward"
        );
        assert!(
            Upgrade::new(tier2_pattern(TIER4.item), TIER4.item).is_err(),
            "tier 4 has no next tier"
        );
        let mut no_unit = tier2_pattern(TIER1.item);
        no_unit[4] = PatternCell::Stack("minecraft:diamond".to_owned());
        assert!(
            Upgrade::new(no_unit, TIER2.item).is_err(),
            "a recipe whose centre is not a unit cannot upgrade"
        );
        let mut two_units = tier2_pattern(TIER1.item);
        two_units[0] = PatternCell::Stack(TIER1.item.to_owned());
        assert!(
            Upgrade::new(two_units, TIER2.item).is_err(),
            "a recipe with two units is ambiguous"
        );
    }

    #[test]
    fn an_upgrade_matches_only_its_own_pattern() {
        let recipe = Upgrade::new(tier2_pattern(TIER1.item), TIER2.item).unwrap();
        let resolve = |id: &str| match id {
            "minecraft:piston" => Some(2_u32),
            "pocketstorage:psu_1" => Some(10),
            "pocketstorage:psu_2" => Some(11),
            _ => None,
        };
        let in_tag = |tag: &str, key: u32| match tag {
            "c:ingots/gold" => key == 1,
            "c:chests" => key == 3,
            _ => false,
        };
        assert!(
            recipe.matches(&tier2_grid(1, 3, 10), resolve, in_tag),
            "the jar's pattern matches"
        );
        assert!(
            !recipe.matches(&tier2_grid(1, 3, 11), resolve, in_tag),
            "a tier-2 unit is not the tier-1 ingredient"
        );
        assert!(
            !recipe.matches(&tier2_grid(4, 3, 10), resolve, in_tag),
            "a corner that is not gold refuses the recipe"
        );
        assert!(
            !recipe.matches(&tier2_grid(1, 4, 10), resolve, in_tag),
            "a side that is not a chest refuses the recipe"
        );
        let mut hole = tier2_grid(1, 3, 10);
        hole[8] = None;
        assert!(
            !recipe.matches(&hole, resolve, in_tag),
            "a missing ingredient refuses the recipe"
        );
        assert!(
            !recipe.matches(&tier2_grid(1, 3, 10), |_| None::<u32>, |_, _| false),
            "with no item resolved and no tag loaded nothing matches"
        );
    }

    #[test]
    fn a_tag_cell_asks_membership_and_an_item_cell_does_not() {
        let recipe = Upgrade::new(tier2_pattern(TIER1.item), TIER2.item).unwrap();
        let asked = std::cell::RefCell::new(Vec::new());
        let resolve = |id: &str| match id {
            "minecraft:piston" => Some(2_u32),
            _ => None,
        };
        let in_tag = |tag: &str, key: u32| {
            asked.borrow_mut().push(tag.to_owned());
            // Gold arrives under both the convention tag and vanilla's own, which is
            // the compatibility the campaign rule asks for: either answer accepts it.
            (tag == "c:ingots/gold" && key == 1) || (tag == "minecraft:gold_ingots" && key == 1)
        };
        let mut unit = tier2_grid(1, 3, 10);
        unit[4] = Some(10);
        // The unit cell is an item cell, so it never reaches the tag lookup.
        assert!(
            !recipe.matches(&unit, resolve, in_tag),
            "the unit does not resolve"
        );
        let asks = asked.borrow();
        assert!(asks.iter().any(|tag| tag == "c:ingots/gold"));
        assert!(
            !asks.iter().any(|tag| tag.contains("psu")),
            "the unit cell is an item, not a tag: {asks:?}"
        );
    }

    #[test]
    fn the_shipped_upgrade_recipes_are_the_jars_three() {
        let tier2 = crate::decode_upgrade(include_str!("../data/pocketstorage/recipe/tier2.json"))
            .expect("tier2.json is a valid upgrade recipe");
        assert_eq!((tier2.unit(), tier2.result()), (TIER1.item, TIER2.item));
        assert_eq!(tier2.unit_cell(), 4);

        let tier3 = crate::decode_upgrade(include_str!("../data/pocketstorage/recipe/tier3.json"))
            .expect("tier3.json is a valid upgrade recipe");
        assert_eq!((tier3.unit(), tier3.result()), (TIER2.item, TIER3.item));

        let tier4 = crate::decode_upgrade(include_str!("../data/pocketstorage/recipe/tier4.json"))
            .expect("tier4.json is a valid upgrade recipe");
        assert_eq!((tier4.unit(), tier4.result()), (TIER3.item, TIER4.item));
    }

    #[test]
    fn the_upgrade_files_name_the_jars_ingredients() {
        // Upstream's three upgrades: tiers 2 and 3 put the tier's material in the
        // corners, a piston above and below and chests left and right; tier 4 turns
        // the shape around (pistons in the corners, chests above and below, nether
        // stars on the sides). The unit is always the centre.
        let tier2 = crate::decode_upgrade(include_str!("../data/pocketstorage/recipe/tier2.json"))
            .expect("a shipped upgrade");
        let tier3 = crate::decode_upgrade(include_str!("../data/pocketstorage/recipe/tier3.json"))
            .expect("a shipped upgrade");
        let tier4 = crate::decode_upgrade(include_str!("../data/pocketstorage/recipe/tier4.json"))
            .expect("a shipped upgrade");
        for (recipe, corner, middle, side) in [
            (tier2, "#c:ingots/gold", "minecraft:piston", "#c:chests"),
            (tier3, "#c:gems/diamond", "minecraft:piston", "#c:chests"),
            (tier4, "minecraft:piston", "#c:chests", "#c:nether_stars"),
        ] {
            let cells = recipe.cells();
            let corner = PatternCell::from_ingredient(corner).unwrap();
            let middle = PatternCell::from_ingredient(middle).unwrap();
            let side = PatternCell::from_ingredient(side).unwrap();
            for index in [0, 2, 6, 8] {
                assert_eq!(
                    cells[index],
                    corner,
                    "corner {index} of {}",
                    recipe.result()
                );
            }
            for index in [1, 7] {
                assert_eq!(
                    cells[index],
                    middle,
                    "middle {index} of {}",
                    recipe.result()
                );
            }
            for index in [3, 5] {
                assert_eq!(cells[index], side, "side {index} of {}", recipe.result());
            }
            assert_eq!(cells[4], PatternCell::Stack(recipe.unit().to_owned()));
        }
    }

    #[test]
    fn the_tier_one_recipe_is_264s_ingredient_form() {
        // A carried recipe written in 1.21.1's object form (`{"item": …}`) never
        // crafts at 26.3 (measured, ARCHITECTURE.md §3): the tier-1 recipe must name
        // bare ids and `#tags` in its key, and it must be the jar's 3×3 shape.
        let file = include_str!("../data/pocketstorage/recipe/tier1.json");
        assert!(
            !file.contains("\"item\"") && !file.contains("\"tag\""),
            "the tier-1 recipe uses the 26.3 ingredient form, not 1.21.1's object form"
        );
        assert!(file.contains("\"#c:ingots/iron\""));
        assert!(file.contains("minecraft:crafting_shaped"));
        assert!(file.contains("\"ABA\"") && file.contains("\"CDC\""));
    }
}
