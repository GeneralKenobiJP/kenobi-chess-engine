//! Move generator
//! Generates a vector of pseudo-legal moves based on the input board position
//! Involves bitboards, magic bitboards.

use std::collections::HashSet;
use std::time::Instant;
use crate::piece::Piece;
use crate::board::{Board, CASTLE_BLACK_KINGSIDE_FLAGS, CASTLE_BLACK_KINGSIDE_MASK, CASTLE_BLACK_QUEENSIDE_FLAGS, CASTLE_BLACK_QUEENSIDE_MASK, CASTLE_WHITE_KINGSIDE_FLAGS, CASTLE_WHITE_KINGSIDE_MASK, CASTLE_WHITE_QUEENSIDE_FLAGS, CASTLE_WHITE_QUEENSIDE_MASK, FILE_1_MASK, FILE_8_MASK, LOWER_RANK_HIGHEST_TILE, NOT_FILE_A_MASK, NOT_FILE_H_MASK, UNCASTLE_BLACK_KINGSIDE_FLAGS, UNCASTLE_BLACK_QUEENSIDE_FLAGS, UNCASTLE_WHITE_KINGSIDE_FLAGS, UNCASTLE_WHITE_QUEENSIDE_FLAGS, UPPER_RANK_LOWEST_TILE};
use crate::piece::Colour::{BLACK, WHITE};
use crate::piece::Piece::{BISHOP, KING, KNIGHT, PAWN, QUEEN, ROOK};
use crate::magic_hasher::{magic_hash_bishop, magic_hash_rook, MAGIC_MASK_BISHOP, MAGIC_MASK_ROOK};

const KNIGHT_SHIFTS: [i8; 8] = [17, 10, -6, -15, -17, -10, 6, 15]; // Beginning on NW, counter-clockwise
const INITIAL_STACK_CAPACITY: usize = 30; // used by MoveList constructor
const NO_CAPTURE: u8 = 1 << 4;
const NO_PASSANT: u8 = 64;
const EN_PASSANT_MASK: u64 = 0x000000FFFF000000;

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct Move {
    pub origin: u8,
    pub target: u8,
    pub promotion: u8,
    pub piece: Piece
}

impl Move {
    /// Converts Move object to an algebraic notation used by UCI
    /// E.g.: Move{54, 63, 2, PAWN} -> b7a8q
    pub fn to_algebraic_notation(&self) -> String {
        let mut notation = String::new();
        notation.push_str(&*Board::decode_square(self.origin));
        notation.push_str(&*Board::decode_square(self.target));

        match self.promotion {
            2 => notation.push('q'),
            3 => notation.push('r'),
            4 => notation.push('b'),
            5 => notation.push('n'),
            _ => ()
        }

        notation
    }
}

pub struct MoveList<'a> {
    board: &'a mut Board,
    moves: Vec<Move>,
    capture_history: Vec<u8>, // used as stack, 16 == no capture
    en_passant_history: Vec<u8>, // used as stack, 64 == no passant
    castling_rights_history: Vec<[bool;4]>, // used as stack
    king_lookup_table: [u64; 64], // should be treated as immutable after setup
    knight_lookup_table: [u64; 64], // should be treated as immutable after setup
    rook_magic_bitboard: Vec<u64>,
    bishop_magic_bitboard: Vec<u64>
}

impl<'a> MoveList<'a> {
    pub fn new(board: &'a mut Board) -> Self {
        MoveList {
            board,
            moves: Vec::new(),
            capture_history: Vec::with_capacity(INITIAL_STACK_CAPACITY),
            en_passant_history: Vec::with_capacity(INITIAL_STACK_CAPACITY),
            castling_rights_history: Vec::with_capacity(INITIAL_STACK_CAPACITY),
            king_lookup_table: Self::setup_king_lookup_table(),
            knight_lookup_table: Self::setup_knight_lookup_table(),
            rook_magic_bitboard: Self::setup_rook_magic_bitboard(),
            bishop_magic_bitboard: Self::setup_bishop_magic_bitboard()
        }
    }

    /// Getter for the move list
    pub fn get_moves(&self) -> &Vec<Move> {
        &self.moves
    }

    /// Getter for the board
    pub fn get_board(&self) -> &Board {
        &self.board
    }
    
    /// Makes a move on the board, given a move.
    pub fn make_move(&mut self, piece_move: &Move) {
        self.en_passant_history.push(self.board.en_passant_possibility);
        self.castling_rights_history.push(self.board.castling_rights);

        let origin = 1 << piece_move.origin;
        let target = 1 << piece_move.target;
        let active_player = self.board.active_player as usize;
        let piece = piece_move.piece.clone() as usize;
        let promotion = piece_move.promotion;
        if promotion == 1 {
            self.make_castling_move(piece_move);
            self.board.switch_active_player();
            return;
        }
        let final_piece = if promotion == 0 {piece} else { promotion as usize };
        let target_mask = !target;

        self.board.main_bitboard ^= origin;
        self.board.main_bitboard |= target;
        self.board.empty_bitboard = !self.board.main_bitboard;
        self.board.colour_bitboards[active_player] ^= origin;
        self.board.colour_bitboards[active_player] |= target;
        self.board.piece_bitboards[6*active_player + piece] ^= origin;
        self.board.piece_bitboards[6*active_player + final_piece] |= target;

        if piece_move.piece == PAWN {
            if piece_move.target == self.board.en_passant_possibility {
                let en_passant_target = ((target << 8) | (target >> 8)) & EN_PASSANT_MASK;
                self.board.main_bitboard ^= en_passant_target;
                self.board.empty_bitboard |= en_passant_target;
                self.board.colour_bitboards[self.board.inactive_player as usize] ^= en_passant_target;
                self.board.piece_bitboards[6 * self.board.inactive_player as usize + PAWN as usize] ^= en_passant_target;

                self.capture_history.push(NO_CAPTURE);
                self.board.en_passant_possibility = NO_PASSANT;

                self.board.switch_active_player();
                return;
            }
            if piece_move.target.abs_diff(piece_move.origin) == 16 {
                self.board.en_passant_possibility = (piece_move.target + piece_move.origin)/2;
            }
            else { self.board.en_passant_possibility = NO_PASSANT; }
        }
        else if piece_move.piece == KING {
            self.board.en_passant_possibility = NO_PASSANT;

            self.board.castling_rights[2*active_player] = false;
            self.board.castling_rights[2*active_player + 1] = false;
        }
        else { self.board.en_passant_possibility = NO_PASSANT; }

        match piece_move.origin {
            0 => self.board.castling_rights[WHITE as usize] = false,
            7 => self.board.castling_rights[WHITE as usize + 1] = false,
            56 => self.board.castling_rights[2 * BLACK as usize] = false,
            63 => self.board.castling_rights[2* BLACK as usize + 1] = false,
            _ => ()
        }
        match piece_move.target {
            0 => self.board.castling_rights[WHITE as usize] = false,
            7 => self.board.castling_rights[WHITE as usize + 1] = false,
            56 => self.board.castling_rights[2 * BLACK as usize] = false,
            63 => self.board.castling_rights[2 * BLACK as usize + 1] = false,
            _ => ()
        }

        self.board.colour_bitboards[self.board.inactive_player as usize] &= target_mask;
        let mut capture: u8 = NO_CAPTURE;
        for index in 6*self.board.inactive_player as usize..=6*self.board.inactive_player as usize + 5 {
            let new_bitboard = self.board.piece_bitboards[index] & target_mask;
            let capture_bitboard = new_bitboard ^ self.board.piece_bitboards[index];
            if capture_bitboard != 0
            {
                capture = index as u8;
                self.board.piece_bitboards[index] = new_bitboard;
            }
        }
        self.capture_history.push(capture);

        self.board.switch_active_player();
    }

    /// Makes a castling move on the board, given a castling move.
    /// Called by make_move, should not be called independently.
    fn make_castling_move(&mut self, piece_move: &Move) {
        let flag_pointer;
        let mask;

        self.board.castling_rights[2 * self.board.active_player as usize] = false;
        self.board.castling_rights[2 * self.board.active_player as usize + 1] = false;

        if piece_move.origin == 3 {
            if piece_move.target == 1 {
                flag_pointer = &CASTLE_WHITE_KINGSIDE_FLAGS;
                mask = CASTLE_WHITE_KINGSIDE_MASK;
            }
            else {
                flag_pointer = &CASTLE_WHITE_QUEENSIDE_FLAGS;
                mask = CASTLE_WHITE_QUEENSIDE_MASK;
            }
        }
        else {
            if piece_move.target == 57 {
                flag_pointer = &CASTLE_BLACK_KINGSIDE_FLAGS;
                mask = CASTLE_BLACK_KINGSIDE_MASK;
            }
            else {
                flag_pointer = &CASTLE_BLACK_QUEENSIDE_FLAGS;
                mask = CASTLE_BLACK_QUEENSIDE_MASK;
            }
        }

        let summed_flag = flag_pointer[0] | flag_pointer[1];

        self.board.main_bitboard &= mask;
        self.board.main_bitboard |= summed_flag;
        self.board.empty_bitboard = !self.board.main_bitboard;
        self.board.colour_bitboards[self.board.active_player as usize] &= mask;
        self.board.colour_bitboards[self.board.active_player as usize] |= summed_flag;
        self.board.piece_bitboards[6*self.board.active_player as usize + KING as usize] &= mask;
        self.board.piece_bitboards[6*self.board.active_player as usize + KING as usize] |= flag_pointer[0];
        self.board.piece_bitboards[6*self.board.active_player as usize + ROOK as usize] &= mask;
        self.board.piece_bitboards[6*self.board.active_player as usize + ROOK as usize] |= flag_pointer[1];

        self.capture_history.push(NO_CAPTURE);
        self.board.en_passant_possibility = NO_PASSANT;
    }

    /// Unmakes a move on the board, given a move.
    pub fn unmake_move(&mut self, piece_move: &Move) {
        self.board.en_passant_possibility = self.en_passant_history.pop().unwrap_or_default();
        self.board.castling_rights = self.castling_rights_history.pop().unwrap_or_default();

        let origin = 1 << piece_move.target;
        let target = 1 << piece_move.origin;
        let active_player = self.board.inactive_player as usize; // !
        let piece = piece_move.piece.clone() as usize;
        let promotion = piece_move.promotion;
        if promotion == 1 {
            self.unmake_castling_move(piece_move);
            self.board.switch_active_player();
            return;
        }
        let target_piece = if promotion == 0 {piece} else { promotion as usize };

        self.board.main_bitboard ^= origin;
        self.board.main_bitboard |= target;
        self.board.empty_bitboard = !self.board.main_bitboard;
        self.board.colour_bitboards[active_player] ^= origin;
        self.board.colour_bitboards[active_player] |= target;
        self.board.piece_bitboards[6*active_player + target_piece] ^= origin;
        self.board.piece_bitboards[6*active_player + piece] |= target;

        if piece_move.piece == PAWN && piece_move.target == self.board.en_passant_possibility {
            let en_passant_target = ((origin << 8) | (origin >> 8)) & EN_PASSANT_MASK;
            self.board.main_bitboard |= en_passant_target;
            self.board.empty_bitboard ^= en_passant_target;
            self.board.colour_bitboards[self.board.active_player as usize] |= en_passant_target;
            self.board.piece_bitboards[6 * self.board.active_player as usize + PAWN as usize] |= en_passant_target;

            self.capture_history.pop();

            self.board.switch_active_player();
            return;
        }

        let capture = self.capture_history.pop().unwrap_or_default();
        if capture == NO_CAPTURE { self.board.switch_active_player(); return; }
        self.board.main_bitboard |= origin;
        self.board.empty_bitboard ^= origin;
        self.board.colour_bitboards[self.board.active_player as usize] |= origin;
        self.board.piece_bitboards[capture as usize] |= origin;

        self.board.switch_active_player();
    }

    /// Unmakes a castling move on the board, given a castling move.
    /// Called by unmake_move, should not be called independently.
    fn unmake_castling_move(&mut self, piece_move: &Move) {
        self.capture_history.pop();

        let flag_pointer;
        let mask;

        if piece_move.origin == 3 {
            if piece_move.target == 1 {
                flag_pointer = &UNCASTLE_WHITE_KINGSIDE_FLAGS;
                mask = CASTLE_WHITE_KINGSIDE_MASK;
            }
            else {
                flag_pointer = &UNCASTLE_WHITE_QUEENSIDE_FLAGS;
                mask = CASTLE_WHITE_QUEENSIDE_MASK;
            }
        }
        else {
            if piece_move.target == 57 {
                flag_pointer = &UNCASTLE_BLACK_KINGSIDE_FLAGS;
                mask = CASTLE_BLACK_KINGSIDE_MASK;
            }
            else {
                flag_pointer = &UNCASTLE_BLACK_QUEENSIDE_FLAGS;
                mask = CASTLE_BLACK_QUEENSIDE_MASK;
            }
        }

        let summed_flag = flag_pointer[0] | flag_pointer[1];

        self.board.main_bitboard &= mask;
        self.board.main_bitboard |= summed_flag;
        self.board.empty_bitboard = !self.board.main_bitboard;
        self.board.colour_bitboards[self.board.inactive_player as usize] &= mask;
        self.board.colour_bitboards[self.board.inactive_player as usize] |= summed_flag;
        self.board.piece_bitboards[6*self.board.inactive_player as usize + KING as usize] &= mask;
        self.board.piece_bitboards[6*self.board.inactive_player as usize + KING as usize] |= flag_pointer[0];
        self.board.piece_bitboards[6*self.board.inactive_player as usize + ROOK as usize] &= mask;
        self.board.piece_bitboards[6*self.board.inactive_player as usize + ROOK as usize] |= flag_pointer[1];
    }

    /// Generates moves and updates move list based on the situation on the board
    pub fn generate_moves(&mut self) {
        self.moves = Vec::new();

        self.generate_king_moves();

        if self.board.active_player == WHITE
        {
            self.generate_white_pawn_moves();
        }
        else {
            self.generate_black_pawn_moves();
        }

        self.generate_knight_moves();
        self.generate_rook_moves();
        self.generate_bishop_moves();
        self.generate_queen_moves();
    }

    /// Generates noisy moves and updates move list based on the situation on the board
    /// Noisy moves include captures, checks and promotions
    /// Should NOT be used for move generation
    /// Used for heuristics
    /// This method is somewhat slow due to generate_checks() being somewhat slow
    pub fn generate_noisy_moves(&mut self) {
        self.moves = Vec::new();

        self.generate_captures();
        if self.board.active_player == WHITE {
            self.generate_white_promotions();
        }
        else { self.generate_black_promotions(); }
        self.generate_checks();
    }

