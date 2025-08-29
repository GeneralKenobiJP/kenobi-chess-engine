//! Evaluation heuristics
//! Call the evaluate function to evaluate the board situation
//! Currently considers material advantage.
//! Value is measured in centipanws, i.e. 1 pawn = 100 centipawns

use std::fmt::Debug;
use num_traits::WrappingNeg;
use crate::board::Board;

// We omit the king
const PIECE_WORTH: [i32; 5] = [100, 900, 500, 300, 300];
pub const NEGATIVE_INFINITY: i32 = i32::MIN + 1;
pub const POSITIVE_INFINITY: i32 = i32::MAX;
pub const DRAW: i32 = 0;
const TOTAL_START_VALUE: i32 = 2 * (8 * PIECE_WORTH[0] + 1 * PIECE_WORTH[1] + 2 * PIECE_WORTH[2] + 2 * PIECE_WORTH[3] + 2 * PIECE_WORTH[4]);

// Piece-square tables
// They are written from the white's perspective, but with reversed indexing.
// Therefore, they need to be reversed for the white, and re-reversed for the black (that is as they are here).
const MIDGAME_PAWN_SQUARES: [i32; 64] = [
    0,  0,  0,  0,  0,  0,  0,  0,  // 8
    100,100,100,100,100,100,100,100,// 7
    40, 40, 45, 60, 60, 45, 40, 40, // 6
    20, 20, 25, 40, 40, 25, 20, 20, // 5
    0,  0,  5,  25, 25, 5,  0,  0,  // 4
    10, 0,  0, -5, -5,  0,  0,  10, // 3
    -5, 0,  0, -20,-20, 0,  0, -5,  // 2
    0,  0,  0,  0,  0,  0,  0,  0,  // 1
];

const ENDGAME_PAWN_SQUARES: [i32; 64] = [
    0,  0,  0,  0,  0,  0,  0,  0,  // 8
    200,200,200,200,200,200,200,200,// 7
    60, 60, 75, 100,100,75, 60, 60, // 6
    45, 45, 45, 50, 50, 45, 45, 45, // 5
    20, 20, 20, 35, 35, 20, 20, 20, // 4
    10, 10, 10, 10, 10, 10, 10, 10, // 3
    -5, 10, 10, 10, 10, 10, 10, -5, // 2
    0,  0,  0,  0,  0,  0,  0,   0, // 1
];

const MIDGAME_ROOK_SQUARES: [i32; 64] = [
    -5,  0,  0,  0,  0,  0,  0, -5,  // 8
     5,  15, 15, 15, 15, 15, 15, 5,// 7
    -5,  0,  0,  0,  0,  0,  0, -5, // 6
    -5,  0,  0,  0,  0,  0,  0, -5, // 5
    -5,  0,  0,  0,  0,  0,  0, -5, // 4
    -5,  0,  0,  0,  0,  0,  0, -5, // 3
    -5,  0,  0,  0,  0,  0,  0, -5, // 2
    -20,-5, -5, -5, -5, -5, -5,-20,// 1
];

const ENDGAME_ROOK_SQUARES: [i32; 64] = [
    0,  20, 20, 20, 20, 20, 20, 0,  // 8
    30, 50, 50, 50, 50, 50, 50, 30,// 7
    0,  20, 20, 20, 20, 20, 20, 0, // 6
    0,  20, 20, 20, 20, 20, 20, 0, // 5
    0,  20, 20, 20, 20, 20, 20, 0, // 4
    0,  20, 20, 20, 20, 20, 20, 0, // 3
    0,  20, 20, 20, 20, 20, 20, 0, // 2
    -5, 10, 10, 10, 10, 10, 10,-5,// 1
];

const MIDGAME_KNIGHT_SQUARES: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50, // 8
    -40,-20,  0,  0,  0,  0,-20,-40, // 7
	-30,  0, 10, 15, 15, 10,  0,-30, // 6
	-30,  5, 15, 20, 20, 15,  5,-30, // 5
	-30,  0, 15, 20, 20, 15,  0,-30, // 4
	-30,  5, 10, 15, 15, 10,  5,-30, // 3
	-40,-20,  0,  5,  5,  0,-20,-40, // 2
	-50,-40,-30,-30,-30,-30,-40,-50, // 1
];

