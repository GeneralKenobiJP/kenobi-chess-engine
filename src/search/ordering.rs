//! Move ordering before the search tree traversal.
//! Implements move ordering heuristics to sort the moves
//! in such way that the likely best moves are evaluated as soon as possible,
//! to maximize the number of branches prunned in the alpha-beta prunning,
//! thus making the search algorithm faster

use std::collections::HashMap;
use crate::evaluation::Evaluator;
use crate::move_generator::{CAPTURE_SQUARE_MASK, Move, MoveList, NO_CAPTURE};
use crate::search::Engine;
use crate::transposition_table::NodeType::EXACT;

type OrderTable = HashMap<Move, i32>;

const HASH_MOVE_PRIORITY: [i32; 3] = [512, 256, 128];
const PV_NODE_PRIORITY: i32 = 192;
const AGGRESSOR_PRIORITY: [i32; 6] = [0, 32, 4, 8, 16, 16];
const VICTIM_PRIORITY: [i32; 6] = [4096, 16, 512, 128, 64, 64];
const RECAPTURE_PRIORITY: i32 = 256;
const KILLER_MOVES_PRIORITY: [i32; 2] = [256, 224];

impl<T: Evaluator> Engine<T> {
    /// Orders moves in-place using order heuristics, given a move list.
    pub fn order_moves(&self, move_list: &mut MoveList) {
        let mut order_table = self.construct_order_table(move_list);

        self.apply_hash_move(&mut order_table, move_list);

        let recapture = move_list.get_last_capture();
        if recapture.is_none() || *recapture.unwrap() == NO_CAPTURE {
            self.apply_mvv_lva(&mut order_table, move_list);
        }
        else {
            self.apply_mvv_lva_with_recapture(&mut order_table, move_list, (*recapture.unwrap() & CAPTURE_SQUARE_MASK) as u8);
        }

        self.apply_killer_moves(&mut order_table);

        move_list.order_moves(&order_table);
    }

    /// Constructs an order table, given a MoveList.
    /// OrderTable is a hashmap of Move as a key, and i32 as a priority value.
    /// It is used for ordering the moves.
    /// The order table is initialized with all the moves inserted and given a priority of 0.
    fn construct_order_table(&self, move_list: &MoveList) -> OrderTable {
        let mut order_table = HashMap::<Move, i32>::with_capacity(move_list.get_moves().len());

        for piece_move in move_list.get_moves() {
            order_table.insert(*piece_move, 0);
        }

        order_table
    }

    /// Applies hash move heuristic for move ordering.
    /// Retrieves the entry of the current position from the transposition table,
    /// regardless of its depth.
    /// Increases priority according to the HASH_MOVE_PRIORITY,
    /// with best move getting the most, and the third best getting the least.
    /// If the node type is EXACT, the best move (PV-node) also gets
    /// a priority boost of PV_NODE_PRIORITY
    fn apply_hash_move(&self, order_table: &mut OrderTable, move_list: &MoveList) {
        match self.transposition_table.get_from_zobrist(move_list.get_board().zobrist) {
            None => { return; }
            Some(transposition) => {

                let mut index = 0;

                // PV-Node
                let mut pv_node = 0;
                if transposition.node_type == EXACT {
                    pv_node = PV_NODE_PRIORITY;
                }

                for entry in transposition.best_moves {
                    match entry {
                        None => { return; }
                        Some(piece_move) => {
                            Self::update_move_priority(order_table, &piece_move, HASH_MOVE_PRIORITY[index] + pv_node);

                            pv_node = 0;
                            index += 1;
                        }
                    }
                }
            }
        }
    }

    /// Updates move priority value in the order table, given a move.
    /// It adds update priority to current priority.
    fn update_move_priority(order_table: &mut OrderTable, piece_move: &Move, added_priority: i32) {
        let order_table_entry = order_table.get(&piece_move);

        // The order table entry MAY BE NONE
        // in the quiescence search phase if we fetch a quiet move or a zobrist collision happens.
        // In such a case, it should simply be ignored.
        if order_table_entry.is_none() {
            return;
        }
        let old_value = order_table_entry.unwrap();
        order_table.insert(*piece_move, old_value + added_priority);
    }

    /// Applies Most-Valuable-Victim-Least-Valuable-Attacker heuristic in situation, where a recapture is possible.
    /// The heuristic states that, whenever possible, capturing a valuable piece (e.g. a queen)
    /// with a less valuable piece (e.g. a pawn) tends to be a good move.
    /// Iterates through all the possible moves to apply the heuristic.
    /// Takes in recapture square as an input.
    /// If the move is a recapture, it applies special bonus, according to the heuristic
    /// that recaptures tend to be good moves.
    fn apply_mvv_lva_with_recapture(&self, order_table: &mut OrderTable, move_list: &MoveList, recapture: u8) {
        let board = move_list.get_board();
        for piece_move in move_list.get_moves() {
            // Efficient check for whether the target square is empty or not
            if board.colour_bitboards[board.inactive_player as usize] & (1 << piece_move.target) == 0 {
                continue;
            }

            let target_piece = board.get_piece_from_square_by_player(piece_move.target, board.inactive_player as usize).unwrap() as usize;
            let mut priority = VICTIM_PRIORITY[target_piece] + AGGRESSOR_PRIORITY[piece_move.piece as usize];

            // Check for whether this is a recapture
            if piece_move.target == recapture {
                priority += RECAPTURE_PRIORITY;
            }

            Self::update_move_priority(order_table, &piece_move, priority);
        }
    }

