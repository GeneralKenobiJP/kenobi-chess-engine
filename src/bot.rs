//! Chess bot
//! Serves as an interface between the game host (i.e. GUI) and the engine.
//! Implements the Universal Chess Interface (UCI).
//! Communication is realized through the message() function.
//! It should be called in the game loop of the main function.

use std::io::{self, Write};
use std::iter::Peekable;
use std::str::SplitWhitespace;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{spawn, JoinHandle};
use std::time::{Duration, Instant};

use scanner_rust::ScannerStr;

use crate::board::{Board, START_POSITION};
use crate::evaluation::{Evaluator, MainEvaluator, POSITIVE_INFINITY};
use crate::move_generator::{Move, MoveList};
use crate::perft::perft_log;
use crate::search::{Engine, SearchControl};
use crate::string_builder::StringBuilder;
use crate::transposition_table::RepetitionTable;

const INFINITE_DEPTH: u32 = 256;
const DEFAULT_MOVE_OVERHEAD_MS: u64 = 10;
const MAX_MOVE_OVERHEAD_MS: u64 = 5_000;

/// Command parsing enum
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Uci,
    Debug(bool),
    IsReady,
    UciNewGame,
    SetOption {name: String, value: Option<String>},
    Position {fen: Option<String>, startpos: bool, moves: Vec<String>},
    Go(SearchSettings),
    Stop,
    PonderHit,
    Quit,
    Invalid(String),
    Unknown(String),
}

/// Settings provided with the `go` command.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SearchSettings {
    pub wtime: Option<u64>,
    pub btime: Option<u64>,
    pub winc: Option<u64>,
    pub binc: Option<u64>,
    pub moves_to_go: Option<u32>,
    pub depth: Option<u32>,
    pub nodes: Option<u64>,
    pub mate: Option<u32>,
    pub move_time: Option<u64>,
    pub infinite: bool,
    pub perft: Option<u32>,
    pub ponder: bool,
    pub search_moves: Vec<String>
}

/// Search handle & state
struct ActiveSearch {
    // Per-search state shared between the UCI control thread and the search worker.
    control: Arc<SearchControl>,
    // Minimal ponder budget.
    ponder_budget: Option<Duration>,
    // Ponder thread control
    ponder_gate: Option<Arc<PonderGate>>,
    // Search thread handle
    handle: Option<JoinHandle<()>>,
}

/// Ponder thread control
struct PonderGate {
    pondering: Mutex<bool>,
    changed: Condvar,
}

impl PonderGate {
    fn new() -> Self {
        Self {
            pondering: Mutex::new(true),
            changed: Condvar::new(),
        }
    }

    /// Sends the signal that allows the engine to stop the pondering as soon as the minimal limit is reached
    fn release(&self) {
        if let Ok(mut pondering) = self.pondering.lock() {
            *pondering = false;
            self.changed.notify_all();
        }
    }

    /// Wait for the pondering mutex release, i.e. for `ponderhit` or `stop` commands.
    /// The pondering should not be aborted beforehand, even if the minimal budget is reached.
    fn wait_until_released(&self) {
        let Ok(mut pondering) = self.pondering.lock() else {
            return;
        };
        while *pondering {
            match self.changed.wait(pondering) {
                Ok(state) => pondering = state,
                Err(_) => return,
            }
        }
    }
}

pub struct Bot<T: Evaluator = MainEvaluator> {
    engine: Arc<Mutex<Engine<T>>>,
    move_list: Arc<Mutex<MoveList>>,
    active_search: Option<ActiveSearch>,
    debug: bool,
    move_overhead_ms: u64,
}

impl<T: Evaluator> Drop for Bot<T> {
    fn drop(&mut self) {
        if let Some(mut search) = self.active_search.take() {
            search.control.request_stop();
            if let Some(gate) = &search.ponder_gate {
                gate.release();
            }
            if let Some(handle) = search.handle.take() {
                let _ = handle.join();
            }
        }
    }
}

impl Bot<MainEvaluator> {
    pub fn new() -> Self {
        Bot {
            engine: Arc::new(Mutex::new(Engine::new())),
            move_list: Arc::new(Mutex::new(MoveList::new())),
            active_search: None,
            debug: false,
            move_overhead_ms: DEFAULT_MOVE_OVERHEAD_MS,
        }
    }

    pub fn with_position(fen: &str) -> Self {
        let evaluator = MainEvaluator {};
        Bot {
            engine: Arc::new(Mutex::new(Engine::with_evaluator(evaluator))),
            move_list: Arc::new(Mutex::new(MoveList::from_fen(fen))),
            active_search: None,
            debug: false,
            move_overhead_ms: DEFAULT_MOVE_OVERHEAD_MS,
        }
    }

    /// Parses the message received from the UCI.
    /// Returns an option of the structured Command enum.
    /// None is returned if empty command or a comment was supplied.
    pub fn parse(message: &str) -> Option<Command> {
        let message = message.trim();

        // Messages starting with '#' should be treated as comments and, thus, ignored.
        if message.is_empty() || message.starts_with('#') {
            return None;
        }

        let mut iter = message.split_whitespace().peekable();
        let Some(head) = iter.next() else { return None; };

        match head.to_ascii_lowercase().as_str() {
            "uci" => Some(Command::Uci),
            "debug" => match iter.next() {
                Some(value) if value.eq_ignore_ascii_case("on") => Some(Command::Debug(true)),
                Some(value) if value.eq_ignore_ascii_case("off") => Some(Command::Debug(false)),
                _ => Some(Command::Invalid(
                    "debug expects either 'on' or 'off'".to_owned(),
                )),
            },
            "isready" => Some(Command::IsReady),
            "ucinewgame" => Some(Command::UciNewGame),
            "stop" => Some(Command::Stop),
            "ponderhit" => Some(Command::PonderHit),
            "quit" => Some(Command::Quit),

            "setoption" => Self::parse_set_option(&mut iter),
            "position" => Self::parse_position(&mut iter),
            "go" => Self::parse_go(&mut iter),
            _ => Some(Command::Unknown(message.to_owned())),
        }
    }

