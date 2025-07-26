//! Best move search algorithm
//! Builds a search tree to find the best possible move according to the evaluation algorithm
//! Uses negamax convention, alpha-beta prunning, quiescence search.

mod ordering;

use std::sync::atomic::{AtomicBool, Ordering};
use crate::board::Board;
use crate::evaluation::{DRAW, Evaluator, MainEvaluator};
use crate::move_generator::{Move, MoveList};
use crate::evaluation::{POSITIVE_INFINITY, NEGATIVE_INFINITY};
use crate::transposition_table::{RepetitionTable, Transposition, TranspositionTable};
use crate::transposition_table::NodeType::{ALPHA, BETA, EXACT};

const KILLER_MOVES_CAPACITY: usize = 1024;
const DEPTH_LIMIT: usize = 128;

static STOP_FLAG: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Eq, PartialEq)]
pub struct Engine<T: Evaluator = MainEvaluator> {
    evaluator: T,
    transposition_table: TranspositionTable,
    repetition_table: RepetitionTable,
    killer_moves: [[Option<Move>; 2]; KILLER_MOVES_CAPACITY],
    depth: u32,
    current_ply: u32,
    best_moves: [[Option<Move>; 3]; DEPTH_LIMIT],
    best_moves_evaluation: [[i32; 3]; DEPTH_LIMIT]
}

impl Engine<MainEvaluator> {
    /// Constructs a new Engine<MainEvaluator> with standard initial capacity of a transposition table.
    pub fn new() -> Self {
        Engine {
            evaluator: MainEvaluator {},
            transposition_table: TranspositionTable::new(),
            repetition_table: RepetitionTable::new(),
            killer_moves: [[None; 2]; KILLER_MOVES_CAPACITY],
            depth: 0,
            current_ply: 0,
            best_moves: [[None; 3]; DEPTH_LIMIT],
            best_moves_evaluation: [[NEGATIVE_INFINITY; 3]; DEPTH_LIMIT]
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Engine {
            evaluator: MainEvaluator {},
            transposition_table: TranspositionTable::with_capacity(capacity),
            repetition_table: RepetitionTable::new(),
            killer_moves: [[None; 2]; KILLER_MOVES_CAPACITY],
            depth: 0,
            current_ply: 0,
            best_moves: [[None; 3]; DEPTH_LIMIT],
            best_moves_evaluation: [[NEGATIVE_INFINITY; 3]; DEPTH_LIMIT]
        }
    }
}

impl Engine {
    /// Sets stop flag to a given boolean.
    /// Stop flag is used by engine to
    /// indicate whether further search should be aborted or not.
    pub fn set_stop_flag(flag: bool) {
        STOP_FLAG.store(flag, Ordering::SeqCst);
    }

    /// Gets stop flag.
    /// Stop flag is used by engine to
    /// indicate whether further search should be aborted or not.
    pub fn get_stop_flag() -> bool {
        STOP_FLAG.load(Ordering::SeqCst)
    }
}

impl<T: Evaluator> Engine<T> {
    /// Constructs a new Engine<T> with a given Evaluator implementing object.
    pub fn with_evaluator(evaluator: T) -> Self {
        Engine {
            evaluator,
            transposition_table: TranspositionTable::new(),
            repetition_table: RepetitionTable::new(),
            killer_moves: [[None; 2]; KILLER_MOVES_CAPACITY],
            depth: 0,
            current_ply: 0,
            best_moves: [[None; 3]; DEPTH_LIMIT],
            best_moves_evaluation: [[NEGATIVE_INFINITY; 3]; DEPTH_LIMIT]
        }
    }

    pub fn with_evaluator_and_capacity(evaluator: T, capacity: usize) -> Self {
        Engine {
            evaluator,
            transposition_table: TranspositionTable::with_capacity(capacity),
            repetition_table: RepetitionTable::new(),
            killer_moves: [[None; 2]; KILLER_MOVES_CAPACITY],
            depth: 0,
            current_ply: 0,
            best_moves: [[None; 3]; DEPTH_LIMIT],
            best_moves_evaluation: [[NEGATIVE_INFINITY; 3]; DEPTH_LIMIT]
        }
    }

