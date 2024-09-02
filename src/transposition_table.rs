//! Transposition table is a database of encountered board posiitons
//! It holds a Zobrist-hashed position as a key,
//! information on depth it was searched for
//! and on the best move as determined by the search algorithm

use crate::board::Board;
use crate::move_generator::Move;
use crate::zobrist::zobrist_hash;

const INITIAL_CAPACITY: usize = 1024 * 1024 * 64; // 67 108 864 entries => 872 415 232 bytes

// 13 Bytes
#[derive(Clone)]
pub struct Transposition {
    depth: u8,
    best_moves: [Move; 3] // Best move, second-best, third-best
}

impl Transposition {
    pub fn new(depth: u8, best_move: &Move, second_move: &Move, third_move: &Move) -> Self {
        Transposition {
            depth,
            best_moves: [best_move.clone(), second_move.clone(), third_move.clone()]
        }
    }
}

pub struct TranspositionTable {
    table: Vec<Transposition>
}

impl TranspositionTable {

    pub fn new() -> Self {
        let mut transposition_table = TranspositionTable {
            table: Vec::<Transposition>::with_capacity(INITIAL_CAPACITY)
        };
        unsafe { transposition_table.table.set_len(INITIAL_CAPACITY) };
        transposition_table
    }

    pub fn save_position(&mut self, board: &Board, depth: u8, best_move: &Move, second_move: &Move, third_move: &Move) {
        self.table[zobrist_hash(board) % INITIAL_CAPACITY] = Transposition::new(depth, best_move, second_move, third_move);
    }
}

#[cfg(test)]
mod tests {
    use crate::board::START_POSITION;
    use crate::piece::Piece::PAWN;

    use super::*;

    #[test]
    fn check_save_position() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut transposition_table = TranspositionTable::new();
        transposition_table.save_position(&board, 2, &Move {origin: 0, target: 1, promotion: 0, piece: PAWN},
                                          &Move {origin: 0, target: 1, promotion: 0, piece: PAWN}, &Move {origin: 0, target: 1, promotion: 0, piece: PAWN});
    }
}