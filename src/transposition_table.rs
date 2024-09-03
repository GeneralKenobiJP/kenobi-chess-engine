//! Transposition table is a database of encountered board posiitons
//! It holds a Zobrist-hashed position as a key,
//! information on depth it was searched for
//! and on the best move as determined by the search algorithm

use crate::board::Board;
use crate::move_generator::Move;
use crate::zobrist::zobrist_hash;

const INITIAL_CAPACITY: usize = 1024 * 1024 * 64; // 67 108 864 entries => 1 610 612 736 Bytes; actual payload: 1 409 286 144 bytes
const HASH_SET_SEARCH_LIMIT: usize = 1024;

// 21 Bytes
// Option of Transposition: 24 Bytes
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transposition {
    zobrist: u64,
    depth: u8,
    best_moves: [Move; 3] // Best move, second-best, third-best
}

impl Transposition {
    pub fn new(board: &Board, depth: u8, best_move: &Move, second_move: &Move, third_move: &Move) -> Self {
        Transposition {
            zobrist: zobrist_hash(board),
            depth,
            best_moves: [best_move.clone(), second_move.clone(), third_move.clone()]
        }
    }

    pub fn from_zobrist(zobrist: u64, depth: u8, best_move: &Move, second_move: &Move, third_move: &Move) -> Self {
        Transposition {
            zobrist,
            depth,
            best_moves: [best_move.clone(), second_move.clone(), third_move.clone()]
        }
    }
}

pub struct TranspositionTable {
    table: Vec<Option<Transposition>>
}

impl TranspositionTable {

    pub fn new() -> Self {
        TranspositionTable {
            table: vec![None; INITIAL_CAPACITY]
        }
    }

    fn hash(&self, key: u64) -> usize {
        let mut hash = key as usize % INITIAL_CAPACITY;

        while hash != (hash + HASH_SET_SEARCH_LIMIT) % INITIAL_CAPACITY {
            let entry = &self.table[hash];

            match entry {
                None => { return hash; },
                Some(transposition) => {
                    if transposition.zobrist == key {
                        return hash;
                    }
                    hash = ( hash + 1 ) % INITIAL_CAPACITY;
                }
            }
        }

        hash
    }

    pub fn put_position(&mut self, board: &Board, depth: u8, best_move: &Move, second_move: &Move, third_move: &Move) {
        let zobrist = zobrist_hash(board);
        let hash = self.hash(zobrist);
        self.table[hash] = Option::from(Transposition::from_zobrist(zobrist, depth, best_move, second_move, third_move));
    }

    pub fn put_transposition(&mut self, transposition: &Transposition) {
        let hash = self.hash(transposition.zobrist);
        self.table[hash] = Option::from(transposition.clone());
    }

    pub fn get_from_position(&self, board: &Board) -> &Option<Transposition> {
        &self.table[self.hash(zobrist_hash(board))]
    }

    pub fn get_from_zobrist(&self, zobrist: u64) -> &Option<Transposition> {
        &self.table[self.hash(zobrist)]
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::board::START_POSITION;
    use crate::piece::Piece::PAWN;

    use super::*;

    #[test]
    fn test_transposition_table_initialization() {
        let transposition_table = TranspositionTable::new();
        assert_eq!(INITIAL_CAPACITY, transposition_table.table.len());
        assert_eq!(None, transposition_table.table[0]);
    }

    #[test]
    fn test_linear_probing() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut transposition_table = TranspositionTable::new();
        let zobrist = zobrist_hash(&board);
        let start = Instant::now();
        let hash = transposition_table.hash(zobrist);
        assert_eq!(zobrist as usize % INITIAL_CAPACITY, hash);
        let duration = start.elapsed();
        println!("hash lasted for: {:?}", duration);
        transposition_table.table[hash] = Option::from(Transposition::from_zobrist(0, 0, &Move::new(0, 0, 0, PAWN),
                                                                                   &Move::new(0,0,0,PAWN), &Move::new(0,0,0,PAWN)));

        let start = Instant::now();
        let hash = transposition_table.hash(zobrist);
        assert_eq!((zobrist as usize + 1) % INITIAL_CAPACITY, hash);
        let duration = start.elapsed();
        println!("hash lasted for: {:?}", duration);
        transposition_table.table[hash] = Option::from(Transposition::from_zobrist(zobrist, 0, &Move::new(0, 0, 0, PAWN),
                                                                                   &Move::new(0,0,0,PAWN), &Move::new(0,0,0,PAWN)));

        let start = Instant::now();
        let hash = transposition_table.hash(zobrist);
        assert_eq!((zobrist as usize + 1) % INITIAL_CAPACITY, hash);
        let duration = start.elapsed();
        println!("hash lasted for: {:?}", duration);
    }