    /// Parses the `setoption` command.
    /// * iter - peekable iterator of the further command
    ///
    /// Returns Some(Command::SetOption) for valid options.
    /// Returns Some(Command::Invalid) for invalid options.
    fn parse_set_option(iter: &mut Peekable<SplitWhitespace>) -> Option<Command> {
        let mut name_tokens: Vec<&str> = Vec::new();
        let mut value_tokens: Vec<&str> = Vec::new();

        let Some(name_token) = iter.next() else {
            return Some(Command::Invalid(
                "setoption must contain the 'name' token".to_owned(),
            ));
        };
        if !name_token.eq_ignore_ascii_case("name") {
            return Some(Command::Invalid(
                "setoption must contain the 'name' token".to_owned(),
            ));
        }

        while let Some(token) = iter.next() {
            if token.eq_ignore_ascii_case("value") {
                break;
            }
            name_tokens.push(token);
        }

        while let Some(token) = iter.next() {
            value_tokens.push(token);
        }

        let name = name_tokens.join(" ");
        if name.is_empty() {
            return Some(Command::Invalid(
                "setoption requires a non-empty option name".to_owned(),
            ));
        }
        let value = if value_tokens.is_empty() { None } else { Some(value_tokens.join(" ")) };

        Some(Command::SetOption { name, value })
    }

    /// Parses the `position` command.
    /// * iter - peekable iterator of the further command
    ///
    /// Returns Some(Command::Position) for valid position.
    /// Returns Some(Command::Invalid) for invalid position.
    fn parse_position(iter: &mut Peekable<SplitWhitespace>) -> Option<Command> {
        let mut startpos = false;
        let mut fen: Option<String> = None;
        let mut moves: Vec<String> = Vec::new();

        match iter.next() {
            Some(token) if token.eq_ignore_ascii_case("startpos") => {
                startpos = true;

                // Consume the "moves" token
                if iter.peek().map_or(false, |token| token.eq_ignore_ascii_case("moves")) {
                    iter.next();
                }
            }
            Some(token) if token.eq_ignore_ascii_case("fen") => {
                let mut fen_tokens: Vec<&str> = Vec::new();
                while let Some(token) = iter.next() {
                    if token.eq_ignore_ascii_case("moves") {
                        // Consume the "moves" token
                        break;
                    }
                    fen_tokens.push(token);
                }
                if fen_tokens.is_empty() {
                    return Some(Command::Invalid(
                        "position fen requires a FEN string".to_owned(),
                    ));
                }
                fen = Some(fen_tokens.join(" "));
            }
            _ => {
                return Some(Command::Invalid(
                    "invalid position command".to_owned(),
                ))
            }
        }

        while let Some(piece_move) = iter.next() {
            moves.push(piece_move.to_string());
        }

        Some(Command::Position { fen, startpos, moves })
    }

    /// Parses the `go` command.
    /// * iter - peekable iterator of the further command
    ///
    /// Returns Some(Command::Go) for valid search options.
    /// Returns Some(Command::Invalid) for invalid search options.
    fn parse_go(iter: &mut Peekable<SplitWhitespace>) -> Option<Command> {
        let mut settings = SearchSettings::default();

        macro_rules! parse_number {
            ($field:expr, $ty:ty, $name:literal) => {
                match iter.next().and_then(|value| value.parse::<$ty>().ok()) {
                    Some(value) => $field = Some(value),
                    None => {
                        return Some(Command::Invalid(format!(
                            "go {} requires a valid non-negative integer",
                            $name
                        )))
                    }
                }
            };
        }

        while let Some(token) = iter.next() {
            match token.to_ascii_lowercase().as_str() {
                "wtime" => parse_number!(settings.wtime, u64, "wtime"),
                "btime" => parse_number!(settings.btime, u64, "btime"),
                "winc" => parse_number!(settings.winc, u64, "winc"),
                "binc" => parse_number!(settings.binc, u64, "binc"),
                "movestogo" => parse_number!(settings.moves_to_go, u32, "movestogo"),
                "depth" => parse_number!(settings.depth, u32, "depth"),
                "nodes" => parse_number!(settings.nodes, u64, "nodes"),
                "mate" => parse_number!(settings.mate, u32, "mate"),
                "movetime" => parse_number!(settings.move_time, u64, "movetime"),
                "infinite" => settings.infinite = true,
                "perft" => parse_number!(settings.perft, u32, "perft"),
                "ponder" => settings.ponder = true,

                "searchmoves" => {
                    while let Some(piece_move) = iter.next() {
                        settings.search_moves.push(piece_move.to_string());
                    }
                    break;
                }

                // UCI requires unknown tokens to be ignored so parsing can
                // continue with extensions added by future GUIs.
                _ => {}
            }
        }

        Some(Command::Go(settings))
    }