    /// Generates captures and updates move list based on the situation on the board
    /// Should NOT be used for move generation
    /// Used for heuristics
    /// Does NOT construct a new vector for moves
    pub fn generate_captures(&mut self) {
        if self.board.active_player == WHITE {
            self.generate_white_pawn_captures();
        }
        else { self.generate_black_pawn_captures() };

        self.generate_king_captures();
        self.generate_knight_captures();
        self.generate_bishop_captures();
        self.generate_rook_captures();
        self.generate_queen_captures();
    }

    /// Generates checks and updates move list based on the situation on the board
    /// Should NOT be used for move generation
    /// Used for heuristics
    /// Does NOT construct a new vector for moves
    /// Note: this method is somewhat slow
    pub fn generate_checks(&mut self) {
        let mut move_list = MoveList::new(&mut self.board);

        move_list.generate_moves();

        let moves = move_list.get_moves().clone();

        for piece_move in moves {
            move_list.make_move(&piece_move);
            let is_check = move_list.is_in_check();
            move_list.unmake_move(&piece_move);
            if is_check && !self.moves.contains(&piece_move) { self.moves.push(piece_move); }
        }
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

        if self.board.active_player == WHITE { self.generate_white_castling() } else { self.generate_black_castling() }
    }

    /// Generates king captures based on the current board situation and updates self
    /// Should NOT be used for regular move generation
    /// Used for heuristics
    fn generate_king_captures(&mut self) {
        let bitboard = self.generate_king_moves_bitboard() & self.board.colour_bitboards[self.board.inactive_player as usize];

        self.convert_king_moves(bitboard);
    }

