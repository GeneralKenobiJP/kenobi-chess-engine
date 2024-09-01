//! Evaluation heuristics
//! Call the evaluate function to evaluate the board situation
//! Currently considers material advantage.
//! Value is measured in centipanws, i.e. 1 pawn = 100 centipawns

use crate::board::Board;

// We omit the king
const PIECE_WORTH: [i32; 5] = [100, 900, 500, 300, 300];
pub const NEGATIVE_INFINITY: i32 = i32::MIN + 1;
pub const POSITIVE_INFINITY: i32 = i32::MAX;

/// Evaluates the current board situation and outputs the evaluation.
/// Uses the negamax convention.
/// Considers material advantage.
/// Checkmate (i.e. lack of king) is evaluated as i32::MAX / i32::MIN
/// (roughly equivalent to +- inf)
pub fn evaluate(board: &Board) -> i32 {
    let mut value = 0;

    value += count_material(board);

    value
}

/// Counts the naive material difference on the board
/// Outputs the difference between our material and opponent's material as i32
/// Checkmate (i.e. lack of king) is evaluated as i32::MAX / i32::MIN
/// (roughly equivalent to +- inf)
fn count_material(board: &Board) -> i32 {
    let mut material = 0;

    let active_player_piece_index = 6 * board.active_player as usize;
    let inactive_player_piece_index = 6 * board.inactive_player as usize;

    if board.piece_bitboards[active_player_piece_index] == 0 { return i32::MIN; }
    if board.piece_bitboards[inactive_player_piece_index] == 0 { return i32::MAX; }

    for piece in 1..6 {
        material += count_pieces(board.piece_bitboards[active_player_piece_index + piece],
            PIECE_WORTH[piece - 1]);
        material -= count_pieces(board.piece_bitboards[inactive_player_piece_index + piece],
            PIECE_WORTH[piece - 1]);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_pieces() {
        assert_eq!(800, count_pieces(0x000000000000FF00, 100));
        assert_eq!(900, count_pieces(0x1003000000000000, 300));
        assert_eq!(900, count_pieces(0x0000000001000000, 900));
    }

    #[test]
    fn count_material_start_position() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(0, count_material(&board));
    }

    #[test]
    fn count_material_position_4() {
        let mut board = Board::new();
        let fen = "r3k3/1Pr5/5pp1/3pPBPP/1b1P2Qq/2R2N2/P7/RK6 w q d6 1 25";
        board.read_fen(fen);

        assert_eq!(600, count_material(&board));
    }

    #[test]
    fn count_material_checkmate_white_loss() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQ1BNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(i32::MIN, count_material(&board));
    }

    #[test]
    fn count_material_checkmate_black_victory() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQ1BNR b KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(i32::MAX, count_material(&board));
    }

    #[test]
    fn count_material_checkmate_black_loss() {
        let mut board = Board::new();
        let fen = "rnbq1bnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(i32::MIN, count_material(&board));
    }

    #[test]
    fn count_material_checkmate_white_victory() {
        let mut board = Board::new();
        let fen = "rnbq1bnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(i32::MAX, count_material(&board));
    }

    #[test]
    fn test_evaluation_start_position() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(0, evaluate(&board));
    }

    #[test]
    fn test_evaluation_position_4() {
        let mut board = Board::new();
        let fen = "r3k3/1Pr5/5pp1/3pPBPP/1b1P2Qq/2R2N2/P7/RK6 w q d6 1 25";
        board.read_fen(fen);

        assert_eq!(600, evaluate(&board));
    }

    #[test]
    fn test_evaluation_checkmate() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQ1BNR w KQkq - 0 1";
        board.read_fen(fen);

        assert_eq!(i32::MIN, evaluate(&board));
    }
}