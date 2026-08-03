//! Chess bot
//! Serves as an interface between the game host (i.e. GUI) and the engine.
//! Implements the Universal Chess Interface (UCI).
//! Communication is realized through the message() function.
//! It should be called in the game loop of the main function.

use std::alloc::System;
use std::fmt::{Debug, format};
use std::iter::Peekable;
use std::ops::Deref;
use std::slice::Iter;
use std::str::SplitWhitespace;
use std::sync::{Arc, Mutex};
use std::thread::{sleep, spawn};
use std::time::{Duration, Instant};

use scanner_rust::ScannerStr;

use crate::board::{Board, START_POSITION};
use crate::evaluation::{Evaluator, MainEvaluator};
use crate::move_generator::{Move, MoveList};
use crate::perft::perft_log;
use crate::search::Engine;
use crate::string_builder::StringBuilder;
use crate::transposition_table::Transposition;

const INFINITE_DEPTH: u32 = 256;

#[derive(Debug)]
pub enum Command {
    Uci,
    IsReady,
    UciNewGame,
    SetOption {name: String, value: Option<String>},
    Position {fen: Option<String>, startpos: bool, moves: Vec<String>},
    Go(SearchSettings),
    Stop,
    PonderHit,
    Quit,
    Unknown(String),
}

#[derive(Debug, Default, Clone)]
enum SearchMode {
    #[default]
    InGame,
    Infinite,
    Depth(Option<u32>),
    Perft(Option<u32>),
    Ponder
}

#[derive(Debug, Default, Clone)]
pub struct SearchSettings {
    pub wtime: Option<u64>,
    pub btime: Option<u64>,
    pub winc: Option<u64>,
    pub binc: Option<u64>,
    pub moves_to_go: Option<u32>,
    // pub depth: Option<u32>,
    pub nodes: Option<u64>,
    pub mate: Option<u32>,
    pub move_time: Option<u64>,
    // pub infinite: bool,
    // pub perft: bool,
    // pub ponder: bool,
    pub mode: SearchMode,
    pub search_moves: Vec<String>
}

pub struct Bot<T: Evaluator = MainEvaluator> {
    engine: Arc<Mutex<Engine<T>>>,
    move_list: Arc<Mutex<MoveList>>,
}

impl Bot<MainEvaluator> {
    pub fn new() -> Self {
        Bot {
            engine: Arc::new(Mutex::new(Engine::new())),
            move_list: Arc::new(Mutex::new(MoveList::new()))
        }
    }

    pub fn with_position(fen: &str) -> Self {
        let evaluator = MainEvaluator {};
        Bot {
            engine: Arc::new(Mutex::new(Engine::with_evaluator(evaluator))),
            move_list: Arc::new(Mutex::new(MoveList::from_fen(fen))),
        }
    }

    pub fn parse(message: &str) -> Option<Command> {
        let message = message.trim();

        // Messages starting with '#' should be treated as comments and, thus, ignored.
        if message.is_empty() || message.starts_with('#') {
            return None;
        }

        let message = message.to_lowercase();
        let mut iter = message.split_whitespace().peekable();
        let Some(head) = iter.next() else { return None; };

        match head {
            "uci" => Some(Command::Uci),
            "isready" => Some(Command::IsReady),
            "ucinewgame" => Some(Command::UciNewGame),
            "stop" => Some(Command::Stop),
            "ponderhit" => Some(Command::PonderHit),
            "quit" => Some(Command::Quit),

            "setoption" => Self::parse_set_option(&mut iter, &message),
            "position" => Self::parse_position(&mut iter, &message),
            "go" => Self::parse_go(&mut iter, &message),
            _ => Some(Command::Unknown(message.to_string())),
        }
    }

    fn parse_set_option(iter: &mut Peekable<SplitWhitespace>, message: &String) -> Option<Command> {
        let mut name_tokens: Vec<&str> = Vec::new();
        let mut value_tokens: Vec<&str> = Vec::new();

        if iter.next()? != "name" {
            return Some(Command::Unknown((*message.clone()).parse().unwrap()));
        }

        while let Some(token) = iter.next() {
            if token == "value" {
                break;
            }
            name_tokens.push(token);
        }

        while let Some(token) = iter.next() {
            value_tokens.push(token);
        }

        let name = name_tokens.join(" ");
        let value = if value_tokens.is_empty() { None } else { Some(value_tokens.join(" ")) };

        Some(Command::SetOption { name, value })
    }

