//! Chess bot
//! Serves as an interface between the game host (i.e. GUI) and the engine.
//! Implements the Universal Chess Interface (UCI).
//! Communication is realized through the message() function.
//! It should be called in the game loop of the main function.

use std::alloc::System;
use std::fmt::{Debug, format};
use std::ops::Deref;
use std::slice::Iter;
use std::sync::{Arc, Mutex};
use std::thread::{spawn};
use std::time::Instant;

use scanner_rust::ScannerStr;

use crate::board::{Board, START_POSITION};
use crate::evaluation::{Evaluator, MainEvaluator};
use crate::move_generator::{Move, MoveList};
use crate::perft::perft_log;
use crate::search::Engine;
use crate::string_builder::StringBuilder;

const INFINITE_DEPTH: u32 = 256;

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

    /// Responds to a given message, according to the UCI standard.
    /// This function is the method used for manipulating the game state in the Bot object.
    /// Should a particular command or option not be implemented, it will notify about the fact
    /// with "Unexpected command. This command might be unsupported by the current version of the engine or by the UCI standard."
    /// Calls relevant response functions that use a StringBuilder to return a response String,
    /// which is printed out in this method.
    /// Returns Option from a response string if no error occurred.
    /// Returns None if the "quit" command was provided
    pub fn message(&mut self, message: &str) -> Vec<Option<String>> {
        let tokens = Self::tokenize(message);
        let mut responses = Vec::new();

        if tokens.is_empty() {
            responses.push(Some(String::from("No command has been found.")));
            return responses;
        }

        let mut tokens_iter = tokens.iter();

        while let Some(command) = tokens_iter.next() {
            let command = command.as_str();
            let response = match command {
                "uci" => Bot::uci(),
                "ucinewgame" => self.new_game(),
                "isready" => Self::readyok(),
                "position" => self.input_position(&mut tokens_iter),
                "go" => self.go(&mut tokens_iter),
                "stop" => self.stop(),
                "quit" => None,
                "player" => self.current_player(),
                "depth" => self.depth(),
                _ => Option::from(String::from(
                    format!("Unexpected command. The command {} might be unsupported by the current version of the engine or by the UCI standard.",
                command))),
            };

            let quit = response.is_none();

            responses.push(response);

            if quit { break; } // break immediately if we want to quit
        }

        responses

        // let mut scanner = ScannerStr::new(message);
        //
        // let command = scanner.next().unwrap_or_default().unwrap_or_default();
        // let response = match command {
        //     "uci" =>  Bot::uci(),
        //     "ucinewgame" => self.new_game(),
        //     "isready" => Self::readyok(),
        //     "position" => self.input_position(&mut scanner),
        //     "go" => self.go(String::from(scanner.next_line().unwrap().unwrap_or_default())),
        //     "stop" => self.stop(),
        //     "quit" => return None,
        //     "player" => { let list = self.move_list.lock().unwrap();
        //         ( list.get_board().active_player as u32).to_string() }
        //     "depth" => { let engine_binding = self.engine.lock().unwrap();
        //         engine_binding.get_current_depth().to_string() }
        //     _ => String::from("Unexpected command. This command might be unsupported by the current version of the engine or by the UCI standard."),
        // };
        //
        // Option::from(response)
    }

    /// Tokenizes a UCI-protocol command message.
    /// Given a message, the function parses it and outputs a vector of substrings contained
    /// within the original message. The delimiter is a whitespace.
    fn tokenize(message: &str) -> Vec<String> {
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

    /// Responds to a "position" command.
    /// Reads in the fen or the "startpos" attribute and adjusts the board accordingly.
    /// Makes moves on the board if there are moves specified with the "moves" attribute
    fn input_position(&mut self, tokens_iter: &mut Iter<String>) -> Option<String> {
        let response = String::new();
        // let mode = scanner.next().unwrap_or_default().unwrap_or_default();
        let mode = tokens_iter.next();
        if mode.is_none() {
            return Option::from(String::from("Invalid position."));
        }

        let mode = mode.unwrap().as_str();

        // we lock the move_list before using it
        let mut move_list_binding = self.move_list.lock().unwrap();
        let mut board = move_list_binding.get_mutable_board();

        match mode {
            "startpos" => *board = Board::from_fen(START_POSITION),
            "fen" => {let fen = Self::extract_fen(tokens_iter);
                *board = Board::from_fen(&fen);},
            _ => return Option::from(String::from("Invalid mode of position input"))
        }

        // we drop the binding, as we no longer need it, and it allows a mutable borrow of self later on
        drop(move_list_binding);

        let option = tokens_iter.next();
        match option {
            None => return Option::from(response),
            Some(subcommand) => if subcommand != "moves" { return Option::from(response) }
        }

        self.input_moves(tokens_iter);

        Option::from(response)
    }

    /// Extracts fen from a string scanner.
    /// The source string of the scanner should begin with the fen.
    fn extract_fen(tokens_iter: &mut Iter<String>) -> String {
        let mut fen = String::new();

        for _i in 0..5 {
            fen.push_str(tokens_iter.next().unwrap());
            fen.push(' ');
        }
        fen.push_str(tokens_iter.next().unwrap());

        fen
    }

    /// Inputs moves after a "moves" subcommand within the "position" command.
    /// Makes moves, one by one, on the board until the moves are exhausted
    fn input_moves(&mut self, tokens_iter: &mut Iter<String>) {
        let mut move_list_binding = self.move_list.lock().unwrap();
        while let Some(input) = tokens_iter.next() {
            let piece_move = Move::from_algebraic_notation(input, move_list_binding.get_board());
            move_list_binding.make_move(&piece_move);
        }
    }

    /// Responds to a "go" command, which requests position analysis.
    /// Supports following options:
    ///     "perft" - calls perft_log, outputting the number of nodes searched, the duration time,
    /// and the number of nodes from each immediate response
    ///     "infinite" - starts an infinite search, which can conclude with the "stop" command
    ///     "depth" - starts a search up to a given depth
    /// The calls are done on a separate thread, so the CLI can resume its work without
    /// waiting for the time-consuming computations.
    /// It responds with an empty string, as it returns before the computing threads conclude their work.
    /// Prints directly the responses of the computation thread.
    fn go(&mut self, tokens_iter: &mut Iter<String>) -> Option<String> {
        Engine::set_stop_flag(false);

        let move_list_arc = self.move_list.clone();
        let engine_arc = self.engine.clone();

        let mode = tokens_iter.next().unwrap().as_str();

        let response = match mode {
            "perft" => Self::go_perft(&mut *move_list_arc.lock().unwrap(), tokens_iter),
            "infinite" => Self::go_infinite(&mut *engine_arc.lock().unwrap(), &mut *move_list_arc.lock().unwrap()),
            "depth" => Self::go_depth(&mut *engine_arc.lock().unwrap(), &mut *move_list_arc.lock().unwrap(),
                                      tokens_iter),
            _ => Self::go_infinite(&mut *engine_arc.lock().unwrap(), &mut *move_list_arc.lock().unwrap())
        };

        response
    }

    /// Responds to a "go perft" command.
    /// Calls the perft_log function, outputting the number of nodes searched, the duration time,
    /// and the number of nodes from each immediate response
    fn go_perft(move_list: &mut MoveList, tokens_iter: &mut Iter<String>) -> Option<String> {
        let depth = tokens_iter.next().unwrap().parse::<u32>().unwrap_or_default();

        let start = Instant::now();
        let nodes = perft_log(move_list, depth);
        let duration = start.elapsed();

        let response = format!("perft {} searched {} nodes in {:?}", depth, nodes, duration);
        Option::from(response)
    }

    /// Responds to a "go infinite" command.
    /// Orders the engine to search indefinitely until the "stop" command is received.
    /// (We limit the depth with an actual finite constant,
    /// though it will not be reached in practice).
    /// Responds with an empty string.
    fn go_infinite(engine: &mut Engine, move_list: &mut MoveList) -> Option<String> {
        engine.search(move_list, INFINITE_DEPTH);

        Self::best_move(engine, move_list)
    }

    /// Responds to a "go depth" command.
    /// Orders the engine to search up to the given depth.
    /// If uninterrupted, responds with the "best_move" communicate,
    /// otherwise, returns an empty string
    fn go_depth(engine: &mut Engine, move_list: &mut MoveList, tokens_iter: &mut Iter<String>) -> Option<String> {
        let depth = tokens_iter.next().unwrap().parse::<u32>().unwrap_or_default();
        engine.search(move_list, depth);

        Self::best_move(engine, move_list)

        // return if !Engine::get_stop_flag() {
        //     Self::best_move(engine, move_list)
        // } else { String::new() }
    }

    /// Responds to a "stop command".
    /// Immediately ceases further position analysis.
    /// Responds with the best move
    fn stop(&mut self) -> Option<String> {
        Engine::set_stop_flag(true);

        // Bot::best_move(&mut self.engine.lock().unwrap(), &mut self.move_list.lock().unwrap())
        Option::from(String::new())
    }

    /// Implements the "best_move" UCI command.
    /// Retrieves what the engine currently deems the best move.
    /// Should not be used before doing any search.
    fn best_move(engine: &mut Engine, move_list: &mut MoveList) -> Option<String> {
        let mut response = String::from("bestmove ");
        response.push_str(&*engine.get_best_move(move_list.get_board()).to_algebraic_notation());

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
//         assert_eq!(expected,  Bot::uci());
//     }
//
//     #[test]
//     fn check_new_game() {
//         let mut bot = Bot::with_position(START_POSITION);
//         bot.engine.lock().unwrap().search(&mut bot.move_list.lock().unwrap(), 1);
//         bot.move_list.lock().unwrap().generate_moves();
//         assert!(!bot.move_list.lock().unwrap().get_moves().is_empty());
//
//         assert_eq!("", bot.new_game());
//
//         assert!(bot.move_list.lock().unwrap().get_moves().is_empty());
//         assert_eq!(Engine::new(), *bot.engine.lock().unwrap().deref());
//     }
//
//     #[test]
//     fn check_readyok() {
//         assert_eq!("readyok",  Bot::readyok());
//     }
//
//     #[test]
//     fn check_input_moves() {
//         let mut bot = Bot::with_position(START_POSITION);
//         bot.input_moves(&mut ScannerStr::new(&"e2e4 e7e5"));
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
//
//         assert_eq!("", bot.input_position(&mut ScannerStr::new(&"startpos")));
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
//
//         assert_eq!("", bot.input_position(&mut ScannerStr::new(&"fen 8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1")));
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
//         assert_eq!(START_POSITION,  Bot::extract_fen(&mut ScannerStr::new(&fen)));
//     }
//
//     #[test]
//     fn check_input_position_fen_moves() {
//         let mut bot = Bot::new();
//
//         assert_eq!("", bot.input_position(&mut ScannerStr::new(&"fen 8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1 moves e2e4")));
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
//
//         assert_eq!("", bot.input_position(&mut ScannerStr::new(&"startpos moves e2e4")));
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
//
//         let expected_regex = Regex::new(r"^perft 1 searched \d+ nodes in (\d+.\d+|\d+)(ns|µs|ms|s)$").unwrap();
//
//         let response = Bot::go_perft(&mut bot.move_list.lock().unwrap(), &mut ScannerStr::new(&"1"));
//         println!("{}", response);
//
//         assert!(expected_regex.is_match(&*response));
//     }
//
//     #[test]
//     fn check_go_depth() {
//         let mut bot = Bot::with_position(START_POSITION);
//
//         let expected_regex = Regex::new(r"^bestmove [a-h][1-8][a-h][1-8]$").unwrap();
//
//         let response = Bot::go_depth(&mut bot.engine.lock().unwrap(), &mut bot.move_list.lock().unwrap(), &mut ScannerStr::new(&"2"));
//         println!("{}", response);
//
//         assert!(expected_regex.is_match(&*response));
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
//         spawn(move || {
//             let response = Bot::go_infinite(&mut engine.lock().unwrap(), &mut move_list.lock().unwrap());
//             println!("{}", response);
//             assert!(response.is_empty());
//         });
//
//         thread::sleep(Duration::from_secs(2));
//         let response = bot.stop();
//
//         assert!(expected_regex.is_match(&*response));
//     }
//
//     #[test]
//     fn check_go() {
//         let mut bot = Bot::with_position(START_POSITION);
//
//         // go perft
//
//         // currently it is impossible to test the output of perft from the go() perspective,
//         // it can only be tested directly at go_perft(),
//         // as the answer is printed in the "left_over" thread of go()
//         let response = bot.go(String::from("perft 1"));
//         assert!(response.is_empty());
//
//         // go depth
//
//         let expected_regex = Regex::new(r"^bestmove [a-h][1-8][a-h][1-8]$").unwrap();
//
//         // currently it is impossible to test the output of depth from the go() perspective,
//         // it can only be tested directly at go_depth(),
//         // as the answer is printed in the "left_over" thread of go()
//         let response = bot.go(String::from("depth 2"));
//         assert!(response.is_empty());
//
//         thread::sleep(Duration::from_secs(1));
//         let response = Bot::best_move(&mut bot.engine.lock().unwrap(), &mut bot.move_list.lock().unwrap());
//         assert!(expected_regex.is_match(&*response));
//
//         // go infinite
//
//         bot.new_game();
//
//         let response = bot.go(String::from("infinite"));
//         assert!(response.is_empty());
//
//         thread::sleep(Duration::from_secs(1));
//         bot.stop();
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
//         let response = bot.stop();
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
//         let response = bot.message("go infinite").unwrap_or_default();
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