    /// Processes the parsed UCI command and executes it.
    pub fn process(&mut self, command: Command) -> Option<String> {
        match command {
            Command::Uci => Self::uci(),
            Command::Debug(enabled) => {
                self.debug = enabled;
                Some(String::new())
            }
            Command::IsReady => Self::readyok(),
            Command::UciNewGame => self.new_game(),
            Command::SetOption { name, value } => self.set_option(name, value),
            Command::Position { fen, startpos, moves } =>
                self.input_position(fen, startpos, moves),
            Command::Go(settings) => self.go(settings),
            Command::Stop => self.stop(),
            Command::PonderHit => self.ponder_hit(),
            Command::Quit => {
                self.stop_and_join_search();
                None
            }
            Command::Invalid(message) => Some(format!("info string {message}")),
            Command::Unknown(message) => self.unsupported_command(message),
        }
    }

    /// Responds to a "uci" command.
    /// Returns the id of the engine (name, version, author) and available options.
    /// Ends the response with "uciok"
    fn uci() -> Option<String> {
        let mut response = StringBuilder::new();
        response.append_line(&Bot::id());
        let options = Bot::option();
        if !options.is_empty() {
            response.append_line(&options);
        }
        response.append_line(&"uciok");
        Option::from(response.build())
    }

    /// Responds to a "isready" command with "readyok".
    fn readyok() -> Option<String> {
        Option::from(String::from("readyok"))
    }

    /// Responds to a "ucinewgame" command.
    /// Creates a new engine object and clears the move list
    fn new_game(&mut self) -> Option<String> {
        self.stop_and_join_search();
        self.engine = Arc::new(Mutex::new(Engine::new()));
        self.move_list.lock().unwrap().clear();
        Option::from(String::new())
    }

    /// Builds the given position on the board and returns the response, which is empty for successful operation.
    fn input_position(&mut self, fen: Option<String>, startpos: bool, moves: Vec<String>) -> Option<String> {
        let response = String::new();

        self.stop_and_join_search();

        let mut move_list_binding = self.move_list.lock().unwrap();
        move_list_binding.clear();
        drop(move_list_binding);

        let mut move_list_binding = self.move_list.lock().unwrap();
        let mut board = move_list_binding.get_mutable_board();
        let mut engine_binding = self.engine.lock().unwrap();
        let mut repetition_table = engine_binding.get_mut_repetition_table();

        repetition_table.clear();

        if startpos {
            *board = Board::from_fen(START_POSITION);
        }
        else {
            match fen {
                Some(fen) => *board = Board::from_fen(& *fen),
                None => return Some(String::from("info string position command has no FEN"))
            }
        }

        repetition_table.visit_position(board.zobrist);
        // we drop the binding, as we no longer need it, and it allows a mutable borrow of self later on
        drop(move_list_binding);

        let mut move_list_binding = self.move_list.lock().unwrap();
        Self::input_moves(&mut move_list_binding, moves, repetition_table);

        Option::from(response)
    }

    /// Inputs moves after a "moves" subcommand within the "position" command.
    /// Makes moves, one by one, on the board until the moves are exhausted
    fn input_moves(move_list_binding: &mut MutexGuard<MoveList>, moves: Vec<String>, repetition_table: &mut RepetitionTable) {
        for input in moves {
            let piece_move = Move::from_algebraic_notation(&*input, move_list_binding.get_board());
            move_list_binding.make_move(&piece_move);
            repetition_table.visit_position(move_list_binding.get_board().zobrist);
        }
    }

    /// Responds to a "go" command, which requests position analysis.
    /// The call is done on a separate thread, so the CLI can resume its work without
    /// waiting for the time-consuming computations.
    /// It responds with an empty string, as it returns before the computing threads conclude their work.
    /// Prints directly the responses of the computation thread.
    /// Implements various settings like `ponder`, `infinite`, `nodes`, time controls, `perft`.
    fn go(&mut self, settings: SearchSettings) -> Option<String> {
        self.stop_and_join_search();

        if let Some(depth) = settings.perft {
            return Self::go_perft(&mut self.move_list.lock().unwrap(), Some(depth));
        }

        let budget = {
            let move_list = self.move_list.lock().unwrap();
            self.compute_time_budget(&settings, &move_list)
        };
        let deadline = if settings.ponder {
            None
        } else {
            budget.map(|duration| Instant::now() + duration)
        };

        let depth = settings
            .depth
            .or_else(|| settings.mate.map(|moves| moves.saturating_mul(2)))
            .unwrap_or(INFINITE_DEPTH)
            .min(INFINITE_DEPTH);

        let control = Arc::new(SearchControl::new(settings.nodes, deadline));
        let thread_control = Arc::clone(&control);
        let engine = Arc::clone(&self.engine);
        let move_list = Arc::clone(&self.move_list);
        let search_moves = settings.search_moves.clone();
        let ponder_gate = settings
            .ponder
            .then(|| Arc::new(PonderGate::new()));
        let thread_ponder_gate = ponder_gate.as_ref().map(Arc::clone);
        let started = Instant::now();

        let handle = spawn(move || {
            let mut engine = match engine.lock() {
                Ok(engine) => engine,
                Err(_) => {
                    Self::write_uci_response("info string engine mutex was poisoned");
                    Self::write_uci_response("bestmove 0000");
                    return;
                }
            };
            let mut move_list = match move_list.lock() {
                Ok(move_list) => move_list,
                Err(_) => {
                    Self::write_uci_response("info string position mutex was poisoned");
                    Self::write_uci_response("bestmove 0000");
                    return;
                }
            };

            let root_moves = if search_moves.is_empty() {
                None
            } else {
                Some(
                    search_moves
                        .iter()
                        .map(|input| Move::from_algebraic_notation(input, move_list.get_board()))
                        .collect::<Vec<_>>(),
                )
            };

            engine.search(
                &mut move_list,
                depth,
                &thread_control,
                root_moves.as_deref(),
            );

            // UCI forbids a ponder search from publishing bestmove before
            // either `ponderhit` or `stop`, even if a finite depth completed.
            if let Some(gate) = thread_ponder_gate {
                gate.wait_until_released();
            }

            let response = Self::best_move(
                &mut engine,
                &mut move_list,
                thread_control.get_nodes(),
                started.elapsed(),
                root_moves.as_deref(),
            );
            Self::write_uci_response(&response);
        });

        self.active_search = Some(ActiveSearch {
            control,
            ponder_budget: if settings.ponder { budget } else { None },
            ponder_gate,
            handle: Some(handle),
        });

        Some(String::new())
    }

