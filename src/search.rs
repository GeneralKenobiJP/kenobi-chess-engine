//! Best move search algorithm
//! Builds a search tree to find the best possible move according to the evaluation algorithm
//! Uses negamax convention, alpha-beta pruning, quiescence search.

mod ordering;

use std::cmp::max;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
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
const STOP_CHECK_MASK: u32 = STOP_CHECK_PERIOD - 1;

static STOP_FLAG: AtomicBool = AtomicBool::new(false);

/// Per-search state shared between the UCI control thread and the search worker.
/// It deliberately lives outside `Engine`, so separate engine instances cannot
/// accidentally stop one another.
#[derive(Debug)]
pub struct SearchControl {
    stop: AtomicBool,
    nodes: AtomicU64,
    node_limit: Option<u64>,
    deadline: Mutex<Option<Instant>>,
}

impl SearchControl {
    pub fn new(node_limit: Option<u64>, deadline: Option<Instant>) -> Self {
        Self {
            stop: AtomicBool::new(false),
            nodes: AtomicU64::new(0),
            node_limit,
            deadline: Mutex::new(deadline),
        }
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub fn set_deadline_from_now(&self, duration: Duration) {
        *self.deadline.lock().unwrap() = Some(Instant::now() + duration);
    }

    pub fn get_nodes(&self) -> u64 {
        self.nodes.load(Ordering::Relaxed)
    }

    fn inc_node(&self, check_clock: bool) -> bool {
        let nodes = self.nodes.fetch_add(1, Ordering::Relaxed) + 1;
        self.is_stopped_at(nodes, check_clock)
    }

    fn is_stopped(&self, check_clock: bool) -> bool {
        self.is_stopped_at(self.nodes.load(Ordering::Relaxed), check_clock)
    }

    fn is_stopped_at(&self, nodes: u64, check_clock: bool) -> bool {
        let externally_stopped = self.stop.load(Ordering::Relaxed);
        let node_limit_reached = self.node_limit.map_or(false, |limit| nodes >= limit);
        let deadline_reached = check_clock
            && self
                .deadline
                .lock()
                .unwrap()
                .map_or(false, |deadline| Instant::now() >= deadline);

        externally_stopped || node_limit_reached || deadline_reached
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
    guard_counter: u32,
    last_root_best: Option<Move>,
    last_root_score: i32,
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
            guard_counter: 0,
            last_root_best: None,
            last_root_score: 0,
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
            guard_counter: 0,
            last_root_best: None,
            last_root_score: 0,
        }
    }
}

impl Engine {
    /// Sets stop flag to a given boolean.
    /// Stop flag is used by engine to
    /// indicate whether further search should be aborted or not.
    pub fn set_stop_flag(flag: bool) {
        STOP_FLAG.store(flag, Ordering::Relaxed);
    }

