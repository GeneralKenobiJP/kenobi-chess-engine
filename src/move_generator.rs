use scanner_rust::generic_array::typenum::Log2;
use crate::piece;
use crate::piece::Piece;
use crate::piece::Colour;
use crate::board;
use crate::board::{Board, UPPER_RANK_LOWEST_TILE};
use crate::piece::Colour::WHITE;

struct Move {
    origin: u8,
    target: u8,
    promotion: u8,
    piece: Piece
}

struct MoveList<'a> {
    board: &'a Board,
    moves: Vec<Move>
}

impl<'a> MoveList<'a> {
    fn new(board: &'a Board) -> Self {
        MoveList {
            board,
            moves: Vec::new()
        }
    }

    fn get_moves(&self) -> &'a Vec<Move> {
        &self.moves
    }

    fn generate_moves(&mut self) {
        // let pawn_moves = if self.board.active_player == WHITE {self.generate_white_pawn_moves()}
        //     else {self.generate_black_pawn_moves()};
        if self.board.active_player == WHITE
        {
            self.generate_white_pawn_moves();
            return;
        }
    }

    fn generate_white_pawn_moves(&mut self) {
        let push_bitboard = self.generate_white_push_bitboard();

        let double_push_bitboard = self.generate_white_double_push_bitboard(push_bitboard);

        let en_passant_tile = 1 << self.board.en_passant_possibility;
        let left_capture_bitboard = self.generate_pawn_capture_bitboard(7, en_passant_tile);

        let right_capture_bitboard: u64 = self.generate_pawn_capture_bitboard(9, en_passant_tile);

        self.convert_white_pawn_moves(push_bitboard, 8);
        self.convert_white_pawn_moves(double_push_bitboard, 16);
        self.convert_white_pawn_moves(left_capture_bitboard, 7);
        self.convert_white_pawn_moves(right_capture_bitboard, 9);
    }

    fn convert_white_pawn_moves(&mut self, push_bitboard: u64, shift: u8) {
        let mut bitboard = push_bitboard as i64;

        loop {
            let tile = bitboard & -bitboard;
            if tile == 0 {break;}

            bitboard -= tile;
            let origin = u64::ilog2(tile as u64) as u8;

            if(origin < UPPER_RANK_LOWEST_TILE)
            {
                self.moves.push(Move {
                    origin,
                    target: origin >> shift,
                    promotion: 0,
                    piece: Piece::PAWN,
                });
                continue;
            }

            for i in (2..6)
            {
                self.moves.push(Move {
                    origin,
                    target: origin >> shift,
                    promotion: i,
                    piece: Piece::PAWN,
                });
            }
        }
    }

    fn generate_pawn_capture_bitboard(&self, shift: u8, en_passant_tile: u64) -> u64 {
        let index = Piece::PAWN as usize;
        let capture_bitboard: u64 = self.board.piece_bitboards[index] << shift
            & (self.board.colour_bitboards[self.board.inactive_player as usize] + en_passant_tile);
        capture_bitboard
    }

    fn generate_white_double_push_bitboard(&self, push_bitboard: u64) -> u64 {
        let double_push_mask = 0b0000000000000000000000000000000000000000111111110000000000000000;
        let double_push_bitboard: u64 =
            (push_bitboard & double_push_mask) << 8
                & self.board.empty_bitboard;
        double_push_bitboard
    }

    fn generate_white_push_bitboard(&self) -> u64 {
        self.board.piece_bitboards[Piece::PAWN as usize] << 8
            & self.board.empty_bitboard
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

        return push_bitboard | double_push_bitboard | left_capture_bitboard | right_capture_bitboard;
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

        // assert_eq!(move_list.generate_white_pawn_moves(), 0b0000000000000000000000000000000011111111111111110000000000000000);
        let x: u8 = 0b10000000;
        x << 1;
        assert_eq!(x, 0b10000000);
    }
}