const ENDGAME_KNIGHT_SQUARES: [i32; 64] = [
    -50,-40,-30,-30,-30,-30,-40,-50, // 8
    -40,-20,  0,  0,  0,  0,-20,-40, // 7
	-30,  0, 10, 15, 15, 10,  0,-30, // 6
	-30,  5, 15, 20, 20, 15,  5,-30, // 5
	-30,  0, 15, 20, 20, 15,  0,-30, // 4
	-30,  5, 10, 15, 15, 10,  5,-30, // 3
	-40,-20,  0,  5,  5,  0,-20,-40, // 2
	-50,-40,-30,-30,-30,-30,-40,-50, // 1
];

const MIDGAME_BISHOP_SQUARES: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20, // 8
    -10,  0,  0,  0,  0,  0,  0,-10, // 7
    -10,  0,  5, 10, 10,  5,  0,-10, // 6
    -10,  5,  5, 10, 10,  5,  5,-10, // 5
	-10,  0, 10, 10, 10, 10,  0,-10, // 4
	-10, 10, 10, 10, 10, 10, 10,-10, // 3
	-10,  5,  0,  0,  0,  0,  5,-10, // 2
	-20,-10,-10,-10,-10,-10,-10,-20, // 1
];

const ENDGAME_BISHOP_SQUARES: [i32; 64] = [
    -20,-10,-10,-10,-10,-10,-10,-20, // 8
    -10,  0,  0,  0,  0,  0,  0,-10, // 7
    -10,  0,  5, 10, 10,  5,  0,-10, // 6
    -10,  5,  5, 10, 10,  5,  5,-10, // 5
	-10,  0, 10, 10, 10, 10,  0,-10, // 4
	-10, 10, 10, 10, 10, 10, 10,-10, // 3
	-10,  5,  0,  0,  0,  0,  5,-10, // 2
	-20,-10,-10,-10,-10,-10,-10,-20, // 1
];

const MIDGAME_QUEEN_SQUARES: [i32; 64] = [
    -50,-30,-30,-15,-15,-30,-30,-50, // 8
    -30,-20,-20,-20,-20,-20,-20,-30, // 7
	-30,-10,-10,-10,-10,-10,-10,-30, // 6
	-5,   0,  5,  5,  5,  5,  0, -5, // 5
	0,    0,  5,  5,  5,  5,  0, -5, // 4
	-10,  5,  5,  5,  5,  5,  0,-10, // 3
	-10,  0,  5,  5,  5,  0,  0,-10, // 2
	-20,-10,-10,  0, 0,-10,-10,-20, // 1
];

const ENDGAME_QUEEN_SQUARES: [i32; 64] = [
    -20,-10,-10, -5, -5,-10,-10,-20, // 8
    -10,  0,  0,  0,  0,  0,  0,-10, // 7
	-10,  0, 25, 25, 25, 25,  0,-10, // 6
	-5,   0, 25, 25, 25, 25,  0, -5, // 5
	0,    0, 25, 25, 25, 25,  0, -5, // 4
	-10, 15, 25, 25, 25, 25,  0,-10, // 3
	-10,  0, 25, 10, 10, 10,  0,-10, // 2
	-20,-10,-10, -5, -5,-10,-10,-20, // 1
];

const MIDGAME_KING_SQUARES: [i32; 64] = [
    -80, -70, -70, -70, -70, -70, -70, -80, // 8
    -60, -60, -60, -60, -60, -60, -60, -60, // 7
	-40, -50, -50, -60, -60, -50, -50, -40, // 6
	-30, -40, -40, -50, -50, -40, -40, -30, // 5
	-20, -30, -30, -40, -40, -30, -30, -20, // 4
	-10, -20, -20, -20, -20, -20, -20, -10, // 3
	20,  20,  -5,  -5,  -5,  -5,  20,  20,  // 2
	20,  50,  10,   0,   0,  10,  50,  20,  // 1
];

const ENDGAME_KING_SQUARES: [i32; 64] = [
    -20, -10, -10, -10, -10, -10, -10, -20, // 8
    -5,   0,   5,   5,   5,   5,   0,  -5,  // 7
    -10, -5,   20,  30,  30,  20,  -5, -10, // 6
	-15, -10,  35,  45,  45,  35, -10, -15, // 5
	-20, -15,  30,  40,  40,  30, -15, -20, // 4
	-25, -20,  20,  25,  25,  20, -20, -25, // 3
	-30, -25,   0,   0,   0,   0, -25, -30, // 2
	-50, -30, -30, -30, -30, -30, -30, -50  // 1
];

