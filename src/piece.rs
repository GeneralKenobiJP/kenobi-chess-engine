pub enum Piece {
    KING,
    PAWN,
    QUEEN,
    ROOK,
    BISHOP,
    KNIGHT
}

#[derive(Clone, Copy, Debug)]
pub enum Colour {
    WHITE,
    BLACK
}

impl PartialEq for Colour {
    fn eq(&self, other: &Self) -> bool {
        return other == self;
    }
}