    /// Responds to a "go perft" command.
    /// Calls the perft_log function, outputting the number of nodes searched, the duration time,
    /// and the number of nodes from each immediate response
    fn go_perft(move_list: &mut MoveList, depth: Option<u32>) -> Option<String> {
        if depth.is_none() {
            return Some(String::from("info string go perft requires a depth"));
        }
        let depth = depth.unwrap();

        let start = Instant::now();
        let nodes = perft_log(move_list, depth);
        let duration = start.elapsed();

        let response = format!(
            "info string perft {} searched {} nodes in {:?}",
            depth, nodes, duration
        );
        Option::from(response)
    }

    /// Computes the time budget for the search, given the search settings and the current move list.
    /// Returns a `Duration` object wrapped in an Option.
    ///
    /// The formula is:
    /// * movetime if `movetime` option is supplied (forcing exact duration of search)
    /// * None if infinite search
    /// * otherwise:
    ///     budget = Math.min(p, Math.max(0, r - m_o)), where:
    ///         p = r/m + 3/4 * i,
    ///         r - time remaining on the clock,
    ///         m - moves to the next time control (min. 1),
    ///         i - time increment,
    ///         m_o - non-search related overhead per each move
    fn compute_time_budget(&self, settings: &SearchSettings, move_list: &MoveList) -> Option<Duration> {
        if let Some(milliseconds) = settings.move_time {
            return Some(Duration::from_millis(milliseconds));
        }
        if settings.infinite {
            return None;
        }

        let white_to_move = move_list.get_board().active_player as u32 == 0;
        let remaining = if white_to_move {
            settings.wtime?
        } else {
            settings.btime?
        };
        let increment = if white_to_move {
            settings.winc.unwrap_or(0)
        } else {
            settings.binc.unwrap_or(0)
        };
        let moves_to_go = u64::from(settings.moves_to_go.unwrap_or(30).max(1));

        // Conservative single-move budget: an equal share of the remaining
        // clock plus 75% of the increment, while retaining the configured
        // communication/move overhead as a reserve.
        let proposed = remaining / moves_to_go + increment.saturating_mul(3) / 4;
        let budget = proposed.min(remaining.saturating_sub(self.move_overhead_ms));
        Some(Duration::from_millis(budget))
    }

    /// Send the stop signal and wait for the search thread to finish.
    fn stop_and_join_search(&mut self) {
        let Some(mut search) = self.active_search.take() else {
            return;
        };

        search.control.request_stop();
        if let Some(gate) = &search.ponder_gate {
            gate.release();
        }
        if let Some(handle) = search.handle.take() {
            if let Err(payload) = handle.join() {
                eprintln!("search worker panicked: {payload:?}");
            }
        }
    }

    /// Writes the response to the stdout channel. Ignores an empty response.
    fn write_uci_response(response: &str) {
        if response.is_empty() {
            return;
        }
        let stdout = io::stdout();
        let mut out = stdout.lock();
        let _ = writeln!(out, "{response}");
        let _ = out.flush();
    }

    /// Responds to a "stop command".
    /// Immediately ceases further position analysis.
    /// Responds with the best move
    fn stop(&mut self) -> Option<String> {
        if let Some(search) = &self.active_search {
            search.control.request_stop();
            if let Some(gate) = &search.ponder_gate {
                gate.release();
            }
        }
        Option::from(String::new())
    }

    /// Sends a `ponderhit` command - allows the ponder search to stop if the minimal budget is reached.
    /// Returns an empty string.
    fn ponder_hit(&mut self) -> Option<String> {
        if let Some(search) = &mut self.active_search {
            if let Some(budget) = search.ponder_budget.take() {
                search.control.set_deadline_from_now(budget);
            }
            if let Some(gate) = &search.ponder_gate {
                gate.release();
            }
        }
        Some(String::new())
    }

    /// Sets an option given its name and value.
    ///
    /// Currently supported options:
    /// * Move Overhead - non-search related overhead per each move
    /// * Clear Hash - clears the transposition table and all caches/
    ///
    /// Returns an empty string if successful, otherwise a string with an error message.
    fn set_option(&mut self, name: String, value: Option<String>) -> Option<String> {
        if name.eq_ignore_ascii_case("Move Overhead") {
            let Some(value) = value.and_then(|value| value.parse::<u64>().ok()) else {
                return Some("info string Move Overhead requires an integer value".to_owned());
            };
            if value > MAX_MOVE_OVERHEAD_MS {
                return Some(format!(
                    "info string Move Overhead must be between 0 and {MAX_MOVE_OVERHEAD_MS}"
                ));
            }
            self.move_overhead_ms = value;
        } else if name.eq_ignore_ascii_case("Clear Hash") {
            self.stop_and_join_search();
            self.engine = Arc::new(Mutex::new(Engine::new()));
        } else if self.debug {
            eprintln!("Ignored unsupported UCI option: {name}");
        }

        Some(String::new())
    }

    /// Returns an empty string if debug statements are not allowed, and a problem description otherwise.
    fn unsupported_command(&self, command: String) -> Option<String> {
        if self.debug {
            eprintln!("Ignored unknown UCI command: {command}");
        }
        Some(String::new())
    }

