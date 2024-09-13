//! Best move search algorithm
//! Builds a search tree to find the best possible move according to the evaluation algorithm
//! Uses negamax convention, alpha-beta prunning, quiescence search.

use std::ops::Deref;
use crate::board::Board;
use crate::evaluation::{DRAW, evaluate};
use crate::move_generator::{Move, MoveList};
use crate::evaluation::{POSITIVE_INFINITY, NEGATIVE_INFINITY};
use crate::transposition_table::{RepetitionTable, Transposition, TranspositionTable};
use crate::transposition_table::NodeType::{ALPHA, BETA, EXACT};

#[derive(PartialEq, Eq, Debug)]
pub struct Engine {
    transposition_table: TranspositionTable,
    repetition_table: RepetitionTable
}

impl Engine {
    
    pub fn new() -> Self {
        Engine {
            transposition_table: TranspositionTable::new(),
            repetition_table: RepetitionTable::new()
        }
    }

    pub fn get_best_move(&self, board: &Board) -> Move {
        self.transposition_table.get_from_zobrist(board.zobrist).clone().unwrap().best_moves[0].unwrap().clone()
    }
    
    /// Calls search algorithm to find the best possible moves in the current situation.
    /// Searches up to the given depth.
    /// Uses the given move list to generate moves in-place and analyze the board situation.
    /// Calls the algorithm using alpha-beta prunning and quiescence search
    // #[inline(never)]
    pub fn search(&mut self, move_list: &mut MoveList, depth: u32) -> i32 {
        // println!("Repetition table: {:?}", self.repetition_table);
        self.search_alpha_beta_prunning(move_list, depth, NEGATIVE_INFINITY, POSITIVE_INFINITY)
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
        if self.repetition_table.visit_position(move_list.get_board().zobrist) {
            self.repetition_table.unvisit_position(move_list.get_board().zobrist);
            return DRAW;
        }

        let original_alpha=  alpha;

        let transposition_entry = self.transposition_table.get_from_zobrist(move_list.get_board().zobrist);

        // Check if we have a proper entry in the transposition table
        if let Some(transposition) = transposition_entry {
            if transposition.depth >= depth {
                // println!("Transposition reached: {:?}", transposition);
                if transposition.node_type == EXACT { self.repetition_table.unvisit_position(move_list.get_board().zobrist); return transposition.value; }
                if transposition.node_type == ALPHA && transposition.value <= alpha { self.repetition_table.unvisit_position(move_list.get_board().zobrist); return transposition.value; } // Our alpha cut-off is even bigger than it was for the put operation
                if /*transposition.node_type == BETA*/ transposition.value >= beta { self.repetition_table.unvisit_position(move_list.get_board().zobrist); return transposition.value; } // Our beta cut-off is even smaller than it was for the put operation
            }
        }

        let mut value: i32 = NEGATIVE_INFINITY;

        if depth == 0 {
            self.repetition_table.unvisit_position(move_list.get_board().zobrist);
            return self.quiescence_search(move_list, -beta, -alpha);
        }

        move_list.generate_moves();

        // println!("Alpha-beta stage depth {} moves: {:?}", depth, move_list.get_moves());

        let moves = move_list.get_moves().clone();

        let mut best_moves: [Option<Move>; 3] = [None; 3];
        let mut best_moves_evaluation: [i32; 2] = [NEGATIVE_INFINITY; 2]; // we omit the first move evaluation, as this is simply the value variable

        for piece_move in moves
        {
            move_list.make_move(&piece_move);
            if !move_list.is_opponent_in_check() {
                let move_evaluation = -self.search_alpha_beta_prunning(move_list, depth - 1, -beta, -alpha);

                // Update the best moves record
                Self::insert_into_best_moves(&mut best_moves, &mut value, &mut best_moves_evaluation, piece_move, move_evaluation);
            }
            move_list.unmake_move(&piece_move);

            alpha = alpha.max(value);
            // println!("alpha: {}", alpha);
            // println!("beta: {}", beta);
            if alpha >= beta { break; }
        }

        if best_moves[0] == None {
            self.repetition_table.unvisit_position(move_list.get_board().zobrist);
            if move_list.is_in_check() { return NEGATIVE_INFINITY; }
            else { return DRAW; }
        }

        // update the transposition table
        let node_type = if value <= original_alpha { ALPHA } else if value >= beta { BETA } else { EXACT };
        self.transposition_table.put_transposition(&Transposition::from_zobrist(move_list.get_board().zobrist, depth, value, &best_moves, node_type));

        self.repetition_table.unvisit_position(move_list.get_board().zobrist);

        value
    }

