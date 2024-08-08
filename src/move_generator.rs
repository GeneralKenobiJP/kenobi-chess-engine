
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

    fn generate_white_pawn_moves(&self) -> u64 {
        let index = Piece::PAWN as usize;
        let push_bitboard: u64 =
            self.board.piece_bitboards[index] << 8
            & self.board.empty_bitboard;

        let en_passant_tile = 1 << self.board.en_passant_possibility;

        let double_push_mask = 0b0000000000000000000000000000000000000000111111110000000000000000;
        let double_push_bitboard: u64 =
            (push_bitboard & double_push_mask) << 8
                & self.board.empty_bitboard;

        let left_capture_bitboard: u64 = self.board.piece_bitboards[index] << 7
            & (self.board.colour_bitboards[self.board.inactive_player as usize] + en_passant_tile);

        let right_capture_bitboard: u64 = self.board.piece_bitboards[index] << 9
            & (self.board.colour_bitboards[self.board.inactive_player as usize] + en_passant_tile);

        return push_bitboard + double_push_bitboard + left_capture_bitboard + right_capture_bitboard;
    }
    fn generate_black_pawn_moves(&self) -> u64 {
        let index = Piece::PAWN as usize + 6;
        let push_bitboard: u64 =
            self.board.piece_bitboards[index] >> 8
            & self.board.empty_bitboard;

        let en_passant_tile = 1 << self.board.en_passant_possibility;

        let double_push_mask = 0b0000000000000000111111110000000000000000000000000000000000000000;
        let double_push_bitboard: u64 =
            (push_bitboard & double_push_mask) >> 8
                & self.board.empty_bitboard;

        let left_capture_bitboard: u64 = self.board.piece_bitboards[index] >> 7
            & (self.board.colour_bitboards[self.board.inactive_player as usize] + en_passant_tile);

        let right_capture_bitboard: u64 = self.board.piece_bitboards[index] >> 9
            & (self.board.colour_bitboards[self.board.inactive_player as usize] + en_passant_tile);

        return push_bitboard + double_push_bitboard + left_capture_bitboard + right_capture_bitboard;
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

        assert_eq!(move_list.generate_white_pawn_moves(), 0b0000000000000000000000000000000011111111111111110000000000000000);
        let x: u8 = 0b10000000;
        x << 1;
        assert_eq!(x, 0b10000000);
    }
}