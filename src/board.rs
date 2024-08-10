//! Board representation and FEN utility
//! Defines the Board struct that holds all the information about current situation on the board
//! i.e. bitboards, active player, castling rights, en passant possibility, etc.
//! Defines some methods for board
//! Implements FEN utility that allows to convert input FEN string into attributes of Board

use scanner_rust::ScannerStr;

use crate::piece::Piece;
use crate::piece::Colour;
use crate::piece::Colour::{BLACK, WHITE};

pub const START_POSITION: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
pub const UPPER_RANK_LOWEST_TILE: u8 = 56;
pub const LOWER_RANK_HIGHEST_TILE: u8 = 7;

pub struct Board {
    pub main_bitboard: u64,
    pub empty_bitboard: u64,
    pub colour_bitboards: [u64; 2], // W B
    pub piece_bitboards: [u64; 12], // white and black separately, kpqrbn wb
    pub active_player: Colour,
    pub inactive_player: Colour,
    pub castling_rights: [bool; 4], // White: KQ, Black: kq
    pub en_passant_possibility: u32, // Tile, where en passant can be made. 64 if no such tile exists
    pub half_moves: u32, // The halfmove clock specifies a decimal number of half moves with respect to the 50 move draw rule.
    // It is reset to zero after a capture or a pawn move and incremented otherwise.
    pub full_moves: u32
}

impl Board {
    pub fn new() -> Self {
        Board {
            main_bitboard: 0,
            empty_bitboard: u64::MAX,
            colour_bitboards: [0; 2],
            piece_bitboards: [0; 12],
            active_player: WHITE,
            inactive_player: BLACK,
            castling_rights: [false; 4],
            en_passant_possibility: 64,
            half_moves: 0,
            full_moves: 1
        }
    }

    pub fn put_piece(&mut self, piece: Piece, colour: Colour, tile: u32) {
        let bit = 1 << tile;
        self.main_bitboard += bit;
        self.empty_bitboard -= bit;
        self.colour_bitboards[colour as usize] += bit;
        let index = piece as usize + 6 * colour as usize;
        self.piece_bitboards[index] += bit;
    }

    pub fn print_board(&self) {
        println!("{}", self.main_bitboard);
        println!("{:?}", self.colour_bitboards);
        println!("{:?}", self.piece_bitboards);
        println!("{:?}", self.active_player);
        println!("{:?}", self.castling_rights);
        println!("{}", self.en_passant_possibility);
        println!("{}", self.half_moves);
        println!("{}", self.full_moves);
    }
}

