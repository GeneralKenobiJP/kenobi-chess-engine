use scanner_rust::ScannerStr;

use crate::piece::Piece;
use crate::piece::Colour;
use crate::piece::Colour::{BLACK, WHITE};

const START_POSITION: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

pub struct Board {
    main_bitboard: u64,
    colour_bitboards: [u64; 2], // W B
    piece_bitboards: [u64; 12], // white and black separately, kpqrbn wb
    active_player: Colour,
    castling_rights: [bool; 4], // White: KQ, Black: kq
    en_passant_possibility: u32, // Tile, where en passant can be made. 64 if no such tile exists
    half_moves: u32, // The halfmove clock specifies a decimal number of half moves with respect to the 50 move draw rule.
    // It is reset to zero after a capture or a pawn move and incremented otherwise.
    full_moves: u32
}

impl Board {
    pub fn new() -> Board {
        Board {
            main_bitboard: 0,
            colour_bitboards: [0; 2],
            piece_bitboards: [0; 12],
            active_player: WHITE,
            castling_rights: [false; 4],
            en_passant_possibility: 64,
            half_moves: 0,
            full_moves: 1
        }
    }

    pub fn put_piece(&mut self, piece: Piece, colour: Colour, tile: u32) {
        let bit = 1 << tile;
        self.main_bitboard += bit;
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
/// ```rust
/// let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
/// let mut board = Board::new();
/// read_fen(&mut board, fen);
/// assert_eq!(board.main_bitboard,
/// ```
pub fn read_fen(board: &mut Board, fen: &str) {
    let fen = if fen == "" {START_POSITION} else {fen};
    println!("Received fen: {}", fen);

    let mut scanner = ScannerStr::new(fen);

    let pieces = scanner.next().unwrap_or_default().unwrap_or_default().split('/');

    let mut tile = 63;
    for rank in pieces {
        for mut char in rank.chars() {
            let colour = if char.is_lowercase() {Colour::BLACK} else {Colour::WHITE};
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
    }
    else {
        board.active_player = WHITE;
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
        let tile = en_passant.parse::<u32>().unwrap_or_default();
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
        assert_eq!(board.colour_bitboards[0], 0b0000000000000000000000000000000000000000000000001111111111111111);
        assert_eq!(board.colour_bitboards[1], 0b1111111111111111000000000000000000000000000000000000000000000000);
    }
}