    /// Applies Most-Valuable-Victim-Least-Valuable-Attacker heuristic.
    /// If a recapture is possible, another version of the function should be used.
    /// The heuristic states that, whenever possible, capturing a valuable piece (e.g. a queen)
    /// with a less valuable piece (e.g. a pawn) tends to be a good move.
    /// Iterates through all the possible moves to apply the heuristic.
    fn apply_mvv_lva(&self, order_table: &mut OrderTable, move_list: &MoveList) {
        let board = move_list.get_board();
        for piece_move in move_list.get_moves() {
            // Efficient check for whether the target square is empty or not
            if board.colour_bitboards[board.inactive_player as usize] & (1 << piece_move.target) == 0 {
                continue;
            }

            let target_piece = board.get_piece_from_square_by_player(piece_move.target, board.inactive_player as usize).unwrap() as usize;
            let priority = VICTIM_PRIORITY[target_piece] + AGGRESSOR_PRIORITY[piece_move.piece as usize];

            Self::update_move_priority(order_table, &piece_move, priority);
        }
    }

    fn apply_killer_moves(&self, order_table: &mut OrderTable) {
        for i in 0..2 {
            if self.killer_moves[self.depth][i].is_none() {
                return;
            }
            if !order_table.contains_key(&self.killer_moves[self.depth][i]) {
                return;
            }
            Self::update_move_priority(&order_table, &self.killer_moves[self.depth][i], KILLER_MOVES_PRIORITY[i]);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::board::{Board, START_POSITION};
    use crate::move_generator::{Move, MoveList};
    use crate::piece::Piece::{BISHOP, KNIGHT, PAWN, QUEEN};
    use crate::search::Engine;
    use crate::transposition_table::NodeType::{BETA, EXACT};
    use super::*;

    #[test]
    fn test_construct_order_table() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let move_list = MoveList::from_board(board);
        let engine = Engine::with_capacity(256);

        let time = Instant::now();
        let order_table = engine.construct_order_table(&move_list);
        let duration = time.elapsed();
        println!("construct_order_table lasted for: {:?}", duration);

        for piece_move in move_list.get_moves() {
            assert_eq!(0, *order_table.get(piece_move).unwrap());
        }
        assert_eq!(move_list.get_moves().len(), order_table.keys().len());
    }

    #[test]
    fn test_order_moves() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut move_list = MoveList::from_board(board);
        move_list.generate_moves();
        let mut engine = Engine::with_capacity(256);
        engine.transposition_table.put_position(move_list.get_board(), 5, 200,
                                         &[Option::from(Move::new(1, 18, 0, KNIGHT)), Option::from(Move::new(12, 28, 0, PAWN)), None],
                                         EXACT);

        let time = Instant::now();
        engine.order_moves(&mut move_list);
        let duration = time.elapsed();
        println!("order_moves lasted for: {:?}", duration);
        assert_eq!(Move::new(1, 18, 0, KNIGHT), move_list.get_moves()[0]);
        assert_eq!(Move::new(12, 28, 0, PAWN), move_list.get_moves()[1]);
    }

    #[test]
    fn check_apply_hash_move_pv_node_exact() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut move_list = MoveList::from_board(board);
        move_list.generate_moves();
        let mut engine = Engine::with_capacity(256);
        engine.transposition_table.put_position(move_list.get_board(), 3, 200,
        &[Option::from(Move::new(1, 18, 0, KNIGHT)),
            Option::from(Move::new(11, 27, 0, PAWN)),
            Option::from(Move::new(8, 16, 0, PAWN))], EXACT);
        let mut order_table = engine.construct_order_table(&move_list);

        let time = Instant::now();
        engine.apply_hash_move(&mut order_table, &move_list);
        let duration = time.elapsed();
        println!("apply_hash_move lasted for: {:?}", duration);