/// Read in the FEN (Forsyth-Edwards Notation) and adjust the board's attributes accordingly
/// parameters:
///     board - Board object we are considering
///     fen - FEN string holding board position
pub fn read_fen(board: &mut Board, fen: &str) {
    let fen = if fen == "" {START_POSITION} else {fen};
    println!("Received fen: {}", fen);

    let mut scanner = ScannerStr::new(fen);

    let pieces = scanner.next().unwrap_or_default().unwrap_or_default().split('/');

    let mut tile = 63;
    for rank in pieces {
        for mut char in rank.chars() {
            let colour = if char.is_lowercase() {BLACK} else {WHITE};
            char.make_ascii_lowercase();
            match char {
                'k' => board.put_piece(Piece::KING, colour, tile),
                'p' => board.put_piece(Piece::PAWN, colour, tile),
                'q' => board.put_piece(Piece::QUEEN, colour, tile),
                'r' => board.put_piece(Piece::ROOK, colour, tile),
                'b' => board.put_piece(Piece::BISHOP, colour, tile),
                'n' => board.put_piece(Piece::KNIGHT, colour, tile),
                _ => { if char.is_ascii_digit() {
                    let digit = char.to_digit(10).unwrap_or_default();
                    tile -= digit - 1 } }
            }

            if tile > 0 {
                tile -= 1;
            }
        }
    }

    let player = scanner.next().unwrap_or_default().unwrap_or_default();
    if player == "b" {
        board.active_player = BLACK;
        board.inactive_player = WHITE;
    }
    else {
        board.active_player = WHITE;
        board.inactive_player = BLACK;
    }

    let castling = scanner.next().unwrap_or_default().unwrap_or_default();
    board.castling_rights = [false; 4];
    for char in castling.chars() {
        match char {
            'K' => board.castling_rights[0] = true,
            'Q' => board.castling_rights[1] = true,
            'k' => board.castling_rights[2] = true,
            'q' => board.castling_rights[3] = true,
            _ => { break; }
        }
    }

    let en_passant = scanner.next().unwrap_or_default().unwrap_or_default();
    if en_passant == "-" {
        board.en_passant_possibility = 64;
    }
    else {
        // let tile = en_passant.parse::<u32>().unwrap_or_default();
        let mut chars = en_passant.chars();
        let file = chars.next().unwrap_or_default() as u32 - 'a' as u32;
        let rank = chars.next().unwrap_or_default().to_digit(10).unwrap_or_default() - 1;
        let tile = file + rank * 8;
        board.en_passant_possibility = tile;
    }

    let half_moves = scanner.next().unwrap_or_default().unwrap_or_default().parse().unwrap_or_default();
    board.half_moves = half_moves;

    let full_moves = scanner.next().unwrap_or_default().unwrap_or_default().parse().unwrap_or_default();
    board.full_moves = full_moves;

    board.print_board();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_position() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert_eq!(fen, START_POSITION);
        read_fen(&mut board, fen);
        assert_eq!(board.main_bitboard, 0b1111111111111111000000000000000000000000000000001111111111111111);
        assert_eq!(board.empty_bitboard, u64::MAX - board.main_bitboard);
        assert_eq!(board.colour_bitboards[0], 0b0000000000000000000000000000000000000000000000001111111111111111);
        assert_eq!(board.colour_bitboards[1], 0b1111111111111111000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[0], 0b0000000000000000000000000000000000000000000000000000000000001000);
        assert_eq!(board.piece_bitboards[1], 0b0000000000000000000000000000000000000000000000001111111100000000);
        assert_eq!(board.piece_bitboards[2], 0b0000000000000000000000000000000000000000000000000000000000010000);
        assert_eq!(board.piece_bitboards[3], 0b0000000000000000000000000000000000000000000000000000000010000001);
        assert_eq!(board.piece_bitboards[4], 0b0000000000000000000000000000000000000000000000000000000000100100);
        assert_eq!(board.piece_bitboards[5], 0b0000000000000000000000000000000000000000000000000000000001000010);
        assert_eq!(board.piece_bitboards[6], 0b0000100000000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[7], 0b0000000011111111000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[8], 0b0001000000000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[9], 0b1000000100000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[10], 0b0010010000000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[11], 0b0100001000000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.active_player, WHITE);
        assert_eq!(board.inactive_player, BLACK);
        assert_eq!(board.castling_rights, [true; 4]);
        assert_eq!(board.en_passant_possibility, 64);
        assert_eq!(board.half_moves, 0);
        assert_eq!(board.full_moves, 1);
    }

    #[test]
    fn no_castling() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - - 0 1";
        read_fen(&mut board, fen);

        assert_eq!(board.castling_rights, [false; 4]);
    }

    #[test]
    fn qw_kb_castling() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w Qk - 0 1";
        read_fen(&mut board, fen);

        assert_eq!(board.castling_rights, [false, true, true, false]);
    }

    #[test]
    fn check_en_passant() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w - f2 0 1";
        read_fen(&mut board, fen);

        assert_eq!(board.en_passant_possibility, 13);
    }

    #[test]
    fn position_2() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
        read_fen(&mut board, fen);
        assert_eq!(board.main_bitboard,       0b1111111111111111000000000000000000001000000000001111011111111111);
        assert_eq!(board.empty_bitboard, u64::MAX - board.main_bitboard);
        assert_eq!(board.colour_bitboards[0], 0b0000000000000000000000000000000000001000000000001111011111111111);
        assert_eq!(board.colour_bitboards[1], 0b1111111111111111000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[0], 0b0000000000000000000000000000000000000000000000000000000000001000);
        assert_eq!(board.piece_bitboards[1],  0b0000000000000000000000000000000000001000000000001111011100000000);
        assert_eq!(board.piece_bitboards[2], 0b0000000000000000000000000000000000000000000000000000000000010000);
        assert_eq!(board.piece_bitboards[3], 0b0000000000000000000000000000000000000000000000000000000010000001);
        assert_eq!(board.piece_bitboards[4], 0b0000000000000000000000000000000000000000000000000000000000100100);
        assert_eq!(board.piece_bitboards[5], 0b0000000000000000000000000000000000000000000000000000000001000010);
        assert_eq!(board.piece_bitboards[6], 0b0000100000000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[7], 0b0000000011111111000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[8], 0b0001000000000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[9], 0b1000000100000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[10], 0b0010010000000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[11], 0b0100001000000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.active_player, BLACK);
        assert_eq!(board.inactive_player, WHITE);
        assert_eq!(board.castling_rights, [true; 4]);
        assert_eq!(board.en_passant_possibility, 20);
        assert_eq!(board.half_moves, 0);
        assert_eq!(board.full_moves, 1);
    }

    #[test]
    fn position_3() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKB1R b K - 1 2";
        read_fen(&mut board, fen);
        assert_eq!(board.main_bitboard,       0b1111111111011111000000000010000000001000000001001111011111111101);
        assert_eq!(board.empty_bitboard, u64::MAX - board.main_bitboard);
        assert_eq!(board.colour_bitboards[0], 0b0000000000000000000000000000000000001000000001001111011111111101);
        assert_eq!(board.colour_bitboards[1], 0b1111111111011111000000000010000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[0],  0b0000000000000000000000000000000000000000000000000000000000001000);
        assert_eq!(board.piece_bitboards[1],  0b0000000000000000000000000000000000001000000000001111011100000000);
        assert_eq!(board.piece_bitboards[2],  0b0000000000000000000000000000000000000000000000000000000000010000);
        assert_eq!(board.piece_bitboards[3],  0b0000000000000000000000000000000000000000000000000000000010000001);
        assert_eq!(board.piece_bitboards[4],  0b0000000000000000000000000000000000000000000000000000000000100100);
        assert_eq!(board.piece_bitboards[5],  0b0000000000000000000000000000000000000000000001000000000001000000);
        assert_eq!(board.piece_bitboards[6],  0b0000100000000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[7],  0b0000000011011111000000000010000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[8],  0b0001000000000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[9],  0b1000000100000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[10], 0b0010010000000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.piece_bitboards[11], 0b0100001000000000000000000000000000000000000000000000000000000000);
        assert_eq!(board.active_player, BLACK);
        assert_eq!(board.inactive_player, WHITE);
        assert_eq!(board.castling_rights, [true, false, false, false]);
        assert_eq!(board.en_passant_possibility, 64);
        assert_eq!(board.half_moves, 1);
        assert_eq!(board.full_moves, 2);
    }
}