    fn parse_position(iter: &mut Peekable<SplitWhitespace>, message: &String) -> Option<Command> {
        let mut startpos = false;
        let mut fen: Option<String> = None;
        let mut moves: Vec<String> = Vec::new();

        match iter.next() {
            Some("startpos") => {
                startpos = true;

                // Consume the "moves" token
                if iter.peek() == Some(&"moves") {
                    iter.next();
                }
            }
            Some("fen") => {
                let mut fen_tokens: Vec<&str> = Vec::new();
                while let Some(token) = iter.next() {
                    if token == "moves" {
                        // Consume the "moves" token
                        break;
                    }
                    fen_tokens.push(token);
                }
                if !fen_tokens.is_empty() {
                    fen = Some(fen_tokens.join(" "));
                }
            }
            _ => return Some(Command::Unknown(message.clone().parse().unwrap())),
        }

        while let Some(piece_move) = iter.next() {
            moves.push(piece_move.to_string());
        }

        Some(Command::Position { fen, startpos, moves })
    }

    fn parse_go(iter: &mut Peekable<SplitWhitespace>, message: &String) -> Option<Command> {
        let mut settings = SearchSettings::default();

        while let Some(token) = iter.next() {
            match token {
                "wtime" => settings.wtime = iter.next().and_then(|s| s.parse().ok()),
                "btime" => settings.btime = iter.next().and_then(|s| s.parse().ok()),
                "winc" => settings.winc = iter.next().and_then(|s| s.parse().ok()),
                "binc" => settings.binc = iter.next().and_then(|s| s.parse().ok()),
                "movestogo" => settings.moves_to_go = iter.next().and_then(|s| s.parse().ok()),
                "depth" => settings.mode = SearchMode::Depth(iter.next().and_then(|s| s.parse().ok())),
                "nodes" => settings.nodes = iter.next().and_then(|s| s.parse().ok()),
                "mate" => settings.mate = iter.next().and_then(|s| s.parse().ok()),
                "movetime" => settings.move_time = iter.next().and_then(|s| s.parse().ok()),
                "infinite" => settings.mode = SearchMode::Infinite,
                "perft" => settings.mode = SearchMode::Perft(iter.next().and_then(|s| s.parse().ok())),
                "ponder" => settings.mode = SearchMode::Ponder,

                "searchmoves" => {
                    while let Some(piece_move) = iter.next() {
                        settings.search_moves.push(piece_move.to_string());
                    }
                    break;
                }

                _ => return Some(Command::Unknown((*message.clone()).parse().unwrap())),
            }
        }

        Some(Command::Go(settings))
    }
    
    pub fn process(&mut self, command: Command) -> Option<String> {
        match command {
            Command::Uci => Self::uci(),
            Command::IsReady => Self::readyok(),
            Command::UciNewGame => self.new_game(),
            Command::SetOption { name, value } => Self::set_option(),
            Command::Position { fen, startpos, moves } =>
                self.input_position(fen, startpos, moves),
            Command::Go(settings) => self.go(settings),
            Command::Stop => Self::stop(),
            Command::PonderHit => self.ponder_hit(),
            Command::Quit => None,
            Command::Unknown(message) => Self::unsupported_command(message),
        }
    }

