//! Move generator
//! Generates a vector of moves based on the input board position
//! Involves bitboards, magic bitboards, etc.

use crate::piece;
use crate::piece::Piece;
use crate::piece::Colour;
use crate::board;
use crate::board::{Board, LOWER_RANK_HIGHEST_TILE, NOT_FILE_A_MASK, NOT_FILE_H_MASK, UPPER_RANK_LOWEST_TILE};
use crate::piece::Colour::{BLACK, WHITE};
use crate::piece::Piece::KING;

#[derive(PartialEq, Eq, Hash, Clone)]
struct Move {
    origin: u8,
    target: u8,
    promotion: u8,
    piece: Piece
}

struct MoveList<'a> {
    board: &'a Board,
    moves: Vec<Move>,
    king_lookup_table: [u64; 64] // should be immutable
}

impl<'a> MoveList<'a> {
    fn new(board: &'a Board) -> Self {
        MoveList {
            board,
            moves: Vec::new(),
            king_lookup_table: Self::setup_king_lookup_table()
        }
    }

    fn get_moves(&self) -> &Vec<Move> {
        &self.moves
    }

    /// Generates moves and updates move list based on the situation on the board
    fn generate_moves(&mut self) {
        self.generate_king_moves();
        if self.board.active_player == WHITE
        {
            self.generate_white_pawn_moves();
            return;
        }
        self.generate_black_pawn_moves();
    }

    /// KING MOVE GENERATION

    /// Outputs a lookup table for bitboards of possible king moves at given square, assuming no blocks
    /// Used by the constructor of the board for initialization of the lookup table
    fn setup_king_lookup_table() -> [u64; 64] {
        let mut lookup_table: [u64; 64] = [0; 64];
        let mut current_bit: u64 = 1;
        for i in 0..64 {
            let is_east: bool = if i % 8 == 0 { true } else { false };
            let is_west: bool = if i % 8 == 7 { true } else { false };
            let is_north: bool = if i / 8 == 7 { true } else { false };
            let is_south: bool = if i / 8 == 0 { true } else { false };

            if !is_east { lookup_table[i] |= current_bit >> 1 };
            if !is_west { lookup_table[i] |= current_bit << 1 };
            if !is_north { lookup_table[i] |= current_bit << 8 };
            if !is_south { lookup_table[i] |= current_bit >> 8 };
            if !is_east && !is_north { lookup_table[i] |= current_bit << 7 };
            if !is_west && !is_north { lookup_table[i] |= current_bit << 9 };
            if !is_west && !is_south { lookup_table[i] |= current_bit >> 7 };
            if !is_east && !is_south { lookup_table[i] |= current_bit >> 9 };

            current_bit <<= 1;
        }

        lookup_table
    }

    /// Generates moves of a king based on the current board situation and updates self
    fn generate_king_moves(&mut self) {
        let bitboard = self.generate_king_moves_bitboard();

        self.convert_king_moves(bitboard);

        if self.board.active_player == WHITE { self.generate_white_castling() }
        else { self.generate_black_castling() }
    }

    /// Outputs a bitboard of king moves, based on the current board situation
    fn generate_king_moves_bitboard(&self) -> u64 {
        self.king_lookup_table[u64::checked_ilog2(self.board.piece_bitboards[6 * self.board.active_player as usize])
            .unwrap_or_default() as usize]
            & self.board.empty_bitboard
    }

    /// Converts a bitboard of king moves into a list of moves and updates self
    /// Does not consider castling
    /// parameters:
    ///     move_bitboard - bitboards of squares targeted by a move subgroup
    fn convert_king_moves(&mut self, move_bitboard: u64) {
        let mut bitboard = move_bitboard;
        let origin = u64::checked_ilog2(self.board.piece_bitboards[6 * self.board.active_player as usize])
            .unwrap_or_default() as u8;

        while bitboard != 0 {
            let tile = bitboard & bitboard.wrapping_neg();
            bitboard -= tile;

            let target = u64::checked_ilog2(tile).unwrap_or_default() as u8;

            self.moves.push(Move {origin, target, promotion: 0, piece: KING});
        }
    }