    /// Given a move and its evaluation, insert into a given array of best moves and array of best moves evaluation at a proper position
    // #[inline(never)]
    fn insert_into_best_moves(&mut self, piece_move: Move, move_evaluation: i32, depth: usize) {
        if move_evaluation > self.best_moves_evaluation[depth][0] {
            self.best_moves_evaluation[depth][2] = self.best_moves_evaluation[depth][1];
            self.best_moves_evaluation[depth][1] = self.best_moves_evaluation[depth][0];
            self.best_moves_evaluation[depth][0] = move_evaluation;
            self.best_moves[depth][2] = self.best_moves[depth][1];
            self.best_moves[depth][1] = self.best_moves[depth][0];
            self.best_moves[depth][0] = Option::from(piece_move);
        } else if move_evaluation > self.best_moves_evaluation[depth][1] {
            self.best_moves_evaluation[depth][2] = self.best_moves_evaluation[depth][1];
            self.best_moves_evaluation[depth][1] = move_evaluation;
            self.best_moves[depth][2] = self.best_moves[depth][1];
            self.best_moves[depth][1] = Option::from(piece_move);
        } else if move_evaluation > self.best_moves_evaluation[depth][2] {
            self.best_moves_evaluation[depth][2] = move_evaluation;
            self.best_moves[depth][2] = Option::from(piece_move);
        }
    }

    /// Retrieves what the engine thinks the best move for a given board situation is,
    /// as stored in the transposition table.
    pub fn get_best_move(&self, board: &Board) -> Move {
        self.transposition_table.get_from_zobrist(board.zobrist).clone().unwrap().best_moves[0].unwrap()
    }

    /// Retrieves what the engine thinks the best moves for the most recent board situation is.
    pub fn get_current_best_moves(&self) -> &[Option<Move>; 3] {
        &self.best_moves[0]
    }
    
    /// Calls search algorithm to find the best possible moves in the current situation.
    /// Searches up to the given depth.
    /// Uses the given move list to generate moves in-place and analyze the board situation.
    /// Calls the algorithm using alpha-beta prunning and quiescence search
    // #[inline(never)]
    pub fn search(&mut self, move_list: &mut MoveList, depth: u32) -> i32 {
        self.current_ply = move_list.get_board().plies;

        let mut value = 0;
        for current_depth in 1..depth+1 {
            self.depth = current_depth;
            value = self.search_alpha_beta_prunning(move_list, current_depth, NEGATIVE_INFINITY, POSITIVE_INFINITY);
            self.best_moves[0] = self.transposition_table.get_from_zobrist(move_list.get_board().zobrist).clone().unwrap().best_moves;
            if STOP_FLAG.load(Ordering::SeqCst) {
                return value;
            }
        }

        value
    }

    /// Calls search algorithm to find the best possible moves in the current situation.
    /// Searches up to the given depth.
    /// Uses the given move list to generate moves in-place and analyze the board situation.
    /// Calls the algorithm using alpha-beta prunning.
    /// NOTE: Does NOT use quiescence search and therefore is inferior to the search() function
    ///     Should be used mainly for testing.
    pub fn search_no_quiescence(&mut self, move_list: &mut MoveList, depth: u32) -> i32 {
        self.search_alpha_beta_prunning_naive(move_list, depth, NEGATIVE_INFINITY, POSITIVE_INFINITY)
    }

