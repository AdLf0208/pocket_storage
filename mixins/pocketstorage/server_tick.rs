// Runs at the head of `Game::native_server_tick` (API 44): a held stack written
// inside a guest turn is resent to that player at the tick's end.
fn server_tick(game: &mut crate::game::Game<'_>, phase: ServerTickPhase) {
    crate::game::pocketstorage_state::on_server_tick(game, phase);
}