    /// Generates legal castling moves for white based on the current board situation and updates self
    /// Updates directly the list of moves
    fn generate_white_castling(&mut self) {
        if self.is_in_check() { return; }

        if self.board.castling_rights[0] && self.board.main_bitboard & 6 == 0
            && !self.is_edge_square_attacked_by_black(2) && !self.is_edge_square_attacked_by_black(1)
        {
            self.moves.push(Move {origin: 3, target: 1, promotion: 1, piece: KING});
        }

        let queenside_bitboard: u64 = 0b0000000000000000000000000000000000000000000000000000000000110000;

        if self.board.castling_rights[1] && self.board.main_bitboard & queenside_bitboard == 0
            && !self.is_edge_square_attacked_by_black(4) && !self.is_edge_square_attacked_by_black(5)
        {
            self.moves.push(Move {origin: 3, target: 5, promotion: 1, piece: KING});
        }
    }

    /// Generates legal castling moves for black based on the current board situation and updates self
    /// Updates directly the list of moves
    fn generate_black_castling(&mut self) {
        if self.is_in_check() { return; }

        let kingside_bitboard: u64 = 0b0000011000000000000000000000000000000000000000000000000000000000;
        if self.board.castling_rights[2] && self.board.main_bitboard & kingside_bitboard == 0
            && !self.is_edge_square_attacked_by_white(58) && !self.is_edge_square_attacked_by_white(57)
        {
            self.moves.push(Move {origin: 59, target: 57, promotion: 1, piece: KING});
        }

        let queenside_bitboard: u64 = 0b0011000000000000000000000000000000000000000000000000000000110000;
        if self.board.castling_rights[3] && self.board.main_bitboard & queenside_bitboard == 0
            && !self.is_edge_square_attacked_by_white(60) && !self.is_edge_square_attacked_by_white(61)
        {
            self.moves.push(Move {origin: 59, target: 61, promotion: 1, piece: KING});
        }
    }

    /// Checks if the given square on the 1st rank is attacked by black
    /// Inputs a given 1st rank square (u8)
    /// Outputs true/false
    fn is_edge_square_attacked_by_black(&self, square: u8) -> bool {
        let tile = 1 << square;

        // Check if a pawn or king attacks the square
        if (tile & NOT_FILE_A_MASK) << 7 & (self.board.piece_bitboards[7] | self.board.piece_bitboards[6]) != 0 { return true; }
        if (tile & NOT_FILE_H_MASK) << 9 & (self.board.piece_bitboards[7] | self.board.piece_bitboards[6]) != 0 { return true; }
        if tile << 8 & self.board.piece_bitboards[6] != 0 { return true; }
        // todo: Check if a knight attacks the square
        // todo: Check if a bishop attacks the square
        // todo: Check if a rook attacks the square
        // todo: Check if a queen attacks the square

        false
    }

    /// Checks if the given square on the 8th rank is attacked by white
    /// Inputs a given 8th rank square (u8)
    /// Outputs true/false
    fn is_edge_square_attacked_by_white(&self, square: u8) -> bool {
        let tile = 1 << square;

        // Check if a pawn or king attacks the square
        if (tile & NOT_FILE_H_MASK) >> 7 & (self.board.piece_bitboards[1] | self.board.piece_bitboards[0]) != 0 { return true; }
        if (tile & NOT_FILE_A_MASK) >> 9 & (self.board.piece_bitboards[1] | self.board.piece_bitboards[0]) != 0 { return true; }
        if tile >> 8 & self.board.piece_bitboards[0] != 0 { return true; }
        // todo: Check if a knight attacks the square
        // todo: Check if a bishop attacks the square
        // todo: Check if a rook attacks the square
        // todo: Check if a queen attacks the square

        false
    }