    /// Returns a string containing info on the search,
    /// given the `Engine` object, the number of nodes visited, and the search duration:
    /// * depth
    /// * score evaluation in centipawns
    /// * time in miliseconds
    /// * number of nodes visited
    /// * nodes per second visited
    fn info(engine: &Engine, nodes: u64, elapsed: Duration) -> String {
        let depth = engine.get_current_depth();
        let score_cp = engine.get_last_root_score().unwrap_or(0);
        let milliseconds = elapsed.as_millis().max(1);
        let nps = u128::from(nodes).saturating_mul(1_000) / milliseconds;

        if score_cp.abs() < POSITIVE_INFINITY {
            format!(
                "info depth {depth} score cp {score_cp} time {milliseconds} nodes {nodes} nps {nps}"
            )
        }
        else {
            let mate_plies = score_cp.abs() - POSITIVE_INFINITY;
            let mate_moves = score_cp.signum() * (mate_plies + 3) / 2;
            format!(
                "info depth {depth} score mate {mate_moves} time {milliseconds} nodes {nodes} nps {nps}"
            )
        }
    }

    /// Implements the "best_move" UCI command.
    /// Retrieves what the engine currently deems the best move.
    /// Should not be used before doing any search.
    fn best_move(
        engine: &mut Engine,
        move_list: &mut MoveList,
        nodes: u64,
        elapsed: Duration,
        allowed_root_moves: Option<&[Move]>,
    ) -> String {
        let mut response = Self::info(engine, nodes, elapsed);
        response.push_str("\nbestmove ");

        // Self::tt_entries(&engine);

        let best_move = match engine.try_get_best_move(move_list.get_board()) {
            Some(best_move)
                if allowed_root_moves
                    .map_or(true, |allowed| allowed.contains(&best_move)) => best_move,
            _ => {
                // If stop arrived before depth one completed, return the first
                // legal root move rather than incorrectly reporting a null move.
                move_list.generate_moves();
                let candidates = move_list.get_moves().clone();
                let mut fallback = None;
                for candidate in candidates {
                    if allowed_root_moves
                        .map_or(false, |allowed| !allowed.contains(&candidate))
                    {
                        continue;
                    }
                    move_list.make_move(&candidate);
                    let legal = !move_list.is_opponent_in_check();
                    move_list.unmake_move(&candidate);
                    if legal {
                        fallback = Some(candidate);
                        break;
                    }
                }
                let Some(best_move) = fallback else {
                    response.push_str("0000");
                    return response;
                };
                best_move
            }
        };
        response.push_str(&*best_move.to_algebraic_notation());

        move_list.make_move(&best_move);
        let ponder_transposition = engine.get_transposition(move_list.get_board());
        move_list.unmake_move(&best_move);

        let Some(ponder_move) = ponder_transposition
            .and_then(|transposition| transposition.best_moves[0])
        else {
            return response;
        };

        response.push_str(" ponder ");
        response.push_str(&ponder_move.to_algebraic_notation());

        response
    }

    /// Responds with the id of the engine: name, version, author
    fn id() -> String {
        let mut response = StringBuilder::new();
        response
            .append_line(&format!("id name Kenobi {}", env!("CARGO_PKG_VERSION")))
            .append_line("id author Jakub Pietrzak");
        response.build()
    }

    /// Responds with the options of the engine.
    /// Option is a setting that can be changed through the UCI.
    fn option() -> String {
        let mut response = StringBuilder::new();
        response
            .append_line(&format!(
                "option name Move Overhead type spin default {DEFAULT_MOVE_OVERHEAD_MS} min 0 max {MAX_MOVE_OVERHEAD_MS}"
            ))
            .append_line(&"option name Clear Hash type button");
        response.build()
    }

    /// Outputs a string containing the information on the current player.
    fn current_player(&self) -> Option<String> {
        let list = self.move_list.lock().unwrap();
        Option::from((list.get_board().active_player as u32).to_string())
    }

    /// Outputs a string containing the information on the depth of the current search.
    fn depth(&self) -> Option<String> {
        let engine_binding = self.engine.lock().unwrap();
        Option::from(engine_binding.get_current_depth().to_string())
    }

    // /// Outputs a string containing the information on the depth of the current search.
    // fn tt_entries(engine: &Engine) {
    //     println!("{}", engine.get_tt_len());
    // }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::ops::Deref;
    use std::thread;
    use std::thread::sleep;
    use std::time::Duration;
    use regex::Regex;

    use crate::piece::Piece::PAWN;

    use super::*;

    const SEARCH_TIMEOUT: Duration = Duration::from_secs(2);

    fn process_message(bot: &mut Bot, message: &str) -> Option<String> {
        match Bot::parse(message) {
            Some(command) => bot.process(command),
            None => Some(String::new()),
        }
    }

    fn wait_for_search_to_start(bot: &Bot) -> Arc<SearchControl> {
        let control = Arc::clone(
            &bot.active_search
                .as_ref()
                .expect("go must register an active search")
                .control,
        );
        let deadline = Instant::now() + SEARCH_TIMEOUT;

        while control.get_nodes() == 0 && Instant::now() < deadline {
            sleep(Duration::from_millis(1));
        }

        assert!(control.get_nodes() > 0, "search worker did not start");
        control
    }

    fn wait_for_search_to_finish(bot: &Bot) {
        let deadline = Instant::now() + SEARCH_TIMEOUT;

        loop {
            let finished = bot
                .active_search
                .as_ref()
                .and_then(|search| search.handle.as_ref())
                .map_or(true, |handle| handle.is_finished());
            if finished {
                return;
            }
            if Instant::now() >= deadline {
                panic!("search worker did not finish within the timeout");
            }
            sleep(Duration::from_millis(1));
        }
    }