    /// Given a move and its evaluation, insert into a given array of best moves and array of best moves evaluation at a proper position
    // #[inline(never)]
    fn insert_into_best_moves(best_moves: &mut [Option<Move>; 3], value: &mut i32, best_moves_evaluation: &mut [i32; 2], piece_move: Move, move_evaluation: i32) {
        if move_evaluation > *value {
            best_moves_evaluation[1] = best_moves_evaluation[0];
            best_moves_evaluation[0] = *value;
            *value = move_evaluation;
            best_moves[2] = best_moves[1];
            best_moves[1] = best_moves[0];
            best_moves[0] = Option::from(piece_move);
        } else if move_evaluation > best_moves_evaluation[0] {
            best_moves_evaluation[1] = best_moves_evaluation[0];
            best_moves_evaluation[0] = move_evaluation;
            best_moves[2] = best_moves[1];
            best_moves[1] = Option::from(piece_move);
        } else if move_evaluation > best_moves_evaluation[1] {
            best_moves_evaluation[1] = move_evaluation;
            best_moves[2] = Option::from(piece_move);
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
        let mut value: i32 = evaluate(move_list.get_board());

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
    #[inline(never)]
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

        let mut value: i32 = NEGATIVE_INFINITY;

        move_list.generate_noisy_moves();
        // println!("Quiescence moves: {:?}", move_list.get_moves());

        if move_list.get_moves().len() == 0 {
            self.repetition_table.unvisit_position(move_list.get_board().zobrist);
            return evaluate(move_list.get_board());
        }

        let mut best_moves: [Option<Move>; 3] = [None; 3];
        let mut best_moves_evaluation: [i32; 2] = [NEGATIVE_INFINITY; 2]; // we omit the first move evaluation, as this is simply the value variable

        let moves = move_list.get_moves().clone();

        for piece_move in moves
        {
            move_list.make_move(&piece_move);
            if !move_list.is_opponent_in_check() {
                let move_evaluation = -self.quiescence_search(move_list, -beta, -alpha);

                Self::insert_into_best_moves(&mut best_moves, &mut value, &mut best_moves_evaluation, piece_move, move_evaluation);
            }
            move_list.unmake_move(&piece_move);

            alpha = alpha.max(value);
            if alpha >= beta { break; }
        }

        if best_moves[0] == None {
            self.repetition_table.unvisit_position(move_list.get_board().zobrist);
            if move_list.is_in_check() { return NEGATIVE_INFINITY; }
            else { return DRAW; }
        }

        // update the transposition table
        let node_type = if value <= original_alpha { ALPHA } else if value >= beta { BETA } else { EXACT };
        self.transposition_table.put_transposition(&Transposition::from_zobrist(move_list.get_board().zobrist, 0, value, &best_moves, node_type));

        self.repetition_table.unvisit_position(move_list.get_board().zobrist);

        value
    }

    /// Finds the best possible move in the search tree.
    /// Searches up to the given depth.
    /// Uses the given move list to generate moves in-place and analyze the board situation.
    /// NOTE: Uses NEITHER quiescence search NOR alpha-beta prunning
    ///     Should be used only for testing.
    fn search_naive(&mut self, move_list: &mut MoveList, depth: u32) -> i32 {
        let mut value: i32 = evaluate(move_list.get_board());

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
    use crate::piece::Piece::{KING, PAWN, QUEEN};
    use super::*;

    #[test]
    fn search_initial_position() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(&mut board);
        
        let mut engine = Engine::new();

        assert_eq!(0, engine.search_naive(&mut move_list, 0));
        assert_eq!(0, engine.search_naive(&mut move_list, 1));
        assert_eq!(0, engine.search_naive(&mut move_list, 2));
        assert_eq!(0, engine.search_naive(&mut move_list, 3));

        assert_eq!(0, engine.search(&mut move_list, 0));
        assert_eq!(0, engine.search(&mut move_list, 1));
        assert_eq!(0, engine.search(&mut move_list, 2));
        assert_eq!(0, engine.search(&mut move_list, 3));
    }

    #[test]
    fn search_material_handling() {
        let mut board = Board::new();
        let fen = "4k3/5r2/8/8/8/5R2/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(&mut board);
        
        let mut engine = Engine::new();

        assert_eq!(0, engine.search_naive(&mut move_list, 0));
        assert_eq!(500, engine.search_naive(&mut move_list, 1));
        assert_eq!(0, engine.search_naive(&mut move_list, 2));

        engine = Engine::new();

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
        let mut move_list = MoveList::from_board(&mut board);

        let mut engine = Engine::new();

        assert_eq!(-600, engine.search_no_quiescence(&mut move_list, 0));
        assert_eq!(300, engine.search_no_quiescence(&mut move_list, 1));
    }

    #[test]
    fn test_alpha_beta_prunning_best_move_last() {
        let mut board = Board::new();
        let fen = "4k3/8/6q1/7P/8/8/PP6/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(&mut board);

        let mut engine = Engine::new();

        assert_eq!(-600, engine.search_no_quiescence(&mut move_list, 0));
        assert_eq!(300, engine.search_no_quiescence(&mut move_list, 1));
    }

    #[test]
    fn test_stalemate() {
        let mut board = Board::new();
        let fen = "8/8/8/R7/6k1/4Q3/8/4K2R b K - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(&mut board);

        let mut engine = Engine::new();

        assert_eq!(DRAW, engine.search_alpha_beta_prunning(&mut move_list, 1, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.is_empty());
        // assert_eq!(DRAW, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY)); // might want to solve this or might not bother at all
    }

    #[test]
    fn test_checkmate() {
        let mut board = Board::new();
        let fen = "7k/6QQ/8/8/8/8/8/4K3 b - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(&mut board);

        let mut engine = Engine::new();

        assert_eq!(NEGATIVE_INFINITY, engine.search_alpha_beta_prunning(&mut move_list, 1, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.is_empty());

        let mut board = Board::new();
        let fen = "7k/6QQ/8/8/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(&mut board);

        let mut engine = Engine::new();

        assert_eq!(POSITIVE_INFINITY, engine.search_alpha_beta_prunning(&mut move_list, 1, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.is_empty());
    }

    #[test]
    fn test_quiescence_search_1() {
        let mut board = Board::new();
        let fen = "8/3pk3/3p4/2P5/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(&mut board);

        let mut engine = Engine::new();

        assert_eq!(0, engine.search_naive(&mut move_list, 1));
        assert_eq!(-100, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.is_empty());
    }

    #[test]
    fn test_quiescence_search_2() {
        let mut board = Board::new();
        let fen = "4k3/8/4pp2/4p3/3P4/3P4/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(&mut board);

        let mut engine = Engine::new();

        assert_eq!(0, engine.search_naive(&mut move_list, 1));
        assert_eq!(-100, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.is_empty());
    }

    #[test]
    fn test_quiescence_search_3() {
        let mut board = Board::new();
        let fen = "8/3pk3/3p3P/2P5/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(&mut board);

        let mut engine = Engine::new();

        assert_eq!(100, engine.search_naive(&mut move_list, 1));

        move_list.make_move(&Move{origin: 40, target: 48, promotion: 0, piece: PAWN});

        assert_eq!(100, engine.search_naive(&mut move_list, 1));
        assert_eq!(-700, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(engine.repetition_table.is_empty());
    }

    #[test]
    fn check_insert_into_best_moves_best_move() {
        let mut best_moves = [Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())];
        let mut value = 0;
        let mut best_moves_evaluation= [-100, -300];

        let piece_move = Move::new(0, 1, 0, QUEEN);
        let evaluation = 200;

        Engine::insert_into_best_moves(&mut best_moves, &mut value, &mut best_moves_evaluation, piece_move, evaluation);

        assert_eq!([Option::from(piece_move), Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN))], best_moves);
        assert_eq!(200, value);
        assert_eq!([0, -100], best_moves_evaluation);
    }

    #[test]
    fn test_threefold_repetition_alpha_beta() {
        let mut board = Board::new();
        let fen = "4k3/4p3/8/8/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(&mut board);

        let mut engine = Engine::new();

        assert_eq!(-100, engine.search_alpha_beta_prunning(&mut move_list, 0, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::new();
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_eq!(-100, engine.search_alpha_beta_prunning(&mut move_list, 0, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::new();
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
        let mut move_list = MoveList::from_board(&mut board);

        let mut engine = Engine::new();

        assert_eq!(-100, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::new();
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_eq!(-100, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::new();
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
        let mut best_moves = [Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())];
        let mut value = 500;
        let mut best_moves_evaluation= [0, -300];

        let piece_move = Move::new(0, 1, 0, QUEEN);
        let evaluation = 100;

        Engine::insert_into_best_moves(&mut best_moves, &mut value, &mut best_moves_evaluation, piece_move, evaluation);

        assert_eq!([Option::from(Move::new(63,62,0,KING)), Option::from(piece_move), Option::from(Move::new(0,8,0, PAWN))], best_moves);
        assert_eq!(500, value);
        assert_eq!([100, 0], best_moves_evaluation);
    }

    #[test]
    fn check_insert_into_best_moves_third_move() {
        let mut best_moves = [Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())];
        let mut value = 500;
        let mut best_moves_evaluation= [0, -300];

        let piece_move = Move::new(0, 1, 0, QUEEN);
        let evaluation = -100;

        Engine::insert_into_best_moves(&mut best_moves, &mut value, &mut best_moves_evaluation, piece_move, evaluation);

        assert_eq!([Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(piece_move)], best_moves);
        assert_eq!(500, value);
        assert_eq!([0, -100], best_moves_evaluation);
    }

    #[test]
    fn check_insert_into_best_moves_weak_move() {
        let mut best_moves = [Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())];
        let mut value = 500;
        let mut best_moves_evaluation= [0, -300];

        let piece_move = Move::new(0, 1, 0, QUEEN);
        let evaluation = -600;

        Engine::insert_into_best_moves(&mut best_moves, &mut value, &mut best_moves_evaluation, piece_move, evaluation);

        assert_eq!([Option::from(Move::new(63,62,0,KING)), Option::from(Move::new(0,8,0, PAWN)), Option::from(Move::empty())], best_moves);
        assert_eq!(500, value);
        assert_eq!([0, -300], best_moves_evaluation);
    }

    #[test]
    fn bench_search_start_position() {
        let mut board = Board::new();
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(&mut board);

        let mut engine = Engine::new();

        let now = Instant::now();
        println!("{}", engine.search(&mut move_list, 15));
        let duration = now.elapsed();
        println!("search lasted for: {:?}", duration);
        println!("Best moves: {:?}", engine.transposition_table.get_from_position(&board).clone().unwrap().best_moves);
    }
    //
    // #[test]
    // fn bench_search_kiwipete_position() {
    //     let mut board = Board::new();
    //     let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    //     board.read_fen(fen);
    //     let mut move_list = MoveList::new(&mut board);
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
}