    /// Outputs a bitboard of king moves, based on the current board situation
    fn generate_king_moves_bitboard(&self) -> u64 {
        self.king_lookup_table[u64::checked_ilog2(self.board.piece_bitboards[6 * self.board.active_player as usize])
            .unwrap_or_default() as usize]
            & (self.board.empty_bitboard | self.board.colour_bitboards[self.board.inactive_player as usize])
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

            self.moves.push(Move { origin, target, promotion: 0, piece: KING });
        }
    }

    /// Generates legal castling moves for white based on the current board situation and updates self
    /// Updates directly the list of moves
    fn generate_white_castling(&mut self) {
        if self.is_in_check() { return; }

        if self.board.castling_rights[0] && self.board.main_bitboard & 6 == 0
            && !self.is_edge_square_attacked_by_black(2) && !self.is_edge_square_attacked_by_black(1)
        {
            self.moves.push(Move { origin: 3, target: 1, promotion: 1, piece: KING });
        }

        let queenside_bitboard: u64 = 0b0000000000000000000000000000000000000000000000000000000001110000;

        if self.board.castling_rights[1] && self.board.main_bitboard & queenside_bitboard == 0
            && !self.is_edge_square_attacked_by_black(4) && !self.is_edge_square_attacked_by_black(5)
        {
            self.moves.push(Move { origin: 3, target: 5, promotion: 1, piece: KING });
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
            self.moves.push(Move { origin: 59, target: 57, promotion: 1, piece: KING });
        }

        let queenside_bitboard: u64 = 0b0111000000000000000000000000000000000000000000000000000000000000;
        if self.board.castling_rights[3] && (self.board.main_bitboard & queenside_bitboard) == 0
            && !self.is_edge_square_attacked_by_white(60) && !self.is_edge_square_attacked_by_white(61)
        {
            self.moves.push(Move { origin: 59, target: 61, promotion: 1, piece: KING });
        }
    }

    /// Checks if the given square on the 1st rank is attacked by black
    /// Inputs a given 1st rank square (u8)
    /// Outputs true/false
    fn is_edge_square_attacked_by_black(&self, square: u8) -> bool {
        let tile = 1 << square;

        // The main idea is to virtually place a piece of type X belonging to us on the given square
        // and check if it can attack any enemy piece of the same type X.
        // In such case, the square is attacked by the enemy's piece of type X.

        // Check if a pawn or king attacks the square
        if (tile & NOT_FILE_A_MASK) << 7 & (self.board.piece_bitboards[7] | self.board.piece_bitboards[6]) != 0 { return true; }
        if (tile & NOT_FILE_H_MASK) << 9 & (self.board.piece_bitboards[7] | self.board.piece_bitboards[6]) != 0 { return true; }
        if tile << 8 & self.board.piece_bitboards[6] != 0 { return true; }
        // Check if a knight/bishop/rook/queen attacks the square
        if self.knight_lookup_table[square as usize] & self.board.piece_bitboards[11] != 0 { return true; }
        if self.get_bishop_magic_bitboard(square) & self.board.piece_bitboards[10] != 0 { return true; }
        if self.get_rook_magic_bitboard(square) & self.board.piece_bitboards[9] != 0 { return true; }
        if (self.get_bishop_magic_bitboard(square) | self.get_rook_magic_bitboard(square))
            & self.board.piece_bitboards[8] != 0 { return true; }

        false
    }

    /// Checks if the given square on the 8th rank is attacked by white
    /// Inputs a given 8th rank square (u8)
    /// Outputs true/false
    fn is_edge_square_attacked_by_white(&self, square: u8) -> bool {
        let tile = 1 << square;

        // The main idea is to virtually place a piece of type X belonging to us on the given square
        // and check if it can attack any enemy piece of the same type X.
        // In such case, the square is attacked by the enemy's piece of type X.

        // Check if a pawn or king attacks the square
        if (tile & NOT_FILE_H_MASK) >> 7 & (self.board.piece_bitboards[1] | self.board.piece_bitboards[0]) != 0 { return true; }
        if (tile & NOT_FILE_A_MASK) >> 9 & (self.board.piece_bitboards[1] | self.board.piece_bitboards[0]) != 0 { return true; }
        if tile >> 8 & self.board.piece_bitboards[0] != 0 { return true; }
        // Check if a knight/bishop/rook/queen attacks the square
        if self.knight_lookup_table[square as usize] & self.board.piece_bitboards[5] != 0 { return true; }
        if self.get_bishop_magic_bitboard(square) & self.board.piece_bitboards[4] != 0 { return true; }
        if self.get_rook_magic_bitboard(square) & self.board.piece_bitboards[3] != 0 { return true; }
        if (self.get_bishop_magic_bitboard(square) | self.get_rook_magic_bitboard(square))
            & self.board.piece_bitboards[2] != 0 { return true; }

        false
    }

    /// Checks if the given square is attacked by white
    /// Inputs a 64-bit number with one bit set to 1 as a tile indication
    /// Outputs true/false
    fn is_square_attacked_by_white(&self, tile: u64) -> bool {
        // The main idea is to virtually place a piece of type X belonging to us on the given square
        // and check if it can attack any enemy piece of the same type X.
        // In such case, the square is attacked by the enemy's piece of type X.

        // Check if a pawn attacks the square
        if (tile & NOT_FILE_H_MASK) >> 9 & self.board.piece_bitboards[1] != 0 { return true; }
        if (tile & NOT_FILE_A_MASK) >> 7 & self.board.piece_bitboards[1] != 0 { return true; }

        let square = u64::checked_ilog2(tile).unwrap_or_default() as u8;
        if tile & self.king_lookup_table[square as usize] != 0 { return true; }
        if self.knight_lookup_table[square as usize] & self.board.piece_bitboards[5] != 0 { return true; }
        if self.get_bishop_magic_bitboard(square) & self.board.piece_bitboards[4] != 0 { return true; }
        if self.get_rook_magic_bitboard(square) & self.board.piece_bitboards[3] != 0 { return true; }
        if (self.get_bishop_magic_bitboard(square) | self.get_rook_magic_bitboard(square))
            & self.board.piece_bitboards[2] != 0 { return true; }

        false
    }

    /// Checks if the given square is attacked by black
    /// Inputs a 64-bit number with one bit set to 1 as a tile indication
    /// Outputs true/false
    fn is_square_attacked_by_black(&self, tile: u64) -> bool {
        // The main idea is to virtually place a piece of type X belonging to us on the given square
        // and check if it can attack any enemy piece of the same type X.
        // In such case, the square is attacked by the enemy's piece of type X.

        // Check if a pawn attacks the square
        if (tile & NOT_FILE_A_MASK) << 9 & self.board.piece_bitboards[7] != 0 { return true; }
        if (tile & NOT_FILE_H_MASK) << 7 & self.board.piece_bitboards[7] != 0 { return true; }

        let square = u64::checked_ilog2(tile).unwrap_or_default() as u8;
        if tile & self.king_lookup_table[u64::checked_ilog2(self.board.piece_bitboards[6]).unwrap_or_default() as usize] != 0 { return true; }
        if self.knight_lookup_table[square as usize] & self.board.piece_bitboards[11] != 0 { return true; }
        if self.get_bishop_magic_bitboard(square) & self.board.piece_bitboards[10] != 0 { return true; }
        if self.get_rook_magic_bitboard(square) & self.board.piece_bitboards[9] != 0 { return true; }
        if (self.get_bishop_magic_bitboard(square) | self.get_rook_magic_bitboard(square))
            & self.board.piece_bitboards[8] != 0 { return true; }

        false
    }

    /// Checks if the king of the current player is in check
    /// Outputs true/false
    pub fn is_in_check(&self) -> bool {
        if self.board.active_player == BLACK {
            return self.is_square_attacked_by_white(self.board.piece_bitboards[6]);
        }
        self.is_square_attacked_by_black(self.board.piece_bitboards[0])
    }
    /// Checks if the king of the inactive player is in check
    /// Outputs true/false
    pub fn is_opponent_in_check(&self) -> bool {
        if self.board.active_player == WHITE {
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

    /// Generates white pawn captures based on the current board situation and updates self
    /// Should NOT be used for regular move generation
    /// Used for heuristics
    fn generate_white_pawn_captures(&mut self) {
        let left_capture_bitboard = self.generate_white_pawn_left_capture_bitboard();

        let right_capture_bitboard: u64 = self.generate_white_pawn_right_capture_bitboard();
        self.convert_white_pawn_moves(left_capture_bitboard, 9);
        self.convert_white_pawn_moves(right_capture_bitboard, 7);
    }

    /// Generates white pawn promotions based on the current board situation and updates self
    /// Should NOT be used for regular move generation
    /// Used for heuristics
    /// Does not count in promotions after captures
    fn generate_white_promotions(&mut self) {
        let push_bitboard = self.generate_white_push_bitboard()
            & FILE_8_MASK;

        self.convert_white_pawn_moves(push_bitboard, 8);
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
            let origin = u64::checked_ilog2((tile) >> shift).unwrap_or_default() as u8;

            if target < UPPER_RANK_LOWEST_TILE
            {
                self.moves.push(Move {
                    origin,
                    target,
                    promotion: 0,
                    piece: PAWN,
                });
                continue;
            }

            for i in 2..6
            {
                self.moves.push(Move {
                    origin,
                    target,
                    promotion: i,
                    piece: PAWN,
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
    /// We do not need to consider edges because of promotions
    fn generate_white_push_bitboard(&self) -> u64 {
        self.board.piece_bitboards[PAWN as usize] << 8
            & self.board.empty_bitboard
    }

    /// Outputs a bitboard of white left pawn captures, based on the current board situation
    /// Shifts by 9 bits
    /// Should be used separately for left captures and right captures.
    /// Accounts for en passant
    fn generate_white_pawn_left_capture_bitboard(&self) -> u64 {
        let mut attack_options = self.board.colour_bitboards[BLACK as usize];
        if self.board.en_passant_possibility < NO_PASSANT {
            let en_passant_tile = 1 << self.board.en_passant_possibility;
            attack_options |= en_passant_tile;
        }
        let index = PAWN as usize;
        let capture_bitboard: u64 = ((self.board.piece_bitboards[index] & NOT_FILE_A_MASK) << 9) & attack_options;
        capture_bitboard
    }
    /// Outputs a bitboard of white pawn captures, based on the current board situation
    /// Shifts by 7 bits
    /// Should be used separately for left captures and right captures.
    /// Accounts for en passant
    fn generate_white_pawn_right_capture_bitboard(&self) -> u64 {
        let mut attack_options = self.board.colour_bitboards[BLACK as usize];
        if self.board.en_passant_possibility < NO_PASSANT {
            let en_passant_tile = 1 << self.board.en_passant_possibility;
            attack_options |= en_passant_tile;
        }
        let index = PAWN as usize;
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

    /// Generates black pawn captures based on the current board situation and updates self
    /// Should NOT be used for regular move generation
    /// Used for heuristics
    fn generate_black_pawn_captures(&mut self) {
        let left_capture_bitboard = self.generate_black_pawn_left_capture_bitboard();
        let right_capture_bitboard: u64 = self.generate_black_pawn_right_capture_bitboard();

        self.convert_black_pawn_moves(left_capture_bitboard, 9);
        self.convert_black_pawn_moves(right_capture_bitboard, 7);
    }

    /// Generates black pawn promotions based on the current board situation and updates self
    /// Should NOT be used for regular move generation
    /// Used for heuristics
    /// Does not count in promotions after captures
    fn generate_black_promotions(&mut self) {
        let push_bitboard = self.generate_black_push_bitboard()
            & FILE_1_MASK;

        self.convert_black_pawn_moves(push_bitboard, 8);
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
            let origin = u64::checked_ilog2(tile << shift).unwrap_or_default() as u8;

            if target > LOWER_RANK_HIGHEST_TILE
            {
                self.moves.push(Move {
                    origin,
                    target,
                    promotion: 0,
                    piece: PAWN,
                });
                continue;
            }

            for i in 2..6
            {
                self.moves.push(Move {
                    origin,
                    target,
                    promotion: i,
                    piece: PAWN,
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
    /// We do not need to consider edges because of promotions
    fn generate_black_push_bitboard(&self) -> u64 {
        self.board.piece_bitboards[PAWN as usize + 6] >> 8
            & self.board.empty_bitboard
    }

    /// Outputs a bitboard of black pawn left captures, based on the current board situation
    /// Shifts by 9 bits
    /// Should be used separately for left captures and right captures.
    /// Accounts for en passant
    fn generate_black_pawn_left_capture_bitboard(&self) -> u64 {
        let mut attack_options = self.board.colour_bitboards[WHITE as usize];
        if self.board.en_passant_possibility < NO_PASSANT {
            let en_passant_tile = 1 << self.board.en_passant_possibility;
            attack_options |= en_passant_tile;
        }
        let index = PAWN as usize + 6;
        let capture_bitboard: u64 = (self.board.piece_bitboards[index] & NOT_FILE_H_MASK) >> 9 & attack_options;
        capture_bitboard
    }
    /// Outputs a bitboard of black pawn right captures, based on the current board situation
    /// Shifts by 7 bits
    /// Should be used separately for left captures and right captures.
    /// Accounts for en passant
    fn generate_black_pawn_right_capture_bitboard(&self) -> u64 {
        let mut attack_options = self.board.colour_bitboards[WHITE as usize];
        if self.board.en_passant_possibility < NO_PASSANT {
            let en_passant_tile = 1 << self.board.en_passant_possibility;
            attack_options |= en_passant_tile;
        }
        let index = PAWN as usize + 6;
        let capture_bitboard: u64 = (self.board.piece_bitboards[index] & NOT_FILE_A_MASK) >> 7 & attack_options;
        capture_bitboard
    }

    /// KNIGHT MOVE GENERATION

    /// Outputs a lookup table for bitboards of possible knight moves at given square, assuming no occupancy
    /// Used by the constructor of the board for initialization of the lookup table
    fn setup_knight_lookup_table() -> [u64; 64] {
        let mut lookup_table: [u64; 64] = [0; 64];
        for origin in 0..64 {
            for direction in 0..8 {
                let target: i8 = origin + KNIGHT_SHIFTS[direction];
                if target > 63 || target < 0 {
                    continue;
                }

                if Board::distance(origin as u8, target as u8) > 3 {
                    continue;
                }

                lookup_table[origin as usize] |= 1 << target;
            }
        }

        lookup_table
    }

    /// Generates moves of knights based on the current board situation and updates self
    fn generate_knight_moves(&mut self) {
        let mut knight_bitboard = self.board.piece_bitboards[5 + 6 * self.board.active_player as usize];

        while knight_bitboard != 0 {
            let tile = knight_bitboard & knight_bitboard.wrapping_neg();
            knight_bitboard -= tile;

            let square = u64::checked_ilog2(tile).unwrap_or_default() as u8;
            let bitboard = self.generate_knight_moves_bitboard(square);
            self.convert_knight_moves(bitboard, square);
        }
    }

    /// Generates knight captures based on the current board situation and updates self
    /// Should NOT be used for regular move generation
    /// Used for heuristics
    fn generate_knight_captures(&mut self) {
        let mut knight_bitboard = self.board.piece_bitboards[5 + 6 * self.board.active_player as usize];

        while knight_bitboard != 0 {
            let tile = knight_bitboard & knight_bitboard.wrapping_neg();
            knight_bitboard -= tile;

            let square = u64::checked_ilog2(tile).unwrap_or_default() as u8;
            let bitboard = self.generate_knight_moves_bitboard(square)
                & self.board.colour_bitboards[self.board.inactive_player as usize];
            self.convert_knight_moves(bitboard, square);
        }
    }

    /// Outputs a bitboard of knight moves, based on the current board occupancy, given the knight's square
    fn generate_knight_moves_bitboard(&self, square: u8) -> u64 {
        self.knight_lookup_table[square as usize] & (self.board.empty_bitboard | self.board.colour_bitboards[self.board.inactive_player as usize])
    }

    /// Converts a bitboard of knight moves into a list of moves and updates self
    /// Takes origin square of the knight as input
    /// Should be used separately for each owned knight
    /// parameters:
    ///     move_bitboard - bitboards of squares targeted by a move subgroup
    ///     origin - number of the square the given knight is on
    fn convert_knight_moves(&mut self, move_bitboard: u64, origin: u8) {
        let mut bitboard = move_bitboard;

        while bitboard != 0 {
            let tile = bitboard & bitboard.wrapping_neg();
            bitboard -= tile;

            let target = u64::checked_ilog2(tile).unwrap_or_default() as u8;

            self.moves.push(Move { origin, target, promotion: 0, piece: KNIGHT });
        }
    }

    /// ROOK MOVE GENERATION

    /// Outputs a magic bitboard of possible rook moves at given square and given occupancy
    /// Used by the constructor of the board for initialization of the magic bitboard
    fn setup_rook_magic_bitboard() -> Vec<u64> {
        let mut magic_bitboard = vec![0u64;1048577];

        for square in 0..64 {
            let full_mask_vector = Self::generate_rook_magic_key_mask(square);

            // Occupancy combinations

            for combination_mask in 0..(1 << full_mask_vector.len()) {
                let raw_key = Self::generate_magic_raw_key(&full_mask_vector, combination_mask);
                let value = Self::generate_rook_magic_value(raw_key, square);
                let key = magic_hash_rook(raw_key, square);

                magic_bitboard[key] = value;
            }

        }

        magic_bitboard
    }

    /// Generates full rook occupancy mask for a given square
    /// Supposes there is a piece on every relevant tile
    /// Used for creating permutations of occupancies while creating magic bitboards
    /// parameters:
    ///     - square - number of the square we are considering (u8)
    /// Outputs bits of the occupancy mask in vector in such a manner
    /// that after concatenation it would be the full rook occupancy mask for the given square
    fn generate_rook_magic_key_mask(square: u8) -> Vec<u64> {
        let mut full_mask: u64 = MAGIC_MASK_ROOK[square as usize];

        let mut full_mask_vector = Vec::<u64>::new();
        while full_mask > 0 {
            let tile = full_mask & full_mask.wrapping_neg();
            full_mask -= tile;

            full_mask_vector.push(tile);
        }
        full_mask_vector
    }

    /// Generates a bitboard of possible rook moves,
    /// given an occupancy mask (i.e. a magic bitboard raw key), and a square of origin
    fn generate_rook_magic_value(key: u64, origin: u8) -> u64 {
        Self::generate_rook_magic_bitboard_left(key, origin)
        | Self::generate_rook_magic_bitboard_right(key, origin)
        | Self::generate_rook_magic_bitboard_top(key, origin)
        | Self::generate_rook_magic_bitboard_bottom(key, origin)
    }

    /// Generates a bitboard of possible rook West moves,
    /// given an occupancy mask (i.e. a magic bitboard raw key), and a square of origin
    /// Called by generate_rook_magic_value, should not be called independently
    fn generate_rook_magic_bitboard_left(key: u64, origin: u8) -> u64 {
        let mut bitboard: u64 = 0;

        if origin % 8 == 7 {return bitboard;}

        let mut square = origin + 1;

        while square % 8 != 7 {
            bitboard |= 1 << square;
            if (key >> square) % 2 == 1 { break; }

            square += 1;
        }
        bitboard |= 1 << square;

        bitboard
    }

    /// Generates a bitboard of possible rook East moves,
    /// given an occupancy mask (i.e. a magic bitboard raw key), and a square of origin
    /// Called by generate_rook_magic_value, should not be called independently
    fn generate_rook_magic_bitboard_right(key: u64, origin: u8) -> u64 {
        let mut bitboard: u64 = 0;

        if origin % 8 == 0 {return bitboard;}

        let mut square = origin - 1;

        while square % 8 != 0 {
            bitboard |= 1 << square;
            if (key >> square) % 2 == 1 { break; }

            square -= 1;
        }
        bitboard |= 1 << square;

        bitboard
    }

    /// Generates a bitboard of possible rook North moves,
    /// given an occupancy mask (i.e. a magic bitboard raw key), and a square of origin
    /// Called by generate_rook_magic_value, should not be called independently
    fn generate_rook_magic_bitboard_top(key: u64, origin: u8) -> u64 {
        let mut bitboard: u64 = 0;

        if origin / 8 == 7 {return bitboard;}

        let mut square = origin + 8;

        while square / 8 != 7 {
            bitboard |= 1 << square;
            if (key >> square) % 2 == 1 { break; }

            square += 8;
        }
        bitboard |= 1 << square;

        bitboard
    }

    /// Generates a bitboard of possible rook South moves,
    /// given an occupancy mask (i.e. a magic bitboard raw key), and a square of origin
    /// Called by generate_rook_magic_value, should not be called independently
    fn generate_rook_magic_bitboard_bottom(key: u64, origin: u8) -> u64 {
        let mut bitboard: u64 = 0;

        if origin / 8 == 0 {return bitboard;}

        let mut square = origin - 8;

        while square / 8 != 0 {
            bitboard |= 1 << square;
            if (key >> square) % 2 == 1 { break; }

            square -= 8;
        }
        bitboard |= 1 << square;

        bitboard
    }

    /// Generates moves of rooks based on the current board situation and updates self
    fn generate_rook_moves(&mut self) {
        let mut rook_bitboard = self.board.piece_bitboards[3 + 6 * self.board.active_player as usize];

        while rook_bitboard != 0 {
            let tile = rook_bitboard & rook_bitboard.wrapping_neg();
            rook_bitboard -= tile;

            let square = u64::checked_ilog2(tile).unwrap_or_default() as u8;
            let bitboard = self.generate_rook_moves_bitboard(square);
            self.convert_rook_moves(bitboard, square);
        }
    }

    /// Generates rook captures based on the current board situation and updates self
    /// Should NOT be used for regular move generation
    /// Used for heuristics
    fn generate_rook_captures(&mut self) {
        let mut rook_bitboard = self.board.piece_bitboards[3 + 6 * self.board.active_player as usize];

        while rook_bitboard != 0 {
            let tile = rook_bitboard & rook_bitboard.wrapping_neg();
            rook_bitboard -= tile;

            let square = u64::checked_ilog2(tile).unwrap_or_default() as u8;
            let bitboard = self.generate_rook_moves_bitboard(square)
                & self.board.colour_bitboards[self.board.inactive_player as usize];
            self.convert_rook_moves(bitboard, square);
        }
    }

    /// Retrieves rook magic bitboard based on a given origin and current board situation
    /// Constructs occupancy mask from the current board situation and
    /// masks it with the relevant magic mask to obtain a raw key,
    /// then hashes using magic hash to obtain a hashed key
    fn get_rook_magic_bitboard(&self, origin: u8) -> u64 {
        let occupancy = self.board.main_bitboard & MAGIC_MASK_ROOK[origin as usize];
        self.rook_magic_bitboard[magic_hash_rook(occupancy, origin)]
    }

    /// Outputs a bitboard of rook moves, based on the current board occupancy, given the rook's square
    fn generate_rook_moves_bitboard(&self, square: u8) -> u64 {
        self.get_rook_magic_bitboard(square) & (self.board.empty_bitboard | self.board.colour_bitboards[self.board.inactive_player as usize])
    }

    /// Converts a bitboard of rook moves into a list of moves and updates self
    /// Takes origin square of the rook as input
    /// Should be used separately for each owned rook
    /// parameters:
    ///     move_bitboard - bitboards of squares targeted by a move subgroup
    ///     origin - number of the square the given rook is on
    fn convert_rook_moves(&mut self, move_bitboard: u64, origin: u8) {
        let mut bitboard = move_bitboard;

        while bitboard != 0 {
            let tile = bitboard & bitboard.wrapping_neg();
            bitboard -= tile;

            let target = u64::checked_ilog2(tile).unwrap_or_default() as u8;

            self.moves.push(Move { origin, target, promotion: 0, piece: ROOK });
        }
    }

    /// ROOK/BISHOP

    /// Generate a raw key for magic bitboard (unhashed)
    /// given a mask vector of bits and a combination mask indicating which bits to consider
    /// Example: [0100 0000, 0001 0000, 0000 1000, 0000 0001], 0b1011
    ///         should output 0100 1001
    fn generate_magic_raw_key(full_mask_vector: &Vec<u64>, combination_mask: i32) -> u64 {
        let mut mask = combination_mask;
        let mut key: u64 = 0;
        let mut index = 0;
        while mask > 0 {
            if mask % 2 == 1 { key |= full_mask_vector[index]; }
            index += 1;
            mask >>= 1;
        }
        key
    }

    /// BISHOP MOVE GENERATION

    /// Outputs a magic bitboard of possible bishop moves at given square and given occupancy
    /// Used by the constructor of the board for initialization of the magic bitboard
    fn setup_bishop_magic_bitboard() -> Vec<u64> {
        let mut magic_bitboard = vec![0u64;262145];

        for square in 0..64 {
            let full_mask_vector = Self::generate_bishop_magic_key_mask(square);

            // Occupancy combinations

            for combination_mask in 0..(1 << full_mask_vector.len()) {
                let raw_key = Self::generate_magic_raw_key(&full_mask_vector, combination_mask);
                let value = Self::generate_bishop_magic_value(raw_key, square);
                let key = magic_hash_bishop(raw_key, square);

                magic_bitboard[key] = value;
            }

        }

        magic_bitboard
    }

    /// Generates full bishop occupancy mask for a given square
    /// Supposes there is a piece on every relevant tile
    /// Used for creating permutations of occupancies while creating magic bitboards
    /// parameters:
    ///     - square - number of the square we are considering (u8)
    /// Outputs bits of the occupancy mask in vector in such a manner
    /// that after concatenation it would be the full rook occupancy mask for the given square
    fn generate_bishop_magic_key_mask(square: u8) -> Vec<u64> {
        let mut full_mask: u64 = MAGIC_MASK_BISHOP[square as usize];

        let mut full_mask_vector = Vec::<u64>::new();
        while full_mask > 0 {
            let tile = full_mask & full_mask.wrapping_neg();
            full_mask -= tile;

            full_mask_vector.push(tile);
        }
        full_mask_vector
    }


    /// Generates a bitboard of possible bishop moves,
    /// given an occupancy mask (i.e. a magic bitboard raw key), and a square of origin
    fn generate_bishop_magic_value(key: u64, origin: u8) -> u64 {
        Self::generate_bishop_magic_bitboard_north_west(key, origin)
        | Self::generate_bishop_magic_bitboard_north_east(key, origin)
        | Self::generate_bishop_magic_bitboard_south_east(key, origin)
        | Self::generate_bishop_magic_bitboard_south_west(key, origin)
    }

    /// Generates a bitboard of possible bishop North-West moves,
    /// given an occupancy mask (i.e. a magic bitboard raw key), and a square of origin
    /// Called by generate_bishop_magic_value, should not be called independently
    fn generate_bishop_magic_bitboard_north_west(key: u64, origin: u8) -> u64 {
        let mut bitboard: u64 = 0;

        if origin % 8 == 7 || origin / 8 == 7 {return bitboard;}

        let mut square = origin + 9;

        while square % 8 != 7 && square / 8 != 7 {
            bitboard |= 1 << square;
            if (key >> square) % 2 == 1 { break; }

            square += 9;
        }
        bitboard |= 1 << square;

        bitboard
    }

    /// Generates a bitboard of possible bishop North-East moves,
    /// given an occupancy mask (i.e. a magic bitboard raw key), and a square of origin
    /// Called by generate_bishop_magic_value, should not be called independently
    fn generate_bishop_magic_bitboard_north_east(key: u64, origin: u8) -> u64 {
        let mut bitboard: u64 = 0;

        if origin % 8 == 0 || origin / 8 == 7 {return bitboard;}

        let mut square = origin + 7;

        while square % 8 != 0 && square / 8 != 7 {
            bitboard |= 1 << square;
            if (key >> square) % 2 == 1 { break; }

            square += 7;
        }
        bitboard |= 1 << square;

        bitboard
    }

    /// Generates a bitboard of possible rook South-East moves,
    /// given an occupancy mask (i.e. a magic bitboard raw key), and a square of origin
    /// Called by generate_bishop_magic_value, should not be called independently
    fn generate_bishop_magic_bitboard_south_east(key: u64, origin: u8) -> u64 {
        let mut bitboard: u64 = 0;

        if origin / 8 == 0 || origin % 8 == 0 {return bitboard;}

        let mut square = origin - 9;

        while square / 8 != 0 && square % 8 != 0 {
            bitboard |= 1 << square;
            if (key >> square) % 2 == 1 { break; }

            square -= 9;
        }
        bitboard |= 1 << square;

        bitboard
    }

    /// Generates a bitboard of possible rook South-West moves,
    /// given an occupancy mask (i.e. a magic bitboard raw key), and a square of origin
    /// Called by generate_bishop_magic_value, should not be called independently
    fn generate_bishop_magic_bitboard_south_west(key: u64, origin: u8) -> u64 {
        let mut bitboard: u64 = 0;

        if origin / 8 == 0 || origin % 8 == 7 {return bitboard;}

        let mut square = origin - 7;

        while square / 8 != 0 && square % 8 != 7 {
            bitboard |= 1 << square;
            if (key >> square) % 2 == 1 { break; }

            square -= 7;
        }
        bitboard |= 1 << square;

        bitboard
    }

    /// Generates moves of bishops based on the current board situation and updates self
    fn generate_bishop_moves(&mut self) {
        let mut bishop_bitboard = self.board.piece_bitboards[4 + 6 * self.board.active_player as usize];

        while bishop_bitboard != 0 {
            let tile = bishop_bitboard & bishop_bitboard.wrapping_neg();
            bishop_bitboard -= tile;

            let square = u64::checked_ilog2(tile).unwrap_or_default() as u8;
            let bitboard = self.generate_bishop_moves_bitboard(square);
            self.convert_bishop_moves(bitboard, square);
        }
    }

    /// Generates bishop captures based on the current board situation and updates self
    /// Should NOT be used for regular move generation
    /// Used for heuristics
    fn generate_bishop_captures(&mut self) {
        let mut bishop_bitboard = self.board.piece_bitboards[4 + 6 * self.board.active_player as usize];

        while bishop_bitboard != 0 {
            let tile = bishop_bitboard & bishop_bitboard.wrapping_neg();
            bishop_bitboard -= tile;

            let square = u64::checked_ilog2(tile).unwrap_or_default() as u8;
            let bitboard = self.generate_bishop_moves_bitboard(square)
                & self.board.colour_bitboards[self.board.inactive_player as usize];
            self.convert_bishop_moves(bitboard, square);
        }
    }

    /// Retrieves bishop magic bitboard based on a given origin and current board situation
    /// Constructs occupancy mask from the current board situation and
    /// masks it with the relevant magic mask to obtain a raw key,
    /// then hashes using magic hash to obtain a hashed key
    fn get_bishop_magic_bitboard(&self, origin: u8) -> u64 {
        let occupancy = self.board.main_bitboard & MAGIC_MASK_BISHOP[origin as usize];
        self.bishop_magic_bitboard[magic_hash_bishop(occupancy, origin)]
    }

    /// Outputs a bitboard of bishop moves, based on the current board occupancy, given the bishop's square
    fn generate_bishop_moves_bitboard(&self, square: u8) -> u64 {
        self.get_bishop_magic_bitboard(square) & (self.board.empty_bitboard | self.board.colour_bitboards[self.board.inactive_player as usize])
    }

    /// Converts a bitboard of bishop moves into a list of moves and updates self
    /// Takes origin square of the bishop as input
    /// Should be used separately for each owned bishop
    /// parameters:
    ///     move_bitboard - bitboards of squares targeted by a move subgroup
    ///     origin - number of the square the given bishop is on
    fn convert_bishop_moves(&mut self, move_bitboard: u64, origin: u8) {
        let mut bitboard = move_bitboard;

        while bitboard != 0 {
            let tile = bitboard & bitboard.wrapping_neg();
            bitboard -= tile;

            let target = u64::checked_ilog2(tile).unwrap_or_default() as u8;

            self.moves.push(Move { origin, target, promotion: 0, piece: BISHOP });
        }
    }

    /// QUEEN MOVE GENERATION

    /// Generates moves of queens based on the current board situation and updates self
    fn generate_queen_moves(&mut self) {
        let mut queen_bitboard = self.board.piece_bitboards[2 + 6 * self.board.active_player as usize];

        while queen_bitboard != 0 {
            let tile = queen_bitboard & queen_bitboard.wrapping_neg();
            queen_bitboard -= tile;

            let square = u64::checked_ilog2(tile).unwrap_or_default() as u8;
            let bitboard = self.generate_queen_moves_bitboard(square);
            self.convert_queen_moves(bitboard, square);
        }
    }

    /// Generates queen captures based on the current board situation and updates self
    /// Should NOT be used for regular move generation
    /// Used for heuristics
    fn generate_queen_captures(&mut self) {
        let mut queen_bitboard = self.board.piece_bitboards[2 + 6 * self.board.active_player as usize];

        while queen_bitboard != 0 {
            let tile = queen_bitboard & queen_bitboard.wrapping_neg();
            queen_bitboard -= tile;

            let square = u64::checked_ilog2(tile).unwrap_or_default() as u8;
            let bitboard = self.generate_queen_moves_bitboard(square)
                & self.board.colour_bitboards[self.board.inactive_player as usize];
            self.convert_queen_moves(bitboard, square);
        }
    }

    /// Outputs a bitboard of queen moves, based on the current board occupancy, given the queen's square
    fn generate_queen_moves_bitboard(&self, square: u8) -> u64 {
        self.generate_rook_moves_bitboard(square) | self.generate_bishop_moves_bitboard(square)
    }

    /// Converts a bitboard of queen moves into a list of moves and updates self
    /// Takes origin square of the queen as input
    /// Should be used separately for each owned queen
    /// parameters:
    ///     move_bitboard - bitboards of squares targeted by a move subgroup
    ///     origin - number of the square the given queen is on
    fn convert_queen_moves(&mut self, move_bitboard: u64, origin: u8) {
        let mut bitboard = move_bitboard;

        while bitboard != 0 {
            let tile = bitboard & bitboard.wrapping_neg();
            bitboard -= tile;

            let target = u64::checked_ilog2(tile).unwrap_or_default() as u8;

            self.moves.push(Move { origin, target, promotion: 0, piece: QUEEN });
        }
    }

}

///                       ///
///                       ///
/// /// /// TESTS /// /// ///
///                       ///
///                       ///

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::time::Instant;
    use crate::board::{Board, START_POSITION};
    use crate::piece::Piece::{PAWN};
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
    fn algebraic_notation() {
        let piece_move1 = Move{origin: 54, target: 63, promotion: 2, piece: PAWN};
        let piece_move2 = Move{origin: 0, target: 6, promotion: 0, piece: ROOK};
        assert_eq!("b7a8q", piece_move1.to_algebraic_notation());
        assert_eq!("h1b1", piece_move2.to_algebraic_notation());
    }

    #[test]
    fn start_position() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);
        let mut move_list = MoveList::new(&mut board);

        // SETUP

        let expected_push_bitboard: u64 = 0b0000000000000000000000000000000000000000111111110000000000000000;
        let expected_double_push_bitboard: u64 = 0b0000000000000000000000000000000011111111000000000000000000000000;
        let expected_left_pawn_capture_bitboard: u64 = 0;
        let expected_right_pawn_capture_bitboard: u64 = 0;
        let expected_king_bitboard: u64 = 0;
        let expected_knight_bitboard1: u64 = 0b0000000000000000000000000000000000000000101000000000000000000000;
        let expected_knight_bitboard2: u64 = 0b0000000000000000000000000000000000000000000001010000000000000000;
        let expected_rook_bitboard1: u64 = 0;
        let expected_rook_bitboard2: u64 = 0;
        let expected_bishop_bitboard1: u64 = 0;
        let expected_bishop_bitboard2: u64 = 0;
        let expected_queen_bitboard: u64 = 0;

        let mut expected_push_moves = Vec::<Move>::new();
        expected_push_moves.push(Move{ origin: 8, target: 16, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 9, target: 17, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 10, target: 18, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 11, target: 19, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 12, target: 20, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 13, target: 21, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 14, target: 22, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 15, target: 23, promotion: 0, piece: PAWN });
        let mut expected_double_push_moves = Vec::<Move>::new();
        expected_double_push_moves.push(Move{ origin: 8, target: 24, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 9, target: 25, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 10, target: 26, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 11, target: 27, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 12, target: 28, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 13, target: 29, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 14, target: 30, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 15, target: 31, promotion: 0, piece: PAWN });
        let mut expected_left_pawn_captures = Vec::<Move>::new();
        let mut expected_right_pawn_captures = Vec::<Move>::new();
        let expected_pawn_moves = [expected_push_moves.clone(), expected_double_push_moves.clone(),
            expected_left_pawn_captures.clone(), expected_right_pawn_captures.clone()].concat();

        // BITBOARDS

        assert_eq!(move_list.generate_white_push_bitboard(), expected_push_bitboard);
        assert_eq!(move_list.generate_white_double_push_bitboard(expected_push_bitboard), expected_double_push_bitboard);
        assert_eq!(move_list.generate_white_pawn_left_capture_bitboard(), expected_left_pawn_capture_bitboard);
        assert_eq!(move_list.generate_white_pawn_right_capture_bitboard(), expected_right_pawn_capture_bitboard);
        assert_eq!(move_list.generate_king_moves_bitboard(), expected_king_bitboard);
        assert_eq!(move_list.generate_knight_moves_bitboard(6), expected_knight_bitboard1);
        assert_eq!(move_list.generate_knight_moves_bitboard(1), expected_knight_bitboard2);
        assert_eq!(move_list.generate_rook_moves_bitboard(0), expected_rook_bitboard1);
        assert_eq!(move_list.generate_rook_moves_bitboard(7), expected_rook_bitboard2);
        assert_eq!(move_list.generate_bishop_moves_bitboard(5), expected_bishop_bitboard1);
        assert_eq!(move_list.generate_bishop_moves_bitboard(2), expected_bishop_bitboard2);
        assert_eq!(move_list.generate_queen_moves_bitboard(3), expected_queen_bitboard);

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

        // KNIGHT MOVES

        let mut expected_knight_moves1 = Vec::<Move>::new();
        expected_knight_moves1.push( Move { origin: 6, target: 23, promotion: 0, piece: KNIGHT });
        expected_knight_moves1.push( Move { origin: 6, target: 21, promotion: 0, piece: KNIGHT });
        let mut expected_knight_moves2 = Vec::<Move>::new();
        expected_knight_moves2.push( Move { origin: 1, target: 16, promotion: 0, piece: KNIGHT });
        expected_knight_moves2.push( Move { origin: 1, target: 18, promotion: 0, piece: KNIGHT });

        let mut expected_knight_moves_all = [expected_knight_moves1.clone(), expected_knight_moves2.clone()].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.convert_knight_moves(expected_knight_bitboard1, 6);
        assert!(compare_vecs(&move_list.moves, &expected_knight_moves1));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_knight_moves(expected_knight_bitboard2, 1);
        assert!(compare_vecs(&move_list.moves, &expected_knight_moves2));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_knight_moves();
        assert!(compare_vecs(&move_list.moves, &expected_knight_moves_all));

        // ROOK MOVES

        let mut expected_rook_moves1 = Vec::<Move>::new();
        let mut expected_rook_moves2 = Vec::<Move>::new();
        let mut expected_rook_moves_all = [expected_rook_moves1.clone(), expected_rook_moves2.clone()].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.convert_rook_moves(expected_rook_bitboard1, 0);
        assert!(compare_vecs(&move_list.moves, &expected_rook_moves1));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_rook_moves(expected_rook_bitboard2, 7);
        assert!(compare_vecs(&move_list.moves, &expected_rook_moves2));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_rook_moves();
        assert!(compare_vecs(&move_list.moves, &expected_rook_moves_all));

        // BISHOP MOVES

        let mut expected_bishop_moves1 = Vec::<Move>::new();
        let mut expected_bishop_moves2 = Vec::<Move>::new();
        let mut expected_bishop_moves_all = [expected_bishop_moves1.clone(), expected_bishop_moves2.clone()].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.convert_bishop_moves(expected_bishop_bitboard1, 5);
        assert!(compare_vecs(&move_list.moves, &expected_bishop_moves1));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_bishop_moves(expected_bishop_bitboard2, 2);
        assert!(compare_vecs(&move_list.moves, &expected_bishop_moves2));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_bishop_moves();
        assert!(compare_vecs(&move_list.moves, &expected_bishop_moves_all));

        // QUEEN MOVES

        let mut expected_queen_moves = Vec::<Move>::new();

        move_list.moves = Vec::<Move>::new();
        move_list.convert_queen_moves(expected_queen_bitboard, 3);
        assert!(compare_vecs(&move_list.moves, &expected_queen_moves));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_queen_moves();
        assert!(compare_vecs(&move_list.moves, &expected_queen_moves));

        // ALL MOVES

        let expected_moves = [expected_pawn_moves, expected_king_moves, expected_knight_moves_all, expected_rook_moves_all, expected_bishop_moves_all, expected_queen_moves].concat();

        move_list.moves = Vec::<Move>::new();
        let start = Instant::now();
        move_list.generate_moves();
        let duration = start.elapsed();
        println!("generate_moves lasted for: {:?}", duration);
        assert!(compare_vecs(&move_list.moves, &expected_moves));
    }

    #[test]
    fn position_2() {
        let mut board = Board::new();
        board.read_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        let mut move_list = MoveList::new(&mut board);

        let expected_push_bitboard: u64 = 0b0000000000000000111111110000000000000000000000000000000000000000;
        let expected_double_push_bitboard: u64 = 0b0000000000000000000000001111111100000000000000000000000000000000;
        let expected_left_pawn_capture_bitboard: u64 = 0;
        let expected_right_pawn_capture_bitboard: u64 = 0;
        let expected_king_bitboard: u64 = 0;
        let expected_knight_bitboard1: u64 = 0b0000000000000000101000000000000000000000000000000000000000000000;
        let expected_knight_bitboard2: u64 = 0b0000000000000000000001010000000000000000000000000000000000000000;
        let expected_rook_bitboard1: u64 = 0;
        let expected_rook_bitboard2: u64 = 0;
        let expected_bishop_bitboard1: u64 = 0;
        let expected_bishop_bitboard2: u64 = 0;
        let expected_queen_bitboard: u64 = 0;

        let mut expected_push_moves = Vec::<Move>::new();
        expected_push_moves.push(Move{ origin: 55, target: 47, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 54, target: 46, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 53, target: 45, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 52, target: 44, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 51, target: 43, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 50, target: 42, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 49, target: 41, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 48, target: 40, promotion: 0, piece: PAWN });
        let mut expected_double_push_moves = Vec::<Move>::new();
        expected_double_push_moves.push(Move{ origin: 55, target: 39, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 54, target: 38, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 53, target: 37, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 52, target: 36, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 51, target: 35, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 50, target: 34, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 49, target: 33, promotion: 0, piece: PAWN });
        expected_double_push_moves.push(Move{ origin: 48, target: 32, promotion: 0, piece: PAWN });
        let mut expected_left_pawn_captures = Vec::<Move>::new();
        let mut expected_right_pawn_captures = Vec::<Move>::new();
        let expected_pawn_moves = [expected_push_moves.clone(), expected_double_push_moves.clone(),
            expected_left_pawn_captures.clone(), expected_right_pawn_captures.clone()].concat();

        assert_eq!(move_list.generate_black_push_bitboard(), expected_push_bitboard);
        assert_eq!(move_list.generate_black_double_push_bitboard(expected_push_bitboard), expected_double_push_bitboard);
        assert_eq!(move_list.generate_black_pawn_left_capture_bitboard(), expected_left_pawn_capture_bitboard);
        assert_eq!(move_list.generate_black_pawn_right_capture_bitboard(), expected_right_pawn_capture_bitboard);
        assert_eq!(move_list.generate_king_moves_bitboard(), expected_king_bitboard);
        assert_eq!(move_list.generate_knight_moves_bitboard(62), expected_knight_bitboard1);
        assert_eq!(move_list.generate_knight_moves_bitboard(57), expected_knight_bitboard2);
        assert_eq!(move_list.generate_rook_moves_bitboard(63), expected_rook_bitboard1);
        assert_eq!(move_list.generate_rook_moves_bitboard(56), expected_rook_bitboard2);
        assert_eq!(move_list.generate_bishop_moves_bitboard(61), expected_bishop_bitboard1);
        assert_eq!(move_list.generate_bishop_moves_bitboard(56), expected_bishop_bitboard2);
        assert_eq!(move_list.generate_queen_moves_bitboard(60), expected_queen_bitboard);

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

        // KNIGHT MOVES

        let mut expected_knight_moves1 = Vec::<Move>::new();
        expected_knight_moves1.push( Move { origin: 62, target: 47, promotion: 0, piece: KNIGHT });
        expected_knight_moves1.push( Move { origin: 62, target: 45, promotion: 0, piece: KNIGHT });
        let mut expected_knight_moves2 = Vec::<Move>::new();
        expected_knight_moves2.push( Move { origin: 57, target: 42, promotion: 0, piece: KNIGHT });
        expected_knight_moves2.push( Move { origin: 57, target: 40, promotion: 0, piece: KNIGHT });

        let mut expected_knight_moves_all = [expected_knight_moves1.clone(), expected_knight_moves2.clone()].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.convert_knight_moves(expected_knight_bitboard1, 62);
        assert!(compare_vecs(&move_list.moves, &expected_knight_moves1));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_knight_moves(expected_knight_bitboard2, 57);
        assert!(compare_vecs(&move_list.moves, &expected_knight_moves2));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_knight_moves();
        assert!(compare_vecs(&move_list.moves, &expected_knight_moves_all));

        // ROOK MOVES

        let mut expected_rook_moves1 = Vec::<Move>::new();
        let mut expected_rook_moves2 = Vec::<Move>::new();
        let expected_rook_moves_all = [expected_rook_moves1.clone(), expected_rook_moves2.clone()].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.convert_rook_moves(expected_rook_bitboard1, 63);
        assert!(compare_vecs(&move_list.moves, &expected_rook_moves1));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_rook_moves(expected_rook_bitboard2, 56);
        assert!(compare_vecs(&move_list.moves, &expected_rook_moves2));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_rook_moves();
        assert!(compare_vecs(&move_list.moves, &expected_rook_moves_all));

        // BISHOP MOVES

        let mut expected_bishop_moves1 = Vec::<Move>::new();
        let mut expected_bishop_moves2 = Vec::<Move>::new();
        let mut expected_bishop_moves_all = [expected_bishop_moves1.clone(), expected_bishop_moves2.clone()].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.convert_bishop_moves(expected_bishop_bitboard1, 61);
        assert!(compare_vecs(&move_list.moves, &expected_bishop_moves1));

        move_list.moves = Vec::<Move>::new();
        move_list.convert_bishop_moves(expected_bishop_bitboard2, 56);
        assert!(compare_vecs(&move_list.moves, &expected_bishop_moves2));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_bishop_moves();
        assert!(compare_vecs(&move_list.moves, &expected_bishop_moves_all));

        // QUEEN MOVES

        let mut expected_queen_moves = Vec::<Move>::new();

        move_list.moves = Vec::<Move>::new();
        move_list.convert_queen_moves(expected_queen_bitboard, 3);
        assert!(compare_vecs(&move_list.moves, &expected_queen_moves));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_queen_moves();
        assert!(compare_vecs(&move_list.moves, &expected_queen_moves));

        // ALL MOVES

        let expected_moves = [expected_pawn_moves, expected_king_moves, expected_knight_moves_all, expected_rook_moves_all, expected_bishop_moves_all, expected_queen_moves].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.generate_moves();
        assert!(compare_vecs(&move_list.moves, &expected_moves));

    }

    #[test]
    fn position_4() {
        let mut board = Board::new();
        board.read_fen("r3k3/1Pr5/5pp1/3pPBPP/1b1P2Qq/2R2N2/P7/RK6 w q d6 1 25");
        let mut move_list = MoveList::new(&mut board);

        let expected_push_bitboard: u64 =               0b0100000000000000000010010000000000000000100000000000000000000000;
        let expected_double_push_bitboard: u64 =        0b0000000000000000000000000000000010000000000000000000000000000000;
        let expected_left_pawn_capture_bitboard: u64 =  0b1000000000000000000101100000000000000000000000000000000000000000;
        let expected_right_pawn_capture_bitboard: u64 = 0b0000000000000000000001000000000000000000000000000000000000000000;
        let expected_king_bitboard: u64 =               0b0000000000000000000000000000000000000000000000000110000000100000;
        let expected_knight_bitboard1: u64 =            0b0000000000000000000000000000000000000001000000000001000100001010;
        let expected_rook_bitboard1: u64 =              0x0020202020D82020;
        let expected_bishop_bitboard1: u64 =            0x20100A0008102000;
        let expected_queen_bitboard: u64 =              0x000000000D030202;

        let mut expected_push_moves = Vec::<Move>::new();
        expected_push_moves.push(Move{ origin: 15, target: 23, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 32, target: 40, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 35, target: 43, promotion: 0, piece: PAWN });
        expected_push_moves.push(Move{ origin: 54, target: 62, promotion: 2, piece: PAWN });
        expected_push_moves.push(Move{ origin: 54, target: 62, promotion: 3, piece: PAWN });
        expected_push_moves.push(Move{ origin: 54, target: 62, promotion: 4, piece: PAWN });
        expected_push_moves.push(Move{ origin: 54, target: 62, promotion: 5, piece: PAWN });
        let mut expected_double_push_moves = Vec::<Move>::new();
        expected_double_push_moves.push(Move{ origin: 15, target: 31, promotion: 0, piece: PAWN });
        let mut expected_left_pawn_captures = Vec::<Move>::new();
        expected_left_pawn_captures.push(Move{ origin: 32, target: 41, promotion: 0, piece: PAWN });
        expected_left_pawn_captures.push(Move{ origin: 33, target: 42, promotion: 0, piece: PAWN });
        expected_left_pawn_captures.push(Move{ origin: 35, target: 44, promotion: 0, piece: PAWN });
        expected_left_pawn_captures.push(Move{ origin: 54, target: 63, promotion: 2, piece: PAWN });
        expected_left_pawn_captures.push(Move{ origin: 54, target: 63, promotion: 3, piece: PAWN });
        expected_left_pawn_captures.push(Move{ origin: 54, target: 63, promotion: 4, piece: PAWN });
        expected_left_pawn_captures.push(Move{ origin: 54, target: 63, promotion: 5, piece: PAWN });
        let mut expected_right_pawn_captures = Vec::<Move>::new();
        expected_right_pawn_captures.push(Move{ origin: 35, target: 42, promotion: 0, piece: PAWN });
        let expected_pawn_moves = [expected_push_moves.clone(), expected_double_push_moves.clone(),
            expected_left_pawn_captures.clone(), expected_right_pawn_captures.clone()].concat();

        assert_eq!(move_list.generate_white_push_bitboard(), expected_push_bitboard);
        assert_eq!(move_list.generate_white_double_push_bitboard(expected_push_bitboard), expected_double_push_bitboard);
        assert_eq!(move_list.generate_white_pawn_left_capture_bitboard(), expected_left_pawn_capture_bitboard);
        assert_eq!(move_list.generate_white_pawn_right_capture_bitboard(), expected_right_pawn_capture_bitboard);
        assert_eq!(move_list.generate_king_moves_bitboard(), expected_king_bitboard);
        assert_eq!(move_list.generate_knight_moves_bitboard(18), expected_knight_bitboard1);
        assert_eq!(move_list.generate_rook_moves_bitboard(21), expected_rook_bitboard1);
        assert_eq!(move_list.generate_bishop_moves_bitboard(34), expected_bishop_bitboard1);
        assert_eq!(move_list.generate_queen_moves_bitboard(25), expected_queen_bitboard);

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

        // KNIGHT MOVES

        let mut expected_knight_moves1 = Vec::<Move>::new();
        expected_knight_moves1.push( Move { origin: 18, target: 1, promotion: 0, piece: KNIGHT });
        expected_knight_moves1.push( Move { origin: 18, target: 3, promotion: 0, piece: KNIGHT });
        expected_knight_moves1.push( Move { origin: 18, target: 8, promotion: 0, piece: KNIGHT });
        expected_knight_moves1.push( Move { origin: 18, target: 12, promotion: 0, piece: KNIGHT });
        expected_knight_moves1.push( Move { origin: 18, target: 24, promotion: 0, piece: KNIGHT });

        let mut expected_knight_moves_all = [expected_knight_moves1.clone()].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.convert_knight_moves(expected_knight_bitboard1, 18);
        assert!(compare_vecs(&move_list.moves, &expected_knight_moves1));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_knight_moves();
        assert!(compare_vecs(&move_list.moves, &expected_knight_moves_all));

        // ROOK MOVES

        let mut expected_rook_moves1 = Vec::<Move>::new();
        expected_rook_moves1.push( Move { origin: 21, target: 29, promotion: 0, piece: ROOK });
        expected_rook_moves1.push( Move { origin: 21, target: 37, promotion: 0, piece: ROOK });
        expected_rook_moves1.push( Move { origin: 21, target: 45, promotion: 0, piece: ROOK });
        expected_rook_moves1.push( Move { origin: 21, target: 53, promotion: 0, piece: ROOK });
        expected_rook_moves1.push( Move { origin: 21, target: 22, promotion: 0, piece: ROOK });
        expected_rook_moves1.push( Move { origin: 21, target: 23, promotion: 0, piece: ROOK });
        expected_rook_moves1.push( Move { origin: 21, target: 20, promotion: 0, piece: ROOK });
        expected_rook_moves1.push( Move { origin: 21, target: 19, promotion: 0, piece: ROOK });
        expected_rook_moves1.push( Move { origin: 21, target: 13, promotion: 0, piece: ROOK });
        expected_rook_moves1.push( Move { origin: 21, target: 5, promotion: 0, piece: ROOK });
        let mut expected_rook_moves_all = [expected_rook_moves1.clone()].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.convert_rook_moves(expected_rook_bitboard1, 21);
        assert!(compare_vecs(&move_list.moves, &expected_rook_moves1));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_rook_moves();
        assert!(compare_vecs(&move_list.moves, &expected_rook_moves_all));

        // BISHOP MOVES

        let mut expected_bishop_moves1 = Vec::<Move>::new();
        expected_bishop_moves1.push( Move { origin: 34, target: 13, promotion: 0, piece: BISHOP });
        expected_bishop_moves1.push( Move { origin: 34, target: 20, promotion: 0, piece: BISHOP });
        expected_bishop_moves1.push( Move { origin: 34, target: 27, promotion: 0, piece: BISHOP });
        expected_bishop_moves1.push( Move { origin: 34, target: 41, promotion: 0, piece: BISHOP });
        expected_bishop_moves1.push( Move { origin: 34, target: 43, promotion: 0, piece: BISHOP });
        expected_bishop_moves1.push( Move { origin: 34, target: 52, promotion: 0, piece: BISHOP });
        expected_bishop_moves1.push( Move { origin: 34, target: 61, promotion: 0, piece: BISHOP });
        let mut expected_bishop_moves_all = [expected_bishop_moves1.clone()].concat();

        move_list.moves = Vec::<Move>::new();
        move_list.convert_bishop_moves(expected_bishop_bitboard1, 34);
        assert!(compare_vecs(&move_list.moves, &expected_bishop_moves1));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_bishop_moves();
        assert!(compare_vecs(&move_list.moves, &expected_bishop_moves_all));

        // QUEEN MOVES

        let mut expected_queen_moves = Vec::<Move>::new();

        move_list.moves = Vec::<Move>::new();
        expected_queen_moves.push( Move { origin: 25, target: 1, promotion: 0, piece: QUEEN });
        expected_queen_moves.push( Move { origin: 25, target: 9, promotion: 0, piece: QUEEN });
        expected_queen_moves.push( Move { origin: 25, target: 16, promotion: 0, piece: QUEEN });
        expected_queen_moves.push( Move { origin: 25, target: 17, promotion: 0, piece: QUEEN });
        expected_queen_moves.push( Move { origin: 25, target: 24, promotion: 0, piece: QUEEN });
        expected_queen_moves.push( Move { origin: 25, target: 26, promotion: 0, piece: QUEEN });
        expected_queen_moves.push( Move { origin: 25, target: 27, promotion: 0, piece: QUEEN });
        move_list.convert_queen_moves(expected_queen_bitboard, 25);
        assert!(compare_vecs(&move_list.moves, &expected_queen_moves));

        move_list.moves = Vec::<Move>::new();
        move_list.generate_queen_moves();
        assert!(compare_vecs(&move_list.moves, &expected_queen_moves));

        // ALL MOVES

        let expected_moves = [expected_pawn_moves, expected_king_moves, expected_knight_moves_all, expected_rook_moves_all, expected_bishop_moves_all, expected_queen_moves].concat();

        move_list.moves = Vec::<Move>::new();
        let start = Instant::now();
        move_list.generate_moves();
        let duration = start.elapsed();
        println!("generate_moves lasted for: {:?}", duration);
        assert!(compare_vecs(&move_list.moves, &expected_moves));
    }

    #[test]
    fn check_king_lookup() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);
        let mut move_list = MoveList::new(&mut board);

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
        let mut move_list = MoveList::new(&mut board);

        let mut expected_move_list = Vec::<Move>::new();

        expected_move_list.push(Move {origin: 3, target: 1, promotion: 1, piece: KING});
        expected_move_list.push(Move {origin: 3, target: 5, promotion: 1, piece: KING});

        move_list.generate_white_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_white_kingside_possible() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/8/8/R3K2R w K - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let mut expected_move_list = Vec::<Move>::new();

        expected_move_list.push(Move {origin: 3, target: 1, promotion: 1, piece: KING});

        move_list.generate_white_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_white_queenside_possible() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/8/8/R3K2R w Q - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let mut expected_move_list = Vec::<Move>::new();

        expected_move_list.push(Move {origin: 3, target: 5, promotion: 1, piece: KING});

        move_list.generate_white_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_white_check() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/8/5p2/R3K2R w KQ - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let expected_move_list = Vec::<Move>::new();

        move_list.generate_white_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_white_block() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/8/5p2/RP2KP1R w KQ - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let expected_move_list = Vec::<Move>::new();

        move_list.generate_white_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_white_guarded() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/8/p5p1/R3K2R w KQ - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 3, target: 5, promotion: 1, piece: KING});

        move_list.generate_white_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_black_both_possible() {
        let mut board = Board::new();
        board.read_fen("r3k2r/8/8/8/8/8/8/8 b kq - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let mut expected_move_list = Vec::<Move>::new();

        expected_move_list.push(Move {origin: 59, target: 57, promotion: 1, piece: KING});
        expected_move_list.push(Move {origin: 59, target: 61, promotion: 1, piece: KING});

        move_list.generate_black_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_black_kingside_possible() {
        let mut board = Board::new();
        board.read_fen("r3k2r/8/8/8/8/8/8/8 b k - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let mut expected_move_list = Vec::<Move>::new();

        expected_move_list.push(Move {origin: 59, target: 57, promotion: 1, piece: KING});

        move_list.generate_black_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_black_queenside_possible() {
        let mut board = Board::new();
        board.read_fen("r3k2r/8/8/8/8/8/8/8 b q - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let mut expected_move_list = Vec::<Move>::new();

        expected_move_list.push(Move {origin: 59, target: 61, promotion: 1, piece: KING});

        move_list.generate_black_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_black_check() {
        let mut board = Board::new();
        board.read_fen("r3k2r/5P2/8/8/8/8/8/8 b kq - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let expected_move_list = Vec::<Move>::new();

        move_list.generate_black_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_black_block() {
        let mut board = Board::new();
        board.read_fen("rp2kp1r/8/8/8/8/8/8/8 b kq - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let expected_move_list = Vec::<Move>::new();

        move_list.generate_black_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_black_guarded() {
        let mut board = Board::new();
        board.read_fen("r3k2r/6P1/8/8/8/8/8/8 b kq - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 59, target: 61, promotion: 1, piece: KING});

        move_list.generate_black_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_white_knight_check() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/5n2/8/R3K2R w KQ - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let expected_move_list = Vec::<Move>::new();

        move_list.generate_white_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_white_bishop_check() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/4b3/8/R3K2R w KQ - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let expected_move_list = Vec::<Move>::new();

        move_list.generate_white_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_white_rook_check() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/5r2/8/R3K2R w KQ - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 3, target: 5, promotion: 1, piece: KING});

        move_list.generate_white_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_castling_white_queen_check() {
        let mut board = Board::new();
        board.read_fen("8/8/8/2q5/8/8/8/R3K2R w KQ - 1 1");
        let mut move_list = MoveList::new(&mut board);

        let expected_move_list = Vec::<Move>::new();

        move_list.generate_white_castling();

        assert!(compare_vecs(&move_list.moves, &expected_move_list));
    }

    #[test]
    fn check_knight_lookup() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);
        let mut move_list = MoveList::new(&mut board);

        assert_eq!(0b0000000000000000000000000000000000000000000000100000010000000000, move_list.knight_lookup_table[0]);
        assert_eq!(0b0000000000000000000000000000000000000000000001010000100000000000, move_list.knight_lookup_table[1]);
        assert_eq!(0b0000000000000000000000000000000000000000000010100001000100000000, move_list.knight_lookup_table[2]);
        assert_eq!(0b0000000000000000000000000000000000000000010000000010000000000000, move_list.knight_lookup_table[7]);
        assert_eq!(0b0000000000000000000000000001010000100010000000000010001000010100, move_list.knight_lookup_table[19]);
        assert_eq!(0b0000010000000000000001000000001000000000000000000000000000000000, move_list.knight_lookup_table[48]);
    }

    #[test]
    fn check_rook_magic_bitboard_value_generation() {
        let key: u64 =          0b0000000000000000000001000000000000001000000000000000000000000000;
        let origin: u8 = 26;

        let value_left: u64 =   0b0000000000000000000000000000000000001000000000000000000000000000;
        let value_right: u64 =  0b0000000000000000000000000000000000000011000000000000000000000000;
        let value_top: u64 =    0b0000000000000000000001000000010000000000000000000000000000000000;
        let value_bottom: u64 = 0b0000000000000000000000000000000000000000000001000000010000000100;

        assert_eq!(value_left, MoveList::generate_rook_magic_bitboard_left(key, origin));
        assert_eq!(value_right, MoveList::generate_rook_magic_bitboard_right(key, origin));
        assert_eq!(value_top, MoveList::generate_rook_magic_bitboard_top(key, origin));
        assert_eq!(value_bottom, MoveList::generate_rook_magic_bitboard_bottom(key, origin));
        assert_eq!(value_left | value_right | value_top | value_bottom, MoveList::generate_rook_magic_value(key, origin));
    }

    #[test]
    fn check_rook_magic_bitboard_value_generation_edge() {
        let key: u64 =          0;
        let origin: u8 = 0;

        let value_left: u64 =   0b0000000000000000000000000000000000000000000000000000000011111110;
        let value_right: u64 =  0b0000000000000000000000000000000000000000000000000000000000000000;
        let value_top: u64 =    0b0000000100000001000000010000000100000001000000010000000100000000;
        let value_bottom: u64 = 0b0000000000000000000000000000000000000000000000000000000000000000;

        assert_eq!(value_left, MoveList::generate_rook_magic_bitboard_left(key, origin));
        assert_eq!(value_right, MoveList::generate_rook_magic_bitboard_right(key, origin));
        assert_eq!(value_top, MoveList::generate_rook_magic_bitboard_top(key, origin));
        assert_eq!(value_bottom, MoveList::generate_rook_magic_bitboard_bottom(key, origin));
        assert_eq!(value_left | value_right | value_top | value_bottom, MoveList::generate_rook_magic_value(key, origin));
    }

    #[test]
    fn check_rook_magic_bitboard_raw_key_generation() {
        let mut key_mask_vector = Vec::<u64>::new();
        // key_mask_vector.push(0b0000000000000000000000000000000000000000000000000000000000000100);
        key_mask_vector.push(0b0000000000000000000000000000000000000000000000000000010000000000);
        key_mask_vector.push(0b0000000000000000000000000000000000000000000001000000000000000000);
        // key_mask_vector.push(0b0000000000000000000000000000000000000001000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000000000000000000000010000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000000000000000000001000000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000000000000000000010000000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000000000000000000100000000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000000000000000001000000000000000000000000000000);
        // key_mask_vector.push(0b0000000000000000000000000000000010000000000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000000000000010000000000000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000001000000000000000000000000000000000000000000);
        key_mask_vector.push(0b0000000000000100000000000000000000000000000000000000000000000000);
        // key_mask_vector.push(0b0000010000000000000000000000000000000000000000000000000000000000);

        let combination_mask = 0b00000000001011;

        let expected_raw_mask=0b0000000000000000000000000000000000001000000001000000010000000000;

        assert_eq!(expected_raw_mask, MoveList::generate_magic_raw_key(&key_mask_vector, combination_mask));
    }

    #[test]
    fn check_rook_magic_bitboard_mask_vector_generation() {
        let mut key_mask_vector = Vec::<u64>::new();
        key_mask_vector.push(0b0000000000000000000000000000000000000000000000000000010000000000);
        key_mask_vector.push(0b0000000000000000000000000000000000000000000001000000000000000000);
        key_mask_vector.push(0b0000000000000000000000000000000000000010000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000000000000000000001000000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000000000000000000010000000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000000000000000000100000000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000000000000000001000000000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000000000000010000000000000000000000000000000000);
        key_mask_vector.push(0b0000000000000000000001000000000000000000000000000000000000000000);
        key_mask_vector.push(0b0000000000000100000000000000000000000000000000000000000000000000);

        // let mut expected_full_mask: u64= 0;
        // for i in key_mask_vector { expected_full_mask += i; }

        assert_eq!(key_mask_vector, MoveList::generate_rook_magic_key_mask(26));
    }

    #[test]
    fn rook_key_mask_vector() {
        let mut expected_vec1 = vec![0x0001000000000000, 0x0000010000000000, 0x0000000100000000, 0x0000000001000000, 0x000000000010000, 0x0000000000000100,
                        0x0000000000000040, 0x0000000000000020, 0x0000000000000010, 0x0000000000000008, 0x0000000000000004, 0x0000000000000002];
        expected_vec1.reverse();
        assert_eq!(expected_vec1, MoveList::generate_rook_magic_key_mask(0));

        let mut expected_vec2 = vec![0x0080000000000000, 0x0000800000000000, 0x0000008000000000, 0x0000000080000000, 0x000000000800000, 0x0000000000008000,
                        0x0000000000000040, 0x0000000000000020, 0x0000000000000010, 0x0000000000000008, 0x0000000000000004, 0x0000000000000002];
        expected_vec2.reverse();
        assert_eq!(expected_vec2, MoveList::generate_rook_magic_key_mask(7));

        let mut expected_vec3 = vec![0x0008000000000000, 0x0000080000000000, 0x0000000800000000, 0x0000000000080000, 0x0000000000000800,
                                     0x0000000040000000, 0x0000000020000000, 0x0000000010000000, 0x0000000004000000, 0x0000000002000000];
        expected_vec3.sort();
        let mut actual = MoveList::generate_rook_magic_key_mask(27);
        actual.sort();
        assert_eq!(expected_vec3, actual); // rook on e4
    }

    #[test]
    fn rook_raw_mask() {
        let mut vec1 = vec![0x0001000000000000, 0x0000010000000000, 0x0000000100000000, 0x0000000001000000, 0x000000000010000, 0x0000000000000100,
                        0x0000000000000040, 0x0000000000000020, 0x0000000000000010, 0x0000000000000008, 0x0000000000000004, 0x0000000000000002];
        vec1.reverse();

        let combination_mask = 0b1001101;

        let expected = vec1[0] + vec1[2] + vec1[3] + vec1[6];

        assert_eq!(expected, MoveList::generate_magic_raw_key(&vec1, combination_mask));
    }

    #[test]
    fn rook_magic_bitboard() {
        let mut vec1 = vec![0x0001000000000000, 0x0000010000000000, 0x0000000100000000, 0x0000000001000000, 0x000000000010000, 0x0000000000000100,
                        0x0000000000000040, 0x0000000000000020, 0x0000000000000010, 0x0000000000000008, 0x0000000000000004, 0x0000000000000002];
        vec1.reverse();

        let key1 = vec1[0] + vec1[2] + vec1[3] + vec1[6];
        let key2 = 0;
        let key3 = vec1[1] + vec1[3] + vec1[9] + vec1[10];

        // no occupancy => 0x01010101010101FE
        assert_eq!(vec1[0] + vec1[6], MoveList::generate_rook_magic_value(key1, 0));
        assert_eq!(0x01010101010101FE, MoveList::generate_rook_magic_value(key2, 0));
        assert_eq!(vec1[0] + vec1[1] + vec1[6] + vec1[7] + vec1[8] + vec1[9], MoveList::generate_rook_magic_value(key3, 0));

        let mut vec2 = vec![0x0080000000000000, 0x0000800000000000, 0x0000008000000000, 0x0000000080000000, 0x000000000800000, 0x0000000000008000,
                        0x0000000000000040, 0x0000000000000020, 0x0000000000000010, 0x0000000000000008, 0x0000000000000004, 0x0000000000000002];
        vec2.reverse();

        let key4 = 0;
        let key5 = vec2[6-1] + vec2[6-3] + vec2[9] + vec2[10];

        assert_eq!(0x808080808080807F, MoveList::generate_rook_magic_value(key4, 7));
        assert_eq!(vec2[6-1] + vec2[6] + vec2[7] + vec2[8] + vec2[9], MoveList::generate_rook_magic_value(key5, 7));
    }

    #[test]
    fn check_rook_magic_bitboard() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);
        let move_list = MoveList::new(&mut board);

        assert_eq!(0x01010101010101FE, move_list.rook_magic_bitboard[0]);
        assert_eq!(0x808080808080807F, move_list.rook_magic_bitboard[0 | (0b111 << 12)]);
        assert_eq!(0x0000000814080000, move_list.rook_magic_bitboard[magic_hash_rook(0x0000000814080000, 27)]);
    }

    #[test]
    fn check_bishop_magic_bitboard_value_generation() {
        let key: u64 =          0x0000000800000000;
        let origin: u8 = 26;

        // let value: u64 =         0x0000010A000A1120;
        let value_north_west: u64 = 0x0000000800000000;
        let value_north_east: u64 = 0x0000010200000000;
        let value_south_east: u64 = 0x0000000000020100;
        let value_south_west: u64 = 0x0000000000081020;

        assert_eq!(value_north_west, MoveList::generate_bishop_magic_bitboard_north_west(key, origin));
        assert_eq!(value_north_east, MoveList::generate_bishop_magic_bitboard_north_east(key, origin));
        assert_eq!(value_south_east, MoveList::generate_bishop_magic_bitboard_south_east(key, origin));
        assert_eq!(value_south_west, MoveList::generate_bishop_magic_bitboard_south_west(key, origin));
        assert_eq!(value_north_west | value_north_east | value_south_east | value_south_west,
                   MoveList::generate_bishop_magic_value(key, origin));
    }

    #[test]
    fn check_bishop_magic_bitboard_value_generation_edge() {
        let key: u64 =          0;
        let origin: u8 = 0;

        let value_north_west: u64 = 0x8040201008040200;
        let value_north_east: u64 = 0;
        let value_south_east: u64 = 0;
        let value_south_west: u64 = 0;

        assert_eq!(value_north_west, MoveList::generate_bishop_magic_bitboard_north_west(key, origin));
        assert_eq!(value_north_east, MoveList::generate_bishop_magic_bitboard_north_east(key, origin));
        assert_eq!(value_south_east, MoveList::generate_bishop_magic_bitboard_south_east(key, origin));
        assert_eq!(value_south_west, MoveList::generate_bishop_magic_bitboard_south_west(key, origin));
        assert_eq!(value_north_west | value_north_east | value_south_east | value_south_west,
                   MoveList::generate_bishop_magic_value(key, origin));
    }

    #[test]
    fn check_bishop_magic_bitboard_value_generation_insignificant_blockers() {
        let key: u64 =              0x0000020400042200;
        let origin: u8 = 27;

        let value_north_west: u64 = 0x8040201000000000;
        let value_north_east: u64 = 0x0000000400000000;
        let value_south_east: u64 = 0x0000000000040000;
        let value_south_west: u64 = 0x0000000000102000;

        assert_eq!(value_north_west, MoveList::generate_bishop_magic_bitboard_north_west(key, origin));
        assert_eq!(value_north_east, MoveList::generate_bishop_magic_bitboard_north_east(key, origin));
        assert_eq!(value_south_east, MoveList::generate_bishop_magic_bitboard_south_east(key, origin));
        assert_eq!(value_south_west, MoveList::generate_bishop_magic_bitboard_south_west(key, origin));
        assert_eq!(value_north_west | value_north_east | value_south_east | value_south_west,
                   MoveList::generate_bishop_magic_value(key, origin));
    }


    #[test]
    fn check_bishop_magic_bitboard_mask_vector_generation() {
        let mut key_mask_vector = Vec::<u64>::new();
        key_mask_vector.push(0x0000000000001000);
        key_mask_vector.push(0x0000000000020000);
        key_mask_vector.push(0x0000000000080000);
        key_mask_vector.push(0x0000000200000000);
        key_mask_vector.push(0x0000000800000000);
        key_mask_vector.push(0x0000100000000000);
        key_mask_vector.push(0x0020000000000000);

        // let mut expected_full_mask: u64= 0;
        // for i in key_mask_vector { expected_full_mask += i; }

        assert_eq!(key_mask_vector, MoveList::generate_bishop_magic_key_mask(26));
    }

    #[test]
    fn check_bishop_magic_bitboard() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);
        let move_list = MoveList::new(&mut board);

        assert_eq!(0x8040201008040200, move_list.bishop_magic_bitboard[0]);
        assert_eq!(0x0102040810204000, move_list.bishop_magic_bitboard[0 | (0b111 << 10)]);
        assert_eq!(0x0000001400140000, move_list.bishop_magic_bitboard[magic_hash_bishop(0x0000001400140000, 27)]);
    }

    #[test]
    fn quiet_move_pawn() {
        let mut board = Board::new();
        let fen = "r3k3/1Pr5/5pp1/3pPBPP/1b1P2Qq/2R2N2/P7/RK6 w q d6 1 25";
        board.read_fen(fen);
        let board_copy = board.clone();
        let mut move_list = MoveList::new(&mut board);

        let main_bitboard = board_copy.main_bitboard;
        let colour_bitboards = board_copy.colour_bitboards.clone();
        let piece_bitboards = board_copy.piece_bitboards.clone();
        let en_passant = board_copy.en_passant_possibility.clone();
        let castling_rights = board_copy.castling_rights.clone();

        let piece_move = Move { origin: 15, target: 23, promotion: 0, piece: PAWN };

        let origin: u64 = 1 << 15;
        let target: u64 = 1 << 23;

        move_list.make_move(&piece_move);

        assert_eq!(main_bitboard - origin + target, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0] - origin + target, move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            if i == PAWN as usize
            {
                assert_eq!(piece_bitboards[i] - origin + target, move_list.board.piece_bitboards[i]);
                continue;
            }

            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }

        assert_eq!(NO_CAPTURE, move_list.capture_history[0]);
        assert_eq!(44, move_list.en_passant_history[0]);
        assert_eq!([false, false, false, true], move_list.castling_rights_history[0]);
        assert_eq!(NO_PASSANT, move_list.board.en_passant_possibility);

        // UNMAKE MOVE

        move_list.unmake_move(&piece_move);

        assert_eq!(main_bitboard, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0], move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }
        assert_eq!(en_passant, move_list.board.en_passant_possibility);
        assert_eq!(castling_rights, move_list.board.castling_rights);
    }

    #[test]
    fn quiet_move_bishop() {
        let mut board = Board::new();
        let fen = "r3k3/1Pr5/5pp1/3pPBPP/1b1P2Qq/2R2N2/P7/RK6 w q d6 1 25";
        board.read_fen(fen);
        let board_copy = board.clone();
        let mut move_list = MoveList::new(&mut board);


        let main_bitboard = board_copy.main_bitboard;
        let colour_bitboards = board_copy.colour_bitboards.clone();
        let piece_bitboards = board_copy.piece_bitboards.clone();
        let en_passant = board_copy.en_passant_possibility.clone();
        let castling_rights = board_copy.castling_rights.clone();

        let piece_move = Move { origin: 34, target: 43, promotion: 0, piece: BISHOP };

        let origin: u64 = 1 << 34;
        let target: u64 = 1 << 43;

        move_list.make_move(&piece_move);

        assert_eq!(main_bitboard - origin + target, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0] - origin + target, move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            if i == BISHOP as usize
            {
                assert_eq!(piece_bitboards[i] - origin + target, move_list.board.piece_bitboards[i]);
                continue;
            }

            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }

        assert_eq!(NO_CAPTURE, move_list.capture_history[0]);
        assert_eq!(44, move_list.en_passant_history[0]);
        assert_eq!([false, false, false, true], move_list.castling_rights_history[0]);
        assert_eq!(NO_PASSANT, move_list.board.en_passant_possibility);

        // UNMAKE MOVE

        move_list.unmake_move(&piece_move);

        assert_eq!(main_bitboard, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0], move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }
        assert_eq!(en_passant, move_list.board.en_passant_possibility);
        assert_eq!(castling_rights, move_list.board.castling_rights);
    }

    #[test]
    fn quiet_move_king() {
        let mut board = Board::new();
        let fen = "r3k3/1Pr5/5pp1/3pPBPP/1b1P2Qq/2R2N2/P7/RK6 w q d6 1 25";
        board.read_fen(fen);
        let board_copy = board.clone();
        let mut move_list = MoveList::new(&mut board);

        let main_bitboard = board_copy.main_bitboard;
        let colour_bitboards = board_copy.colour_bitboards.clone();
        let piece_bitboards = board_copy.piece_bitboards.clone();
        let en_passant = board_copy.en_passant_possibility.clone();
        let castling_rights = board_copy.castling_rights.clone();

        let piece_move = Move { origin: 6, target: 5, promotion: 0, piece: KING };

        let origin: u64 = 1 << 6;
        let target: u64 = 1 << 5;

        move_list.make_move(&piece_move);

        assert_eq!(main_bitboard - origin + target, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0] - origin + target, move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            if i == KING as usize
            {
                assert_eq!(piece_bitboards[i] - origin + target, move_list.board.piece_bitboards[i]);
                continue;
            }

            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }

        assert_eq!(NO_CAPTURE, move_list.capture_history[0]);
        assert_eq!(44, move_list.en_passant_history[0]);
        assert_eq!([false, false, false, true], move_list.castling_rights_history[0]);
        assert_eq!(NO_PASSANT, move_list.board.en_passant_possibility);

        // UNMAKE MOVE

        move_list.unmake_move(&piece_move);

        assert_eq!(main_bitboard, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0], move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }
        assert_eq!(en_passant, move_list.board.en_passant_possibility);
        assert_eq!(castling_rights, move_list.board.castling_rights);
    }

    #[test]
    fn quiet_move_black_pawn() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b K - 1 2";
        board.read_fen(fen);
        let board_copy = board.clone();
        let mut move_list = MoveList::new(&mut board);

        let main_bitboard = board_copy.main_bitboard;
        let colour_bitboards = board_copy.colour_bitboards.clone();
        let piece_bitboards = board_copy.piece_bitboards.clone();
        let en_passant = board_copy.en_passant_possibility.clone();
        let castling_rights = board_copy.castling_rights.clone();

        let piece_move = Move { origin: 37, target: 29, promotion: 0, piece: PAWN };

        let origin: u64 = 1 << 37;
        let target: u64 = 1 << 29;

        move_list.make_move(&piece_move);

        assert_eq!(main_bitboard - origin + target, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[1] - origin + target, move_list.board.colour_bitboards[1]);
        assert_eq!(colour_bitboards[0], move_list.board.colour_bitboards[0]);
        for i in 0..12 {
            if i == PAWN as usize + 6
            {
                assert_eq!(piece_bitboards[i] - origin + target, move_list.board.piece_bitboards[i]);
                continue;
            }

            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }

        assert_eq!(NO_CAPTURE, move_list.capture_history[0]);
        assert_eq!(64, move_list.en_passant_history[0]);
        assert_eq!([true, false, false, false], move_list.castling_rights_history[0]);
        assert_eq!(NO_PASSANT, move_list.board.en_passant_possibility);

        // UNMAKE MOVE

        move_list.unmake_move(&piece_move);

        assert_eq!(main_bitboard, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0], move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }
        assert_eq!(en_passant, move_list.board.en_passant_possibility);
        assert_eq!(castling_rights, move_list.board.castling_rights);
    }

    #[test]
    fn double_move_black_pawn() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b K - 1 2";
        board.read_fen(fen);
        let board_copy = board.clone();
        let mut move_list = MoveList::new(&mut board);

        let main_bitboard = board_copy.main_bitboard;
        let colour_bitboards = board_copy.colour_bitboards.clone();
        let piece_bitboards = board_copy.piece_bitboards.clone();
        let en_passant = board_copy.en_passant_possibility.clone();
        let castling_rights = board_copy.castling_rights.clone();

        let piece_move = Move { origin: 55, target: 39, promotion: 0, piece: PAWN };

        let origin: u64 = 1 << 55;
        let target: u64 = 1 << 39;

        move_list.make_move(&piece_move);

        assert_eq!(main_bitboard - origin + target, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[1] - origin + target, move_list.board.colour_bitboards[1]);
        assert_eq!(colour_bitboards[0], move_list.board.colour_bitboards[0]);
        for i in 0..12 {
            if i == PAWN as usize + 6
            {
                assert_eq!(piece_bitboards[i] - origin + target, move_list.board.piece_bitboards[i]);
                continue;
            }

            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }

        assert_eq!(NO_CAPTURE, move_list.capture_history[0]);
        assert_eq!(64, move_list.en_passant_history[0]);
        assert_eq!([true, false, false, false], move_list.castling_rights_history[0]);
        assert_eq!(47, move_list.board.en_passant_possibility);

        // UNMAKE MOVE

        move_list.unmake_move(&piece_move);

        assert_eq!(main_bitboard, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0], move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }
        assert_eq!(en_passant, move_list.board.en_passant_possibility);
        assert_eq!(castling_rights, move_list.board.castling_rights);
    }

    #[test]
    fn capture() {
        let mut board = Board::new();
        let fen = "r3k3/1Pr5/5pp1/3pPBPP/1b1P2Qq/2R2N2/P7/RK6 w q d6 1 25";
        board.read_fen(fen);
        let board_copy = board.clone();
        let mut move_list = MoveList::new(&mut board);

        let main_bitboard = board_copy.main_bitboard;
        let colour_bitboards = board_copy.colour_bitboards.clone();
        let piece_bitboards = board_copy.piece_bitboards.clone();
        let en_passant = board_copy.en_passant_possibility.clone();
        let castling_rights = board_copy.castling_rights.clone();

        let piece_move = Move { origin: 32, target: 41, promotion: 0, piece: PAWN };

        let origin: u64 = 1 << 32;
        let target: u64 = 1 << 41;

        move_list.make_move(&piece_move);

        assert_eq!(main_bitboard - origin | target, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0] - origin | target, move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1] - target, move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            if i == PAWN as usize
            {
                assert_eq!(piece_bitboards[i] - origin + target, move_list.board.piece_bitboards[i]);
                continue;
            }
            if i == PAWN as usize + 6
            {
                assert_eq!(piece_bitboards[i] - target, move_list.board.piece_bitboards[i]);
                continue;
            }

            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }

        assert_eq!(7, move_list.capture_history[0]);
        assert_eq!(44, move_list.en_passant_history[0]);
        assert_eq!([false, false, false, true], move_list.castling_rights_history[0]);

        // UNMAKE MOVE

        move_list.unmake_move(&piece_move);

        assert_eq!(main_bitboard, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0], move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }
        assert_eq!(en_passant, move_list.board.en_passant_possibility);
        assert_eq!(castling_rights, move_list.board.castling_rights);
    }

    #[test]
    fn promotion() {
        let mut board = Board::new();
        let fen = "r3k3/1Pr5/5pp1/3pPBPP/1b1P2Qq/2R2N2/P7/RK6 w q d6 1 25";
        board.read_fen(fen);
        let board_copy = board.clone();
        let mut move_list = MoveList::new(&mut board);

        let main_bitboard = board_copy.main_bitboard;
        let colour_bitboards = board_copy.colour_bitboards.clone();
        let piece_bitboards = board_copy.piece_bitboards.clone();
        let en_passant = board_copy.en_passant_possibility.clone();
        let castling_rights = board_copy.castling_rights.clone();

        let piece_move = Move { origin: 54, target: 63, promotion: 2, piece: PAWN };

        let origin: u64 = 1 << 54;
        let target: u64 = 1 << 63;

        let start = Instant::now();
        move_list.make_move(&piece_move);
        let duration = start.elapsed();
        println!("make_move lasted for: {:?}", duration);

        assert_eq!(main_bitboard - origin | target, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0] - origin | target, move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1] - target, move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            if i == PAWN as usize
            {
                assert_eq!(piece_bitboards[i] - origin, move_list.board.piece_bitboards[i]);
                continue;
            }
            if i == QUEEN as usize
            {
                assert_eq!(piece_bitboards[i] + target, move_list.board.piece_bitboards[i]);
                continue;
            }
            if i == ROOK as usize + 6
            {
                assert_eq!(piece_bitboards[i] - target, move_list.board.piece_bitboards[i]);
                continue;
            }

            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }

        assert_eq!(9, move_list.capture_history[0]);
        assert_eq!(44, move_list.en_passant_history[0]);
        assert_eq!([false, false, false, true], move_list.castling_rights_history[0]);

        // UNMAKE MOVE

        let start = Instant::now();
        move_list.unmake_move(&piece_move);
        let duration = start.elapsed();
        println!("unmake_move lasted for: {:?}", duration);

        assert_eq!(main_bitboard, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0], move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }
        assert_eq!(en_passant, move_list.board.en_passant_possibility);
        assert_eq!(castling_rights, move_list.board.castling_rights);
    }

    #[test]
    fn castling_white_kingside() {
        let mut board = Board::new();
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 1 1";
        board.read_fen(fen);
        let board_copy = board.clone();
        let mut move_list = MoveList::new(&mut board);

        let main_bitboard = board_copy.main_bitboard;
        let colour_bitboards = board_copy.colour_bitboards.clone();
        let piece_bitboards = board_copy.piece_bitboards.clone();
        let en_passant = board_copy.en_passant_possibility.clone();
        let castling_rights = board_copy.castling_rights.clone();

        let piece_move = Move { origin: 3, target: 1, promotion: 1, piece: KING };

        let origin: u64 = 1 << 3;
        let target: u64 = 1 << 1;
        let rook_origin: u64 = 1 << 0;
        let rook_target: u64 = 1 << 2;

        move_list.make_move(&piece_move);

        assert_eq!(main_bitboard - origin + target - rook_origin + rook_target, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0] - origin + target - rook_origin + rook_target, move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            if i == KING as usize
            {
                assert_eq!(piece_bitboards[i] - origin + target, move_list.board.piece_bitboards[i]);
                continue;
            }
            if i == ROOK as usize
            {
                assert_eq!(piece_bitboards[i] - rook_origin + rook_target, move_list.board.piece_bitboards[i]);
                continue;
            }

            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }

        assert_eq!(NO_CAPTURE, move_list.capture_history[0]);
        assert_eq!([true, true, true, true], move_list.castling_rights_history[0]);
        assert_eq!([false, false, true, true], move_list.board.castling_rights);

        // UNMAKE MOVE

        move_list.unmake_move(&piece_move);

        assert_eq!(main_bitboard, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0], move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }
        assert_eq!(en_passant, move_list.board.en_passant_possibility);
        assert_eq!(castling_rights, move_list.board.castling_rights);
    }

    #[test]
    fn castling_white_queenside() {
        let mut board = Board::new();
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 1 1";
        board.read_fen(fen);
        let board_copy = board.clone();
        let mut move_list = MoveList::new(&mut board);

        let main_bitboard = board_copy.main_bitboard;
        let colour_bitboards = board_copy.colour_bitboards.clone();
        let piece_bitboards = board_copy.piece_bitboards.clone();

        let piece_move = Move { origin: 3, target: 5, promotion: 1, piece: KING };

        let origin: u64 = 1 << 3;
        let target: u64 = 1 << 5;
        let rook_origin: u64 = 1 << 7;
        let rook_target: u64 = 1 << 4;

        move_list.make_move(&piece_move);

        assert_eq!(main_bitboard - origin + target - rook_origin + rook_target, board.main_bitboard);
        assert_eq!(colour_bitboards[0] - origin + target - rook_origin + rook_target, board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], board.colour_bitboards[1]);
        for i in 0..12 {
            if i == KING as usize
            {
                assert_eq!(piece_bitboards[i] - origin + target, board.piece_bitboards[i]);
                continue;
            }
            if i == ROOK as usize
            {
                assert_eq!(piece_bitboards[i] - rook_origin + rook_target, board.piece_bitboards[i]);
                continue;
            }

            assert_eq!(piece_bitboards[i], board.piece_bitboards[i]);
        }
    }

    #[test]
    fn castling_black_kingside() {
        let mut board = Board::new();
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 1 1";
        board.read_fen(fen);
        let board_copy = board.clone();
        let mut move_list = MoveList::new(&mut board);

        let main_bitboard = board_copy.main_bitboard;
        let colour_bitboards = board_copy.colour_bitboards.clone();
        let piece_bitboards = board_copy.piece_bitboards.clone();
        let en_passant = board_copy.en_passant_possibility.clone();
        let castling_rights = board_copy.castling_rights.clone();

        let piece_move = Move { origin: 59, target: 57, promotion: 1, piece: KING };

        let origin: u64 = 1 << 59;
        let target: u64 = 1 << 57;
        let rook_origin: u64 = 1 << 56;
        let rook_target: u64 = 1 << 58;

        move_list.make_move(&piece_move);

        assert_eq!(main_bitboard - origin + target - rook_origin + rook_target, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[1] - origin + target - rook_origin + rook_target, move_list.board.colour_bitboards[1]);
        assert_eq!(colour_bitboards[0], move_list.board.colour_bitboards[0]);
        for i in 0..12 {
            if i == KING as usize + 6
            {
                assert_eq!(piece_bitboards[i] - origin + target, move_list.board.piece_bitboards[i]);
                continue;
            }
            if i == ROOK as usize + 6
            {
                assert_eq!(piece_bitboards[i] - rook_origin + rook_target, move_list.board.piece_bitboards[i]);
                continue;
            }

            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }

        assert_eq!(NO_CAPTURE, move_list.capture_history[0]);
        assert_eq!([true, true, true, true], move_list.castling_rights_history[0]);
        assert_eq!([true, true, false, false], move_list.board.castling_rights);

        // UNMAKE MOVE

        move_list.unmake_move(&piece_move);

        assert_eq!(main_bitboard, move_list.board.main_bitboard);
        assert_eq!(colour_bitboards[0], move_list.board.colour_bitboards[0]);
        assert_eq!(colour_bitboards[1], move_list.board.colour_bitboards[1]);
        for i in 0..12 {
            assert_eq!(piece_bitboards[i], move_list.board.piece_bitboards[i]);
        }
        assert_eq!(en_passant, move_list.board.en_passant_possibility);
        assert_eq!(castling_rights, move_list.board.castling_rights);
    }

    #[test]
    fn castling_black_queenside() {
        let mut board = Board::new();
        let fen = "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 1 1";
        board.read_fen(fen);
        let board_copy = board.clone();
        let mut move_list = MoveList::new(&mut board);

        let main_bitboard = board_copy.main_bitboard;
        let mut colour_bitboards = board_copy.colour_bitboards;
        let mut piece_bitboards = board_copy.piece_bitboards;

        let piece_move = Move { origin: 59, target: 61, promotion: 1, piece: KING };

        let origin: u64 = 1 << 59;
        let target: u64 = 1 << 61;
        let rook_origin: u64 = 1 << 63;
        let rook_target: u64 = 1 << 60;

        move_list.make_move(&piece_move);

        assert_eq!(main_bitboard - origin + target - rook_origin + rook_target, board.main_bitboard);
        assert_eq!(colour_bitboards[1] - origin + target - rook_origin + rook_target, board.colour_bitboards[1]);
        assert_eq!(colour_bitboards[0], board.colour_bitboards[0]);
        for i in 0..12 {
            if i == KING as usize + 6
            {
                assert_eq!(piece_bitboards[i] - origin + target, board.piece_bitboards[i]);
                continue;
            }
            if i == ROOK as usize + 6
            {
                assert_eq!(piece_bitboards[i] - rook_origin + rook_target, board.piece_bitboards[i]);
                continue;
            }

            assert_eq!(piece_bitboards[i], board.piece_bitboards[i]);
        }
    }

    #[test]
    fn bench_is_in_check() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut move_list = MoveList::new(&mut board);

        let start = Instant::now();
        move_list.is_in_check();
        let duration = start.elapsed();
        println!("is_in_check lasted for {:?}", duration);
    }

    #[test]
    fn check_generate_king_captures() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/8/4p3/3bK2R w K - 0 1");

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 3, target: 4, promotion: 0, piece: KING});
        expected_move_list.push(Move {origin: 3, target: 11, promotion: 0, piece: KING});

        let mut move_list = MoveList::new(&mut board);

        move_list.generate_king_captures();

        assert!(compare_vecs(move_list.get_moves(), &expected_move_list));
    }

    #[test]
    fn check_generate_white_pawn_captures() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/8/3p4/4P3 w - - 0 1");

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 3, target: 12, promotion: 0, piece: PAWN});

        let mut move_list = MoveList::new(&mut board);

        move_list.generate_white_pawn_captures();

        assert!(compare_vecs(move_list.get_moves(), &expected_move_list));
    }

    #[test]
    fn check_generate_black_pawn_captures() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/8/3p4/4P3 b - - 0 1");

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 12, target: 3, promotion: 2, piece: PAWN});
        expected_move_list.push(Move {origin: 12, target: 3, promotion: 3, piece: PAWN});
        expected_move_list.push(Move {origin: 12, target: 3, promotion: 4, piece: PAWN});
        expected_move_list.push(Move {origin: 12, target: 3, promotion: 5, piece: PAWN});

        let mut move_list = MoveList::new(&mut board);

        move_list.generate_black_pawn_captures();

        assert!(compare_vecs(move_list.get_moves(), &expected_move_list));
    }

    #[test]
    fn check_generate_knight_captures() {
        let mut board = Board::new();
        board.read_fen("8/8/8/8/8/3p4/8/4N3 w - - 0 1");

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 3, target: 20, promotion: 0, piece: KNIGHT});

        let mut move_list = MoveList::new(&mut board);

        move_list.generate_knight_captures();

        assert!(compare_vecs(move_list.get_moves(), &expected_move_list));
    }

    #[test]
    fn check_generate_rook_captures() {
        let mut board = Board::new();
        board.read_fen("8/8/4p3/8/8/8/8/2p1R3 w - - 0 1");

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 3, target: 43, promotion: 0, piece: ROOK});
        expected_move_list.push(Move {origin: 3, target: 5, promotion: 0, piece: ROOK});

        let mut move_list = MoveList::new(&mut board);

        move_list.generate_rook_captures();

        assert!(compare_vecs(move_list.get_moves(), &expected_move_list));
    }

    #[test]
    fn check_generate_bishop_captures() {
        let mut board = Board::new();
        board.read_fen("8/8/7p/2p5/8/4B3/8/8 w - - 0 1");

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 19, target: 40, promotion: 0, piece: BISHOP});
        expected_move_list.push(Move {origin: 19, target: 37, promotion: 0, piece: BISHOP});

        let mut move_list = MoveList::new(&mut board);

        move_list.generate_bishop_captures();

        assert!(compare_vecs(move_list.get_moves(), &expected_move_list));
    }

    #[test]
    fn check_generate_queen_captures() {
        let mut board = Board::new();
        board.read_fen("8/4p3/7p/2p5/8/2p1Q3/8/8 w - - 0 1");

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 19, target: 40, promotion: 0, piece: QUEEN});
        expected_move_list.push(Move {origin: 19, target: 37, promotion: 0, piece: QUEEN});
        expected_move_list.push(Move {origin: 19, target: 21, promotion: 0, piece: QUEEN});
        expected_move_list.push(Move {origin: 19, target: 51, promotion: 0, piece: QUEEN});

        let mut move_list = MoveList::new(&mut board);

        move_list.generate_queen_captures();

        assert!(compare_vecs(move_list.get_moves(), &expected_move_list));
    }

    #[test]
    fn check_generate_captures() {
        let mut board = Board::new();
        board.read_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R1p1K2R w KQkq - ");

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 7, target: 5, promotion: 0, piece: ROOK});
        expected_move_list.push(Move {origin: 9, target: 16, promotion: 0, piece: PAWN});
        expected_move_list.push(Move {origin: 18, target: 16, promotion: 0, piece: QUEEN});
        expected_move_list.push(Move {origin: 18, target: 42, promotion: 0, piece: QUEEN});
        expected_move_list.push(Move {origin: 11, target: 47, promotion: 0, piece: BISHOP});
        expected_move_list.push(Move {origin: 12, target: 5, promotion: 0, piece: BISHOP});
        expected_move_list.push(Move {origin: 36, target: 43, promotion: 0, piece: PAWN});
        expected_move_list.push(Move {origin: 35, target: 41, promotion: 0, piece: KNIGHT});
        expected_move_list.push(Move {origin: 35, target: 50, promotion: 0, piece: KNIGHT});
        expected_move_list.push(Move {origin: 35, target: 52, promotion: 0, piece: KNIGHT});

        let mut move_list = MoveList::new(&mut board);

        move_list.generate_captures();

        assert!(compare_vecs(move_list.get_moves(), &expected_move_list));
    }

    #[test]
    fn check_generate_white_promotions() {
        let mut board = Board::new();
        board.read_fen("8/P7/7P/8/8/8/4P3/8 w - - 0 1");

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 55, target: 63, promotion: 2, piece: PAWN});
        expected_move_list.push(Move {origin: 55, target: 63, promotion: 3, piece: PAWN});
        expected_move_list.push(Move {origin: 55, target: 63, promotion: 4, piece: PAWN});
        expected_move_list.push(Move {origin: 55, target: 63, promotion: 5, piece: PAWN});

        let mut move_list = MoveList::new(&mut board);

        move_list.generate_white_promotions();

        assert!(compare_vecs(move_list.get_moves(), &expected_move_list));
    }

    #[test]
    fn check_generate_black_promotions() {
        let mut board = Board::new();
        board.read_fen("8/4p3/8/8/8/1p6/7p/8 w - - 0 11");

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 8, target: 0, promotion: 2, piece: PAWN});
        expected_move_list.push(Move {origin: 8, target: 0, promotion: 3, piece: PAWN});
        expected_move_list.push(Move {origin: 8, target: 0, promotion: 4, piece: PAWN});
        expected_move_list.push(Move {origin: 8, target: 0, promotion: 5, piece: PAWN});

        let mut move_list = MoveList::new(&mut board);

        move_list.generate_black_promotions();

        assert!(compare_vecs(move_list.get_moves(), &expected_move_list));
    }
    #[test]
    fn check_generate_checks() {
        let mut board = Board::new();
        board.read_fen("4k3/8/1R3P2/8/8/3B4/8/8 w - - 0 1");

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 46, target: 62, promotion: 0, piece: ROOK});
        expected_move_list.push(Move {origin: 46, target: 43, promotion: 0, piece: ROOK});
        expected_move_list.push(Move {origin: 42, target: 50, promotion: 0, piece: PAWN});
        expected_move_list.push(Move {origin: 20, target: 41, promotion: 0, piece: BISHOP});
        expected_move_list.push(Move {origin: 20, target: 38, promotion: 0, piece: BISHOP});

        let mut move_list = MoveList::new(&mut board);

        let start = Instant::now();
        move_list.generate_checks();
        let duration = start.elapsed();
        println!("generate_checks lasted for {:?}", duration);

        assert!(compare_vecs(move_list.get_moves(), &expected_move_list));
    }

    #[test]
    fn check_generate_noisy_moves() {
        let mut board = Board::new();
        board.read_fen("4k3/6P1/8/8/3B4/2Qp4/p2Rp3/8 w - - 0 1");

        let mut expected_move_list = Vec::<Move>::new();
        expected_move_list.push(Move {origin: 49, target: 57, promotion: 2, piece: PAWN});
        expected_move_list.push(Move {origin: 49, target: 57, promotion: 3, piece: PAWN});
        expected_move_list.push(Move {origin: 49, target: 57, promotion: 4, piece: PAWN});
        expected_move_list.push(Move {origin: 49, target: 57, promotion: 5, piece: PAWN});
        expected_move_list.push(Move {origin: 21, target: 61, promotion: 0, piece: QUEEN});
        expected_move_list.push(Move {origin: 21, target: 45, promotion: 0, piece: QUEEN});
        expected_move_list.push(Move {origin: 21, target: 20, promotion: 0, piece: QUEEN});
        expected_move_list.push(Move {origin: 12, target: 11, promotion: 0, piece: ROOK});
        expected_move_list.push(Move {origin: 12, target: 15, promotion: 0, piece: ROOK});
        expected_move_list.push(Move {origin: 12, target: 20, promotion: 0, piece: ROOK});

        let mut move_list = MoveList::new(&mut board);

        let start = Instant::now();
        move_list.generate_noisy_moves();
        let duration = start.elapsed();
        println!("generate_checks lasted for {:?}", duration);

        assert!(compare_vecs(move_list.get_moves(), &expected_move_list));
    }
}