    /// Uses alpha-beta prunning to find the best possible move in the search tree.
    /// Searches up to the given depth.
    /// Uses the given move list to generate moves in-place and analyze the board situation.
    /// Calls quiescence search if depth is zero
    /// Alpha - minimum score the current player is assured of (we found a move of at least this value earlier at this depth)
    /// Beta - maximum score the opponent is assured of (the best value the parent node recorded)
    // #[inline(never)]
    fn search_alpha_beta_prunning(&mut self, move_list: &mut MoveList, depth: u32, mut alpha: i32, beta: i32) -> i32 {
        let zobrist = move_list.get_board().zobrist;

        if self.repetition_table.visit_position(zobrist) {
            self.repetition_table.unvisit_position(zobrist);
            return DRAW;
        }

        let original_alpha=  alpha;

        let transposition_entry = self.transposition_table.get_from_zobrist(zobrist);

        // Check if we have a proper entry in the transposition table
        if let Some(transposition) = transposition_entry {
            if transposition.depth >= depth {
                if transposition.node_type == EXACT { self.repetition_table.unvisit_position(zobrist); return transposition.value; }
                if transposition.node_type == ALPHA && transposition.value <= alpha { self.repetition_table.unvisit_position(zobrist); return transposition.value; } // Our alpha cut-off is even bigger than it was for the put operation
                if /*transposition.node_type == BETA*/ transposition.value >= beta { self.repetition_table.unvisit_position(zobrist); return transposition.value; } // Our beta cut-off is even smaller than it was for the put operation
            }
        }

        if depth == 0 {
            self.repetition_table.unvisit_position(zobrist);
            return self.quiescence_search(move_list, -beta, -alpha);
        }

        move_list.generate_moves();
        self.order_moves(move_list);

        let moves = move_list.get_moves().clone();

        let mut cutoff_move = Move::empty();

        // Index for the best moves buffer.
        // self.depth - target depth that we want to reach with our current search
        // depth - depth left to search
        // When we start, we have depth = self.depth, so we fill in the first buffer space.
        // When we end, we have depth = 1, so we fill the buffer space indexed self.depth - 1
        let buffer_index = (self.depth - depth) as usize;

        for piece_move in moves
        {
            move_list.make_move(&piece_move);
            if !move_list.is_opponent_in_check() {
                let move_evaluation = -self.search_alpha_beta_prunning(move_list, depth - 1, -beta, -alpha);

                // Update the best moves record
                self.insert_into_best_moves(piece_move, move_evaluation, buffer_index);
            }
            move_list.unmake_move(&piece_move);

            alpha = alpha.max(self.best_moves_evaluation[buffer_index][0]);
            if alpha >= beta { cutoff_move = piece_move; break; }

            if STOP_FLAG.load(Ordering::SeqCst) {
                break;
            }
        }

        if self.best_moves[buffer_index][0] == None {
            self.repetition_table.unvisit_position(zobrist);
            return if move_list.is_in_check() { NEGATIVE_INFINITY } else { DRAW }
        }

        // update the transposition table
        let node_type = if self.best_moves_evaluation[buffer_index][0] <= original_alpha { ALPHA }
            else if self.best_moves_evaluation[buffer_index][0] >= beta { self.store_killer_move(&cutoff_move, move_list.get_board().plies as usize); BETA } else { EXACT };
        self.transposition_table.put_transposition(&Transposition::from_zobrist(zobrist, depth, self.best_moves_evaluation[buffer_index][0], &self.best_moves[buffer_index], node_type));

        self.repetition_table.unvisit_position(zobrist);

        self.best_moves_evaluation[buffer_index][0]
    }

    /// Stores a killer move in the killer moves array
    /// (refer to ordering.rs on what killer moves are).
    /// If there are no killer moves stored at the given depth - store at index 0.
    /// If there is a killer move at index 0 - move the old killer move to index 1
    /// and store the new move at index 0.
    /// Parameters:
    ///     - killer_move - a killer move that we want to store
    ///     - ply - depth at which the move occurred
    fn store_killer_move(&mut self, killer_move: &Move, ply: usize) {
        if self.killer_moves[ply][0].is_none() {
            self.killer_moves[ply][0] = Option::from(*killer_move);
        }
        else if self.killer_moves[ply][0].unwrap() != *killer_move {
            self.killer_moves[ply][1] = self.killer_moves[ply][0];
            self.killer_moves[ply][0] = Option::from(*killer_move);
        }
    }