    /// Checks if the given square is attacked by white
    /// Inputs a 64-bit number with one bit set to 1 as a tile indication
    /// Outputs true/false
    fn is_square_attacked_by_white(&self, tile: u64) -> bool {
        // Check if a pawn attacks the square
        if (tile & NOT_FILE_H_MASK) >> 7 & self.board.piece_bitboards[1] != 0 { return true; }
        if (tile & NOT_FILE_A_MASK) >> 9 & self.board.piece_bitboards[1] != 0 { return true; }
        if tile & self.king_lookup_table[u64::checked_ilog2(self.board.piece_bitboards[0])
            .unwrap_or_default() as usize] != 0 { return true; }
        // todo: Check if a knight attacks the square
        // todo: Check if a bishop attacks the square
        // todo: Check if a rook attacks the square
        // todo: Check if a queen attacks the square

        false
    }

    /// Checks if the given square is attacked by black
    /// Inputs a 64-bit number with one bit set to 1 as a tile indication
    /// Outputs true/false
    fn is_square_attacked_by_black(&self, tile: u64) -> bool {
        // Check if a pawn attacks the square
        if (tile & NOT_FILE_A_MASK) << 7 & self.board.piece_bitboards[7] != 0 { return true; }
        if (tile & NOT_FILE_H_MASK) << 9 & self.board.piece_bitboards[7] != 0 { return true; }
        if tile & self.king_lookup_table[u64::checked_ilog2(self.board.piece_bitboards[6]).unwrap_or_default() as usize] != 0 { return true; }
        // todo: Check if a knight attacks the square
        // todo: Check if a bishop attacks the square
        // todo: Check if a rook attacks the square
        // todo: Check if a queen attacks the square

        false
    }

    /// Checks if the king of the current player is in check
    /// Outputs true/false
    fn is_in_check(&self) -> bool {
        if self.board.active_player == BLACK {
            return self.is_square_attacked_by_white(self.board.piece_bitboards[6]);
        }
        self.is_square_attacked_by_black(self.board.piece_bitboards[0])
    }

    /// PAWN MOVE GENERATION

    /// WHITE PAWN MOVE GENERATION

    /// Generates moves of white pawns based on the current board situation and updates self
    fn generate_white_pawn_moves(&mut self) {
        let push_bitboard = self.generate_white_push_bitboard();

        let double_push_bitboard = self.generate_white_double_push_bitboard(push_bitboard);

        let left_capture_bitboard = self.generate_white_pawn_left_capture_bitboard();

        let right_capture_bitboard: u64 = self.generate_white_pawn_right_capture_bitboard();

        self.convert_white_pawn_moves(push_bitboard, 8);
        self.convert_white_pawn_moves(double_push_bitboard, 16);
        self.convert_white_pawn_moves(left_capture_bitboard, 9);
        self.convert_white_pawn_moves(right_capture_bitboard, 7);
    }

