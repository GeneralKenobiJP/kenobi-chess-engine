//! Enums that define pieces and players
//! Piece (piece type)
//! Colour (piece colour / player colour)
//!

use crate::piece::Colour::{BLACK, WHITE};
use num_derive::FromPrimitive;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug, FromPrimitive)]
pub enum Piece {
    KING = 0,
    PAWN = 1,
    QUEEN = 2,
    ROOK = 3,
    BISHOP = 4,
    KNIGHT = 5
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    WHITE = 0,
    BLACK = 1
}

impl Colour {
    pub fn switch_player(&self) -> Colour {
        if *self == WHITE { return BLACK }
        WHITE
    }
}