    /// Gets stop flag.
    /// Stop flag is used by engine to
    /// indicate whether further search should be aborted or not.
    pub fn get_stop_flag() -> bool {
        STOP_FLAG.load(Ordering::Relaxed)
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
            guard_counter: 0,
            last_root_best: None,
            last_root_score: 0,
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
            guard_counter: 0,
            last_root_best: None,
            last_root_score: 0,
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

    /// Returns a move only after at least one root iteration completed.
    pub fn try_get_best_move(&self, board: &Board) -> Option<Move> {
        self.last_root_best.or_else(|| {
            self.transposition_table
                .get_from_zobrist(board.zobrist).clone()
                .and_then(|entry| entry.best_moves[0])
        })
    }

    pub fn get_last_root_score(&self) -> Option<i32> {
        self.last_root_best.map(|_| self.last_root_score)
    }

    pub fn get_transposition(&self, board: &Board) -> Option<Transposition> {
        self.transposition_table.get_from_zobrist(board.zobrist).clone()
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
    /// Calls the algorithm using alpha-beta pruning and quiescence search.
    ///
    /// Uses aspiration window to limit the search space.
    /// Aspiration window works as follows: we act on the assumption that the evaluation at depth x
    /// should not differ too much from the one at depth x-1, so we take alpha and beta
    /// close to the former evaluation.
    /// If the assumption turns out wrong, we retry with the alpha and beta values twice as high.
    /// We retry up to ASPIRATION_LIMIT times, afterwards - we abandon the window and take infinities.
    /// The aspiration window is turned off before the ASPIRATION_START_DEPTH.
    // #[inline(never)]
    pub fn search(
        &mut self, 
        move_list: &mut MoveList,
        depth: u32, 
        control: &SearchControl, 
        root_moves: Option<&[Move]>
    ) -> i32 {
        let depth = depth.min((DEPTH_LIMIT - 2) as u32);
        let board = move_list.get_board();
        let transposition_entry = self.transposition_table.get_from_zobrist(board.zobrist);

        // Check if we have a proper entry in the transposition table
        if root_moves.is_none() {
            if let Some(transposition) = transposition_entry {
                if transposition.depth >= depth && transposition.node_type == EXACT {
                    self.last_root_best = transposition.best_moves[0];
                    self.last_root_score = transposition.value;
                    return transposition.value;
                }
            }
        }

        self.current_ply = board.plies;
        self.guard_counter = 0;
        self.last_root_best = None;
        self.last_root_score = T::evaluate(board);

        // Clear best moves (and evaluations) that were used up in the previous search
        for i in 0..self.max_depth as usize {
            self.best_moves[i] = [None; 3];
            self.best_moves_evaluation[i] = [NEGATIVE_INFINITY; 3];
        }

        self.max_depth = depth;

        let mut value = self.last_root_score;
        for current_depth in 1..=depth {
            self.depth = current_depth;

            let previous_value = value;
            let mut delta = ASPIRATION_MARGIN;
            let mut retries = 0;

            loop {
                let (alpha, beta) = if current_depth < ASPIRATION_START_DEPTH {
                    (NEGATIVE_INFINITY, POSITIVE_INFINITY)
                } else {
                    (
                        previous_value.saturating_sub(delta),
                        previous_value.saturating_add(delta),
                    )
                };

                let Some(candidate) = self.search_alpha_beta_pruning(
                    move_list,
                    current_depth,
                    alpha,
                    beta,
                    control,
                    root_moves,
                ) else {
                    return value;
                };

                if candidate > alpha && candidate < beta {
                    value = candidate;
                    break;
                }

                retries += 1;
                if retries > ASPIRATION_LIMIT || current_depth < ASPIRATION_START_DEPTH {
                    let Some(candidate) = self.search_alpha_beta_pruning(
                        move_list,
                        current_depth,
                        NEGATIVE_INFINITY,
                        POSITIVE_INFINITY,
                        control,
                        root_moves,
                    ) else {
                        return value;
                    };
                    value = candidate;
                    break;
                }
                delta = delta.saturating_mul(2);
            }

            self.last_root_best = self.best_moves[0][0];
            self.last_root_score = value;
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
    /// Calls the algorithm using alpha-beta pruning.
    /// NOTE: Does NOT use quiescence search and therefore is inferior to the search() function
    ///     Should be used mainly for testing.
    pub fn search_no_quiescence(&mut self, move_list: &mut MoveList, depth: u32) -> i32 {
        self.search_alpha_beta_pruning_naive(move_list, depth, NEGATIVE_INFINITY, POSITIVE_INFINITY)
    }

    /// Uses alpha-beta pruning to find the best possible move in the search tree.
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
    fn search_alpha_beta_pruning(
        &mut self,
        move_list: &mut MoveList,
        depth: u32,
        mut alpha: i32,
        beta: i32,
        control: &SearchControl,
        root_moves: Option<&[Move]>,
    ) -> Option<i32> {
        self.guard_counter = self.guard_counter.wrapping_add(1);
        if control.inc_node((self.guard_counter & STOP_CHECK_MASK) == 0) {
            return None;
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
        if root_moves.is_none() {
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
        }

        if depth == 0 {
            self.repetition_table.unvisit_position(zobrist);
            return self.quiescence_search(move_list, -beta, -alpha, control);
        }

        // Index for the best moves buffer.
        // remaining depth = total depth of search -> buffer_index = 0
        // remaining depth = total depth of search - 1 -> buffer_index = 1
        // (This is not entirely a true description due to lmr and null move pruning,
        // but serves as a good intuition)
        let buffer_index = (move_list.get_board().plies - self.current_ply) as usize;
        if buffer_index >= DEPTH_LIMIT - 1 {
            self.repetition_table.unvisit_position(zobrist);
            return Some(T::evaluate(move_list.get_board()));
        }
        self.best_moves[buffer_index] = [None; 3];
        self.best_moves_evaluation[buffer_index] = [NEGATIVE_INFINITY; 3];

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
            let child = self.search_alpha_beta_pruning(
                move_list,
                reduced_depth,
                -beta,
                -beta + 1,
                control,
                None,
            );
            move_list.unmake_null_move();
            let value = match child {
                Some(value) => -value,
                None => {
                    self.repetition_table.unvisit_position(zobrist);
                    return None;
                }
            };

            if control.is_stopped(false) {
                self.repetition_table.unvisit_position(zobrist);
                return None;
            }

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
        if buffer_index == 0 {
            if let Some(allowed) = root_moves {
                self.moves_buffer[buffer_index].retain(|piece_move| allowed.contains(piece_move));
            }
        }

        let mut cutoff_move = Move::empty();

        for i in 0..self.moves_buffer[buffer_index].len()
        {
            let piece_move = { let current_buffer = &self.moves_buffer[buffer_index];  current_buffer[i]};

            move_list.make_move(&piece_move);
            let legal = !move_list.is_opponent_in_check();
            let child_result = if !legal {
                Some(0)
            } else if depth < LMR_DEPTH_LIMIT || i < LMR_MOVE_NUMBER_LIMIT {
                self.search_alpha_beta_pruning(
                    move_list,
                    depth - 1,
                    -beta,
                    -alpha,
                    control,
                    None,
                )
                .map(|value| -value)
            } else {
                let lmr_depth = Self::apply_late_move_reduction(depth, i + 1);
                match self.search_alpha_beta_pruning(
                    move_list,
                    lmr_depth,
                    -alpha - 1,
                    -alpha,
                    control,
                    None,
                ) {
                    Some(reduced_value) if -reduced_value > alpha => {
                        self.search_alpha_beta_pruning(
                            move_list,
                            depth - 1,
                            -beta,
                            -alpha,
                            control,
                            None,
                        )
                        .map(|value| -value)
                    }
                    Some(reduced_value) => {
                        Some(-reduced_value)
                    }
                    None => None,
                }
            };
            move_list.unmake_move(&piece_move);

            if legal {
                let move_evaluation = match child_result {
                    Some(value) => value,
                    None => {
                        self.repetition_table.unvisit_position(zobrist);
                        return None;
                    }
                };
                self.insert_into_best_moves(piece_move, move_evaluation, buffer_index);
            }

            if control.is_stopped(false) {
                self.repetition_table.unvisit_position(zobrist);
                return None;
            }

            alpha = alpha.max(self.best_moves_evaluation[buffer_index][0]);
            if alpha >= beta { cutoff_move = piece_move; break; }
        }

        if self.best_moves[buffer_index][0] == None {
            self.repetition_table.unvisit_position(zobrist);
            return if move_list.is_in_check() { Some(NEGATIVE_INFINITY) } else { Some(DRAW) }
        }

        // update the transposition table
        let node_type = if self.best_moves_evaluation[buffer_index][0] <= original_alpha { ALPHA }
            else if self.best_moves_evaluation[buffer_index][0] >= beta { self.store_killer_move(&cutoff_move, buffer_index); BETA } else { EXACT };
        self.transposition_table.put_transposition_with_validation(&Transposition::from_zobrist(zobrist, depth, self.best_moves_evaluation[buffer_index][0], &self.best_moves[buffer_index], node_type));

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

        let Some(target_piece) = target_piece else { return false; };

        PIECE_WORTH[target_piece as usize - 1] + DELTA_MARGIN <= diff
    }

    /// Uses alpha-beta pruning to find the best possible move in the search tree.
    /// Searches until it finds a 'quiet' position, where no captures, promotions or checks can be made.
    /// It avoids the horizon effect by not ignoring threats at depth zero.
    /// Uses the given move list to generate moves in-place and analyze the board situation.
    /// Alpha - minimum score the current player is assured of (we found a move of at least this value earlier at this depth)
    /// Beta - maximum score the opponent is assured of (the best value the parent node recorded)
    // #[inline(never)]
    fn quiescence_search(
        &mut self,
        move_list: &mut MoveList,
        mut alpha: i32,
        beta: i32,
        control: &SearchControl,
    ) -> Option<i32> {
        self.guard_counter = self.guard_counter.wrapping_add(1);
        if control.inc_node((self.guard_counter & STOP_CHECK_MASK) == 0) {
            return None;
        }

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

        let in_check = move_list.is_in_check();
        let stand_pat = if !in_check {
            let value = T::evaluate(move_list.get_board());
            if value >= beta {
                self.repetition_table.unvisit_position(zobrist);
                return Some(beta);
            }
            alpha = alpha.max(value);
            move_list.generate_noisy_moves();
            Some(value)
        } else {
            // In check, quiet evasions are not optional; searching captures only
            // can falsely report checkmate.
            move_list.generate_moves();
            None
        };

        if move_list.get_moves().len() == 0 {
            self.repetition_table.unvisit_position(zobrist);
            return if in_check {
                Some(NEGATIVE_INFINITY)
            } else {
                Some(alpha)
            };
        }

        self.order_moves(move_list);

        // Index for the best moves buffer.
        // We subtract the difference between the board's ply and the original ply.
        let buffer_index = (move_list.get_board().plies - self.current_ply) as usize;
        if buffer_index >= DEPTH_LIMIT - 1 {
            self.repetition_table.unvisit_position(zobrist);
            return Some(alpha);
        }
        self.max_depth = max(self.max_depth, buffer_index as u32);
        self.best_moves[buffer_index] = [None; 3];
        self.best_moves_evaluation[buffer_index] = [NEGATIVE_INFINITY; 3];

        // Efficient copying of the move list
        self.moves_buffer[buffer_index].clear();
        self.moves_buffer[buffer_index].extend_from_slice(&move_list.get_moves());

        // Precompute game phase for the sake of delta pruning
        let board = move_list.get_board();
        let game_phase = compute_game_phase_factor(&board.piece_counter);

        for i in 0..self.moves_buffer[buffer_index].len()
        {
            let piece_move = { let current_buffer = &self.moves_buffer[buffer_index];  current_buffer[i]};

            if !in_check && Self::delta_pruning(&piece_move, move_list.get_board(),
                                   alpha.saturating_sub(stand_pat.unwrap()), game_phase) {
                continue;
            }

            move_list.make_move(&piece_move);
            let legal = !move_list.is_opponent_in_check();
            let child = if legal {
                self.quiescence_search(move_list, -beta, -alpha, control)
            } else {
                Some(0)
            };
            move_list.unmake_move(&piece_move);

            if legal {
                let move_evaluation = match child {
                    Some(value) => -value,
                    None => {
                        self.repetition_table.unvisit_position(zobrist);
                        return None;
                    }
                };
                self.insert_into_best_moves(piece_move, move_evaluation, buffer_index);
            }

            if control.is_stopped(false) {
                self.repetition_table.unvisit_position(zobrist);
                return None;
            }

            alpha = alpha.max(self.best_moves_evaluation[buffer_index][0]);
            if alpha >= beta { break; }
        }

        if self.best_moves[buffer_index][0] == None {
            self.repetition_table.unvisit_position(zobrist);
            return if in_check { Some(NEGATIVE_INFINITY) } else { Some(alpha) }
        }

        // update the transposition table
        let node_type = if alpha <= original_alpha {
            ALPHA
        } else if alpha >= beta {
            BETA
        } else {
            NOISY_ONLY
        };
        self.transposition_table.put_transposition_with_validation(&Transposition::from_zobrist(
            zobrist,
            0,
            alpha,
            &self.best_moves[buffer_index],
            node_type,
        ));

        self.repetition_table.unvisit_position(zobrist);

        Some(alpha)
    }

    /// Uses alpha-beta pruning to find the best possible move in the search tree.
    /// Searches up to the given depth.
    /// Uses the given move list to generate moves in-place and analyze the board situation.
    /// Alpha - minimum score the current player is assured of (we found a move of at least this value earlier at this depth)
    /// Beta - maximum score the opponent is assured of (the best value the parent node recorded)
    /// NOTE: Does NOT use quiescence search and therefore is inferior to the search_alpha_beta_pruning() function
    ///     Should be used mainly for testing.
    fn search_alpha_beta_pruning_naive(&mut self, move_list: &mut MoveList, depth: u32, mut alpha: i32, beta: i32) -> i32 {
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
                    -self.search_alpha_beta_pruning_naive(move_list, depth - 1, -beta, -alpha)
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
    /// NOTE: Uses NEITHER quiescence search NOR alpha-beta pruning
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

        let search_control = SearchControl::new(None, None);

        // assert_eq!(0, engine.search(&mut move_list, 0));
        assert_eq!(0, engine.search(&mut move_list, 1, &search_control, None));
        assert_eq!(0, engine.search(&mut move_list, 2, &search_control, None));
        assert_eq!(0, engine.search(&mut move_list, 3, &search_control, None));
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
        let search_control = SearchControl::new(None, None);

        println!("depth 0");
        assert_eq!(0, engine.search(&mut move_list, 0, &search_control, None));
        println!("depth 1");
        assert_eq!(0, engine.search(&mut move_list, 1, &search_control, None));
        println!("depth 2");
        assert_eq!(0, engine.search(&mut move_list, 2, &search_control, None));
    }

    #[test]
    fn test_alpha_beta_pruning_best_move_first() {
        let mut board = Board::new();
        let fen = "4k3/8/8/8/8/5PP1/1q6/P3K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});

        assert_eq!(-600, engine.search_no_quiescence(&mut move_list, 0));
        assert_eq!(300, engine.search_no_quiescence(&mut move_list, 1));
    }

    #[test]
    fn test_alpha_beta_pruning_best_move_last() {
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
        let search_control = SearchControl::new(None, None);

        assert_eq!(DRAW, engine.search_alpha_beta_pruning(&mut move_list, 1, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control, None).unwrap());
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
        let search_control = SearchControl::new(None, None);

        assert_eq!(NEGATIVE_INFINITY, engine.search_alpha_beta_pruning(&mut move_list, 1, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control, None).unwrap());
        assert!(engine.repetition_table.is_empty());

        let mut board = Board::new();
        let fen = "7k/6QQ/8/8/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::new();
        engine.depth = 1;
        let search_control = SearchControl::new(None, None);

        assert_eq!(POSITIVE_INFINITY, engine.search_alpha_beta_pruning(&mut move_list, 1, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control, None).unwrap());
        assert!(engine.repetition_table.is_empty());
    }

    #[test]
    fn test_quiescence_search_1() {
        let mut board = Board::new();
        let fen = "8/3pk3/3p4/2P5/8/8/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});
        let search_control = SearchControl::new(None, None);

        assert_eq!(0, engine.search_naive(&mut move_list, 1));
        assert_eq!(-100, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control).unwrap());
        assert!(engine.repetition_table.is_empty());
    }

    #[test]
    fn test_quiescence_search_2() {
        let mut board = Board::new();
        let fen = "4k3/8/4pp2/4p3/3P4/3P4/8/4K3 w - - 0 1";
        board.read_fen(fen);
        let mut move_list = MoveList::from_board(board);

        let mut engine = Engine::with_evaluator(MockMaterialEvaluator {});
        let search_control = SearchControl::new(None, None);

        assert_eq!(0, engine.search_naive(&mut move_list, 1));
        assert_eq!(-100, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control).unwrap());
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
        let search_control = SearchControl::new(None, None);

        assert_eq!(100, engine.search_naive(&mut move_list, 1));
        
        //Stand-pat makes it 0, otherwise it is -700 (black is not forced to capture the pawn, which directly leads to promotion)
        assert_eq!(0, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control).unwrap());
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

        let search_control = SearchControl::new(None, None);

        assert_ne!(DRAW, engine.search_alpha_beta_pruning(&mut move_list, 0, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control, None).unwrap());
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::with_evaluator(MockMaterialEvaluator{});
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_ne!(DRAW, engine.search_alpha_beta_pruning(&mut move_list, 0, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control, None).unwrap());
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::with_evaluator(MockMaterialEvaluator{});
        engine.repetition_table.visit_position(move_list.get_board().zobrist);
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_eq!(DRAW, engine.search_alpha_beta_pruning(&mut move_list, 0, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control, None).unwrap());
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
        let search_control = SearchControl::new(None, None);

        assert_ne!(DRAW, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control).unwrap());
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::with_evaluator(MockMaterialEvaluator {});
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_ne!(DRAW, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control).unwrap());
        assert!(!engine.repetition_table.visit_position(move_list.get_board().zobrist));
        assert!(engine.repetition_table.visit_position(move_list.get_board().zobrist));

        engine = Engine::with_evaluator(MockMaterialEvaluator {});
        engine.repetition_table.visit_position(move_list.get_board().zobrist);
        engine.repetition_table.visit_position(move_list.get_board().zobrist);

        assert_eq!(DRAW, engine.quiescence_search(&mut move_list, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control).unwrap());
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

        let search_control = SearchControl::new(None, None);

        engine.search(&mut move_list, 7, &search_control, None);
        move_list.make_move(&Move::new(1, 18, 0, KNIGHT));
        engine.search(&mut move_list, 7, &search_control, None);
        move_list.make_move(&Move::new(48, 40, 0, PAWN));
        let board = move_list.get_board().clone();
        engine.search(&mut move_list, 7, &search_control, None);
        assert!(engine.try_get_best_move(&board).unwrap().origin < 24);
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
        let search_control = SearchControl::new(None, None);

        engine.depth = 3;
        engine.search_alpha_beta_pruning(&mut move_list, 3, NEGATIVE_INFINITY, POSITIVE_INFINITY, &search_control, None);
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