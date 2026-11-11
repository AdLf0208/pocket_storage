// Runs at the head of `Game::native_right_click_block` (API 21): a sneak right
// click on a container empties the held unit into it, all but one of each kind.
fn observe_right_click_block(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    canceled: &mut bool,
) {
    crate::game::pocketstorage_state::on_right_click_block(game, pos, face, canceled);
}
