// Runs at the head of `Game::native_perform_command` (API 8): `/psu list`,
// `/psu put <item> [n]` and `/psu help`, with the player's state swapped in.
fn perform_command(
    game: &mut crate::game::Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::pocketstorage_state::on_perform_command(game, player, command, ran);
}
