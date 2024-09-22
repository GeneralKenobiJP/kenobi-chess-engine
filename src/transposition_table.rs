//! Transposition table is a database of encountered board posiitons
//! It holds a Zobrist-hashed position as a key,
//! information on depth it was searched for
//! and on the best move as determined by the search algorithm

use crate::board::Board;
use crate::move_generator::Move;
use crate::zobrist::zobrist_hash;

const INITIAL_CAPACITY: usize = 1024 * 1024 * 64; // 67 108 864 entries => 2 147 483 648 Bytes
const HASH_SET_SEARCH_LIMIT: usize = 1024;
const REPETITION_CAPACITY: usize = 1024 * 2; // 2048 entries => 65 536 Bytes
const THREEFOLD_REPETITION: u8 = 3;

// 24 Bytes => 24 Bytes (must be a multiple of 8)
// Option of Transposition: 24 Bytes
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transposition {
    pub zobrist: u64,
    pub depth: u32,
    pub value: i32,
    pub best_moves: [Option<Move>; 3], // Best move, second-best, third-best
    pub node_type: NodeType
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum NodeType {
    EXACT = 0, // We know the EXACT value of the node
    ALPHA = 1, // We know the UPPER-bound value of the node - it was cutoff as alpha due to it being certainly worse than another move
    BETA = 2   // We know the LOWER-bound value of the node - it was cutoff as beta due to it being possible to be stopped from happening in the previous move
}

impl Transposition {
    pub fn from_position(board: &Board, depth: u32, value: i32, best_moves: &[Option<Move>; 3], node_type: NodeType) -> Self {
        Transposition {
            zobrist: zobrist_hash(board),
            depth,
            value,
            best_moves: best_moves.clone(),
            node_type
        }
    }

    pub fn from_zobrist(zobrist: u64, depth: u32, value: i32, best_moves: &[Option<Move>; 3], node_type: NodeType) -> Self {
        Transposition {
            zobrist,
            depth,
            value,
            best_moves: best_moves.clone(),
            node_type
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct TranspositionTable {
    table: Vec<Option<Transposition>>
}

impl TranspositionTable {

    pub fn new() -> Self {
        TranspositionTable {
            table: vec![None; INITIAL_CAPACITY]
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        TranspositionTable {
            table: vec![None; capacity]
        }
    }

    /// Hashes the key by implementing linear probing
    /// Should be called for a zobrist-hashed key
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

    /// Put a position in a transposition table, given a board situation, depth, best move evaluation,
    /// three best moves for the given board and depth, and the node type based on the alpha-beta cutoff decision
    /// (exact / alpha / beta)
    pub fn put_position(&mut self, board: &Board, depth: u32, value: i32, best_moves: &[Option<Move>; 3], node_type: NodeType) {
        let zobrist = zobrist_hash(board);
        let hash = self.hash(zobrist);
        self.table[hash] = Option::from(Transposition::from_zobrist(zobrist, depth, value, best_moves, node_type));
    }

    /// Put a transposition in a transposition table, given a transposition object
    pub fn put_transposition(&mut self, transposition: &Transposition) {
        let hash = self.hash(transposition.zobrist);
        self.table[hash] = Option::from(transposition.clone());
    }

    /// Fetches a position from a transposition table, given a board situation
    pub fn get_from_position(&self, board: &Board) -> &Option<Transposition> {
        &self.table[self.hash(zobrist_hash(board))]
    }

    /// Fetches a position from a transposition table, given a zobrist hash of a board position
    pub fn get_from_zobrist(&self, zobrist: u64) -> &Option<Transposition> {
        &self.table[self.hash(zobrist)]
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct RepetitionTable {
    table: Vec<u8>,
    keys: Vec<Option<u64>>
}

impl RepetitionTable {
    pub fn new() -> Self {
        RepetitionTable {
            table: vec![0; REPETITION_CAPACITY],
            keys: vec![None; REPETITION_CAPACITY]
        }
    }

    /// Hashes the key by implementing linear probing
    /// Should be called for a zobrist-hashed key
    fn hash(&self, key: u64) -> usize {
        let mut hash = key as usize % REPETITION_CAPACITY;

        while hash != (hash + REPETITION_CAPACITY - 1) % REPETITION_CAPACITY {
            let entry = self.keys[hash];

            match entry {
                None => { return hash; }
                Some(repetition) => {
                    if repetition == key { return hash; }
                    hash = (hash + 1) % REPETITION_CAPACITY;
                }
            }
        }

        hash
    }

    /// Increase the repetition counter for the position with a given zobrist key.
    /// Returns true if the threefold repetition occurred, otherwise returns false.
    pub fn visit_position(&mut self, zobrist: u64) -> bool {
        let hash = self.hash(zobrist);
        self.table[hash] += 1;
        self.keys[hash] = Option::from(zobrist);

        self.table[hash] >= THREEFOLD_REPETITION
    }

    /// Decrease the repetition counter for the position with a given zobrist key.
    /// Do NOT use for unvisited positions
    pub fn unvisit_position(&mut self, zobrist: u64) {
        let hash = self.hash(zobrist);
        self.table[hash] -= 1;

        if self.table[hash] == 0 {
            self.keys[hash] = None;
        }
    }

    /// Checks if the repetition table is, i.e. if it stores any visit
    pub fn is_empty(&self) -> bool {
        for i in 0..REPETITION_CAPACITY {
            if self.table[i] != 0 || self.keys[i] != None { return false }
        }

        true
    }

    /// Returns the number of visits that occurred for a position with a given zobrist
    pub fn get_repetition(&self, zobrist: u64) -> u8 {
        self.table[self.hash(zobrist)]
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::board::START_POSITION;
    use crate::transposition_table::NodeType::EXACT;
    use super::*;

    // Use for type size checking, not as an actual test
    // #[test]
    // fn check_type_size() {
    //     println!("{}", size_of::<NodeType>());
    //     println!("{}", size_of::<Transposition>());
    //     println!("{}", size_of::<Option<Transposition>>());
    // }

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
        transposition_table.table[hash] = Option::from(Transposition::from_zobrist(zobrist, 0, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT));

        let start = Instant::now();
        let hash = transposition_table.hash(zobrist + 1);
        assert_eq!((zobrist as usize + 1) % INITIAL_CAPACITY, hash);
        let duration = start.elapsed();
        println!("hash lasted for: {:?}", duration);
        transposition_table.table[hash] = Option::from(Transposition::from_zobrist(zobrist + 1, 0, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT));

        let start = Instant::now();
        let hash = transposition_table.hash(zobrist + 1);
        assert_eq!((zobrist as usize + 1) % INITIAL_CAPACITY, hash);
        let duration = start.elapsed();
        println!("hash lasted for: {:?}", duration);
    }

    #[test]
    fn check_put_position() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut transposition_table = TranspositionTable::new();

        transposition_table.put_position(&board, 4, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT);
        let expected = Option::from(Transposition::from_position(&board, 4, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT));
        assert_eq!(expected, transposition_table.table[zobrist_hash(&board) as usize % INITIAL_CAPACITY]);

        transposition_table.put_position(&board, 6, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT);
        let expected2 = Option::from(Transposition::from_position(&board, 6, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT));
        assert_eq!(expected2, transposition_table.table[zobrist_hash(&board) as usize % INITIAL_CAPACITY]);

        board = Board::new();
        board.read_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        transposition_table.put_position(&board, 4, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT);
        let expected3 = Option::from(Transposition::from_position(&board, 4, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT));
        assert_eq!(expected3, transposition_table.table[zobrist_hash(&board) as usize % INITIAL_CAPACITY]);
        assert_ne!(expected, expected3);
    }

    #[test]
    fn check_put_transposition() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut transposition_table = TranspositionTable::new();

        let transposition = Transposition::from_position(&board, 4, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT);
        transposition_table.put_transposition(&transposition);
        let expected = Option::from(transposition);
        assert_eq!(expected, transposition_table.table[zobrist_hash(&board) as usize % INITIAL_CAPACITY]);

        let transposition2 = Transposition::from_position(&board, 6, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT);
        transposition_table.put_transposition(&transposition2);
        let expected2 = Option::from(Transposition::from_position(&board, 6, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT));
        assert_eq!(expected2, transposition_table.table[zobrist_hash(&board) as usize % INITIAL_CAPACITY]);

        board = Board::new();
        board.read_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        let transposition3 = Transposition::from_position(&board, 4, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT);
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

        transposition_table.put_position(&board, 4, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT);
        let expected = Option::from(Transposition::from_position(&board, 4, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT));
        assert_eq!(expected, *transposition_table.get_from_position(&board));

        transposition_table.put_position(&board, 6, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT);
        let expected2 = Option::from(Transposition::from_position(&board, 6, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT));
        assert_eq!(expected2, *transposition_table.get_from_position(&board));

        board = Board::new();
        board.read_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        transposition_table.put_position(&board, 4, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT);
        let expected3 = Option::from(Transposition::from_position(&board, 4, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT));
        assert_eq!(expected3, *transposition_table.get_from_position(&board));
        assert_ne!(expected, expected3);
    }

    #[test]
    fn check_get_from_zobrist() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut transposition_table = TranspositionTable::new();

        let transposition = Transposition::from_position(&board, 4, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT);
        transposition_table.put_transposition(&transposition);
        let expected = Option::from(transposition.clone());
        assert_eq!(expected, *transposition_table.get_from_zobrist(transposition.zobrist));

        let transposition2 = Transposition::from_position(&board, 6, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT);
        transposition_table.put_transposition(&transposition2.clone());
        let expected2 = Option::from(Transposition::from_position(&board, 6, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT));
        assert_eq!(expected2, *transposition_table.get_from_zobrist(transposition2.zobrist));

        board = Board::new();
        board.read_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1");
        let transposition3 = Transposition::from_position(&board, 4, 100,&[Option::from(Move::empty()), Option::from(Move::empty()), Option::from(Move::empty())], EXACT);
        transposition_table.put_transposition(&transposition3.clone());
        let expected3 = Option::from(transposition3.clone());
        assert_eq!(expected3, *transposition_table.get_from_zobrist(transposition3.zobrist));
        assert_ne!(expected, expected3);
    }

    #[test]
    fn test_linear_probing_repetition() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut repetition_table = RepetitionTable::new();
        let zobrist = zobrist_hash(&board);
        let start = Instant::now();
        let hash = repetition_table.hash(zobrist);
        assert_eq!(zobrist as usize % REPETITION_CAPACITY, hash);
        let duration = start.elapsed();
        println!("hash lasted for: {:?}", duration);
        repetition_table.keys[hash] = Option::from(zobrist);

        let start = Instant::now();
        let hash = repetition_table.hash(zobrist + 1);
        assert_eq!((zobrist as usize + 1) % REPETITION_CAPACITY, hash);
        let duration = start.elapsed();
        println!("hash lasted for: {:?}", duration);
        repetition_table.keys[hash] = Option::from(zobrist + 1);

        let start = Instant::now();
        let hash = repetition_table.hash(zobrist + 1);
        assert_eq!((zobrist as usize + 1) % REPETITION_CAPACITY, hash);
        let duration = start.elapsed();
        println!("hash lasted for: {:?}", duration);
    }

    #[test]
    fn check_visit_unvisit_position() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut repetition_table = RepetitionTable::new();

        for i in 0..REPETITION_CAPACITY {
            assert_eq!(0, repetition_table.table[i]);
            assert_eq!(None, repetition_table.keys[i]);
        }

        assert!(!repetition_table.visit_position(board.zobrist));
        assert_eq!(1, repetition_table.table[repetition_table.hash(board.zobrist)]);
        assert_eq!(board.zobrist, repetition_table.keys[repetition_table.hash(board.zobrist)].unwrap_or_default());

        assert!(!repetition_table.visit_position(board.zobrist));
        assert_eq!(2, repetition_table.table[repetition_table.hash(board.zobrist)]);
        assert_eq!(board.zobrist, repetition_table.keys[repetition_table.hash(board.zobrist)].unwrap_or_default());

        assert!(repetition_table.visit_position(board.zobrist));
        assert_eq!(3, repetition_table.table[repetition_table.hash(board.zobrist)]);
        assert_eq!(board.zobrist, repetition_table.keys[repetition_table.hash(board.zobrist)].unwrap_or_default());

        repetition_table.unvisit_position(board.zobrist);
        assert_eq!(2, repetition_table.table[repetition_table.hash(board.zobrist)]);
        assert_eq!(board.zobrist, repetition_table.keys[repetition_table.hash(board.zobrist)].unwrap_or_default());

        let mut board = Board::new();
        board.read_fen("rnbqkbnr/pppppppp/8/8/8/7P/PPPPPPP1/RNBQKBNR w KQkq - 0 1");

        assert!(!repetition_table.visit_position(board.zobrist));
        assert_eq!(1, repetition_table.table[repetition_table.hash(board.zobrist)]);
        assert_eq!(board.zobrist, repetition_table.keys[repetition_table.hash(board.zobrist)].unwrap_or_default());

        repetition_table.unvisit_position(board.zobrist);
        assert_eq!(0, repetition_table.table[repetition_table.hash(board.zobrist)]);
        assert_eq!(None, repetition_table.keys[repetition_table.hash(board.zobrist)]);
    }
}