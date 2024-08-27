//! Evaluation heuristics
//! Call the evaluate function to evaluate the board situation
//! Currently considers material advantage.

use crate::board::Board;

/// Evaluates the current board situation and outputs the evaluation.
/// Uses the negamax convention.
/// Considers material advantage.
pub fn evaluate(board: &Board) -> f32 {
    let mut value = 0.0;

    value += count_material(board) as f32;

    value
}

/// Counts the naive material difference on the board
/// Outputs the difference between our material and opponent's material as i32
fn count_material(board: &Board) -> i32 {
    let mut material = 0;

    let active_player_piece_index = 6 * board.active_player as usize;
    let inactive_player_piece_index = 6 * board.inactive_player as usize;

    for piece in 0..6 {
        material += count_pieces(board.piece_bitboards[active_player_piece_index]);
        material -= count_pieces(board.piece_bitboards[inactive_player_piece_index]);
    }

    material
}

fn count_pieces(piece_bitboard: u64) -> i32 {

}