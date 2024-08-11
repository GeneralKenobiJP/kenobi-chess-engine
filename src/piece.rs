//! Enums that define pieces and players
//! Piece (piece type)
//! Colour (piece colour / player colour)

use crate::piece::Colour::WHITE;
#[derive(PartialEq, Eq, Clone, Hash)]
pub enum Piece {
    KING = 0,
    PAWN = 1,
    QUEEN = 2,
    ROOK = 3,
    BISHOP = 4,
    KNIGHT = 5
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Colour {
    WHITE = 0,
    BLACK = 1
}