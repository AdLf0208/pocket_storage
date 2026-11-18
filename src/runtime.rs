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
}