    /// Uses alpha-beta prunning to find the best possible move in the search tree.
    /// Searches up to the given depth.
    /// Uses the given move list to generate moves in-place and analyze the board situation.
    /// Alpha - minimum score the current player is assured of (we found a move of at least this value earlier at this depth)
    /// Beta - maximum score the opponent is assured of (the best value the parent node recorded)
    /// NOTE: Does NOT use quiescence search and therefore is inferior to the search_alpha_beta_prunning() function
    ///     Should be used mainly for testing.
    fn search_alpha_beta_prunning_naive(&mut self, move_list: &mut MoveList, depth: u32, mut alpha: i32, beta: i32) -> i32 {
        let mut value: i32 = T::evaluate(move_list.get_board());

        if depth == 0 {
            return value;
        }

        move_list.generate_moves();

        let moves = move_list.get_moves().clone();

        for piece_move in moves
        {
            move_list.make_move(&piece_move);
            if !move_list.is_opponent_in_check() {
                value = value.max(
                    -self.search_alpha_beta_prunning_naive(move_list, depth - 1, -beta, -alpha)
                );
            }
            move_list.unmake_move(&piece_move);

            alpha = alpha.max(value);
            if alpha >= beta { break; }
        }

        value
    }

    /// Uses alpha-beta prunning to find the best possible move in the search tree.
    /// Searches until it finds a 'quiet' position, where no captures, promotions or checks can be made.
    /// It avoids the horizon effect by not ignoring threats at depth zero.
    /// Uses the given move list to generate moves in-place and analyze the board situation.
    /// Alpha - minimum score the current player is assured of (we found a move of at least this value earlier at this depth)
    /// Beta - maximum score the opponent is assured of (the best value the parent node recorded)
    // #[inline(never)]
    fn quiescence_search(&mut self, move_list: &mut MoveList, mut alpha: i32, beta: i32) -> i32 {
        // println!("Is repetition table empty?: {}", self.repetition_table.is_empty());
        // println!("Visits to this position before: {}", self.repetition_table.get_repetition(move_list.get_board().zobrist));
        if self.repetition_table.visit_position(move_list.get_board().zobrist) {
            // println!("Repetition alert MADAFAKA");
            self.repetition_table.unvisit_position(move_list.get_board().zobrist);
            return DRAW;
        }

        let original_alpha = alpha;

        let transposition_entry = self.transposition_table.get_from_zobrist(move_list.get_board().zobrist);

        // Check if we have a proper entry in the transposition table
        if let Some(transposition) = transposition_entry {
            // println!("Quiescence transposition reached: {:?}", transposition);
            if transposition.node_type == EXACT { self.repetition_table.unvisit_position(move_list.get_board().zobrist); return transposition.value; }
            if transposition.node_type == ALPHA && transposition.value <= alpha { self.repetition_table.unvisit_position(move_list.get_board().zobrist); return transposition.value; } // Our alpha cut-off is even bigger than it was for the put operation
            if /*transposition.node_type == BETA*/ transposition.value >= beta { self.repetition_table.unvisit_position(move_list.get_board().zobrist); return transposition.value; } // Our beta cut-off is even smaller than it was for the put operation
        }

        move_list.generate_noisy_moves();
        // println!("Quiescence moves: {:?}", move_list.get_moves());

        if move_list.get_moves().len() == 0 {
            self.repetition_table.unvisit_position(move_list.get_board().zobrist);
            return T::evaluate(move_list.get_board());
        }

        self.order_moves(move_list);

        // Index for the best moves buffer.
        // We subtract the difference between the board's ply and the original ply.
        let buffer_index = (move_list.get_board().plies - self.current_ply) as usize;

        let moves = move_list.get_moves().clone();

        for piece_move in moves
        {
            move_list.make_move(&piece_move);
            if !move_list.is_opponent_in_check() {
                let move_evaluation = -self.quiescence_search(move_list, -beta, -alpha);

                self.insert_into_best_moves(piece_move, move_evaluation, buffer_index);
            }
            move_list.unmake_move(&piece_move);

            alpha = alpha.max(self.best_moves_evaluation[buffer_index][0]);
            if alpha >= beta { break; }
            if STOP_FLAG.load(Ordering::SeqCst) {
                break;
            }
        }

        if self.best_moves[buffer_index][0] == None {
            self.repetition_table.unvisit_position(move_list.get_board().zobrist);
            return if move_list.is_in_check() { NEGATIVE_INFINITY } else { DRAW }
        }

        // update the transposition table
        let node_type = if self.best_moves_evaluation[buffer_index][0] <= original_alpha { ALPHA } else if self.best_moves_evaluation[buffer_index][0] >= beta { BETA } else { EXACT };
        self.transposition_table.put_transposition(&Transposition::from_zobrist(move_list.get_board().zobrist, 0, self.best_moves_evaluation[buffer_index][0], &self.best_moves[buffer_index], node_type));

        self.repetition_table.unvisit_position(move_list.get_board().zobrist);

        self.best_moves_evaluation[buffer_index][0]
    }

