use scanner_rust::generic_array::typenum::Log2;
use crate::piece;
use crate::piece::Piece;
use crate::piece::Colour;
use crate::board;
use crate::board::{Board, LOWER_RANK_HIGHEST_TILE, UPPER_RANK_LOWEST_TILE};
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

    fn get_moves(&self) -> &Vec<Move> {
        &self.moves
    }

    fn generate_moves(&mut self) {
        if self.board.active_player == WHITE
        {
            self.generate_white_pawn_moves();
            return;
        }
        self.generate_black_pawn_moves();
    }

    /// WHITE PAWN MOVE GENERATION

    fn generate_white_pawn_moves(&mut self) {
        let push_bitboard = self.generate_white_push_bitboard();

        let double_push_bitboard = self.generate_white_double_push_bitboard(push_bitboard);

        let en_passant_tile = 1 << self.board.en_passant_possibility;
        let left_capture_bitboard = self.generate_pawn_capture_bitboard(7);

        let right_capture_bitboard: u64 = self.generate_pawn_capture_bitboard(9);

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
            let target = origin >> shift;

            if(target < UPPER_RANK_LOWEST_TILE)
            {
                self.moves.push(Move {
                    origin,
                    target,
                    promotion: 0,
                    piece: Piece::PAWN,
                });
                continue;
            }

            for i in (2..6)
            {
                self.moves.push(Move {
                    origin,
                    target,
                    promotion: i,
                    piece: Piece::PAWN,
                });
            }
        }
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

    /// BLACK PAWN MOVE GENERATION

    fn generate_black_pawn_moves(&mut self) {
        let push_bitboard = self.generate_black_push_bitboard();

        let double_push_bitboard = self.generate_black_double_push_bitboard(push_bitboard);

        let en_passant_tile = 1 << self.board.en_passant_possibility;
        let left_capture_bitboard = self.generate_pawn_capture_bitboard(-7);

        let right_capture_bitboard: u64 = self.generate_pawn_capture_bitboard(-9);

        self.convert_black_pawn_moves(push_bitboard, 8);
        self.convert_black_pawn_moves(double_push_bitboard, 16);
        self.convert_black_pawn_moves(left_capture_bitboard, 7);
        self.convert_black_pawn_moves(right_capture_bitboard, 9);
    }

    fn convert_black_pawn_moves(&mut self, push_bitboard: u64, shift: u8) {
        let mut bitboard = push_bitboard as i64;

        loop {
            let tile = bitboard & -bitboard;
            if tile == 0 {break;}

            bitboard -= tile;
            let origin = u64::ilog2(tile as u64) as u8;
            let target = origin << shift;

            if(target > LOWER_RANK_HIGHEST_TILE)
            {
                self.moves.push(Move {
                    origin,
                    target,
                    promotion: 0,
                    piece: Piece::PAWN,
                });
                continue;
            }

            for i in (2..6)
            {
                self.moves.push(Move {
                    origin,
                    target,
                    promotion: i,
                    piece: Piece::PAWN,
                });
            }
        }
    }

    fn generate_black_double_push_bitboard(&self, push_bitboard: u64) -> u64 {
        let double_push_mask = 0b0000000000000000111111110000000000000000000000000000000000000000;
        let double_push_bitboard: u64 =
            (push_bitboard & double_push_mask) >> 8
                & self.board.empty_bitboard;
        double_push_bitboard
    }

    fn generate_black_push_bitboard(&self) -> u64 {
        self.board.piece_bitboards[Piece::PAWN as usize] >> 8
            & self.board.empty_bitboard
    }

    /// NEUTRAL

    fn generate_pawn_capture_bitboard(&self, shift: i8) -> u64 {
        let mut attack_options = self.board.colour_bitboards[self.board.inactive_player as usize];
        if self.board.en_passant_possibility < 64 {
            let en_passant_tile = 1 << self.board.en_passant_possibility;
            attack_options |= en_passant_tile;
        }
        let index = Piece::PAWN as usize;
        let capture_bitboard: u64 = self.board.piece_bitboards[index] << shift & attack_options;
        capture_bitboard
    }
}

#[cfg(test)]
mod tests {
    use crate::board::{read_fen, START_POSITION};
    use super::*;

    #[test]
    fn start_position() {
        let mut board = Board::new();
        read_fen(&mut board, START_POSITION);
        let mut move_list = MoveList::new(&board);

        let mut expected_push_bitboard: u64 = 0b0000000000000000000000000000000000000000111111110000000000000000;
        let mut expected_double_push_bitboard: u64 = 0b0000000000000000000000000000000011111111000000000000000000000000;
        let mut expected_left_pawn_capture_bitboard: u64 = 0;
        let mut expected_right_pawn_capture_bitboard: u64 = 0;

        let mut expected_pawn_push = Vec::<Move>::new();
        expected_pawn_push.push(Move{ origin: 8, target: 16, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 9, target: 17, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 10, target: 18, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 11, target: 19, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 12, target: 20, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 13, target: 21, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 14, target: 22, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 15, target: 23, promotion: 0, piece: Piece::PAWN });

        let mut expected_pawn_double_push = Vec::<Move>::new();
        expected_pawn_push.push(Move{ origin: 8, target: 24, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 9, target: 25, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 10, target: 26, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 11, target: 27, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 12, target: 28, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 13, target: 29, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 14, target: 30, promotion: 0, piece: Piece::PAWN });
        expected_pawn_push.push(Move{ origin: 15, target: 31, promotion: 0, piece: Piece::PAWN });

        let mut expected_left_pawn_capture = Vec::<Move>::new();
        let mut expected_right_pawn_capture = Vec::<Move>::new();

        // move_list.generate_moves();
        move_list.generate_white_push_bitboard();

        assert_eq!(move_list.generate_white_push_bitboard(), expected_push_bitboard);
        assert_eq!(move_list.generate_white_double_push_bitboard(expected_push_bitboard), expected_double_push_bitboard);
        assert_eq!(move_list.generate_pawn_capture_bitboard(7), expected_left_pawn_capture_bitboard);
        assert_eq!(move_list.generate_pawn_capture_bitboard(9), expected_right_pawn_capture_bitboard);
    }
}