        assert_eq!(HASH_MOVE_PRIORITY[0] + PV_NODE_PRIORITY, *order_table.get(&Move::new(1, 18, 0, KNIGHT)).unwrap());
        assert_eq!(HASH_MOVE_PRIORITY[1], *order_table.get(&Move::new(11, 27, 0, PAWN)).unwrap());
        assert_eq!(HASH_MOVE_PRIORITY[2], *order_table.get(&Move::new(8, 16, 0, PAWN)).unwrap());
    }

    #[test]
    fn check_apply_hash_move_not_exact() {
        let mut board = Board::new();
        board.read_fen(START_POSITION);

        let mut move_list = MoveList::from_board(board);
        move_list.generate_moves();
        let mut engine = Engine::with_capacity(256);
        engine.transposition_table.put_position(move_list.get_board(), 3, 200,
        &[Option::from(Move::new(1, 18, 0, KNIGHT)),
            Option::from(Move::new(11, 27, 0, PAWN)),
            None], BETA);
        let mut order_table = engine.construct_order_table(&move_list);

        let time = Instant::now();
        engine.apply_hash_move(&mut order_table, &move_list);
        let duration = time.elapsed();
        println!("apply_hash_move lasted for: {:?}", duration);

        assert_eq!(HASH_MOVE_PRIORITY[0], *order_table.get(&Move::new(1, 18, 0, KNIGHT)).unwrap());
        assert_eq!(HASH_MOVE_PRIORITY[1], *order_table.get(&Move::new(11, 27, 0, PAWN)).unwrap());
    }

    #[test]
    fn check_apply_mvv_lva() {
        let board = Board::from_fen(&"r1bqkbnr/ppp2ppp/8/3p1n2/2BpP3/6N1/PPP2PPP/RNBQK2R w KQkq - 0 1");

        let mut move_list = MoveList::from_board(board);
        move_list.generate_moves();
        let mut engine = Engine::with_capacity(256);
        let mut order_table = engine.construct_order_table(&move_list);

        let time = Instant::now();
        engine.apply_mvv_lva(&mut order_table, &move_list);
        let duration = time.elapsed();
        println!("apply_mvv_lva lasted for: {:?}", duration);

        assert_eq!(VICTIM_PRIORITY[PAWN as usize] + AGGRESSOR_PRIORITY[BISHOP as usize],
                   *order_table.get(&Move::new(29, 36, 0, BISHOP)).unwrap());
        assert_eq!(VICTIM_PRIORITY[PAWN as usize] + AGGRESSOR_PRIORITY[PAWN as usize],
                   *order_table.get(&Move::new(27, 36, 0, PAWN)).unwrap());
        assert_eq!(VICTIM_PRIORITY[PAWN as usize] + AGGRESSOR_PRIORITY[QUEEN as usize],
                   *order_table.get(&Move::new(4, 28, 0, QUEEN)).unwrap());
        assert_eq!(VICTIM_PRIORITY[KNIGHT as usize] + AGGRESSOR_PRIORITY[PAWN as usize],
                   *order_table.get(&Move::new(27, 34, 0, PAWN)).unwrap());
        assert_eq!(VICTIM_PRIORITY[KNIGHT as usize] + AGGRESSOR_PRIORITY[KNIGHT as usize],
                   *order_table.get(&Move::new(17, 34, 0, KNIGHT)).unwrap());
    }

    #[test]
    fn check_apply_mvv_lva_with_recapture() {
        let board = Board::from_fen(&"r1bqkbnr/ppp2ppp/8/3p1n2/2BpP3/6N1/PPP2PPP/RNBQK2R w KQkq - 0 1");

        let mut move_list = MoveList::from_board(board);
        move_list.generate_moves();
        let mut engine = Engine::with_capacity(256);
        let mut order_table = engine.construct_order_table(&move_list);

        let time = Instant::now();
        engine.apply_mvv_lva_with_recapture(&mut order_table, &move_list, 28);
        let duration = time.elapsed();
        println!("apply_mvv_lva lasted for: {:?}", duration);

        assert_eq!(VICTIM_PRIORITY[PAWN as usize] + AGGRESSOR_PRIORITY[BISHOP as usize],
                   *order_table.get(&Move::new(29, 36, 0, BISHOP)).unwrap());
        assert_eq!(VICTIM_PRIORITY[PAWN as usize] + AGGRESSOR_PRIORITY[PAWN as usize],
                   *order_table.get(&Move::new(27, 36, 0, PAWN)).unwrap());
        assert_eq!(VICTIM_PRIORITY[PAWN as usize] + AGGRESSOR_PRIORITY[QUEEN as usize] + RECAPTURE_PRIORITY,
                   *order_table.get(&Move::new(4, 28, 0, QUEEN)).unwrap());
        assert_eq!(VICTIM_PRIORITY[KNIGHT as usize] + AGGRESSOR_PRIORITY[PAWN as usize],
                   *order_table.get(&Move::new(27, 34, 0, PAWN)).unwrap());
        assert_eq!(VICTIM_PRIORITY[KNIGHT as usize] + AGGRESSOR_PRIORITY[KNIGHT as usize],
                   *order_table.get(&Move::new(17, 34, 0, KNIGHT)).unwrap());
    }
}