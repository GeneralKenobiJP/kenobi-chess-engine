//! Best move search algorithm
//! Builds a search tree to find the best possible move according to the evaluation algorithm
//! Uses negamax convention, alpha-beta prunning, quiescence search.

mod ordering;

use std::cmp::max;
use std::sync::atomic::{AtomicBool, Ordering};
use num_traits::real::Real;
use crate::board::Board;
use crate::evaluation::{compute_game_phase_factor, DRAW, Evaluator, MainEvaluator, PIECE_WORTH};
use crate::move_generator::{MAX_MOVES_IN_POSITION, Move, MoveList};
use crate::evaluation::{POSITIVE_INFINITY, NEGATIVE_INFINITY};
use crate::transposition_table::{RepetitionTable, Transposition, TranspositionTable};
use crate::transposition_table::NodeType::{ALPHA, BETA, EXACT, NOISY_ONLY};

const KILLER_MOVES_CAPACITY: usize = 1024;
const DEPTH_LIMIT: usize = 1024;
const ASPIRATION_MARGIN: i32 = 50;
const ASPIRATION_LIMIT: usize = 2;
const ASPIRATION_START_DEPTH: u32 = 3;
const LMR_DEPTH_LIMIT: u32 = 3;
const LMR_MOVE_NUMBER_LIMIT: usize = 3;
const NULL_MOVE_PRUNING_DEPTH_LIMIT: u32 = 4;
const NULL_MOVE_REDUCTION: u32 = 3;
const NULL_MOVE_DEPTH_SCALING_FACTOR: u32 = 4;
const NULL_MOVE_PHASE_LIMIT: i32 = 20;
const DELTA_MARGIN: i32 = 200;
const DELTA_PRUNING_PHASE_LIMIT: i32 = 20;
const STOP_CHECK_PERIOD: u32 = 1 << 12;

static STOP_FLAG: AtomicBool = AtomicBool::new(false);

macro_rules! search_alpha_beta_pruning_or_return_target {
    // ($target, $self_expr, $move_list, $depth, $alpha, $beta)
    ($target:ident, $self_expr:expr, $move_list:expr, $depth:expr, $alpha:expr, $beta:expr) => {
        match $self_expr.search_alpha_beta_prunning($move_list, $depth, $alpha, $beta) {
            Some(__v) => __v,
            None => return $target,
        }
    }
}

macro_rules! search_alpha_beta_pruning_or_return_none {
    // ($target, $self_expr, $move_list, $depth, $alpha, $beta)
    ($self_expr:expr, $move_list:expr, $depth:expr, $alpha:expr, $beta:expr) => {
        match $self_expr.search_alpha_beta_prunning($move_list, $depth, $alpha, $beta) {
            Some(__v) => __v,
            None => return None,
        }
    }
}

