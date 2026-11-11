// Runs at the head of `Game::native_register_commands` (API 8): the `/psu`
// fallback for the screen this slice does not carry.
fn register_commands(commands: &mut Vec<NativeCommand>) {
    crate::game::pocketstorage_state::on_register_commands(commands);
}
