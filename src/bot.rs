//! Chess bot
//! Serves as an interface between the game host (i.e. GUI) and the engine.
//! Implements the Universal Chess Interface (UCI).
//! Communication is realized through the message() function.
//! It should be called in the game loop of the main function.

use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::thread::{spawn};
use std::time::Instant;

use scanner_rust::ScannerStr;

use crate::board::{Board, START_POSITION};
use crate::move_generator::{Move, MoveList};
use crate::perft::perft_log;
use crate::search::Engine;
use crate::string_builder::StringBuilder;

const INFINITE_DEPTH: u32 = 256;

pub struct Bot {
    engine: Arc<Mutex<Engine>>,
    move_list: Arc<Mutex<MoveList>>,
}

impl Bot {
    pub fn new() -> Self {
        Bot {
            engine: Arc::new(Mutex::new(Engine::new())),
            move_list: Arc::new(Mutex::new(MoveList::new()))
        }
    }

    pub fn with_position(fen: &str) -> Self {
        Bot {
            engine: Arc::new(Mutex::new(Engine::new())),
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
    pub fn message(&mut self, message: &str) -> Option<String> {
        let mut scanner = ScannerStr::new(message);

        let command = scanner.next().unwrap_or_default().unwrap_or_default();
        let response = match command {
            "uci" => Bot::uci(),
            "ucinewgame" => self.new_game(),
            "isready" => Self::readyok(),
            "position" => self.input_position(&mut scanner),
            "go" => self.go(String::from(scanner.next_line().unwrap().unwrap_or_default())),
            "stop" => self.stop(),
            "quit" => return None,
            _ => String::from("Unexpected command. This command might be unsupported by the current version of the engine or by the UCI standard."),
        };

        Option::from(response)
    }

    /// Responds to a "uci" command.
    /// Returns the id of the engine (name, version, author) and available options.
    /// Ends the response with "uciok"
    fn uci() -> String {
        let mut response = StringBuilder::new();
        response
            .append_line(&Bot::id())
            .append_line(&Bot::option())
            .append_line(&"uciok");
        response.build()
    }

    /// Responds to a "isready" command with "readyok".
    fn readyok() -> String {
        String::from("readyok")
    }

    /// Responds to a "ucinewgame" command.
    /// Creates a new engine object and clears the move list
    fn new_game(&mut self) -> String {
        self.engine = Arc::new(Mutex::new(Engine::new()));
        self.move_list.lock().unwrap().clear();
        String::new()
    }

    /// Responds to a "position" command.
    /// Reads in the fen or the "startpos" attribute and adjusts the board accordingly.
    /// Makes moves on the board if there are moves specified with the "moves" attribute
    // #[inline(never)]
    fn input_position(&mut self, scanner: &mut ScannerStr) -> String {
        let response = String::new();
        let mode = scanner.next().unwrap_or_default().unwrap_or_default();

        // we lock the move_list before using it
        let mut move_list_binding = self.move_list.lock().unwrap();
        let mut board = move_list_binding.get_mutable_board();

        match mode {
            "startpos" => board.read_fen(START_POSITION),
            "fen" => {let fen = Self::extract_fen(scanner);
                board.read_fen(&fen);},
            _ => return response
        }

        // we drop the binding, as we no longer need it, and it allows a mutable borrow of self later on
        drop(move_list_binding);

        let option = scanner.next().unwrap_or_default();
        match option {
            None => return response,
            Some(subcommand) => if subcommand != "moves" { return response }
        }

        self.input_moves(scanner);

        response
    }

    /// Extracts fen from a string scanner
    // #[inline(never)]
    fn extract_fen(scanner: &mut ScannerStr) -> String {
        let mut fen = String::new();

        for _i in 0..5 {
            fen.push_str(scanner.next().unwrap_or_default().unwrap_or_default());
            fen.push(' ');
        }
        fen.push_str(scanner.next().unwrap_or_default().unwrap_or_default());

        fen
    }

    /// Inputs moves after a "moves" subcommand within the "position" command.
    /// Makes moves, one by one, on the board until the moves are exhausted
    // #[inline(never)]
    fn input_moves(&mut self, scanner: &mut ScannerStr) {
        while let Some(input) = scanner.next().unwrap_or_default() {
            let mut move_list_binding = self.move_list.lock().unwrap();
            let piece_move = Move::from_algebraic_notation(input, move_list_binding.get_board());
            move_list_binding.make_move(&piece_move);
        }
    }

    /// Responds to a "go" command, which requests position analysis.
    /// Supports following options:
    ///     "perft" - calls perft_log, outputting the number of nodes searched, the duration time,
    /// and the number of nodes from each immediate response
    ///     "infinite" - starts an infinite search, which can conclude with the "stop" command
    fn go(&mut self, command_line: String) -> String {
        Engine::set_stop_flag(false);

        // let command_line_clone = command_line.clone();
        let move_list_arc = self.move_list.clone();
        let engine_arc = self.engine.clone();

        spawn(move || {
            let mut scanner = ScannerStr::new(&command_line);
            let mode = scanner.next().unwrap_or_default().unwrap_or_default();

            let response = match mode {
                "perft" => Self::go_perft(&mut *move_list_arc.lock().unwrap(), &mut scanner),
                "infinite" => Self::go_infinite(&mut *engine_arc.lock().unwrap(), &mut *move_list_arc.lock().unwrap()),
                "depth" => Self::go_depth(&mut *engine_arc.lock().unwrap(), &mut *move_list_arc.lock().unwrap(), &mut scanner),
                _ => Self::go_infinite(&mut *engine_arc.lock().unwrap(), &mut *move_list_arc.lock().unwrap())
            };

            println!("{}", response);
        });

        String::new()
    }

    /// Responds to a "go perft" command.
    /// Calls the perft_log function, outputting the number of nodes searched, the duration time,
    /// and the number of nodes from each immediate response
    fn go_perft(move_list: &mut MoveList, scanner: &mut ScannerStr) -> String {
        let depth = scanner.next().unwrap_or_default().unwrap_or_default().parse::<u32>().unwrap_or_default();
        println!("{}", depth);

        let start = Instant::now();
        let nodes = perft_log(move_list, depth);
        let duration = start.elapsed();

        let response = format!("perft {} searched {} nodes in {:?}", depth, nodes, duration);
        response
    }

    /// Responds to a "go infinite" command.
    /// todo: TBD
    fn go_infinite(engine: &mut Engine, move_list: &mut MoveList) -> String {
        engine.search(move_list, INFINITE_DEPTH);

        String::new()
    }

    /// Responds to a "go depth" command.
    /// todo: TBD
    fn go_depth(engine: &mut Engine, move_list: &mut MoveList, scanner: &mut ScannerStr) -> String {
        let depth = scanner.next().unwrap_or_default().unwrap_or_default().parse::<u32>().unwrap_or_default();
        engine.search(move_list, depth);

        String::new()
    }

    /// Responds to a "stop command".
    /// Immediately ceases further position analysis.
    /// Responds with the best move
    fn stop(&mut self) -> String {
        Engine::set_stop_flag(true);

        self.best_move()
    }

    /// Implements the "best_move" UCI command.
    /// Retrieves what the engine currently deems the best move
    fn best_move(&mut self) -> String {
        let mut response = String::from("bestmove ");
        response.push_str(&*self.engine.lock().unwrap().get_best_move(self.move_list.lock().unwrap().get_board()).to_algebraic_notation());

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
        response.build()
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;
    use std::ops::Deref;
    use regex::Regex;

    use crate::piece::Piece::PAWN;

    use super::*;

    #[test]
    fn check_id() {
        let expected = format!("id name Kenobi {}\nid author Jakub Pietrzak", env!("CARGO_PKG_VERSION"));
        assert_eq!(expected, Bot::id());
    }

    #[test]
    fn check_option() {
        let expected = "";
        assert_eq!(expected, Bot::option());
    }

    #[test]
    fn check_uci() {
        let expected = format!("id name Kenobi {}\nid author Jakub Pietrzak\nuciok", env!("CARGO_PKG_VERSION"));
        assert_eq!(expected, Bot::uci());
    }

    #[test]
    fn check_new_game() {
        let mut bot = Bot::with_position(START_POSITION);
        bot.engine.lock().unwrap().search(&mut bot.move_list.lock().unwrap(), 1);
        bot.move_list.lock().unwrap().generate_moves();
        assert!(!bot.move_list.lock().unwrap().get_moves().is_empty());

        assert_eq!("", bot.new_game());

        assert!(bot.move_list.lock().unwrap().get_moves().is_empty());
        assert_eq!(Engine::new(), *bot.engine.lock().unwrap().deref());
    }

    #[test]
    fn check_readyok() {
        assert_eq!("readyok", Bot::readyok());
    }

    #[test]
    fn check_input_moves() {
        let mut bot = Bot::with_position(START_POSITION);
        bot.input_moves(&mut ScannerStr::new(&"e2e4 e7e5"));

        let mut expected_bot = Bot::with_position(START_POSITION);
        expected_bot.move_list.lock().unwrap().make_move(&Move{origin: 11, target: 27, promotion: 0, piece: PAWN});
        expected_bot.move_list.lock().unwrap().make_move(&Move{origin: 51, target: 35, promotion: 0, piece: PAWN});

        assert_eq!(expected_bot.move_list.lock().unwrap().get_board(), bot.move_list.lock().unwrap().get_board());
    }

    #[test]
    fn check_input_position_start_position() {
        let mut bot = Bot::new();

        assert_eq!("", bot.input_position(&mut ScannerStr::new(&"startpos")));

        let mut expected_board = Board::new();
        expected_board.read_fen(START_POSITION);

        assert_eq!(expected_board, *bot.move_list.lock().unwrap().get_board());
    }

    #[test]
    fn check_input_position_fen() {
        let mut bot = Bot::new();

        assert_eq!("", bot.input_position(&mut ScannerStr::new(&"fen 8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1")));

        let mut expected_board = Board::new();
        expected_board.read_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1");

        assert_eq!(expected_board, *bot.move_list.lock().unwrap().get_board());
    }

    #[test]
    fn check_extract_fen() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 blabla yadayada";
        assert_eq!(START_POSITION, Bot::extract_fen(&mut ScannerStr::new(&fen)));
    }

    #[test]
    fn check_input_position_fen_moves() {
        let mut bot = Bot::new();

        assert_eq!("", bot.input_position(&mut ScannerStr::new(&"fen 8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1 moves e2e4")));

        let mut expected_board = Board::new();
        expected_board.read_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1");
        let mut expected_move_list = MoveList::from_board(expected_board);
        expected_move_list.make_move(&Move{origin: 11, target: 27, promotion: 0, piece: PAWN});

        assert_eq!(expected_move_list.get_board(), bot.move_list.lock().unwrap().get_board());
    }

    #[test]
    fn check_input_position_startpos_moves() {
        let mut bot = Bot::new();

        assert_eq!("", bot.input_position(&mut ScannerStr::new(&"startpos moves e2e4")));

        let mut expected_board = Board::new();
        expected_board.read_fen(START_POSITION);
        let mut expected_move_list = MoveList::from_board(expected_board);
        expected_move_list.make_move(&Move{origin: 11, target: 27, promotion: 0, piece: PAWN});

        assert_eq!(expected_move_list.get_board(), bot.move_list.lock().unwrap().get_board());
    }

    #[test]
    fn check_go_perft() {
        let mut bot = Bot::with_position(START_POSITION);

        let expected_regex = Regex::new(r"^perft 1 searched \d+ nodes in (\d+.\d+|\d+)(ns|µs|ms|s)$").unwrap();

        let response = Bot::go_perft(&mut bot.move_list.lock().unwrap(), &mut ScannerStr::new(&"1"));
        println!("{}", response);

        assert!(expected_regex.is_match(&*response));
    }

    // #[test]
    // fn check_go() {
    //     let mut bot = Bot::with_position(START_POSITION);
    //
    //     let expected_regex = Regex::new(r"^perft 1 searched \d+ nodes in (\d+.\d+|\d+)(ns|µs|ms|s)$").unwrap();
    //
    //     let response = bot.go(&mut ScannerStr::new(&"perft 1"));
    //     println!("response: {}", response);
    //
    //     assert!(expected_regex.is_match(&*response));
    //
    //     // todo: test the infinite option once it's properly implemented
    // }

    #[test]
    fn check_stop() {
        let mut bot = Bot::with_position(START_POSITION);

        bot.engine.lock().unwrap().search(&mut bot.move_list.lock().unwrap(), 1);

        let expected_regex = Regex::new(r"^bestmove [a-h][1-8][a-h][1-8]$").unwrap();

        let response = bot.stop();
        assert!(expected_regex.is_match(&*response));

    }

    #[test]
    fn test_message() {
        let mut bot = Bot::with_position(START_POSITION);

        assert_eq!(None, bot.message("quit"));
        assert_eq!(Option::from(
            String::from("Unexpected command. This command might be unsupported by the current version of the engine or by the UCI standard.")),
                   bot.message("illegal command"));
        assert_eq!(Option::from(
            String::from(format!("id name Kenobi {}\nid author Jakub Pietrzak\nuciok", env!("CARGO_PKG_VERSION")))),
                   bot.message("uci"));

        bot.engine.lock().unwrap().search(&mut bot.move_list.lock().unwrap(), 1);
        bot.move_list.lock().unwrap().generate_moves();
        assert_eq!(Option::from(
            String::from("")),
                   bot.message("ucinewgame"));
        assert!(bot.move_list.lock().unwrap().get_moves().is_empty());
        assert_eq!(Engine::new(), *bot.engine.lock().unwrap().deref());

        assert_eq!(Option::from(
            String::from("readyok")),
                   bot.message("isready"));

        assert_eq!(Option::from(
            String::from("")),
                   bot.message("position startpos moves e2e4 e7e5"));
        let mut expected_bot = Bot::with_position(START_POSITION);
        expected_bot.move_list.lock().unwrap().make_move(&Move{origin: 11, target: 27, promotion: 0, piece: PAWN});
        expected_bot.move_list.lock().unwrap().make_move(&Move{origin: 51, target: 35, promotion: 0, piece: PAWN});
        assert_eq!(expected_bot.move_list.lock().unwrap().get_board(), bot.move_list.lock().unwrap().get_board());

        let expected_regex = Regex::new(r"^perft 1 searched \d+ nodes in (\d+.\d+|\d+)(ns|µs|ms|s)$").unwrap();
        let response = bot.message("go perft 1").unwrap_or_default();
        assert!(expected_regex.is_match(&*response));

        let expected_regex = Regex::new(r"^bestmove [a-h][1-8][a-h][1-8]$").unwrap();
        bot.engine.lock().unwrap().search(&mut bot.move_list.lock().unwrap(), 1);
        let response = bot.stop();
        assert!(expected_regex.is_match(&*response));
    }

}