    fn join_finished_search(bot: &mut Bot) {
        wait_for_search_to_finish(bot);
        let mut search = bot
            .active_search
            .take()
            .expect("finished worker must still be registered");
        let handle = search
            .handle
            .take()
            .expect("active search must own a worker handle");
        assert!(handle.join().is_ok(), "search worker panicked");
        assert!(bot.active_search.is_none());
    }

    #[test]
    fn check_id() {
        let expected = format!("id name Kenobi {}\nid author Jakub Pietrzak", env!("CARGO_PKG_VERSION"));
        assert_eq!(expected,  Bot::id());
    }

    #[test]
    fn check_option() {
        let expected = "option name Move Overhead type spin default 10 min 0 max 5000\noption name Clear Hash type button";
        assert_eq!(expected,  Bot::option());
    }

    #[test]
    fn check_uci() {
        let expected = format!("id name Kenobi {}\nid author Jakub Pietrzak\n\
        option name Move Overhead type spin default 10 min 0 max 5000\n\
        option name Clear Hash type button\nuciok", env!("CARGO_PKG_VERSION"));
        assert_eq!(Some(expected),  Bot::uci());
    }

    #[test]
    fn check_new_game() {
        let mut bot = Bot::with_position(START_POSITION);
        let search_control = SearchControl::new(None, None);
        bot.engine.lock().unwrap().search(&mut bot.move_list.lock().unwrap(), 1, &search_control, None);
        bot.move_list.lock().unwrap().generate_moves();
        assert!(!bot.move_list.lock().unwrap().get_moves().is_empty());

        assert_eq!(Some(String::from("")), bot.new_game());

        assert!(bot.move_list.lock().unwrap().get_moves().is_empty());
        assert_eq!(Engine::new(), *bot.engine.lock().unwrap().deref());
    }

    #[test]
    fn check_readyok() {
        assert_eq!(Some(String::from("readyok")),  Bot::readyok());
    }

    #[test]
    fn check_input_moves() {
        let mut bot = Bot::with_position(START_POSITION);
        let move_strings = vec!(String::from("e2e4"), String::from("e7e5"));
        {
            let mut move_list_binding = bot.move_list.lock().unwrap();
            let mut engine_binding = bot.engine.lock().unwrap();
            let mut repetition_table = engine_binding.get_mut_repetition_table();

            Bot::input_moves(&mut move_list_binding, move_strings, &mut repetition_table);
        }

        let mut expected_bot = Bot::with_position(START_POSITION);
        expected_bot.move_list.lock().unwrap().make_move(&Move{origin: 11, target: 27, promotion: 0, piece: PAWN});
        let zobrist1 = expected_bot.move_list.lock().unwrap().get_board().zobrist;
        expected_bot.move_list.lock().unwrap().make_move(&Move{origin: 51, target: 35, promotion: 0, piece: PAWN});
        let zobrist2 = expected_bot.move_list.lock().unwrap().get_board().zobrist;

        let expected_board = expected_bot.move_list.lock().unwrap().get_board().clone();
        let board = bot.move_list.lock().unwrap().get_board().clone();

        assert_eq!(expected_board, board);
        assert_eq!(1, bot.engine.lock().unwrap().get_repetition_table().get_repetition(zobrist1));
        assert_eq!(1, bot.engine.lock().unwrap().get_repetition_table().get_repetition(zobrist2));
    }

    #[test]
    fn check_input_position_start_position() {
        let mut bot = Bot::new();

        assert_eq!(Some(String::from("")), bot.input_position(None, true, Vec::new()));

        let mut expected_board = Board::new();
        expected_board.read_fen(START_POSITION);

        assert_eq!(expected_board, *bot.move_list.lock().unwrap().get_board());
    }

    #[test]
    fn check_input_position_fen() {
        let mut bot = Bot::new();
        let position = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";

        assert_eq!(Some(String::from("")), bot.input_position(Some(position.parse().unwrap()), false, Vec::new()));

        let mut expected_board = Board::new();
        expected_board.read_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1");

        assert_eq!(expected_board, *bot.move_list.lock().unwrap().get_board());
    }

    #[test]
    fn check_input_position_fen_moves() {
        let mut bot = Bot::new();
        let string = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
        let moves = vec!(String::from("e2e4"));

        assert_eq!(Some(String::from("")), bot.input_position(Some(string.parse().unwrap()), false, moves));

        let mut expected_board = Board::new();
        expected_board.read_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1");
        let mut expected_move_list = MoveList::from_board(expected_board);
        expected_move_list.make_move(&Move{origin: 11, target: 27, promotion: 0, piece: PAWN});

        assert_eq!(expected_move_list.get_board(), bot.move_list.lock().unwrap().get_board());
    }

    #[test]
    fn check_input_position_startpos_moves() {
        let mut bot = Bot::new();
        let moves = vec!(String::from("e2e4"));

        assert_eq!(Some(String::from("")), bot.input_position(None, true, moves));

        let mut expected_board = Board::new();
        expected_board.read_fen(START_POSITION);
        let mut expected_move_list = MoveList::from_board(expected_board);
        expected_move_list.make_move(&Move { origin: 11, target: 27, promotion: 0, piece: PAWN });

        assert_eq!(expected_move_list.get_board(), bot.move_list.lock().unwrap().get_board());
    }