const MIDGAME_PIECE_SQUARES: [[i32; 64]; 12] = [
    reverse_array(MIDGAME_KING_SQUARES),
    reverse_array(MIDGAME_PAWN_SQUARES),
    reverse_array(MIDGAME_QUEEN_SQUARES),
    reverse_array(MIDGAME_ROOK_SQUARES),
    reverse_array(MIDGAME_BISHOP_SQUARES),
    reverse_array(MIDGAME_KNIGHT_SQUARES),
    MIDGAME_KING_SQUARES,
    MIDGAME_PAWN_SQUARES,
    MIDGAME_QUEEN_SQUARES,
    MIDGAME_ROOK_SQUARES,
    MIDGAME_BISHOP_SQUARES,
    MIDGAME_KNIGHT_SQUARES
];

const ENDGAME_PIECE_SQUARES: [[i32; 64]; 12] = [
    reverse_array(ENDGAME_KING_SQUARES),
    reverse_array(ENDGAME_PAWN_SQUARES),
    reverse_array(ENDGAME_QUEEN_SQUARES),
    reverse_array(ENDGAME_ROOK_SQUARES),
    reverse_array(ENDGAME_BISHOP_SQUARES),
    reverse_array(ENDGAME_KNIGHT_SQUARES),
    ENDGAME_KING_SQUARES,
    ENDGAME_PAWN_SQUARES,
    ENDGAME_QUEEN_SQUARES,
    ENDGAME_ROOK_SQUARES,
    ENDGAME_BISHOP_SQUARES,
    ENDGAME_KNIGHT_SQUARES
];

/// Reverses a deep copy of an array and returns it.
const fn reverse_array(mut array: [i32; 64]) -> [i32;64] {
    let mut index = 0;

    while index < 32 {
        let temp = array[index];
        array[index] = array[63 - index];
        array[63-index] = temp;

        index += 1;
    }
    array
}

pub trait Evaluator: Debug + Eq + PartialEq {
    fn evaluate(board: &Board) -> i32;
}

#[derive(Debug, Eq, PartialEq)]
pub struct MainEvaluator;

impl Evaluator for MainEvaluator {