    /// Responds to a given message, according to the UCI standard.
    /// This function is the method used for manipulating the game state in the Bot object.
    /// Should a particular command or option not be implemented, it will notify about the fact
    /// with "Unexpected command. This command might be unsupported by the current version of the engine or by the UCI standard."
    /// Calls relevant response functions that use a StringBuilder to return a response String,
    /// which is printed out in this method.
    /// Returns Option from a response string if no error occurred.
    /// Returns None if the "quit" command was provided
    pub fn message(&mut self, message: &str) -> Option<String> {
        let command = Self::parse(message);

        if command.is_none() {
            return None;
        }

        let command = command.unwrap();
        self.process(command)
    }
    // pub fn message(&mut self, message: &str) -> Vec<Option<String>> {
    //     let tokens = Self::tokenize(message);
    //     let mut responses = Vec::new();
    // 
    //     if tokens.is_empty() {
    //         responses.push(Some(String::from("No command has been found.")));
    //         return responses;
    //     }
    // 
    //     let mut tokens_iter = tokens.iter();
    // 
    //     while let Some(command) = tokens_iter.next() {
    //         let command = command.as_str();
    //         let response = match command {
    //             "uci" => Bot::uci(),
    //             "ucinewgame" => self.new_game(),
    //             "isready" => Self::readyok(),
    //             "position" => self.input_position(&mut tokens_iter),
    //             "go" => self.go(&mut tokens_iter),
    //             "stop" => Self::stop(),
    //             "quit" => None,
    //             "player" => self.current_player(),
    //             "depth" => self.depth(),
    //             _ => Option::from(String::from(
    //                 format!("Unexpected command. The command {} might be unsupported by the current version of the engine or by the UCI standard.",
    //             command))),
    //         };
    // 
    //         let quit = response.is_none();
    // 
    //         responses.push(response);
    // 
    //         if quit { break; } // break immediately if we want to quit
    //     }
    // 
    //     responses
    // 
    //     // let mut scanner = ScannerStr::new(message);
    //     //
    //     // let command = scanner.next().unwrap_or_default().unwrap_or_default();
    //     // let response = match command {
    //     //     "uci" =>  Bot::uci(),
    //     //     "ucinewgame" => self.new_game(),
    //     //     "isready" => Self::readyok(),
    //     //     "position" => self.input_position(&mut scanner),
    //     //     "go" => self.go(String::from(scanner.next_line().unwrap().unwrap_or_default())),
    //     //     "stop" => self.stop(),
    //     //     "quit" => return None,
    //     //     "player" => { let list = self.move_list.lock().unwrap();
    //     //         ( list.get_board().active_player as u32).to_string() }
    //     //     "depth" => { let engine_binding = self.engine.lock().unwrap();
    //     //         engine_binding.get_current_depth().to_string() }
    //     //     _ => String::from("Unexpected command. This command might be unsupported by the current version of the engine or by the UCI standard."),
    //     // };
    //     //
    //     // Option::from(response)
    // }

    /// Tokenizes a UCI-protocol command message.
    /// Given a message, the function parses it and outputs a vector of substrings contained
    /// within the original message. The delimiter is a whitespace.
    pub fn tokenize(message: &str) -> Vec<String> {
        let mut scanner = ScannerStr::new(message);
        let mut tokens = Vec::new();

        while let response = scanner.next() {
            if response.is_err() {
                eprintln!("Error: could not parse the message. Tokenizing failed.");
                return tokens;
            }

            let response_unwrapped = response.unwrap();
            if response_unwrapped.is_none() {break;}
            tokens.push(String::from(response_unwrapped.unwrap_or_default()));
        }

        tokens
    }

    /// Responds to a "uci" command.
    /// Returns the id of the engine (name, version, author) and available options.
    /// Ends the response with "uciok"
    fn uci() -> Option<String> {
        let mut response = StringBuilder::new();
        response
            .append_line(&Bot::id())
            .append_line(&Bot::option())
            .append_line(&"uciok");
        Option::from(response.build())
    }

    /// Responds to a "isready" command with "readyok".
    fn readyok() -> Option<String> {
        Option::from(String::from("readyok"))
    }

    /// Responds to a "ucinewgame" command.
    /// Creates a new engine object and clears the move list
    fn new_game(&mut self) -> Option<String> {
        self.engine = Arc::new(Mutex::new(Engine::new()));
        self.move_list.lock().unwrap().clear();
        Option::from(String::new())
    }