    #[test]
    fn check_parse_ignored_and_simple_commands() {
        assert_eq!(None, Bot::parse(""));
        assert_eq!(None, Bot::parse("   "));
        assert_eq!(None, Bot::parse("# ignored UCI script comment"));

        assert_eq!(Some(Command::Uci), Bot::parse("uci"));
        assert_eq!(Some(Command::IsReady), Bot::parse("ISREADY"));
        assert_eq!(Some(Command::UciNewGame), Bot::parse("ucinewgame"));
        assert_eq!(Some(Command::Stop), Bot::parse("stop"));
        assert_eq!(Some(Command::PonderHit), Bot::parse("ponderhit"));
        assert_eq!(Some(Command::Quit), Bot::parse("quit"));
        assert_eq!(Some(Command::Debug(true)), Bot::parse("debug on"));
        assert_eq!(Some(Command::Debug(false)), Bot::parse("debug OFF"));
    }

    #[test]
    fn check_parse_setoption() {
        assert_eq!(
            Some(Command::SetOption {
                name: String::from("Move Overhead"),
                value: Some(String::from("25")),
            }),
            Bot::parse("setoption name Move Overhead value 25")
        );
        assert_eq!(
            Some(Command::SetOption {
                name: String::from("Clear Hash"),
                value: None,
            }),
            Bot::parse("setoption name Clear Hash")
        );
    }

    #[test]
    fn check_parse_position() {
        let fen = "4K3/8/8/8/8/8/8/4k3 w - - 0 1";

        assert_eq!(
            Some(Command::Position {
                fen: None,
                startpos: true,
                moves: vec!(String::from("e2e4"), String::from("e7e5")),
            }),
            Bot::parse("position startpos moves e2e4 e7e5")
        );
        assert_eq!(
            Some(Command::Position {
                fen: Some(String::from(fen)),
                startpos: false,
                moves: vec!(String::from("e1e2")),
            }),
            Bot::parse(&format!("position fen {fen} moves e1e2"))
        );
    }

    #[test]
    fn check_parse_go() {
        assert_eq!(
            Some(Command::Go(SearchSettings::default())),
            Bot::parse("go")
        );

        let expected = SearchSettings {
            wtime: Some(60_000),
            btime: Some(59_000),
            winc: Some(500),
            binc: Some(250),
            moves_to_go: Some(20),
            depth: Some(12),
            nodes: Some(123_456),
            mate: Some(4),
            move_time: Some(1_500),
            infinite: true,
            perft: Some(3),
            ponder: true,
            search_moves: vec!(String::from("e2e4"), String::from("d2d4")),
        };

        assert_eq!(
            Some(Command::Go(expected)),
            Bot::parse(
                "go wtime 60000 btime 59000 winc 500 binc 250 movestogo 20 \
                 depth 12 nodes 123456 mate 4 movetime 1500 infinite perft 3 \
                 ponder searchmoves e2e4 d2d4"
            )
        );

        assert_eq!(
            Some(Command::Go(SearchSettings {
                depth: Some(4),
                ..SearchSettings::default()
            })),
            Bot::parse("go futureextension 99 depth 4")
        );
    }

    #[test]
    fn check_parse_invalid_and_unknown_commands() {
        for message in [
            "debug maybe",
            "setoption",
            "setoption value 10",
            "setoption name value 10",
            "position",
            "position fen",
            "go depth",
            "go depth -1",
            "go nodes invalid",
        ] {
            assert!(
                matches!(Bot::parse(message), Some(Command::Invalid(_))),
                "parser accepted invalid command: {message}"
            );
        }

        assert_eq!(
            Some(Command::Unknown(String::from("nonstandard command"))),
            Bot::parse("nonstandard command")
        );
    }

    #[test]
    fn check_time_budget_search_settings() {
        let bot = Bot::with_position(START_POSITION);
        let move_list = bot.move_list.lock().unwrap();

        let settings = SearchSettings {
            move_time: Some(250),
            infinite: true,
            ..SearchSettings::default()
        };
        assert_eq!(
            Some(Duration::from_millis(250)),
            bot.compute_time_budget(&settings, &move_list)
        );

        let settings = SearchSettings {
            infinite: true,
            ..SearchSettings::default()
        };
        assert_eq!(None, bot.compute_time_budget(&settings, &move_list));

        let settings = SearchSettings {
            wtime: Some(60_000),
            winc: Some(1_000),
            moves_to_go: Some(30),
            ..SearchSettings::default()
        };
        assert_eq!(
            Some(Duration::from_millis(2_750)),
            bot.compute_time_budget(&settings, &move_list)
        );
    }

    #[test]
    fn check_go_depth() {
        let mut bot = Bot::with_position(START_POSITION);
        let settings = SearchSettings {
            depth: Some(1),
            ..SearchSettings::default()
        };

        assert_eq!(Some(String::new()), bot.go(settings));
        wait_for_search_to_start(&bot);
        join_finished_search(&mut bot);

        assert_eq!(Some(String::from("1")), bot.depth());
        let engine = bot.engine.lock().unwrap();
        let move_list = bot.move_list.lock().unwrap();
        assert!(engine.try_get_best_move(move_list.get_board()).is_some());
    }

    #[test]
    fn check_go_nodes() {
        let mut bot = Bot::with_position(START_POSITION);
        let settings = SearchSettings {
            nodes: Some(1),
            ..SearchSettings::default()
        };

        assert_eq!(Some(String::new()), bot.go(settings));
        let control = wait_for_search_to_start(&bot);
        join_finished_search(&mut bot);

        assert!(control.get_nodes() >= 1);
    }