macro_rules! quiescence_search_or_return_none {
    // ($target, $self_expr, $move_list, $depth, $alpha, $beta)
    ($self_expr:expr, $move_list:expr, $alpha:expr, $beta:expr) => {
        match $self_expr.quiescence_search($move_list, $alpha, $beta) {
            Some(__v) => __v,
            None => return None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Engine<T: Evaluator = MainEvaluator> {
    evaluator: T,
    transposition_table: TranspositionTable,
    repetition_table: RepetitionTable,
    killer_moves: [[Option<Move>; 2]; KILLER_MOVES_CAPACITY],
    depth: u32,
    max_depth: u32,
    current_ply: u32,
    moves_buffer: [Vec<Move>; DEPTH_LIMIT],
    best_moves: [[Option<Move>; 3]; DEPTH_LIMIT],
    best_moves_evaluation: [[i32; 3]; DEPTH_LIMIT],
    guard_counter: u32
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
            max_depth: 0,
            current_ply: 0,
            moves_buffer: core::array::from_fn(|_i| Vec::with_capacity(MAX_MOVES_IN_POSITION)),
            best_moves: [[None; 3]; DEPTH_LIMIT],
            best_moves_evaluation: [[NEGATIVE_INFINITY; 3]; DEPTH_LIMIT],
            guard_counter: 0
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Engine {
            evaluator: MainEvaluator {},
            transposition_table: TranspositionTable::with_capacity(capacity),
            repetition_table: RepetitionTable::new(),
            killer_moves: [[None; 2]; KILLER_MOVES_CAPACITY],
            depth: 0,
            max_depth: 0,
            current_ply: 0,
            moves_buffer: core::array::from_fn(|_i| Vec::with_capacity(MAX_MOVES_IN_POSITION)),
            best_moves: [[None; 3]; DEPTH_LIMIT],
            best_moves_evaluation: [[NEGATIVE_INFINITY; 3]; DEPTH_LIMIT],
            guard_counter: 0
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
            max_depth: 0,
            current_ply: 0,
            moves_buffer: core::array::from_fn(|_i| Vec::with_capacity(MAX_MOVES_IN_POSITION)),
            best_moves: [[None; 3]; DEPTH_LIMIT],
            best_moves_evaluation: [[NEGATIVE_INFINITY; 3]; DEPTH_LIMIT],
            guard_counter: 0
        }
    }

    pub fn with_evaluator_and_capacity(evaluator: T, capacity: usize) -> Self {
        Engine {
            evaluator,
            transposition_table: TranspositionTable::with_capacity(capacity),
            repetition_table: RepetitionTable::new(),
            killer_moves: [[None; 2]; KILLER_MOVES_CAPACITY],
            depth: 0,
            max_depth: 0,
            current_ply: 0,
            moves_buffer: core::array::from_fn(|_i| Vec::with_capacity(MAX_MOVES_IN_POSITION)),
            best_moves: [[None; 3]; DEPTH_LIMIT],
            best_moves_evaluation: [[NEGATIVE_INFINITY; 3]; DEPTH_LIMIT],
            guard_counter: 0
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

    /// Retrieves the depth at which the engine is currently conducting a search
    /// or the depth at which the engine has conducted a search if the engine is idle
    pub fn get_current_depth(&self) -> u32 {
        self.depth
    }
    
    /// Calls search algorithm to find the best possible moves in the current situation.
    /// Searches up to the given depth.
    /// Uses the given move list to generate moves in-place and analyze the board situation.
    /// Calls the algorithm using alpha-beta prunning and quiescence search.
    ///
    /// Uses aspiration window to limit the search space.
    /// Aspiration window works as follows: we act on the assumption that the evaluation at depth x
    /// should not differ too much from the one at depth x-1, so we take alpha and beta
    /// close to the former evaluation.
    /// If the assumption turns out wrong, we retry with the alpha and beta values twice as high.
    /// We retry up to ASPIRATION_LIMIT times, afterwards - we abandon the window and take infinities.
    /// The aspiration window is turned off before the ASPIRATION_START_DEPTH.
    // #[inline(never)]
    pub fn search(&mut self, move_list: &mut MoveList, depth: u32) -> i32 {
        let board = move_list.get_board();
        let transposition_entry = self.transposition_table.get_from_zobrist(board.zobrist);

        // Check if we have a proper entry in the transposition table
        if let Some(transposition) = transposition_entry {
            if transposition.depth >= depth && transposition.node_type == EXACT {
                return transposition.value;
            }
        }

        self.current_ply = board.plies;

        // Clear best moves (and evaluations) that were used up in the previous search
        for i in 0..self.max_depth as usize {
            self.best_moves[i] = [None; 3];
            self.best_moves_evaluation[i] = [NEGATIVE_INFINITY; 3];
        }

        self.max_depth = depth;

        let mut value = 0;
        for current_depth in 1..depth+1 {
            self.depth = current_depth;

            // Check if we should use an aspiration window
            let (mut alpha, mut beta) = if current_depth < ASPIRATION_START_DEPTH {
                (NEGATIVE_INFINITY, POSITIVE_INFINITY)
            } else {
                let delta = ASPIRATION_MARGIN;
                (value - delta, value + delta)
            };

            // value = self.search_alpha_beta_prunning(move_list, current_depth, alpha, beta);
            // value = match self.search_alpha_beta_prunning(move_list, current_depth, alpha, beta) {
            //     Some(val) => val,
            //     None => return value
            // };
            value = search_alpha_beta_pruning_or_return_target!(value, self, move_list, current_depth, alpha, beta);

            // Increase the aspiration window
            let mut idx = 0;
            while (value <= alpha || value >= beta) && idx < ASPIRATION_LIMIT {
                // value = self.search_alpha_beta_prunning(move_list, current_depth, alpha, beta);
                value = search_alpha_beta_pruning_or_return_target!(value, self, move_list, current_depth, alpha, beta);
                alpha = alpha.saturating_mul(2);
                beta = beta.saturating_mul(2);
                idx += 1;
            }

            // Abandon the aspiration window
            value = if value <= alpha || value >= beta {
                // self.search_alpha_beta_prunning(move_list, current_depth, NEGATIVE_INFINITY, POSITIVE_INFINITY)
                search_alpha_beta_pruning_or_return_target!(value, self, move_list, current_depth, NEGATIVE_INFINITY, POSITIVE_INFINITY)
            } else {
                value
            };

            if STOP_FLAG.load(Ordering::SeqCst) {
                return value;
            }
        }

        value
    }

    /// Applies late move reduction, given the remaining depth, and the move number (in ordering),
    /// and outputs the reduced depth.
    ///
    /// The rationale is that the later moves in the sorted ordering (so those we deem worse)
    /// do not look promising, and therefore we search them only with a reduced depth to limit computation.
    /// If the search fails high, however, we retry the search with the full ab window and full depth.
    ///
    /// The formula used is: R = 0.5 + 0.2 * log2(depth)^1.3 * log2(move_number).
    /// All logarithms operate on integers exclusively to accelerate computation.
    ///
    /// Should be called after checking for depth and move number constraints.
    fn apply_late_move_reduction(depth: u32, move_number: usize) -> u32 {
        (depth - 1).saturating_sub((0.5 + ((depth.ilog2() as f32).powf(1.3) * move_number.ilog2() as f32 * 0.2)) as u32)
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
    ///
    /// Implements null move pruning - in an unchecked non-(pawn-endgame) situation,
    /// we perform a null move, under an assumption that in almost all situations doing
    /// something is more beneficial than doing nothing, to observe the conservative
    /// estimate of the opponent's position - if this search causes a beta cutoff, we return
    /// otherwise - we retry with a normal search.
    ///
    /// Implements late move reduction - the later the move is in the heuristically ordered list
    /// of moves, the lesser the depth of search at this node.
    // #[inline(never)]
    fn search_alpha_beta_prunning(&mut self, move_list: &mut MoveList, depth: u32, mut alpha: i32, beta: i32) -> Option<i32> {
        if (self.guard_counter & STOP_CHECK_PERIOD) != 0 {
            if STOP_FLAG.load(Ordering::SeqCst) {
                return None;
            }
        }

        let zobrist = move_list.get_board().zobrist;

        if self.repetition_table.visit_position(zobrist) {
            self.repetition_table.unvisit_position(zobrist);
            return Some(DRAW);
        }

        let original_alpha=  alpha;

        let transposition_entry = self.transposition_table.get_from_zobrist(zobrist);
        let mut skip_null = false;

        // Check if we have a proper entry in the transposition table
        if let Some(transposition) = transposition_entry {
            if transposition.depth >= depth {
                if transposition.node_type == EXACT { self.repetition_table.unvisit_position(zobrist); return Some(transposition.value); }
                if transposition.node_type == ALPHA && transposition.value <= alpha { self.repetition_table.unvisit_position(zobrist); return Some(transposition.value); } // Our alpha cut-off is even bigger than it was for the put operation
                if /*transposition.node_type == BETA*/ transposition.value >= beta { self.repetition_table.unvisit_position(zobrist); return Some(transposition.value); } // Our beta cut-off is even smaller than it was for the put operation
            }
            else {
                // We do not want to perform null move pruning on an EXACT or BETA node
                if transposition.node_type == EXACT || transposition.node_type == BETA { skip_null = true; }
            }
        }

        if depth == 0 {
            self.repetition_table.unvisit_position(zobrist);
            return self.quiescence_search(move_list, -beta, -alpha);
        }

        // Index for the best moves buffer.
        // remaining depth = total depth of search -> buffer_index = 0
        // remaining depth = total depth of search - 1 -> buffer_index = 1
        // (This is not entirely a true description due to lmr and null move pruning,
        // but serves as a good intuition)
        let buffer_index = (move_list.get_board().plies - self.current_ply) as usize;

        // Null move pruning
        // The assumption is that usually doing anything is better than (hypothetically) doing nothing,
        // so a null move provides a conservative lower bound estimate on our position.
        // This serves as a way to limit the computation needed -
        // if the opponent cannot beat beta even with us sitting idle,
        // then this position cannot fail-high, i.e. opponent will choose a different play
        // that will lead to a different position, therefore examination of this node is unnecessary.
        //
        // Since this is an estimate anyway, we do not need to perform full-depth search,
        // instead we reduce depth by NULL_MOVE_REDUCTION.
        //
        // Used only if not in check and the chance for a zugzwang position is minimal,
        // i.e. there is some number of pieces left on the board, ideally stronger than pawns,
        // as otherwise the null move observation may not necessarily hold true.
        // (Zugzwang is a situation, where we are forced to make an unfavourable move, whereas
        // waiting idly would be beneficial)
        let board = move_list.get_board();
        if !skip_null && depth >= NULL_MOVE_PRUNING_DEPTH_LIMIT && beta < POSITIVE_INFINITY && !move_list.is_in_check() &&
            !board.is_pawn_and_king_endgame() && compute_game_phase_factor(&board.piece_counter) >= NULL_MOVE_PHASE_LIMIT {
            let reduced_depth = depth.saturating_sub(1 + NULL_MOVE_REDUCTION + depth / NULL_MOVE_DEPTH_SCALING_FACTOR);

            move_list.make_null_move();
            let value = -search_alpha_beta_pruning_or_return_none!(self, move_list, reduced_depth, alpha, beta);
            // let value = -self.search_alpha_beta_prunning(move_list, reduced_depth, -beta, -beta + 1);
            move_list.unmake_null_move();

            if value >= beta {
                self.repetition_table.unvisit_position(zobrist);
                self.transposition_table.put_transposition_with_validation(&Transposition::from_zobrist(zobrist, reduced_depth, value, &self.best_moves[buffer_index], BETA));
                return Some(beta);
            }
        }

        move_list.generate_moves();
        self.order_moves(move_list);

        // Efficient copying of move list
        self.moves_buffer[buffer_index].clear();
        self.moves_buffer[buffer_index].extend_from_slice(&move_list.get_moves());

        let mut cutoff_move = Move::empty();
        let mut used_depth = depth;

        for i in 0..self.moves_buffer[buffer_index].len()
        {
            let piece_move = { let current_buffer = &self.moves_buffer[buffer_index];  current_buffer[i]};

            move_list.make_move(&piece_move);
            if !move_list.is_opponent_in_check() {

                // Do we want to apply late move reduction
                let move_evaluation = if depth < LMR_DEPTH_LIMIT || i < LMR_MOVE_NUMBER_LIMIT {
                    -search_alpha_beta_pruning_or_return_none!(self, move_list, depth - 1, -beta, -alpha)
                    // -self.search_alpha_beta_prunning(move_list, depth - 1, -beta, -alpha)
                } else {
                    // Late Move Reduction
                    let lmr_depth = Self::apply_late_move_reduction(depth, i + 1);
                    let move_evaluation = -search_alpha_beta_pruning_or_return_none!(self, move_list, lmr_depth, -alpha-1, -alpha);
                    // let move_evaluation = -self.search_alpha_beta_prunning(move_list, lmr_depth, -alpha-1, -alpha);
                    if move_evaluation > alpha {
                        // LMR failed high - apply full window instead
                        used_depth = depth;
                        -search_alpha_beta_pruning_or_return_none!(self, move_list, depth - 1, -beta, -alpha)
                        // -self.search_alpha_beta_prunning(move_list, depth - 1, -beta, -alpha)
                    }
                    else {
                        used_depth = lmr_depth + 1;
                        move_evaluation
                    }
                };

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
            return if move_list.is_in_check() { Some(NEGATIVE_INFINITY) } else { Some(DRAW) }
        }

        // update the transposition table
        let node_type = if self.best_moves_evaluation[buffer_index][0] <= original_alpha { ALPHA }
            else if self.best_moves_evaluation[buffer_index][0] >= beta { self.store_killer_move(&cutoff_move, move_list.get_board().plies as usize); BETA } else { EXACT };
        self.transposition_table.put_transposition_with_validation(&Transposition::from_zobrist(zobrist, used_depth, self.best_moves_evaluation[buffer_index][0], &self.best_moves[buffer_index], node_type));

        self.repetition_table.unvisit_position(zobrist);

        Some(self.best_moves_evaluation[buffer_index][0])
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

    /// Delta pruning checks if it is worth at all to examine this particular node
    /// during quiescence search.
    /// The idea is as follows:
    /// If we are making a capture, and the worth of the captured piece with some safety margin
    /// will not raise alpha, then evaluating this capture is useless, we prune the node instead.
    /// For the sake of conservatism, we are not pruning any non-capture noisy moves.
    ///
    /// Parameters:
    ///     - piece_move - the move we are evaluating
    ///     - board - the board object
    ///     - alpha - difference between the alpha (lower bound) we are trying to raise and
    /// the current value of the node
    ///     - game_phase - the metric of the midgame vs. endgame heuristic. Precomputed for
    /// the sake of speed. We do NOT want to perform delta pruning in the endgame, as it may
    /// cause the engine to be blind to some low material endgames.
    fn delta_pruning(piece_move: &Move, board: &Board, diff: i32, game_phase: i32) -> bool {
        if diff == NEGATIVE_INFINITY {
            return false;
        }
        if game_phase <= DELTA_PRUNING_PHASE_LIMIT {
            return false;
        }

        let target_square = piece_move.target;
        let target_piece = board.get_piece_from_square_by_player(
            target_square, board.inactive_player as usize);

        if target_piece.is_none() { return false; }

        return PIECE_WORTH[target_piece.unwrap() as usize - 1] + DELTA_MARGIN <= diff;
    }

    /// Uses alpha-beta prunning to find the best possible move in the search tree.
    /// Searches until it finds a 'quiet' position, where no captures, promotions or checks can be made.
    /// It avoids the horizon effect by not ignoring threats at depth zero.
    /// Uses the given move list to generate moves in-place and analyze the board situation.
    /// Alpha - minimum score the current player is assured of (we found a move of at least this value earlier at this depth)
    /// Beta - maximum score the opponent is assured of (the best value the parent node recorded)
    // #[inline(never)]
    fn quiescence_search(&mut self, move_list: &mut MoveList, mut alpha: i32, beta: i32) -> Option<i32> {
        let zobrist = move_list.get_board().zobrist;
        
        if self.repetition_table.visit_position(zobrist) {
            self.repetition_table.unvisit_position(zobrist);
            return Some(DRAW);
        }

        let original_alpha = alpha;

        let transposition_entry = self.transposition_table.get_from_zobrist(zobrist);
        // Check if we have a proper entry in the transposition table
        if let Some(transposition) = transposition_entry {
            if transposition.node_type == EXACT { self.repetition_table.unvisit_position(zobrist); return Some(transposition.value); }
            if transposition.node_type == ALPHA && transposition.value <= alpha { self.repetition_table.unvisit_position(zobrist); return Some(transposition.value); } // Our alpha cut-off is even bigger than it was for the put operation
            if /*transposition.node_type == BETA*/ transposition.value >= beta { self.repetition_table.unvisit_position(zobrist); return Some(transposition.value); } // Our beta cut-off is even smaller than it was for the put operation
        }

        move_list.generate_noisy_moves();

        if move_list.get_moves().len() == 0 {
            self.repetition_table.unvisit_position(zobrist);
            return Some(T::evaluate(move_list.get_board()));
        }

        self.order_moves(move_list);

        // Index for the best moves buffer.
        // We subtract the difference between the board's ply and the original ply.
        let buffer_index = (move_list.get_board().plies - self.current_ply) as usize;
        self.max_depth = max(self.max_depth, buffer_index as u32);

        // Efficient copying of the move list
        self.moves_buffer[buffer_index].clear();
        self.moves_buffer[buffer_index].extend_from_slice(&move_list.get_moves());

        // Precompute game phase for the sake of delta pruning
        let board = move_list.get_board();
        let game_phase = compute_game_phase_factor(&board.piece_counter);

        for i in 0..self.moves_buffer[buffer_index].len()
        {
            let piece_move = { let current_buffer = &self.moves_buffer[buffer_index];  current_buffer[i]};

            if Self::delta_pruning(&piece_move, move_list.get_board(),
                                   alpha.saturating_sub(self.best_moves_evaluation[buffer_index][0]), game_phase) {
                continue;
            }

            move_list.make_move(&piece_move);
            if !move_list.is_opponent_in_check() {
                let move_evaluation = -quiescence_search_or_return_none!(self, move_list, -beta, -alpha);
                // let move_evaluation = -self.quiescence_search(move_list, -beta, -alpha);

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
            self.repetition_table.unvisit_position(zobrist);
            return if move_list.is_in_check() { Some(NEGATIVE_INFINITY) } else { Some(DRAW) }
        }

        // update the transposition table
        let node_type = if self.best_moves_evaluation[buffer_index][0] <= original_alpha { ALPHA } else if self.best_moves_evaluation[buffer_index][0] >= beta { BETA } else { NOISY_ONLY };
        self.transposition_table.put_transposition_with_validation(&Transposition::from_zobrist(zobrist, 0, self.best_moves_evaluation[buffer_index][0], &self.best_moves[buffer_index], node_type));

        self.repetition_table.unvisit_position(zobrist);

        Some(self.best_moves_evaluation[buffer_index][0])
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
    use crate::piece::Piece::{KING, KNIGHT, PAWN, QUEEN, ROOK};
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

        assert_eq!(DRAW, engine.search_alpha_beta_prunning(&mut move_list, 1, NEGATIVE_INFINITY, POSITIVE_INFINITY).unwrap());
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

        assert_eq!(NEGATIVE_INFINITY, engine.search_alpha_beta_prunning(&mut move_list, 1, NEGATIVE_INFINITY, POSITIVE_INFINITY).unwrap());
        assert!(engine.repetition_table.is_empty());

        let mut board = Board::new();
        let fen = "7k/6QQ/8/8/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::new();
        engine.depth = 1;

        assert_eq!(POSITIVE_INFINITY, engine.search_alpha_beta_prunning(&mut move_list, 1, NEGATIVE_INFINITY, POSITIVE_INFINITY).unwrap());
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
        assert_eq!(-100, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY).unwrap());
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
        assert_eq!(-100, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY).unwrap());
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
        assert_eq!(-700, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY).unwrap());
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

        assert_ne!(DRAW, engine.search_alpha_beta_prunning(&mut move_list, 0, NEGATIVE_INFINITY, POSITIVE_INFINITY).unwrap());
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::with_evaluator(MockMaterialEvaluator{});
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_ne!(DRAW, engine.search_alpha_beta_prunning(&mut move_list, 0, NEGATIVE_INFINITY, POSITIVE_INFINITY).unwrap());
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::with_evaluator(MockMaterialEvaluator{});
        engine.repetition_table.visit_position(move_list.get_board().zobrist);
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_eq!(DRAW, engine.search_alpha_beta_prunning(&mut move_list, 0, NEGATIVE_INFINITY, POSITIVE_INFINITY).unwrap());
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

        assert_ne!(DRAW, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY).unwrap());
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::with_evaluator(MockMaterialEvaluator {});
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_ne!(DRAW, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY).unwrap());
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::with_evaluator(MockMaterialEvaluator {});
        engine.repetition_table.visit_position(move_list.get_board().zobrist);
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_eq!(DRAW, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY).unwrap());
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

    #[test]
    fn check_tt_stores_own_copy_of_move() {
        let mut engine = Engine::new();
        let mut ml = MoveList::from_fen(START_POSITION);

        // generate a fake best move into engine.best_moves[0]
        engine.best_moves[0][0] = Some(Move::new(48, 40, 0, PAWN));
        let current_board = ml.get_board().clone();
        let zob = current_board.zobrist;

        // store transposition using this array
        engine.transposition_table.put_transposition(&Transposition::from_zobrist(zob, 1, 0, &engine.best_moves[0], EXACT));

        // mutate engine.best_moves[0][0]
        engine.best_moves[0][0] = Some(Move::new(1, 18, 0, KNIGHT));

        // get from TT
        if let Some(t) = engine.transposition_table.get_from_zobrist(zob) {
            assert_eq!(t.best_moves[0], Some(Move::new(48, 40, 0, PAWN)), "TT did not preserve its copy of the move");
        } else {
            panic!("TT entry missing");
        }
    }

    #[test]
    fn check_sequence() {
        let mut engine = Engine::new();
        let mut move_list = MoveList::from_fen(START_POSITION);

        engine.search(&mut move_list, 7);
        move_list.make_move(&Move::new(1, 18, 0, KNIGHT));
        engine.search(&mut move_list, 7);
        move_list.make_move(&Move::new(48, 40, 0, PAWN));
        let board = move_list.get_board().clone();
        engine.search(&mut move_list, 7);
        assert!(engine.get_best_move(&board).origin < 24);
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

        engine.depth = 3;
        engine.search_alpha_beta_prunning(&mut move_list, 3, NEGATIVE_INFINITY, POSITIVE_INFINITY);
        // There are obvious killer moves when we try to move our queen so that it can be captured by a pawn
        assert!(!engine.killer_moves[1][0].is_none());
        assert!(!engine.killer_moves[1][1].is_none());
    }

    #[test]
    fn test_delta_pruning() {
        let mut board = Board::new();
        let fen = "4k3/8/8/8/8/8/6qp/4K2Q w - - 0 1";
        board.read_fen(fen);

        let game_phase = 10;
        let piece_move = Move::new(0, 8, 0, QUEEN);
        let diff = 500;

        assert!(!Engine::<MockMaterialEvaluator>::delta_pruning(&piece_move, &board, diff, game_phase));

        let game_phase = 70;
        assert!(Engine::<MockMaterialEvaluator>::delta_pruning(&piece_move, &board, diff, game_phase));

        let diff = 299;
        assert!(!Engine::<MockMaterialEvaluator>::delta_pruning(&piece_move, &board, diff, game_phase));

        let piece_move = Move::new(0, 9, 0, QUEEN);
        assert!(!Engine::<MockMaterialEvaluator>::delta_pruning(&piece_move, &board, diff, game_phase));

        let diff = 1100;
        assert!(Engine::<MockMaterialEvaluator>::delta_pruning(&piece_move, &board, diff, game_phase));
    }
}