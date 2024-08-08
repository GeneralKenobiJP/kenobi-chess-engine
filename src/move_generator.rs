
use crate::piece;
use crate::piece::Piece;
use crate::piece::Colour;
use crate::board;
use crate::board::Board;
use crate::piece::Colour::WHITE;

struct Move {
    origin: u8,
    target: u8,
    promotion: u8,
}

struct MoveList<'a> {
    board: &'a Board,
}

impl<'a> MoveList<'a> {
    fn new(board: &'a Board) -> Self {
        MoveList {
            board
        }
    }

    fn generate_pawn_moves(&self) -> u64 {
        let push_bitboard: u64 =
            self.board.piece_bitboards[Piece::PAWN as usize + 6 * self.board.active_player as usize] << 8
            & self.board.empty_bitboard;
        let double_push_mask = if self.board.active_player == WHITE {
            0b0000000000000000000000000000000000000000111111110000000000000000
        }
        else {
            0b0000000000000000111111110000000000000000000000000000000000000000
        };

        let double_push_bitboard: u64 =
            (push_bitboard & double_push_mask) << 8
                & self.board.empty_bitboard;
        return push_bitboard + double_push_bitboard;
    }
}

#[cfg(test)]
mod tests {
    use crate::board::{read_fen, START_POSITION};
    use super::*;

    #[test]
    fn placeholder_name() {
        let mut board = Board::new();
        read_fen(&mut board, START_POSITION);
        let move_list = MoveList::new(&board);

        assert_eq!(move_list.generate_pawn_moves(), 0b0000000000000000000000000000000011111111111111110000000000000000);
    }
}