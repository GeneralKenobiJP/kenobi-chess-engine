//! Evaluation heuristics
//! Call the evaluate function to evaluate the board situation
//! Currently considers material advantage.

use crate::board::Board;

// We omit the king
const PIECE_WORTH: [i32; 5] = [1, 9, 5, 3, 3];

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

    for piece in 1..6 {
        material += count_pieces(board.piece_bitboards[active_player_piece_index + piece],
            PIECE_WORTH[piece]);
        material -= count_pieces(board.piece_bitboards[inactive_player_piece_index + piece],
            PIECE_WORTH[piece]);
    }

    material
}

/// Counts the material worth by counting the number of pieces of a given type.
/// Takes as input the bitboard of pieces of the given type
/// and the worth of a single piece of the given type.
fn count_pieces(piece_bitboard: u64, piece_value: i32) -> i32 {
    let mut material = 0;
    let mut bitboard = piece_bitboard;

    while bitboard > 0 {
        bitboard -= bitboard & bitboard.wrapping_neg();

        material += piece_value;
    }

    material
}