    #[test]
    fn check_go_searchmoves() {
        let mut bot = Bot::with_position(START_POSITION);
        let expected_move = {
            let move_list = bot.move_list.lock().unwrap();
            Move::from_algebraic_notation("e2e4", move_list.get_board())
        };
        let settings = SearchSettings {
            depth: Some(1),
            search_moves: vec!(String::from("e2e4")),
            ..SearchSettings::default()
        };

        assert_eq!(Some(String::new()), bot.go(settings));
        wait_for_search_to_start(&bot);
        join_finished_search(&mut bot);

        let engine = bot.engine.lock().unwrap();
        let move_list = bot.move_list.lock().unwrap();
        assert_eq!(
            Some(expected_move),
            engine.try_get_best_move(move_list.get_board())
        );
    }

    #[test]
    fn check_go_ponder() {
        let mut bot = Bot::with_position(START_POSITION);
        let settings = SearchSettings {
            // The one-node limit makes the actual search finish immediately;
            // only the ponder gate should keep the worker alive.
            nodes: Some(1),
            ponder: true,
            ..SearchSettings::default()
        };

        assert_eq!(Some(String::new()), bot.go(settings));
        wait_for_search_to_start(&bot);
        sleep(Duration::from_millis(25));

        let worker_finished = bot
            .active_search
            .as_ref()
            .and_then(|search| search.handle.as_ref())
            .map_or(true, |handle| handle.is_finished());
        assert!(!worker_finished, "ponder search published bestmove before ponderhit");

        assert_eq!(Some(String::new()), bot.ponder_hit());
        join_finished_search(&mut bot);
    }

    #[test]
    fn check_go_perft() {
        let mut bot = Bot::with_position(START_POSITION);
        let depth = 1;

        let expected_regex = Regex::new(r"^info string perft 1 searched \d+ nodes in (\d+.\d+|\d+)(ns|µs|ms|s)$").unwrap();

        let response = Bot::go_perft(&mut bot.move_list.lock().unwrap(), Some(depth)).unwrap();
        println!("{}", response);

        assert!(expected_regex.is_match(&*response));
    }

    #[test]
    fn check_stop() {
        let mut bot = Bot::with_position(START_POSITION);
        let settings = SearchSettings {
            infinite: true,
            ..SearchSettings::default()
        };

        assert_eq!(Some(String::new()), bot.go(settings));
        wait_for_search_to_start(&bot);
        assert_eq!(Some(String::new()), bot.stop());
        join_finished_search(&mut bot);
    }

    #[test]
    fn check_best_move() {
        let mut bot = Bot::with_position(START_POSITION);
        let search_control = Arc::new(SearchControl::new(None, None));
        let mut engine = bot.engine.lock().unwrap();
        let mut move_list = bot.move_list.lock().unwrap();

        engine.search(&mut move_list, 2, &search_control, None);
        let response = Bot::best_move(
            &mut engine,
            &mut move_list,
            search_control.get_nodes(),
            Duration::from_millis(300),
            None,
        );

        let expected_regex = Regex::new(
            r"^info depth \d+ score cp -?\d+ time \d+ nodes \d+ nps \d+\nbestmove [a-h][1-8][a-h][1-8]( ponder [a-h][1-8][a-h][1-8])?$"
        ).unwrap();

        assert!(expected_regex.is_match(&response));
    }

    #[test]
    fn test_engine_pipeline() {
        let mut bot = Bot::new();

        assert_eq!(Bot::uci(), process_message(&mut bot, "uci"));
        assert_eq!(Some(String::new()), process_message(&mut bot, "debug on"));
        assert!(bot.debug);
        assert_eq!(
            Some(String::new()),
            process_message(&mut bot, "setoption name Move Overhead value 25")
        );
        assert_eq!(25, bot.move_overhead_ms);
        assert_eq!(
            Some(String::from("readyok")),
            process_message(&mut bot, "isready")
        );
        assert_eq!(
            Some(String::from(
                "info string go depth requires a valid non-negative integer"
            )),
            process_message(&mut bot, "go depth invalid")
        );

        assert_eq!(
            Some(String::new()),
            process_message(&mut bot, "ucinewgame")
        );
        assert_eq!(
            Some(String::new()),
            process_message(&mut bot, "position startpos moves e2e4 e7e5")
        );

        let mut expected_bot = Bot::with_position(START_POSITION);
        {
            let mut engine_binding = expected_bot.engine.lock().unwrap();
            let mut repetition_table = engine_binding.get_mut_repetition_table();
            let mut move_list_binding = expected_bot.move_list.lock().unwrap();
            Bot::input_moves(&mut move_list_binding, vec!(String::from("e2e4"), String::from("e7e5")), repetition_table);
        }
        assert_eq!(
            expected_bot.move_list.lock().unwrap().get_board(),
            bot.move_list.lock().unwrap().get_board()
        );

        assert_eq!(
            Some(String::new()),
            process_message(&mut bot, "go depth 1 searchmoves g1f3")
        );
        wait_for_search_to_start(&bot);
        join_finished_search(&mut bot);

        {
            let engine = bot.engine.lock().unwrap();
            let move_list = bot.move_list.lock().unwrap();
            let expected_move =
                Move::from_algebraic_notation("g1f3", move_list.get_board());
            assert_eq!(
                Some(expected_move),
                engine.try_get_best_move(move_list.get_board())
            );
        }

        assert_eq!(
            Some(String::new()),
            process_message(&mut bot, "go infinite")
        );
        wait_for_search_to_start(&bot);
        assert_eq!(
            Some(String::from("readyok")),
            process_message(&mut bot, "isready")
        );
        assert_eq!(Some(String::new()), process_message(&mut bot, "stop"));
        join_finished_search(&mut bot);

        assert_eq!(None, process_message(&mut bot, "quit"));
        assert!(bot.active_search.is_none());
    }

}