    #[test]
    fn check_put_position() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut transposition_table = TranspositionTable::new();

        transposition_table.put_position(&board, 4, &Move::empty(), &Move::empty(), &Move::empty());
        let expected = Option::from(Transposition::new(&board, 4, &Move::empty(), &Move::empty(), &Move::empty()));
        assert_eq!(expected, transposition_table.table[zobrist_hash(&board) as usize % INITIAL_CAPACITY]);

        transposition_table.put_position(&board, 6, &Move::empty(), &Move::empty(), &Move::empty());
        let expected2 = Option::from(Transposition::new(&board, 6, &Move::empty(), &Move::empty(), &Move::empty()));
        assert_eq!(expected2, transposition_table.table[zobrist_hash(&board) as usize % INITIAL_CAPACITY]);

        board = Board::new();
        board.read_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        transposition_table.put_position(&board, 4, &Move::empty(), &Move::empty(), &Move::empty());
        let expected3 = Option::from(Transposition::new(&board, 4, &Move::empty(), &Move::empty(), &Move::empty()));
        assert_eq!(expected3, transposition_table.table[zobrist_hash(&board) as usize % INITIAL_CAPACITY]);
        assert_ne!(expected, expected3);
    }

    #[test]
    fn check_put_transposition() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut transposition_table = TranspositionTable::new();

        let transposition = Transposition::new(&board, 4, &Move::empty(), &Move::empty(), &Move::empty());
        transposition_table.put_transposition(&transposition);
        let expected = Option::from(transposition);
        assert_eq!(expected, transposition_table.table[zobrist_hash(&board) as usize % INITIAL_CAPACITY]);

        let transposition2 = Transposition::new(&board, 6, &Move::empty(), &Move::empty(), &Move::empty());
        transposition_table.put_transposition(&transposition2);
        let expected2 = Option::from(Transposition::new(&board, 6, &Move::empty(), &Move::empty(), &Move::empty()));
        assert_eq!(expected2, transposition_table.table[zobrist_hash(&board) as usize % INITIAL_CAPACITY]);

        board = Board::new();
        board.read_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        let transposition3 = Transposition::new(&board, 4, &Move::empty(), &Move::empty(), &Move::empty());
        transposition_table.put_transposition(&transposition3);
        let expected3 = Option::from(transposition3);
        assert_eq!(expected3, transposition_table.table[zobrist_hash(&board) as usize % INITIAL_CAPACITY]);
        assert_ne!(expected, expected3);
    }

    #[test]
    fn check_get_from_position() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut transposition_table = TranspositionTable::new();

        transposition_table.put_position(&board, 4, &Move::empty(), &Move::empty(), &Move::empty());
        let expected = Option::from(Transposition::new(&board, 4, &Move::empty(), &Move::empty(), &Move::empty()));
        assert_eq!(expected, *transposition_table.get_from_position(&board));

        transposition_table.put_position(&board, 6, &Move::empty(), &Move::empty(), &Move::empty());
        let expected2 = Option::from(Transposition::new(&board, 6, &Move::empty(), &Move::empty(), &Move::empty()));
        assert_eq!(expected2, *transposition_table.get_from_position(&board));

        board = Board::new();
        board.read_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        transposition_table.put_position(&board, 4, &Move::empty(), &Move::empty(), &Move::empty());
        let expected3 = Option::from(Transposition::new(&board, 4, &Move::empty(), &Move::empty(), &Move::empty()));
        assert_eq!(expected3, *transposition_table.get_from_position(&board));
        assert_ne!(expected, expected3);
    }

    #[test]
    fn check_get_from_zobrist() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut transposition_table = TranspositionTable::new();

        let transposition = Transposition::new(&board, 4, &Move::empty(), &Move::empty(), &Move::empty());
        transposition_table.put_transposition(&transposition);
        let expected = Option::from(transposition.clone());
        assert_eq!(expected, *transposition_table.get_from_zobrist(transposition.zobrist));

        let transposition2 = Transposition::new(&board, 6, &Move::empty(), &Move::empty(), &Move::empty());
        transposition_table.put_transposition(&transposition2.clone());
        let expected2 = Option::from(Transposition::new(&board, 6, &Move::empty(), &Move::empty(), &Move::empty()));
        assert_eq!(expected2, *transposition_table.get_from_zobrist(transposition2.zobrist));

        board = Board::new();
        board.read_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        let transposition3 = Transposition::new(&board, 4, &Move::empty(), &Move::empty(), &Move::empty());
        transposition_table.put_transposition(&transposition3.clone());
        let expected3 = Option::from(transposition3.clone());
        assert_eq!(expected3, *transposition_table.get_from_zobrist(transposition3.zobrist));
        assert_ne!(expected, expected3);
    }
}