    fn input_position(&mut self, fen: Option<String>, startpos: bool, moves: Vec<String>) -> Option<String> {
        let response = String::new();

        Engine::set_stop_flag(true);

        // we lock the move_list before using it
        let mut move_list_binding = self.move_list.lock().unwrap();
        let mut board = move_list_binding.get_mutable_board();

        if startpos {
            *board = Board::from_fen(START_POSITION);
        }
        else {
            match fen {
                Some(fen) => *board = Board::from_fen(& *fen),
                None => return Some(String::from("No FEN has been detected! Provide a valid position."))
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
        Engine::set_stop_flag(false);

        let move_list_arc = self.move_list.clone();
        let engine_arc = self.engine.clone();

        let response = match settings.mode {
            SearchMode::InGame => Self::go_ingame(&mut *engine_arc.lock().unwrap(), &mut *move_list_arc.lock().unwrap()),
            SearchMode::Infinite => Self::go_infinite(&mut *engine_arc.lock().unwrap(), &mut *move_list_arc.lock().unwrap()),
            SearchMode::Depth(depth) => Self::go_depth(&mut *engine_arc.lock().unwrap(), &mut *move_list_arc.lock().unwrap(),
                                      depth),
            SearchMode::Perft(depth) => Self::go_perft(&mut *move_list_arc.lock().unwrap(), depth),
            SearchMode::Ponder => Self::go_ingame(&mut *engine_arc.lock().unwrap(), &mut *move_list_arc.lock().unwrap()),
            //_ => Self::go_infinite(&mut *engine_arc.lock().unwrap(), &mut *move_list_arc.lock().unwrap())
        };

        response
    }

    /// Responds to a "go perft" command.
    /// Calls the perft_log function, outputting the number of nodes searched, the duration time,
    /// and the number of nodes from each immediate response
    fn go_perft(move_list: &mut MoveList, depth: Option<u32>) -> Option<String> {
        if depth.is_none() {
            return Some(String::from("No depth has been found!"));
        }
        let depth = depth.unwrap();

        let start = Instant::now();
        let nodes = perft_log(move_list, depth);
        let duration = start.elapsed();

        let response = format!("perft {} searched {} nodes in {:?}", depth, nodes, duration);
        Option::from(response)
    }

    /// Responds to a "go Infinite" command.
    /// Orders the engine to search indefinitely until the "stop" command is received.
    /// (We limit the depth with an actual finite constant,
    /// though it will not be reached in practice).
    /// Responds with an empty string.
    fn go_infinite(engine: &mut Engine, move_list: &mut MoveList) -> Option<String> {
        engine.search(move_list, INFINITE_DEPTH);

        Self::best_move(engine, move_list)
    }

    /// Responds to a "go Infinite" command.
    /// Orders the engine to search indefinitely until the "stop" command is received.
    /// (We limit the depth with an actual finite constant,
    /// though it will not be reached in practice).
    /// Responds with an empty string.
    fn go_ingame(engine: &mut Engine, move_list: &mut MoveList) -> Option<String> {
        let depth = 6;

        engine.search(move_list, depth);

        Self::best_move(engine, move_list)
    }

    /// Responds to a "go depth" command.
    /// Orders the engine to search up to the given depth.
    /// If uninterrupted, responds with the "best_move" communicate,
    /// otherwise, returns an empty string
    fn go_depth(engine: &mut Engine, move_list: &mut MoveList, depth: Option<u32>) -> Option<String> {
        if depth.is_none() {
            return Some(String::from("No depth has been found!"));
        }
        let depth = depth.unwrap();

        engine.search(move_list, depth);

        Self::best_move(engine, move_list)
    }

    /// Responds to a "stop command".
    /// Immediately ceases further position analysis.
    /// Responds with the best move
    fn stop() -> Option<String> {
        Engine::set_stop_flag(true);

        // Bot::best_move(&mut self.engine.lock().unwrap(), &mut self.move_list.lock().unwrap())
        Option::from(String::new())
    }

    fn ponder_hit(&mut self) -> Option<String> {
        loop {
            let engine_binding = self.engine.lock().unwrap();
            if engine_binding.get_current_depth() >= 6 {
                Engine::set_stop_flag(true);
                break;
            }
            sleep(Duration::from_millis(50));
        }

        let mut engine_binding = self.engine.lock().unwrap();
        let mut move_list_binding = self.move_list.lock().unwrap();

        Self::best_move(&mut engine_binding, &mut move_list_binding)
    }

    fn set_option() -> Option<String> {
        Option::from(String::new())
    }

    fn unsupported_command(command: String) -> Option<String> {
        Option::from(String::from(
            format!("Unexpected command. The command {} might be unsupported by the current version of the engine or by the UCI standard.",
                    command)))
    }

    fn info(engine: &mut Engine, move_list: &mut MoveList) -> Option<String> {
        let transposition = engine.get_transposition(move_list.get_board()).unwrap();

        //todo: add time
        //todo: add nodes
        //todo: add nps
        //todo: add cpuload
        let depth = engine.get_current_depth();
        let score_cp = transposition.value;
        Option::from(String::from(format!("info depth {} score cp {}", depth, score_cp)))
    }

    /// Implements the "best_move" UCI command.
    /// Retrieves what the engine currently deems the best move.
    /// Should not be used before doing any search.
    fn best_move(engine: &mut Engine, move_list: &mut MoveList) -> Option<String> {
        let mut response = Self::info(engine, move_list).unwrap();
        response.push_str("\nbestmove ");

        let best_move = engine.get_best_move(move_list.get_board());
        response.push_str(&*best_move.to_algebraic_notation());

        move_list.make_move(&best_move);
        let ponder_transposition = engine.get_transposition(move_list.get_board());
        move_list.unmake_move(&best_move);

        if ponder_transposition.is_none() {
            return Option::from(response)
        }
        let ponder_move = ponder_transposition.unwrap().best_moves[0];
        if ponder_move.is_none() {
            return Option::from(response)
        }

        response.push_str(" ponder ");
        response.push_str(&*ponder_move.unwrap().to_algebraic_notation());

        Option::from(response)
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