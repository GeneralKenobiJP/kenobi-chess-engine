//! Chess bot
//! Serves as an interface between the game host (i.e. GUI) and the engine.
//! Implements the Universal Chess Interface (UCI).
//! Communication is realized through the message() function.
//! It should be called in the game loop of the main function.

use std::io::{self, Write};
use std::iter::Peekable;
use std::str::SplitWhitespace;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{spawn, JoinHandle};
use std::time::{Duration, Instant};

use scanner_rust::ScannerStr;

use crate::board::{Board, START_POSITION};
use crate::evaluation::{Evaluator, MainEvaluator};
use crate::move_generator::{Move, MoveList};
use crate::perft::perft_log;
use crate::search::{Engine, SearchControl};
use crate::string_builder::StringBuilder;

const INFINITE_DEPTH: u32 = 256;
const DEFAULT_MOVE_OVERHEAD_MS: u64 = 10;
const MAX_MOVE_OVERHEAD_MS: u64 = 5_000;

#[derive(Debug)]
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

#[derive(Debug, Default, Clone)]
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

struct ActiveSearch {
    control: Arc<SearchControl>,
    ponder_budget: Option<Duration>,
    ponder_gate: Option<Arc<PonderGate>>,
    handle: Option<JoinHandle<()>>,
}

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

    fn release(&self) {
        if let Ok(mut pondering) = self.pondering.lock() {
            *pondering = false;
            self.changed.notify_all();
        }
    }

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
                _ => Some(Command::Debug(true)),
            },
            "isready" => Some(Command::IsReady),
            "ucinewgame" => Some(Command::UciNewGame),
            "stop" => Some(Command::Stop),
            "ponderhit" => Some(Command::PonderHit),
            "quit" => Some(Command::Quit),

            "setoption" => Self::parse_set_option(&mut iter, &message),
            "position" => Self::parse_position(&mut iter, &message),
            "go" => Self::parse_go(&mut iter, &message),
            _ => Some(Command::Unknown(message.to_owned())),
        }
    }

    fn parse_set_option(iter: &mut Peekable<SplitWhitespace>, _message: &str) -> Option<Command> {
        let mut name_tokens: Vec<&str> = Vec::new();
        let mut value_tokens: Vec<&str> = Vec::new();

        if !iter.next()?.eq_ignore_ascii_case("name") {
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

    fn parse_position(iter: &mut Peekable<SplitWhitespace>, message: &str) -> Option<Command> {
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
                if !fen_tokens.is_empty() {
                    fen = Some(fen_tokens.join(" "));
                }
            }
            _ => {
                return Some(Command::Invalid(format!(
                    "invalid position command: {message}"
                )))
            }
        }

        while let Some(piece_move) = iter.next() {
            moves.push(piece_move.to_string());
        }

        Some(Command::Position { fen, startpos, moves })
    }

    fn parse_go(iter: &mut Peekable<SplitWhitespace>, _message: &str) -> Option<Command> {
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

    // /// Responds to a given message, according to the UCI standard.
    // /// This function is the method used for manipulating the game state in the Bot object.
    // /// Should a particular command or option not be implemented, it will notify about the fact
    // /// with "Unexpected command. This command might be unsupported by the current version of the engine or by the UCI standard."
    // /// Calls relevant response functions that use a StringBuilder to return a response String,
    // /// which is printed out in this method.
    // /// Returns Option from a response string if no error occurred.
    // /// Returns None if the "quit" command was provided
    // pub fn message(&mut self, message: &str) -> Option<String> {
    //     let command = Self::parse(message);
    //
    //     match command {
    //         Some(command) => self.process(command),
    //         None => Some(String::new()),
    //     }
    // }

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

    fn input_position(&mut self, fen: Option<String>, startpos: bool, moves: Vec<String>) -> Option<String> {
        let response = String::new();

        self.stop_and_join_search();

        // we lock the move_list before using it
        let mut move_list_binding = self.move_list.lock().unwrap();
        let mut board = move_list_binding.get_mutable_board();

        if startpos {
            *board = Board::from_fen(START_POSITION);
        }
        else {
            match fen {
                Some(fen) => *board = Board::from_fen(& *fen),
                None => return Some(String::from("info string position command has no FEN"))
            }
        }

        // we drop the binding, as we no longer need it, and it allows a mutable borrow of self later on
        drop(move_list_binding);

        self.input_moves(moves);

        Option::from(response)
    }

    /// Inputs moves after a "moves" subcommand within the "position" command.
    /// Makes moves, one by one, on the board until the moves are exhausted
    fn input_moves(&mut self, moves: Vec<String>) {
        let mut move_list_binding = self.move_list.lock().unwrap();
        for input in moves {
            let piece_move = Move::from_algebraic_notation(&*input, move_list_binding.get_board());
            move_list_binding.make_move(&piece_move);
        }
    }

    /// Responds to a "go" command, which requests position analysis.
    /// Supports following options:
    ///     "perft" - calls perft_log, outputting the number of nodes searched, the duration time,
    /// and the number of nodes from each immediate response
    ///     "Infinite" - starts an Infinite search, which can conclude with the "stop" command
    ///     "depth" - starts a search up to a given depth
    /// The calls are done on a separate thread, so the CLI can resume its work without
    /// waiting for the time-consuming computations.
    /// It responds with an empty string, as it returns before the computing threads conclude their work.
    /// Prints directly the responses of the computation thread.
    fn go(&mut self, settings: SearchSettings) -> Option<String> {
        self.stop_and_join_search();

        if let Some(depth) = settings.perft {
            return Self::go_perft(&mut self.move_list.lock().unwrap(), Some(depth));
        }

        let budget = {
            let move_list = self.move_list.lock().unwrap();
            self.time_budget(&settings, &move_list)
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

    fn time_budget(&self, settings: &SearchSettings, move_list: &MoveList) -> Option<Duration> {
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

    fn stop_and_join_search(&mut self) {
        let Some(mut search) = self.active_search.take() else {
            return;
        };

        search.control.request_stop();
        if let Some(gate) = &search.ponder_gate {
            gate.release();
        }
        if let Some(handle) = search.handle.take() {
            let _ = handle.join();
        }
    }

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

    fn unsupported_command(&self, command: String) -> Option<String> {
        if self.debug {
            eprintln!("Ignored unknown UCI command: {command}");
        }
        Some(String::new())
    }

    fn info(engine: &Engine, nodes: u64, elapsed: Duration) -> String {
        let depth = engine.get_current_depth();
        let score_cp = engine.get_last_root_score().unwrap_or(0);
        let milliseconds = elapsed.as_millis().max(1);
        let nps = u128::from(nodes).saturating_mul(1_000) / milliseconds;
        format!(
            "info depth {depth} score cp {score_cp} time {milliseconds} nodes {nodes} nps {nps}"
        )
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

    fn current_player(&self) -> Option<String> {
        let list = self.move_list.lock().unwrap();
        Option::from((list.get_board().active_player as u32).to_string())
    }

    fn depth(&self) -> Option<String> {
        let engine_binding = self.engine.lock().unwrap();
        Option::from(engine_binding.get_current_depth().to_string())
    }
}

// #[cfg(test)]
// mod tests {
//     use std::fmt::Debug;
//     use std::ops::Deref;
//     use std::thread;
//     use std::time::Duration;
//     use regex::Regex;
//
//     use crate::piece::Piece::PAWN;
//
//     use super::*;
//
//     #[test]
//     fn check_id() {
//         let expected = format!("id name Kenobi {}\nid author Jakub Pietrzak", env!("CARGO_PKG_VERSION"));
//         assert_eq!(expected,  Bot::id());
//     }
//
//     #[test]
//     fn check_option() {
//         let expected = "";
//         assert_eq!(expected,  Bot::option());
//     }
//
//     #[test]
//     fn check_uci() {
//         let expected = format!("id name Kenobi {}\nid author Jakub Pietrzak\nuciok", env!("CARGO_PKG_VERSION"));
//         assert_eq!(Some(expected),  Bot::uci());
//     }
//
//     #[test]
//     fn check_new_game() {
//         let mut bot = Bot::with_position(START_POSITION);
//         bot.engine.lock().unwrap().search(&mut bot.move_list.lock().unwrap(), 1);
//         bot.move_list.lock().unwrap().generate_moves();
//         assert!(!bot.move_list.lock().unwrap().get_moves().is_empty());
//
//         assert_eq!(Some(String::from("")), bot.new_game());
//
//         assert!(bot.move_list.lock().unwrap().get_moves().is_empty());
//         assert_eq!(Engine::new(), *bot.engine.lock().unwrap().deref());
//     }
//
//     #[test]
//     fn check_readyok() {
//         assert_eq!(Some(String::from("readyok")),  Bot::readyok());
//     }
//
//     #[test]
//     fn check_input_moves() {
//         let mut bot = Bot::with_position(START_POSITION);
//         let mut tokens = vec!(String::from("e2e4"), String::from("e7e5")).iter();
//         bot.input_moves(&mut tokens);
//
//         assert!(tokens.next().is_none());
//
//         let mut expected_bot = Bot::with_position(START_POSITION);
//         expected_bot.move_list.lock().unwrap().make_move(&Move{origin: 11, target: 27, promotion: 0, piece: PAWN});
//         expected_bot.move_list.lock().unwrap().make_move(&Move{origin: 51, target: 35, promotion: 0, piece: PAWN});
//
//         assert_eq!(expected_bot.move_list.lock().unwrap().get_board(), bot.move_list.lock().unwrap().get_board());
//     }
//
//     #[test]
//     fn check_input_position_start_position() {
//         let mut bot = Bot::new();
//         let mut tokens = vec!(String::from("startpos")).iter();
//
//         assert_eq!(Some(String::from("")), bot.input_position(&mut tokens));
//
//         assert!(tokens.next().is_none());
//
//         let mut expected_board = Board::new();
//         expected_board.read_fen(START_POSITION);
//
//         assert_eq!(expected_board, *bot.move_list.lock().unwrap().get_board());
//     }
//
//     #[test]
//     fn check_input_position_fen() {
//         let mut bot = Bot::new();
//         let mut tokens = Bot::tokenize("fen 8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").iter();
//
//         assert_eq!(Some(String::from("")), bot.input_position(&mut tokens));
//
//         assert!(tokens.next().is_none());
//
//         let mut expected_board = Board::new();
//         expected_board.read_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1");
//
//         assert_eq!(expected_board, *bot.move_list.lock().unwrap().get_board());
//     }
//
//     #[test]
//     fn check_extract_fen() {
//         let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 blabla yadayada";
//         let mut tokens = Bot::tokenize(fen).iter();
//
//         assert_eq!(START_POSITION,  Bot::extract_fen(&mut tokens));
//         assert_eq!(*tokens.next().unwrap(), String::from("blabla"));
//     }
//
//     #[test]
//     fn check_input_position_fen_moves() {
//         let mut bot = Bot::new();
//         let string = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1 moves e2e4";
//         let mut tokens = Bot::tokenize(string).iter();
//
//         assert_eq!(Some(String::from("")), bot.input_position(&mut tokens));
//
//         assert!(tokens.next().is_none());
//
//         let mut expected_board = Board::new();
//         expected_board.read_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1");
//         let mut expected_move_list = MoveList::from_board(expected_board);
//         expected_move_list.make_move(&Move{origin: 11, target: 27, promotion: 0, piece: PAWN});
//
//         assert_eq!(expected_move_list.get_board(), bot.move_list.lock().unwrap().get_board());
//     }
//
//     #[test]
//     fn check_input_position_startpos_moves() {
//         let mut bot = Bot::new();
//         let string = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1 moves e2e4";
//         let mut tokens = Bot::tokenize(string).iter();
//
//         assert_eq!(Some(String::from("")), bot.input_position(&mut tokens));
//
//         assert!(tokens.next().is_none());
//
//         let mut expected_board = Board::new();
//         expected_board.read_fen(START_POSITION);
//         let mut expected_move_list = MoveList::from_board(expected_board);
//         expected_move_list.make_move(&Move{origin: 11, target: 27, promotion: 0, piece: PAWN});
//
//         assert_eq!(expected_move_list.get_board(), bot.move_list.lock().unwrap().get_board());
//     }
//
//     #[test]
//     fn check_go_perft() {
//         let mut bot = Bot::with_position(START_POSITION);
//         let mut tokens = vec!(String::from("1")).iter();
//
//         let expected_regex = Regex::new(r"^perft 1 searched \d+ nodes in (\d+.\d+|\d+)(ns|µs|ms|s)$").unwrap();
//
//         let response = Bot::go_perft(&mut bot.move_list.lock().unwrap(), &mut tokens).unwrap();
//         println!("{}", response);
//
//         assert!(expected_regex.is_match(&*response));
//         assert!(tokens.next().is_none());
//     }
//
//     #[test]
//     fn check_go_depth() {
//         let mut bot = Bot::with_position(START_POSITION);
//         let mut tokens = vec!(String::from("1")).iter();
//
//         let expected_regex = Regex::new(r"^bestmove [a-h][1-8][a-h][1-8]$").unwrap();
//
//         let response = Bot::go_depth(&mut bot.engine.lock().unwrap(), &mut bot.move_list.lock().unwrap(), &mut tokens).unwrap();
//         println!("{}", response);
//
//         assert!(expected_regex.is_match(&*response));
//         assert!(tokens.next().is_none());
//     }
//
//     #[test]
//     fn check_go_infinite() {
//         let mut bot = Bot::with_position(START_POSITION);
//
//         let expected_regex = Regex::new(r"^bestmove [a-h][1-8][a-h][1-8]$").unwrap();
//
//         let mut engine = bot.engine.clone();
//         let mut move_list = bot.move_list.clone();
//
//         // finished flag: false until worker sets it to true
//         let finished = Arc::new(Mutex::new(false));
//         let finished_clone = Arc::clone(&finished);
//
//         let handle = spawn(move || {
//             let response = Bot::go_infinite(&mut engine.lock().unwrap(), &mut move_list.lock().unwrap()).unwrap();
//             println!("{}", response);
//             assert!(expected_regex.is_match(&*response));
//
//             let mut done = finished_clone.lock().unwrap();
//             *done = true;
//         });
//
//         thread::sleep(Duration::from_secs(2));
//         let response = Bot::stop();
//
//         assert!(response.unwrap().is_empty());
//
//         thread::sleep(Duration::from_secs(2));
//         let done = *finished.lock().unwrap();
//         if !done {
//             // join the handle to clean up (ignore join result because we will panic anyway)
//             let _ = handle.join();
//             panic!("Spawned worker did not finish within the 2 second timeout");
//         }
//
//     }
//
//     #[test]
//     fn check_go() {
//         let mut bot = Arc::new(Mutex::new(Bot::with_position(START_POSITION)));
//         let expected_regex = Regex::new(r"^bestmove [a-h][1-8][a-h][1-8]$").unwrap();
//
//         let mut bot_binding = bot.lock().unwrap();
//
//         // go perft
//         let mut tokens = Bot::tokenize("perft 1").iter();
//
//         let response = bot_binding.go(&mut tokens).unwrap();
//         assert!(expected_regex.is_match(&*response));
//         assert!(tokens.next().is_none());
//
//         // go depth
//         let mut tokens = Bot::tokenize("depth 2").iter();
//
//         let response = bot_binding.go(&mut tokens);
//         assert!(expected_regex.is_match(&*response));
//         assert!(tokens.next().is_none());
//
//         thread::sleep(Duration::from_secs(1));
//         let response = Bot::best_move(&mut bot_binding.engine.lock().unwrap(), &mut bot_binding.move_list.lock().unwrap());
//         assert!(expected_regex.is_match(&*response));
//
//         // go Infinite
//
//         bot_binding.new_game();
//         drop(bot_binding);
//         let mut tokens = vec!(String::from("Infinite")).iter();
//
//         let handle = spawn(move || {
//             let mut bot_binding = bot.lock().unwrap();
//             let response = bot_binding.go(&mut tokens);
//             assert!(expected_regex.is_match(&*response));
//         });
//
//         thread::sleep(Duration::from_secs(1));
//         Bot::stop();
//         let response = Bot::best_move(&mut bot.engine.lock().unwrap(), &mut bot.move_list.lock().unwrap());
//         assert!(expected_regex.is_match(&*response));
//     }
//
//     #[test]
//     fn check_stop() {
//         let mut bot = Bot::with_position(START_POSITION);
//
//         bot.engine.lock().unwrap().search(&mut bot.move_list.lock().unwrap(), 1);
//
//         let expected_regex = Regex::new(r"^bestmove [a-h][1-8][a-h][1-8]$").unwrap();
//
//         let response = Bot::stop();
//         assert!(expected_regex.is_match(&*response));
//     }
//
//     #[test]
//     fn check_best_move() {
//         let mut bot = Bot::with_position(START_POSITION);
//
//         bot.engine.lock().unwrap().search(&mut bot.move_list.lock().unwrap(), 1);
//
//         let expected_regex = Regex::new(r"^bestmove [a-h][1-8][a-h][1-8]$").unwrap();
//
//         let response = Bot::best_move(&mut bot.engine.lock().unwrap(), &mut bot.move_list.lock().unwrap());
//         assert!(expected_regex.is_match(&*response));
//     }
//
//     #[test]
//     fn test_message() {
//         let mut bot = Bot::with_position(START_POSITION);
//
//         assert_eq!(None, bot.message("quit"));
//         assert_eq!(Option::from(
//             String::from("Unexpected command. This command might be unsupported by the current version of the engine or by the UCI standard.")),
//                    bot.message("illegal command"));
//         assert_eq!(Option::from(
//             String::from(format!("id name Kenobi {}\nid author Jakub Pietrzak\nuciok", env!("CARGO_PKG_VERSION")))),
//                    bot.message("uci"));
//
//         bot.engine.lock().unwrap().search(&mut bot.move_list.lock().unwrap(), 1);
//         bot.move_list.lock().unwrap().generate_moves();
//         assert_eq!(Option::from(
//             String::from("")),
//                    bot.message("ucinewgame"));
//         assert!(bot.move_list.lock().unwrap().get_moves().is_empty());
//         assert_eq!(Engine::new(), *bot.engine.lock().unwrap().deref());
//
//         assert_eq!(Option::from(
//             String::from("readyok")),
//                    bot.message("isready"));
//
//         assert_eq!(Option::from(
//             String::from("")),
//                    bot.message("position startpos moves e2e4 e7e5"));
//         let mut expected_bot = Bot::with_position(START_POSITION);
//         expected_bot.move_list.lock().unwrap().make_move(&Move{origin: 11, target: 27, promotion: 0, piece: PAWN});
//         expected_bot.move_list.lock().unwrap().make_move(&Move{origin: 51, target: 35, promotion: 0, piece: PAWN});
//         assert_eq!(expected_bot.move_list.lock().unwrap().get_board(), bot.move_list.lock().unwrap().get_board());
//
//         let response = bot.message("go perft 1").unwrap_or_default();
//         assert!(response.is_empty());
//
//         let expected_regex = Regex::new(r"^bestmove [a-h][1-8][a-h][1-8]$").unwrap();
//         let response = bot.message("go depth 2").unwrap_or_default();
//         assert!(response.is_empty());
//         thread::sleep(Duration::from_secs(1));
//         let response = Bot::best_move(&mut bot.engine.lock().unwrap(), &mut bot.move_list.lock().unwrap());
//         assert!(expected_regex.is_match(&*response));
//
//         let response = bot.message("go Infinite").unwrap_or_default();
//         assert!(response.is_empty());
//         thread::sleep(Duration::from_secs(2));
//         let response = bot.stop();
//         assert!(expected_regex.is_match(&*response));
//
//         let response = bot.message("quit");
//         assert!(response.is_none());
//     }
//
//     #[test]
//     fn check_tokenize_empty_message() {
//         let message = "";
//         assert_eq!(Vec::<String>::new(), Bot::tokenize(message));
//     }
//
//     #[test]
//     fn check_tokenize_one_word() {
//         let message = "uci";
//         assert_eq!(vec!(String::from("uci")), Bot::tokenize(message));
//     }
//
//     #[test]
//     fn check_tokenize_long_message() {
//         let message = "go wtime 59780 winc 0 btime 60000 binc 0";
//         assert_eq!(vec!(
//             String::from("go"), String::from("wtime"), String::from("59780"), String::from("winc"),
//             String::from("0"), String::from("btime"), String::from("60000"), String::from("binc"),
//             String::from("0")),
//                    Bot::tokenize(message));
//     }
//
//     #[test]
//     fn check_tokenize_double_whitespace() {
//         let message = "go  depth    7";
//         assert_eq!(vec!(
//             String::from("go"), String::from("depth"), String::from("7")),
//                    Bot::tokenize(message));
//     }
//
// }