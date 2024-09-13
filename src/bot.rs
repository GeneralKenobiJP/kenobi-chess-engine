//! Chess bot
//! Serves as an interface between the game host (i.e. GUI) and the engine.
//! Implements the Universal Chess Interface (UCI).
//! Communication is realized through the message() function.
//! It should be called in the game loop of the main function.

use std::fmt::format;
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
    /// Calls relevant response functions that use a StringBuilder to return a response String,
    /// which is printed out in this method.
    /// Returns true if no error occurred.
    /// Returns false if the "quit" command was provided
    pub fn message(&mut self, message: &str) -> bool {
        let mut scanner = ScannerStr::new(message);

        let command = scanner.next().unwrap_or_default().unwrap_or_default();
        let response = match command {
            "uci" => Bot::uci(),
            "ucinewgame" => self.new_game(),
            "isready" => Self::readyok(),
            "position" => self.input_position(&mut scanner),
            "go" => self.go(&mut scanner),
            "quit" => return false,
            _ => String::from("Unexpected command. This command might be unsupported by the current version of the engine or by the UCI standard."),
        };

        println!("{}", response);

        true
    }

    /// Responds to a "uci" command.
    /// Returns the id of the engine (name, version, author) and available options.
    /// Ends the response with "uciok"
    fn uci() -> String {
        let mut response = String::new();
        response.push_str(&Bot::id());
        response.push_str(&Bot::option());

        response.push_str(&"uciok");
        response
    }

    /// Responds to a "isready" command with "readyok".
    fn readyok() -> String {
        String::from("readyok")
    }

    /// Responds to a "ucinewgame" command.
    /// Creates a new engine object and clears the move list
    fn new_game(&mut self) -> String {
        self.engine = Engine::new();
        self.move_list.clear();
        String::new()
    }

    /// Responds to a "position" command.
    /// Reads in the fen or the "startpos" attribute and adjusts the board accordingly.
    /// Makes moves on the board if there are moves specified with the "moves" attribute
    fn input_position(&mut self, scanner: &mut ScannerStr) -> String {
        let response = String::new();
        let mode = scanner.next().unwrap_or_default().unwrap_or_default();

        let mut board = self.move_list.get_mutable_board();

        match mode {
            "startpos" => board.read_fen(START_POSITION),
            "fen" => {let fen = scanner.next().unwrap_or_default().unwrap_or_default();
                board.read_fen(fen);},
            _ => return response
        }

        let option = scanner.next().unwrap_or_default();
        match option {
            None => return response,
            Some(subcommand) => if subcommand != "moves" { return response }
        }

        self.input_moves(scanner);

        response
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
    fn go(&mut self, scanner: &mut ScannerStr) -> String {
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
    fn go_perft(&mut self, scanner: &mut ScannerStr) -> String {
        let depth = scanner.next().unwrap_or_default().unwrap_or_default().parse::<u32>().unwrap_or_default();

        let start = Instant::now();
        let nodes = perft_log(&mut self.move_list, depth);
        let duration = start.elapsed();

        let response = format!("perft {} searched {} nodes in {:?}", depth, nodes, duration);
        response
    }

    /// Responds to a "go infinite" command.
    /// todo: TBD
    fn go_infinite(&mut self) -> String {
        self.engine.search(&mut self.move_list, 1);
        let best_move = self.engine.get_best_move(self.move_list.get_board());
        let response = best_move.to_algebraic_notation();

        response
    }

    /// Responds with the id of the engine: name, version, author
    fn id() -> String {
        let mut response = String::new();
        response.push_str(&format!("id name Kenobi {}", env!("CARGO_PKG_VERSION")));
        response.push_str("id author Jakub Pietrzak");
        response
    }

    /// Responds with the options of the engine.
    /// Option is a setting that can be changed through the UCI.
    fn option() -> String {
        let mut response = String::new();
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_bot_new() {
        let mut board = Board::new();
        let bot = Bot::new(&mut board);
    }

}