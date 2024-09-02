//! Transposition table is a database of encountered board posiitons
//! It holds a Zobrist-hashed position as a key,
//! information on depth it was searched for
//! and on the best move as determined by the search algorithm

use crate::board::Board;
use crate::move_generator::Move;
use crate::zobrist::zobrist_hash;

const INITIAL_CAPACITY: usize = 4096;

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
        TranspositionTable {
            table: Vec::<Transposition>::with_capacity(INITIAL_CAPACITY)
        }
    }
}