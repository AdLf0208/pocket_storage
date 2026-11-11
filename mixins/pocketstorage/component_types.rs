// Runs once before content loads: registers the storage unit's component id, so
// a saved stack's `pocketstorage:contents` is a known, networked component.
fn register_component_types(types: &mut Vec<String>) {
    crate::game::pocketstorage_state::register_component_types(types);
}