    /// Evaluates the current board situation and outputs the evaluation.
    /// Uses the negamax convention.
    /// Considers material advantage, 50-move rule, structural advantage.
    /// Checkmate (i.e. lack of king) is evaluated as i32::MAX / i32::MIN
    /// (roughly equivalent to +- inf)
    fn evaluate(board: &Board) -> i32 {
        let mut value = 0;

        if board.half_moves == 100 { return DRAW; }

        let phase_factor = compute_game_phase_factor(&board.piece_counter);

        value += count_material(board);
        if value == NEGATIVE_INFINITY || value == POSITIVE_INFINITY {
            return value;
        }
        value += evaluate_structure(board, phase_factor);

        value
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct MockMaterialEvaluator;

impl Evaluator for MockMaterialEvaluator {
    /// Evaluates the current board situation and outputs the evaluation.
    /// Uses the negamax convention.
    /// Considers material advantage, 50-move rule only
    /// (Mock used for testing purposes)
    /// Checkmate (i.e. lack of king) is evaluated as i32::MAX / i32::MIN
    /// (roughly equivalent to +- inf)
    fn evaluate(board: &Board) -> i32 {
        let mut value = 0;

        if board.half_moves == 100 { return DRAW; }

        value += count_material(board);

        value
    }
}

/// Counts the naive material difference on the board with consideration of naive position evaluation.
/// Outputs the difference between our material and opponent's material
/// and considers piece-square tables
/// Checkmate (i.e. lack of king) is evaluated as POSITIVE_INFINITY/NEGATIVE_INFINITY
/// (i32::MAX / i32::MIN + 1), treat as +- inf
fn count_material(board: &Board) -> i32 {
    let mut material = 0;

    let active_player_piece_index = 6 * board.active_player as usize;
    let inactive_player_piece_index = 6 * board.inactive_player as usize;

    if board.piece_counter[active_player_piece_index] == 0 { return NEGATIVE_INFINITY; }
    if board.piece_counter[inactive_player_piece_index] == 0 { return POSITIVE_INFINITY; }

    for piece in 1..6 {
        material += count_pieces(PIECE_WORTH[piece - 1], board.piece_counter[active_player_piece_index + piece]);
        material -= count_pieces(PIECE_WORTH[piece - 1], board.piece_counter[inactive_player_piece_index + piece]);
    }

    material
}

/// Counts the material worth by counting the number of pieces of a given type.
/// Takes as input the bitboard of pieces of the given type
/// and the worth of a single piece of the given type.
fn count_pieces(piece_value: i32, piece_count: u8) -> i32 {
    let material = piece_value * piece_count as i32;

    material
}

/// Evaluates structural advantage
/// Takes in phase_factor, which is an integer in [0,100],
/// where 0 means endgame, 100 means midgame.
/// Returns a negamax difference between players' structures
fn evaluate_structure(board: &Board, phase_factor: i32) -> i32 {
    let mut structure = 0;

    let active_player_piece_index = 6 * board.active_player as usize;
    let inactive_player_piece_index = 6 * board.inactive_player as usize;

    for index in 0..6 {
        structure += evaluate_player_piece_structure(board, active_player_piece_index + index, phase_factor);
        structure -= evaluate_player_piece_structure(board, inactive_player_piece_index + index, phase_factor);
    }

    structure
}

/// Evaluates the piece structure of a given piece type, of a given player.
/// Takes in phase_factor, which is an integer in [0,100],
/// where 0 means endgame, 100 means midgame.
/// Takes in piece_index indicating type and colour (in [0,11]).
/// Returns the evaluation of the structure of the given pieces.
fn evaluate_player_piece_structure(board: &Board, piece_index: usize, phase_factor: i32) -> i32 {
    let mut structure = 0;
    let mut bitboard = board.piece_bitboards[piece_index];

    while bitboard > 0 {
        let tile = bitboard & bitboard.wrapping_neg();
        bitboard -= tile;

        structure += evaluate_piece_position(tile, piece_index, phase_factor);
    }
    structure
}

/// Evaluates position of a given piece on a given tile, considering a given phase factor.
/// Takes in phase_factor, which is an integer in [0,100],
/// where 0 means endgame, 100 means midgame.
/// Takes in piece_index indicating type and colour (in [0,11]).
/// Takes in a 64-bit representation of the tile
/// Returns the evaluation of the structure of the given piece.
fn evaluate_piece_position(tile: u64, piece_index: usize, phase_factor: i32) -> i32 {
    let index = tile.checked_ilog2().unwrap_or_default() as usize;
    let mut value = MIDGAME_PIECE_SQUARES[piece_index][index] * phase_factor + ENDGAME_PIECE_SQUARES[piece_index][index] * (100-phase_factor);
    value /= 100;

    value
}

/// Computes the game phase factor for the current board situation.
/// Game phase factor is an integer in [0,100],
/// 0 means endgame, 100 means midgame.
/// Takes in a pointer to the piece counter array of a Board object.
pub fn compute_game_phase_factor(piece_count: &[u8; 12]) -> i32 {
    let mut factor: i32 = 0;
    for color in 0..2 {
        for piece in 1..6 {
            factor += piece_count[piece + 6 * color] as i32 * PIECE_WORTH[piece - 1]
        }
    }
    factor = (factor * 100) / TOTAL_START_VALUE;

    factor
}

#[cfg(test)]
mod tests {
    use crate::board::START_POSITION;
    use super::*;

    #[test]
    fn test_count_pieces() {
        assert_eq!(800, count_pieces(100, 8));
        assert_eq!(900, count_pieces(300, 3));
        assert_eq!(900, count_pieces(900, 1));
    }

    #[test]
    fn check_count_material_start_position() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(0, count_material(&board));
    }

    #[test]
    fn check_count_material_position_4() {
        let mut board = Board::new();
        let fen = "r3k3/1Pr5/5pp1/3pPBPP/1b1P2Qq/2R2N2/P7/RK6 w q d6 1 25";
        board.read_fen(fen);

        assert_eq!(600, count_material(&board));
    }

    #[test]
    fn check_count_material_checkmate_white_loss() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQ1BNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(NEGATIVE_INFINITY, count_material(&board));
    }

