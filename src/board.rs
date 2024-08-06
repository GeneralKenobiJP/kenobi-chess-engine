use std::ops::Add;
use scanner_rust::ScannerStr;

use crate::piece::Piece;
use crate::piece::Colour;

const START_POSITION: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0";

pub struct Board {
    main_bitboard: u64,
    colour_bitboards: [u64; 2], // W B
    piece_bitboards: [u64; 12] // white and black separately, kpqrbn wb
}

impl Board {
    pub fn new() -> Board {
        Board {
            main_bitboard: 0,
            colour_bitboards: [0;2],
            piece_bitboards: [0;12]
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
    }
}

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
                    let mut digit = char.to_digit(10).unwrap_or_default();
                    tile -= digit - 1 } }
            }

            tile -= 1;
        }
    }

    board.print_board();
}