    /// Converts a bitboard of white pawn moves and a move shift into a list of moves and updates self
    /// Should be used separately for single pushes, double pushes, left captures, right captures
    /// parameters:
    ///     move_bitboard - bitboards of squares targeted by a move subgroup
    ///     shift - move shift that is needed to decode the origin square for a given move subgroup, given their target squares
    fn convert_white_pawn_moves(&mut self, move_bitboard: u64, shift: u8) {
        let mut bitboard = move_bitboard;

        while bitboard != 0 {
            let tile = bitboard & bitboard.wrapping_neg();

            bitboard -= tile;
            let target = u64::checked_ilog2(tile).unwrap_or_default() as u8;
            let origin = u64::checked_ilog2((tile) >> shift).unwrap_or_default() as u8 ;

            if target < UPPER_RANK_LOWEST_TILE
            {
                self.moves.push(Move {
                    origin,
                    target,
                    promotion: 0,
                    piece: Piece::PAWN,
                });
                continue;
            }

            for i in 2..6
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

    /// Outputs a bitboard of double push white pawn moves, given the input of single push bitboard, based on the current board situation
    /// parameters:
    ///     push_bitboard - bitboard of white pawn single pushes
    fn generate_white_double_push_bitboard(&self, push_bitboard: u64) -> u64 {
        let double_push_mask = 0b0000000000000000000000000000000000000000111111110000000000000000;
        let double_push_bitboard: u64 =
            (push_bitboard & double_push_mask) << 8
                & self.board.empty_bitboard;
        double_push_bitboard
    }

    /// Outputs a bitboard of single push white pawn moves, based on the current board situation
    fn generate_white_push_bitboard(&self) -> u64 {
        self.board.piece_bitboards[Piece::PAWN as usize] << 8
            & self.board.empty_bitboard
    }

    /// Outputs a bitboard of white left pawn captures, based on the current board situation
    /// Shifts by 9 bits
    /// Should be used separately for left captures and right captures.
    /// Accounts for en passant
    fn generate_white_pawn_left_capture_bitboard(&self) -> u64 {
        let mut attack_options = self.board.colour_bitboards[BLACK as usize];
        if self.board.en_passant_possibility < 64 {
            let en_passant_tile = 1 << self.board.en_passant_possibility;
            attack_options |= en_passant_tile;
        }
        let index = Piece::PAWN as usize;
        let capture_bitboard: u64 = ((self.board.piece_bitboards[index] & NOT_FILE_A_MASK) << 9) & attack_options;
        capture_bitboard
    }
    /// Outputs a bitboard of white pawn captures, based on the current board situation
    /// Shifts by 7 bits
    /// Should be used separately for left captures and right captures.
    /// Accounts for en passant
    fn generate_white_pawn_right_capture_bitboard(&self) -> u64 {
        let mut attack_options = self.board.colour_bitboards[BLACK as usize];
        if self.board.en_passant_possibility < 64 {
            let en_passant_tile = 1 << self.board.en_passant_possibility;
            attack_options |= en_passant_tile;
        }
        let index = Piece::PAWN as usize;
        let capture_bitboard: u64 = ((self.board.piece_bitboards[index] & NOT_FILE_H_MASK) << 7) & attack_options;
        capture_bitboard
    }

    /// BLACK PAWN MOVE GENERATION

    /// Generates moves of black pawns based on the current board situation and updates self
    fn generate_black_pawn_moves(&mut self) {
        let push_bitboard = self.generate_black_push_bitboard();

        let double_push_bitboard = self.generate_black_double_push_bitboard(push_bitboard);

        let left_capture_bitboard = self.generate_black_pawn_left_capture_bitboard();

        let right_capture_bitboard: u64 = self.generate_black_pawn_right_capture_bitboard();

        self.convert_black_pawn_moves(push_bitboard, 8);
        self.convert_black_pawn_moves(double_push_bitboard, 16);
        self.convert_black_pawn_moves(left_capture_bitboard, 9);
        self.convert_black_pawn_moves(right_capture_bitboard, 7);
    }

    /// Converts a bitboard of black pawn moves and a move shift into a list of moves and updates self
    /// Should be used separately for single pushes, double pushes, left captures, right captures
    /// parameters:
    ///     move_bitboard - bitboards of squares targeted by a move subgroup
    ///     shift - move shift that is needed to decode the origin square for a given move subgroup, given their target squares
    fn convert_black_pawn_moves(&mut self, push_bitboard: u64, shift: u8) {
        let mut bitboard = push_bitboard;

        while bitboard != 0 {
            let tile = bitboard & bitboard.wrapping_neg();

            bitboard -= tile;
            let target = u64::checked_ilog2(tile).unwrap_or_default() as u8;
            let origin = u64::checked_ilog2(tile << shift).unwrap_or_default() as u8 ;

            if target > LOWER_RANK_HIGHEST_TILE
            {
                self.moves.push(Move {
                    origin,
                    target,
                    promotion: 0,
                    piece: Piece::PAWN,
                });
                continue;
            }

            for i in 2..6
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

    /// Outputs a bitboard of double push black pawn moves, given the input of single push bitboard, based on the current board situation
    /// parameters:
    ///     push_bitboard - bitboard of black pawn single pushes
    fn generate_black_double_push_bitboard(&self, push_bitboard: u64) -> u64 {
        let double_push_mask = 0b0000000000000000111111110000000000000000000000000000000000000000;
        let double_push_bitboard: u64 =
            (push_bitboard & double_push_mask) >> 8
                & self.board.empty_bitboard;
        double_push_bitboard
    }

    /// Outputs a bitboard of single push black pawn moves, based on the current board situation
    fn generate_black_push_bitboard(&self) -> u64 {
        self.board.piece_bitboards[Piece::PAWN as usize + 6] >> 8
            & self.board.empty_bitboard
    }

    /// Outputs a bitboard of black pawn left captures, based on the current board situation
    /// Shifts by 9 bits
    /// Should be used separately for left captures and right captures.
    /// Accounts for en passant
    fn generate_black_pawn_left_capture_bitboard(&self) -> u64 {
        let mut attack_options = self.board.colour_bitboards[WHITE as usize];
        if self.board.en_passant_possibility < 64 {
            let en_passant_tile = 1 << self.board.en_passant_possibility;
            attack_options |= en_passant_tile;
        }
        let index = Piece::PAWN as usize + 6;
        let capture_bitboard: u64 = (self.board.piece_bitboards[index] & NOT_FILE_H_MASK) >> 9 & attack_options;
        capture_bitboard
    }
    /// Outputs a bitboard of black pawn right captures, based on the current board situation
    /// Shifts by 7 bits
    /// Should be used separately for left captures and right captures.
    /// Accounts for en passant
    fn generate_black_pawn_right_capture_bitboard(&self) -> u64 {
        let mut attack_options = self.board.colour_bitboards[WHITE as usize];
        if self.board.en_passant_possibility < 64 {
            let en_passant_tile = 1 << self.board.en_passant_possibility;
            attack_options |= en_passant_tile;
        }
        let index = Piece::PAWN as usize + 6;
        let capture_bitboard: u64 = (self.board.piece_bitboards[index] & NOT_FILE_A_MASK) >> 7 & attack_options;
        capture_bitboard
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use crate::board::{Board, START_POSITION};
    use super::*;

    fn compare_vecs<Move: PartialEq + Eq + std::hash::Hash + Clone>
    (vec1: &Vec<Move>, vec2: &Vec<Move>) -> bool {
        if vec1.len() != vec2.len() {
            return false;
        }

        let set1: HashSet<Move> = HashSet::from_iter(vec1.iter().cloned());
        let set2: HashSet<Move> = HashSet::from_iter(vec2.iter().cloned());

        return set1 == set2;
    }

    #[test]
    fn start_position() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);
        let mut move_list = MoveList::new(&board);

        let expected_push_bitboard: u64 = 0b0000000000000000000000000000000000000000111111110000000000000000;
        let expected_double_push_bitboard: u64 = 0b0000000000000000000000000000000011111111000000000000000000000000;
        let expected_left_pawn_capture_bitboard: u64 = 0;
        let expected_right_pawn_capture_bitboard: u64 = 0;
        let expected_king_bitboard: u64 = 0;

        let mut expected_push_moves = Vec::<Move>::new();
        expected_push_moves.push(Move{ origin: 8, target: 16, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 9, target: 17, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 10, target: 18, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 11, target: 19, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 12, target: 20, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 13, target: 21, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 14, target: 22, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 15, target: 23, promotion: 0, piece: Piece::PAWN });
        let mut expected_double_push_moves = Vec::<Move>::new();
        expected_double_push_moves.push(Move{ origin: 8, target: 24, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 9, target: 25, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 10, target: 26, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 11, target: 27, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 12, target: 28, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 13, target: 29, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 14, target: 30, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 15, target: 31, promotion: 0, piece: Piece::PAWN });
        let mut expected_left_pawn_captures = Vec::<Move>::new();
        let mut expected_right_pawn_captures = Vec::<Move>::new();
        let expected_pawn_moves = [expected_push_moves.clone(), expected_double_push_moves.clone(),
            expected_left_pawn_captures.clone(), expected_right_pawn_captures.clone()].concat();

        assert_eq!(move_list.generate_white_push_bitboard(), expected_push_bitboard);
        assert_eq!(move_list.generate_white_double_push_bitboard(expected_push_bitboard), expected_double_push_bitboard);
        assert_eq!(move_list.generate_white_pawn_left_capture_bitboard(), expected_left_pawn_capture_bitboard);
        assert_eq!(move_list.generate_white_pawn_right_capture_bitboard(), expected_right_pawn_capture_bitboard);
        assert_eq!(move_list.generate_king_moves_bitboard(), expected_king_bitboard);

        // PAWN MOVES

        move_list.convert_white_pawn_moves(expected_push_bitboard, 8);
        assert!(compare_vecs(&move_list.moves, &expected_push_moves));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_white_pawn_moves(expected_double_push_bitboard, 16);
        assert!(compare_vecs(&move_list.moves, &expected_double_push_moves));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_white_pawn_moves(expected_left_pawn_capture_bitboard, 9);
        assert!(compare_vecs(&move_list.moves, &expected_left_pawn_captures));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_white_pawn_moves(expected_right_pawn_capture_bitboard, 7);
        assert!(compare_vecs(&move_list.moves, &expected_right_pawn_captures));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_white_pawn_moves();
        assert!(compare_vecs(&move_list.moves, &expected_pawn_moves));

        // KING MOVES

        let mut expected_king_moves = Vec::<Move>::new();

        move_list.moves = Vec::<Move>::new();
        move_list.generate_king_moves();
        assert!(compare_vecs(&move_list.moves, &expected_king_moves));

        // ALL MOVES

        let expected_moves = [expected_pawn_moves].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.generate_moves();
        assert!(compare_vecs(&move_list.moves, &expected_moves));
    }

    #[test]
    fn position_2() {
        let mut board = Board::new();
        board.read_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        let mut move_list = MoveList::new(&board);

        let expected_push_bitboard: u64 = 0b0000000000000000111111110000000000000000000000000000000000000000;
        let expected_double_push_bitboard: u64 = 0b0000000000000000000000001111111100000000000000000000000000000000;
        let expected_left_pawn_capture_bitboard: u64 = 0;
        let expected_right_pawn_capture_bitboard: u64 = 0;
        let expected_king_bitboard: u64 = 0;

        let mut expected_push_moves = Vec::<Move>::new();
        expected_push_moves.push(Move{ origin: 55, target: 47, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 54, target: 46, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 53, target: 45, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 52, target: 44, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 51, target: 43, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 50, target: 42, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 49, target: 41, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 48, target: 40, promotion: 0, piece: Piece::PAWN });
        let mut expected_double_push_moves = Vec::<Move>::new();
        expected_double_push_moves.push(Move{ origin: 55, target: 39, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 54, target: 38, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 53, target: 37, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 52, target: 36, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 51, target: 35, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 50, target: 34, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 49, target: 33, promotion: 0, piece: Piece::PAWN });
        expected_double_push_moves.push(Move{ origin: 48, target: 32, promotion: 0, piece: Piece::PAWN });
        let mut expected_left_pawn_captures = Vec::<Move>::new();
        let mut expected_right_pawn_captures = Vec::<Move>::new();
        let expected_pawn_moves = [expected_push_moves.clone(), expected_double_push_moves.clone(),
            expected_left_pawn_captures.clone(), expected_right_pawn_captures.clone()].concat();

        assert_eq!(move_list.generate_black_push_bitboard(), expected_push_bitboard);
        assert_eq!(move_list.generate_black_double_push_bitboard(expected_push_bitboard), expected_double_push_bitboard);
        assert_eq!(move_list.generate_black_pawn_left_capture_bitboard(), expected_left_pawn_capture_bitboard);
        assert_eq!(move_list.generate_black_pawn_right_capture_bitboard(), expected_right_pawn_capture_bitboard);
        assert_eq!(move_list.generate_king_moves_bitboard(), expected_king_bitboard);

        // PAWN MOVES

        move_list.convert_black_pawn_moves(expected_push_bitboard, 8);
        assert!(compare_vecs(&move_list.moves, &expected_push_moves));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_black_pawn_moves(expected_double_push_bitboard, 16);
        assert!(compare_vecs(&move_list.moves, &expected_double_push_moves));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_black_pawn_moves(expected_left_pawn_capture_bitboard, 9);
        assert!(compare_vecs(&move_list.moves, &expected_left_pawn_captures));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_black_pawn_moves(expected_right_pawn_capture_bitboard, 7);
        assert!(compare_vecs(&move_list.moves, &expected_right_pawn_captures));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_black_pawn_moves();
        assert!(compare_vecs(&move_list.moves, &expected_pawn_moves));

        // KING MOVES

        let mut expected_king_moves = Vec::<Move>::new();

        move_list.moves = Vec::<Move>::new();
        move_list.generate_king_moves();
        assert!(compare_vecs(&move_list.moves, &expected_king_moves));

        // ALL MOVES

        let expected_moves = [expected_pawn_moves].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.generate_moves();
        assert!(compare_vecs(&move_list.moves, &expected_moves));

    }

    #[test]
    fn position_4() {
        let mut board = Board::new();
        board.read_fen("r3k3/1Pr5/5pp1/3pPBPP/1b1P2Qq/5N2/P7/RK6 w q d6 1 25");
        let mut move_list = MoveList::new(&board);

        let expected_push_bitboard: u64 =               0b0100000000000000000010010000000000000000100000000000000000000000;
        let expected_double_push_bitboard: u64 =        0b0000000000000000000000000000000010000000000000000000000000000000;
        let expected_left_pawn_capture_bitboard: u64 =  0b1000000000000000000101100000000000000000000000000000000000000000;
        let expected_right_pawn_capture_bitboard: u64 = 0b0000000000000000000001000000000000000000000000000000000000000000;
        let expected_king_bitboard: u64 =               0b0000000000000000000000000000000000000000000000000110000000100000;

        let mut expected_push_moves = Vec::<Move>::new();
        expected_push_moves.push(Move{ origin: 15, target: 23, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 32, target: 40, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 35, target: 43, promotion: 0, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 54, target: 62, promotion: 2, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 54, target: 62, promotion: 3, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 54, target: 62, promotion: 4, piece: Piece::PAWN });
        expected_push_moves.push(Move{ origin: 54, target: 62, promotion: 5, piece: Piece::PAWN });
        let mut expected_double_push_moves = Vec::<Move>::new();
        expected_double_push_moves.push(Move{ origin: 15, target: 31, promotion: 0, piece: Piece::PAWN });
        let mut expected_left_pawn_captures = Vec::<Move>::new();
        expected_left_pawn_captures.push(Move{ origin: 32, target: 41, promotion: 0, piece: Piece::PAWN });
        expected_left_pawn_captures.push(Move{ origin: 33, target: 42, promotion: 0, piece: Piece::PAWN });
        expected_left_pawn_captures.push(Move{ origin: 35, target: 44, promotion: 0, piece: Piece::PAWN });
        expected_left_pawn_captures.push(Move{ origin: 54, target: 63, promotion: 2, piece: Piece::PAWN });
        expected_left_pawn_captures.push(Move{ origin: 54, target: 63, promotion: 3, piece: Piece::PAWN });
        expected_left_pawn_captures.push(Move{ origin: 54, target: 63, promotion: 4, piece: Piece::PAWN });
        expected_left_pawn_captures.push(Move{ origin: 54, target: 63, promotion: 5, piece: Piece::PAWN });
        let mut expected_right_pawn_captures = Vec::<Move>::new();
        expected_right_pawn_captures.push(Move{ origin: 35, target: 42, promotion: 0, piece: Piece::PAWN });
        let expected_pawn_moves = [expected_push_moves.clone(), expected_double_push_moves.clone(),
            expected_left_pawn_captures.clone(), expected_right_pawn_captures.clone()].concat();

        assert_eq!(move_list.generate_white_push_bitboard(), expected_push_bitboard);
        assert_eq!(move_list.generate_white_double_push_bitboard(expected_push_bitboard), expected_double_push_bitboard);
        assert_eq!(move_list.generate_white_pawn_left_capture_bitboard(), expected_left_pawn_capture_bitboard);
        assert_eq!(move_list.generate_white_pawn_right_capture_bitboard(), expected_right_pawn_capture_bitboard);
        assert_eq!(move_list.generate_king_moves_bitboard(), expected_king_bitboard);

        // PAWN MOVES

        move_list.convert_white_pawn_moves(expected_push_bitboard, 8);
        assert!(compare_vecs(&move_list.moves, &expected_push_moves));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_white_pawn_moves(expected_double_push_bitboard, 16);
        assert!(compare_vecs(&move_list.moves, &expected_double_push_moves));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_white_pawn_moves(expected_left_pawn_capture_bitboard, 9);
        assert!(compare_vecs(&move_list.moves, &expected_left_pawn_captures));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_white_pawn_moves(expected_right_pawn_capture_bitboard, 7);
        assert!(compare_vecs(&move_list.moves, &expected_right_pawn_captures));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_white_pawn_moves();
        assert!(compare_vecs(&move_list.moves, &expected_pawn_moves));

        // KING MOVES

        let mut expected_king_moves = Vec::<Move>::new();
        expected_king_moves.push(Move{ origin: 6, target: 5, promotion: 0, piece: KING });
        expected_king_moves.push(Move{ origin: 6, target: 13, promotion: 0, piece: KING });
        expected_king_moves.push(Move{ origin: 6, target: 14, promotion: 0, piece: KING });

        move_list.moves = Vec::<Move>::new();
        move_list.generate_king_moves();
        assert!(compare_vecs(&move_list.moves, &expected_king_moves));

        // ALL MOVES

        let expected_moves = [expected_pawn_moves, expected_king_moves].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.generate_moves();
        assert!(compare_vecs(&move_list.moves, &expected_moves));
    }

    #[test]
    fn check_king_lookup() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);
        let mut move_list = MoveList::new(&board);

        assert_eq!(0b0000000000000000000000000000000000000000000000000000001100000010, move_list.king_lookup_table[0]);
        assert_eq!(0b0000000000000000000000000000000000000000000000000000011100000101, move_list.king_lookup_table[1]);
        assert_eq!(0b0000000000000000000000000000000000000000000000001100000001000000, move_list.king_lookup_table[7]);
        assert_eq!(0b0000000000000000000000000000000000011100000101000001110000000000, move_list.king_lookup_table[19]);
        assert_eq!(0b0000000000000000000000000000000000000011000000100000001100000000, move_list.king_lookup_table[16]);
        assert_eq!(0b0000000000000000000000000000000011000000010000001100000000000000, move_list.king_lookup_table[23]);
        assert_eq!(0b0100000011000000000000000000000000000000000000000000000000000000, move_list.king_lookup_table[63]);
        assert_eq!(0b1010000011100000000000000000000000000000000000000000000000000000, move_list.king_lookup_table[62]);
        assert_eq!(0b0000001000000011000000000000000000000000000000000000000000000000, move_list.king_lookup_table[56]);
    }

    #[test]
    fn check_castling_white_both_possible() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/8/8/R3K2R w KQ - 1 1");
        let mut move_list = MoveList::new(&board);

        let mut expected_move_list = Vec::<Move>::new();

        expected_move_list.push(Move {origin: 3, target: 1, promotion: 1, piece: KING});
        expected_move_list.push(Move {origin: 3, target: 5, promotion: 1, piece: KING});

        move_list.generate_white_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }
}