    #[test]
    fn check_count_material_checkmate_black_victory() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQ1BNR b KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(POSITIVE_INFINITY, count_material(&board));
    }

    #[test]
    fn check_count_material_checkmate_black_loss() {
        let mut board = Board::new();
        let fen = "rnbq1bnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(NEGATIVE_INFINITY, count_material(&board));
    }

    #[test]
    fn check_count_material_checkmate_white_victory() {
        let mut board = Board::new();
        let fen = "rnbq1bnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(POSITIVE_INFINITY, count_material(&board));
    }

    #[test]
    fn check_compute_game_phase_factor_start_position() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        assert_eq!(100, compute_game_phase_factor(&board.piece_counter));
    }

    #[test]
    fn check_compute_game_phase_factor_empty_board() {
        let mut board = Board::new();
        let fen = "4k3/8/8/8/8/8/8/4K3 b - - 1 1";
        board.read_fen(fen);

        assert_eq!(0, compute_game_phase_factor(&board.piece_counter));
    }

    #[test]
    fn check_compute_game_phase_factor_midgame() {
        let mut board = Board::new();
        let fen = "r3k3/2n3n1/1p6/8/8/8/5PPP/3QKB2 w q - 0 1";
        board.read_fen(fen);

        assert_eq!((1*PIECE_WORTH[1] + 1*PIECE_WORTH[3] + 1*PIECE_WORTH[2] + 4*PIECE_WORTH[0] + 2*PIECE_WORTH[4]) * 100 / TOTAL_START_VALUE,
                   compute_game_phase_factor(&board.piece_counter));
        println!("{}", compute_game_phase_factor(&board.piece_counter));
    }

    #[test]
    fn test_evaluate_piece_position() {
        // White pawn on rank 7
        assert_eq!(100, evaluate_piece_position(0x0010000000000000, 1, 100));
        assert_eq!(200, evaluate_piece_position(0x0010000000000000, 1, 0));
        assert_eq!(180, evaluate_piece_position(0x0010000000000000, 1, 20));

        // Black pawn on e7
        assert_eq!(-20, evaluate_piece_position(0x0010000000000000, 7, 100));
        assert_eq!(10, evaluate_piece_position(0x0010000000000000, 7, 0));
        assert_eq!(4, evaluate_piece_position(0x0010000000000000, 7, 20));

        // White rook on b8
        assert_eq!(0, evaluate_piece_position(0x4000000000000000, 3, 100));
        assert_eq!(20, evaluate_piece_position(0x4000000000000000, 3, 0));
        assert_eq!(16, evaluate_piece_position(0x4000000000000000, 3, 20));

        // Black king on g8
        assert_eq!(50, evaluate_piece_position(0x0200000000000000, 6, 100));
        assert_eq!(-30, evaluate_piece_position(0x0200000000000000, 6, 0));
        assert_eq!(-14, evaluate_piece_position(0x0200000000000000, 6, 20));
    }

    #[test]
    fn test_evaluate_player_piece_structure() {
        let board = Board::from_fen(START_POSITION);
        let phase_factor = compute_game_phase_factor(&board.piece_counter);

        assert_eq!(0, evaluate_player_piece_structure(&board, 0, phase_factor));
        assert_eq!(0, evaluate_player_piece_structure(&board, 6, phase_factor));
        assert_eq!(-50, evaluate_player_piece_structure(&board, 1, phase_factor));
        assert_eq!(-50, evaluate_player_piece_structure(&board, 7, phase_factor));
        assert_eq!(0, evaluate_player_piece_structure(&board, 2, phase_factor));
        assert_eq!(0, evaluate_player_piece_structure(&board, 8, phase_factor));
        assert_eq!(-40, evaluate_player_piece_structure(&board, 3, phase_factor));
        assert_eq!(-40, evaluate_player_piece_structure(&board, 9, phase_factor));
        assert_eq!(-20, evaluate_player_piece_structure(&board, 4, phase_factor));
        assert_eq!(-20, evaluate_player_piece_structure(&board, 10, phase_factor));
        assert_eq!(-80, evaluate_player_piece_structure(&board, 5, phase_factor));
        assert_eq!(-80, evaluate_player_piece_structure(&board, 11, phase_factor));

        let board = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBN1 w KQkq - 0 1");
        assert_eq!(-20, evaluate_player_piece_structure(&board, 3, phase_factor));
        assert_eq!(-40, evaluate_player_piece_structure(&board, 9, phase_factor));
    }

    #[test]
    fn check_evaluate_structure_e2e4() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(45, evaluate_structure(&board, 100));
    }

    #[test]
    fn check_evaluate_structure_e2e4_endgame_phase() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(25, evaluate_structure(&board, 0));
    }

    #[test]
    fn check_evaluate_structure_bongcloud() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPPKPPP/RNBQ1BNR w kq - 0 1";
        board.read_fen(fen);

        assert_eq!(-5, evaluate_structure(&board, 100));
    }

    #[test]
    fn check_evaluate_structure_a2a3_e7e5() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/8/P7/1PPPPPPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(-30, evaluate_structure(&board, 100));
    }

    #[test]
    fn check_evaluate_structure_endgame() {
        let mut board = Board::new();
        let fen = "r7/pp6/8/8/8/4k3/PP1pP3/R5NK w - - 0 1";
        board.read_fen(fen);

        let phase_factor = compute_game_phase_factor(&board.piece_counter);
        println!("{}", phase_factor);

        let expected = evaluate_piece_position(1u64 << 1, 6*0 + 5, phase_factor) +
            evaluate_piece_position(1u64 << 11, 6*0 + 1, phase_factor) +
            evaluate_piece_position(1u64 << 0, 6*0 + 0, phase_factor) +
            evaluate_piece_position(1u64 << 7, 6*0 + 3, phase_factor) +
            evaluate_piece_position(1u64 << 14, 6*0 + 1, phase_factor) +
            evaluate_piece_position(1u64 << 15, 6*0 + 1, phase_factor) -
            evaluate_piece_position(1u64 << 12, 6*1 + 1, phase_factor) -
            evaluate_piece_position(1u64 << 19, 6*1 + 0, phase_factor) -
            evaluate_piece_position(1u64 << 63, 6*1 + 3, phase_factor) -
            evaluate_piece_position(1u64 << 55, 6*1 + 1, phase_factor) -
            evaluate_piece_position(1u64 << 54, 6*1 + 1, phase_factor);

        assert_eq!(expected, evaluate_structure(&board, phase_factor));
    }

    #[test]
    fn test_evaluation_start_position() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(0, MainEvaluator::evaluate(&board));
    }

    #[test]
    fn test_evaluation_position_4() {
        let mut board = Board::new();
        let fen = "r3k3/1Pr5/5pp1/3pPBPP/1b1P2Qq/2R2N2/P7/RK6 w q d6 1 25";
        board.read_fen(fen);

        let phase_factor = compute_game_phase_factor(&board.piece_counter);

        let expected_structure = -evaluate_piece_position(1u64 << 63, 6*1 + 3, phase_factor) -
            evaluate_piece_position(1u64 << 59, 6*1 + 0, phase_factor) +
            evaluate_piece_position(1u64 << 54, 6*0 + 1, phase_factor) -
            evaluate_piece_position(1u64 << 53, 6*1 + 3, phase_factor) -
            evaluate_piece_position(1u64 << 42, 6*1 + 1, phase_factor) -
            evaluate_piece_position(1u64 << 41, 6*1 + 1, phase_factor) -
            evaluate_piece_position(1u64 << 36, 6*1 + 1, phase_factor) +
            evaluate_piece_position(1u64 << 35, 6*0 + 1, phase_factor) +
            evaluate_piece_position(1u64 << 34, 6*0 + 4, phase_factor) +
            evaluate_piece_position(1u64 << 33, 6*0 + 1, phase_factor) +
            evaluate_piece_position(1u64 << 32, 6*0 + 1, phase_factor) -
            evaluate_piece_position(1u64 << 30, 6*1 + 4, phase_factor) +
            evaluate_piece_position(1u64 << 28, 6*0 + 1, phase_factor) +
            evaluate_piece_position(1u64 << 25, 6*0 + 2, phase_factor) -
            evaluate_piece_position(1u64 << 24, 6*1 + 2, phase_factor) +
            evaluate_piece_position(1u64 << 21, 6*0 + 3, phase_factor) +
            evaluate_piece_position(1u64 << 18, 6*0 + 5, phase_factor) +
            evaluate_piece_position(1u64 << 15, 6*0 + 1, phase_factor) +
            evaluate_piece_position(1u64 << 7, 6*0 + 3, phase_factor) +
            evaluate_piece_position(1u64 << 6, 6*0 + 0, phase_factor);

        assert_eq!(600 + expected_structure, MainEvaluator::evaluate(&board));
    }

    #[test]
    fn test_evaluation_checkmate() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQ1BNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(NEGATIVE_INFINITY, MainEvaluator::evaluate(&board));
    }

    #[test]
    fn test_evaluation_fifty_moves() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 100 1";
        board.read_fen(fen);

        assert_eq!(DRAW, MainEvaluator::evaluate(&board));
    }
}