// Runs at the head of `Game::native_left_click_block` (API 30): a sneak left
// click on a container takes back the kinds the held unit already holds, and
// cancels the dig so a withdraw never breaks the container.
fn observe_left_click_block(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    canceled: &mut bool,
) {
    crate::game::pocketstorage_state::on_left_click_block(game, pos, face, canceled);
}
