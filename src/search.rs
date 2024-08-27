use crate::board::Board;
use crate::evaluation::evaluate;
use crate::move_generator::MoveList;
use crate::perft::perft;

pub fn search(move_list: &mut MoveList, depth: u32) -> f32 {
    let mut value: f32 = 0.0;

    if depth == 0 {
        return evaluate(move_list.get_board());
    }

    move_list.generate_moves();

    let moves = move_list.get_moves().clone();

    for piece_move in moves
    {
        move_list.make_move(&piece_move);
        if !move_list.is_opponent_in_check() {
            value = -value.max(search(move_list, depth - 1))
        }
        move_list.unmake_move(&piece_move);
    }

    value
}