    /// Finds the best possible move in the search tree.
    /// Searches up to the given depth.
    /// Uses the given move list to generate moves in-place and analyze the board situation.
    /// NOTE: Uses NEITHER quiescence search NOR alpha-beta prunning
    ///     Should be used only for testing.
    fn search_naive(&mut self, move_list: &mut MoveList, depth: u32) -> i32 {
        let mut value: i32 = T::evaluate(move_list.get_board());

        if depth == 0 {
            return value;
        }

        move_list.generate_moves();

        let moves = move_list.get_moves().clone();

        for piece_move in moves
        {
            move_list.make_move(&piece_move);
            if !move_list.is_opponent_in_check() {
                value = value.max(-self.search_naive(move_list, depth - 1))
            }
            move_list.unmake_move(&piece_move);
        }

        value
    }
}


#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::board::START_POSITION;
    use crate::piece::Piece::{KING, PAWN, QUEEN, ROOK};
    use super::*;
    use crate::evaluation::MockMaterialEvaluator;

    #[test]
    fn search_initial_position() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);
        
        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});

        assert_eq!(0, engine.search_naive(&mut move_list, 0));
        assert_eq!(0, engine.search_naive(&mut move_list, 1));
        assert_eq!(0, engine.search_naive(&mut move_list, 2));
        assert_eq!(0, engine.search_naive(&mut move_list, 3));

        // assert_eq!(0, engine.search(&mut move_list, 0));
        assert_eq!(0, engine.search(&mut move_list, 1));
        assert_eq!(0, engine.search(&mut move_list, 2));
        assert_eq!(0, engine.search(&mut move_list, 3));
    }

    #[test]
    fn search_material_handling() {
        let mut board = Board::new();
        let fen = "4k3/5r2/8/8/8/5R2/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);
        
        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});

        assert_eq!(0, engine.search_naive(&mut move_list, 0));
        assert_eq!(500, engine.search_naive(&mut move_list, 1));
        assert_eq!(0, engine.search_naive(&mut move_list, 2));

        engine = Engine::with_evaluator(MockMaterialEvaluator {});

        println!("depth 0");
        assert_eq!(0, engine.search(&mut move_list, 0));
        println!("depth 1");
        assert_eq!(0, engine.search(&mut move_list, 1));
        println!("depth 2");
        assert_eq!(0, engine.search(&mut move_list, 2));
    }

    #[test]
    fn test_alpha_beta_prunning_best_move_first() {
        let mut board = Board::new();
        let fen = "4k3/8/8/8/8/5PP1/1q6/P3K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});

        assert_eq!(-600, engine.search_no_quiescence(&mut move_list, 0));
        assert_eq!(300, engine.search_no_quiescence(&mut move_list, 1));
    }

    #[test]
    fn test_alpha_beta_prunning_best_move_last() {
        let mut board = Board::new();
        let fen = "4k3/8/6q1/7P/8/8/PP6/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});

        assert_eq!(-600, engine.search_no_quiescence(&mut move_list, 0));
        assert_eq!(300, engine.search_no_quiescence(&mut move_list, 1));
    }

    #[test]
    fn test_stalemate() {
        let mut board = Board::new();
        let fen = "8/8/8/R7/6k1/4Q3/8/4K2R b K - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});
        engine.depth = 1;

        assert_eq!(DRAW, engine.search_alpha_beta_prunning(&mut move_list, 1, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.is_empty());
        // assert_eq!(DRAW, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY)); // might want to solve this or might not bother at all
    }

    #[test]
    fn test_checkmate() {
        let mut board = Board::new();
        let fen = "7k/6QQ/8/8/8/8/8/4K3 b - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});
        engine.depth = 1;

        assert_eq!(NEGATIVE_INFINITY, engine.search_alpha_beta_prunning(&mut move_list, 1, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.is_empty());

        let mut board = Board::new();
        let fen = "7k/6QQ/8/8/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::new();
        engine.depth = 1;

        assert_eq!(POSITIVE_INFINITY, engine.search_alpha_beta_prunning(&mut move_list, 1, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.is_empty());
    }

    #[test]
    fn test_quiescence_search_1() {
        let mut board = Board::new();
        let fen = "8/3pk3/3p4/2P5/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});

        assert_eq!(0, engine.search_naive(&mut move_list, 1));
        assert_eq!(-100, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.is_empty());
    }

    #[test]
    fn test_quiescence_search_2() {
        let mut board = Board::new();
        let fen = "4k3/8/4pp2/4p3/3P4/3P4/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});

        assert_eq!(0, engine.search_naive(&mut move_list, 1));
        assert_eq!(-100, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.is_empty());
    }

    #[test]
    fn test_quiescence_search_3() {
        let mut board = Board::new();
        let fen = "8/3pk3/3p3P/2P5/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});

        assert_eq!(100, engine.search_naive(&mut move_list, 1));

        move_list.make_move(&Move{origin: 40, target: 48, promotion: 0, piece: PAWN});

        assert_eq!(100, engine.search_naive(&mut move_list, 1));
        assert_eq!(-700, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.is_empty());
    }

    #[test]
    fn check_insert_into_best_moves_best_move() {
        let piece_move = Move::new(0, 1, 0, QUEEN);
        let evaluation = 200;

        let mut engine = Engine::with_capacity(1);
        engine.best_moves[0] = [Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())];
        engine.best_moves_evaluation[0] = [0,-100,-300];

        engine.insert_into_best_moves(piece_move, evaluation, 0);

        assert_eq!([Option::from(piece_move), Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN))], engine.best_moves[0]);
        assert_eq!([None, None, None], engine.best_moves[1]);
        assert_eq!([200, 0, -100], engine.best_moves_evaluation[0]);
        assert_eq!([NEGATIVE_INFINITY, NEGATIVE_INFINITY, NEGATIVE_INFINITY], engine.best_moves_evaluation[1]);
    }

    #[test]
    fn test_threefold_repetition_alpha_beta() {
        let mut board = Board::new();
        let fen = "4k3/4p3/8/8/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});
        engine.depth = 0;

        assert_ne!(DRAW, engine.search_alpha_beta_prunning(&mut move_list, 0, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::with_evaluator(MockMaterialEvaluator{});
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_ne!(DRAW, engine.search_alpha_beta_prunning(&mut move_list, 0, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::with_evaluator(MockMaterialEvaluator{});
        engine.repetition_table.visit_position(move_list.get_board().zobrist);
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_eq!(DRAW, engine.search_alpha_beta_prunning(&mut move_list, 0, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine.repetition_table.unvisit_position(move_list.get_board().zobrist);
        engine.repetition_table.unvisit_position(move_list.get_board().zobrist);
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));
    }

    #[test]
    fn test_threefold_repetition_quiescence() {
        let mut board = Board::new();
        let fen = "4k3/4p3/8/8/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});

        assert_ne!(DRAW, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::with_evaluator(MockMaterialEvaluator {});
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_ne!(DRAW, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::with_evaluator(MockMaterialEvaluator {});
        engine.repetition_table.visit_position(move_list.get_board().zobrist);
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_eq!(DRAW, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine.repetition_table.unvisit_position(move_list.get_board().zobrist);
        engine.repetition_table.unvisit_position(move_list.get_board().zobrist);
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));
    }

    #[test]
    fn check_insert_into_best_moves_second_move() {
        let piece_move = Move::new(0, 1, 0, QUEEN);
        let evaluation = 100;

        let mut engine = Engine::with_capacity(1);
        engine.best_moves[0] = [Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())];
        engine.best_moves_evaluation[0] = [500, 0,-300];

        engine.insert_into_best_moves(piece_move, evaluation, 0);

        assert_eq!([Option::from(Move::new(63,62,0,KING)), Option::from(piece_move), Option::from(Move::new(0,8,0, PAWN))], engine.best_moves[0]);
        assert_eq!([None, None, None], engine.best_moves[1]);
        assert_eq!([500, 100, 0], engine.best_moves_evaluation[0]);
        assert_eq!([NEGATIVE_INFINITY, NEGATIVE_INFINITY, NEGATIVE_INFINITY], engine.best_moves_evaluation[1]);
    }

    #[test]
    fn check_insert_into_best_moves_third_move() {
        let piece_move = Move::new(0, 1, 0, QUEEN);
        let evaluation = -100;

        let mut engine = Engine::with_capacity(1);
        engine.best_moves[0] = [Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())];
        engine.best_moves_evaluation[0] = [500, 0,-300];

        engine.insert_into_best_moves(piece_move, evaluation, 0);

        assert_eq!([Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(piece_move)], engine.best_moves[0]);
        assert_eq!([None, None, None], engine.best_moves[1]);
        assert_eq!([500, 0, -100], engine.best_moves_evaluation[0]);
        assert_eq!([NEGATIVE_INFINITY, NEGATIVE_INFINITY, NEGATIVE_INFINITY], engine.best_moves_evaluation[1]);
    }

    #[test]
    fn check_insert_into_best_moves_weak_move() {
        let piece_move = Move::new(0, 1, 0, QUEEN);
        let evaluation = -600;

        let mut engine = Engine::with_capacity(1);
        engine.best_moves[0] = [Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())];
        engine.best_moves_evaluation[0] = [500, 0,-300];

        engine.insert_into_best_moves(piece_move, evaluation, 0);

        assert_eq!([Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())], engine.best_moves[0]);
        assert_eq!([None, None, None], engine.best_moves[1]);
        assert_eq!([500, 0, -300], engine.best_moves_evaluation[0]);
        assert_eq!([NEGATIVE_INFINITY, NEGATIVE_INFINITY, NEGATIVE_INFINITY], engine.best_moves_evaluation[1]);
    }

    #[test]
    fn check_insert_into_best_moves_deep() {
        let piece_move = Move::new(0, 1, 0, QUEEN);
        let evaluation = 100;

        let mut engine = Engine::with_capacity(1);
        engine.best_moves[0] = [Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())];
        engine.best_moves[6] = [Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())];
        engine.best_moves_evaluation[0] = [500, 0,-300];
        engine.best_moves_evaluation[6] = [500, 0,-300];

        engine.insert_into_best_moves(piece_move, evaluation, 6);

        assert_eq!([Option::from(Move::new(63,62,0,KING)), Option::from(piece_move), Option::from(Move::new(0,8,0, PAWN))], engine.best_moves[6]);
        assert_eq!([Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())], engine.best_moves[0]);
        assert_eq!([None, None, None], engine.best_moves[1]);
        assert_eq!([500, 100, 0], engine.best_moves_evaluation[6]);
        assert_eq!([500, 0, -300], engine.best_moves_evaluation[0]);
        assert_eq!([NEGATIVE_INFINITY, NEGATIVE_INFINITY, NEGATIVE_INFINITY], engine.best_moves_evaluation[1]);
    }

    // #[test]
    // fn bench_search_start_position() {
    //     let mut board = Board::new();
    //     let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    //     board.read_fen(fen);
    //     let mut move_list = MoveList::from_board(board);;
    //
    //     let mut engine = Engine::new();
    //
    //     let now = Instant::now();
    //     println!("{}", engine.search(&mut move_list, 8));
    //     let duration = now.elapsed();
    //     println!("search lasted for: {:?}", duration);
    //     println!("Best moves: {:?}", engine.transposition_table.get_from_position(&board).clone().unwrap().best_moves);
    // }
    //
    // #[test]
    // fn bench_search_kiwipete_position() {
    //     let mut board = Board::new();
    //     let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    //     board.read_fen(fen);
    //     let mut move_list = MoveList::from_board(board);;
    //
    //     let mut engine = Engine::new();
    //
    //     let now = Instant::now();
    //     println!("{}", engine.search(&mut move_list, 1));
    //     let duration = now.elapsed();
    //     println!("search lasted for: {:?}", duration);
    //     println!("Best moves: {:?}", engine.transposition_table.get_from_position(&board).clone().unwrap().best_moves);
    // }
    //
    // #[test]
    // fn trace_moves() {
    //     let mut board = Board::new();
    //     let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    //     board.read_fen(fen);
    //     let mut move_list = MoveList::new(&mut board);
    //
    //     let mut engine = Engine::new();
    //
    //     loop {
    //         let evaluation = engine.search(&mut move_list, 0);
    //         println!("Position evaluated as: {}", evaluation);
    //         let transposition = engine.transposition_table.get_from_position(move_list.get_board());
    //         if transposition.is_none() { println!("No transposition found"); break; }
    //         let best_move = transposition.clone().unwrap().best_moves[0];
    //         if best_move == None { println!("No move found"); break; }
    //         let best_move = best_move.unwrap();
    //         println!("Best move: {:?}", best_move);
    //         move_list.make_move(&best_move);
    //     }
    //
    //     // r3k2r/p1ppq1b1/bn4p1/3n4/8/3P1QPp/P1PBBP1P/R3K2R w - - 0 1
    // }

    #[test]
    fn test_store_killer_moves() {
        let mut engine = Engine::with_capacity(256);

        assert_eq!(None, engine.killer_moves[3][0]);
        assert_eq!(None, engine.killer_moves[3][1]);

        let first_move = Move::new(0, 1, 0, ROOK);
        let second_move = Move::new(8, 16, 0, PAWN);

        engine.store_killer_move(&first_move, 3);

        assert_eq!(first_move, engine.killer_moves[3][0].unwrap());
        assert_eq!(None, engine.killer_moves[3][1]);

        engine.store_killer_move(&second_move, 3);

        assert_eq!(second_move, engine.killer_moves[3][0].unwrap());
        assert_eq!(first_move, engine.killer_moves[3][1].unwrap());

        engine.store_killer_move(&first_move, 2);

        assert_eq!(second_move, engine.killer_moves[3][0].unwrap());
        assert_eq!(first_move, engine.killer_moves[3][1].unwrap());
        assert_eq!(first_move, engine.killer_moves[2][0].unwrap());
        assert_eq!(None, engine.killer_moves[2][1]);
    }

    #[test]
    fn check_killer_moves_after_search() {
        let mut board = Board::new();
        let fen = "4k3/4p3/8/8/5Q2/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator_and_capacity(MockMaterialEvaluator {}, 1024);

        engine.depth = 2;
        engine.search_alpha_beta_prunning(&mut move_list, 2, NEGATIVE_INFINITY, POSITIVE_INFINITY);
        // There is an obvious killer moves when we try to move our queen so that it can be captured by a pawn
        assert!(!engine.killer_moves[1][0].is_none());
        assert!(!engine.killer_moves[1][1].is_none());
    }
}