//! Chess bot
//! Serves as an interface between the game host (i.e. GUI) and the engine.
//! Implements the Universal Chess Interface (UCI).
//! Communication is realized through the message() function.
//! It should be called in the game loop of the main function.

use std::time::Instant;
use scanner_rust::ScannerStr;
use crate::board::{Board, START_POSITION};
use crate::move_generator::{Move, MoveList};
use crate::perft::perft_log;
use crate::search::Engine;

pub struct Bot<'a> {
    engine: Engine,
    move_list: MoveList<'a>
}

impl<'a> Bot<'a> {
    pub fn new(board: &'a mut Board) -> Self {
        Bot {
            engine: Engine::new(),
            move_list: MoveList::from_board(board)
        }
    }

    /// Responds to a given message, according to the UCI standard.
    /// This function is the method used for manipulating the game state in the Bot object.
    /// Should a particular command or option not be implemented, it will notify about the fact
    /// with "Unexpected command. This command might be unsupported by the current version of the engine or by the UCI standard."
    /// Returns true if no error occurred.
    /// Returns false if the "quit" command was provided
    pub fn message(&mut self, message: &str) -> bool {
        let mut scanner = ScannerStr::new(message);

        let command = scanner.next().unwrap_or_default().unwrap_or_default();
        match command {
            "uci" => Bot::uci(),
            "ucinewgame" => self.new_game(),
            "isready" => println!("readyok"),
            "position" => self.input_position(&mut scanner),
            "go" => self.go(&mut scanner),
            "quit" => return false,
            _ => println!("Unexpected command. This command might be unsupported by the current version of the engine or by the UCI standard."),
        }

        true
    }

    /// Responds to a "uci" command.
    /// Prints the id of the engine (name, version, author) and available options.
    /// Stops the response with "uciok"
    fn uci() {
        Bot::id();
        Bot::option();

        println!("uciok");
    }

    /// Responds to a "ucinewgame" command.
    /// Creates a new engine object and clears the move list
    fn new_game(&mut self) {
        self.engine = Engine::new();
        self.move_list.clear();
    }

    /// Responds to a "position" command.
    /// Reads in the fen or the "startpos" attribute and adjusts the board accordingly.
    /// Makes moves on the board if there are moves specified with the "moves" attribute
    fn input_position(&mut self, scanner: &mut ScannerStr) {
        let mode = scanner.next().unwrap_or_default().unwrap_or_default();

        let mut board = self.move_list.get_mutable_board();

        match mode {
            "startpos" => board.read_fen(START_POSITION),
            "fen" => {let fen = scanner.next().unwrap_or_default().unwrap_or_default();
                board.read_fen(fen);},
            _ => return
        }

        let option = scanner.next().unwrap_or_default();
        match option {
            None => return,
            Some(subcommand) => if subcommand != "moves" { return }
        }

        self.input_moves(scanner);
    }

    /// Inputs moves after a "moves" subcommand within the "position" command.
    /// Makes moves, one by one, on the board until the moves are exhausted
    fn input_moves(&mut self, scanner: &mut ScannerStr) {
        while let Some(input) = scanner.next().unwrap_or_default() {
            self.move_list.make_move(&Move::from_algebraic_notation(input, self.move_list.get_board()));
        }
    }

    /// Responds to a "go" command, which requests position analysis.
    /// Supports following options:
    ///     "perft" - calls perft_log, outputting the number of nodes searched, the duration time,
    /// and the number of nodes from each immediate response
    ///     "infinite" - starts an infinite search, which can conclude with the "stop" command
    fn go(&mut self, scanner: &mut ScannerStr) {
        let mode = scanner.next().unwrap_or_default().unwrap_or_default();

        match mode {
            "perft" => self.go_perft(scanner),
            "infinite" => self.go_infinite(),
            _ => self.go_infinite()
        }
    }

    /// Responds to a "go perft" command.
    /// Calls the perft_log function, outputting the number of nodes searched, the duration time,
    /// and the number of nodes from each immediate response
    fn go_perft(&mut self, scanner: &mut ScannerStr) {
        let depth = scanner.next().unwrap_or_default().unwrap_or_default().parse::<u32>().unwrap_or_default();

        let start = Instant::now();
        let nodes = perft_log(&mut self.move_list, depth);
        let duration = start.elapsed();

        println!("perft {} searched {} nodes in {:?}", depth, nodes, duration);
    }

    /// Responds to a "go infinite" command.
    /// todo: TBD
    fn go_infinite(&mut self) {

    }

    /// Prints the id of the engine: name, version, author
    fn id() {
        println!("id name Kenobi {}", env!("CARGO_PKG_VERSION"));
        println!("id author Jakub Pietrzak");
    }

    /// Prints the options of the engine.
    /// Option is a setting that can be changed through the UCI.
    fn option() {

    }
}