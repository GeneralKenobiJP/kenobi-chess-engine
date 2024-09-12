use scanner_rust::ScannerStr;
use crate::board::Board;
use crate::initiate_bot;
use crate::move_generator::MoveList;
use crate::search::Engine;

pub struct Bot {
    engine: Engine,
    // move_list: MoveList<'a>
}

impl Bot {
    pub fn new() -> Self {
        Bot {
            engine: Engine::new(),
            // move_list: MoveList::new()
        }
    }

    pub fn message(&mut self, message: &str) {
        let mut scanner = ScannerStr::new(message);

        let command = scanner.next().unwrap_or_default().unwrap_or_default();
        match command {
            "uci" => Bot::uci(),
            "ucinewgame" => self.new_game(),
            "isready" => println!("readyok"),
            // "position" => self.input_position(&mut scanner),
            // "quit" => break,
            _ => println!("unexpected command"),
        }
    }

    fn uci() {
        Bot::id();
        Bot::option();

        println!("uciok");
    }

    fn new_game(&mut self) {
        self.engine = Engine::new();
    }

    // fn input_position(&mut self, scanner: &mut ScannerStr) {
    //     let mode = scanner.next().unwrap_or_default().unwrap_or_default();
    //
    //     if mode == "startpos" {
    //         self.move_list.
    //     }
    // }

    fn id() {
        println!("id name Kenobi {}", env!("CARGO_PKG_VERSION"));
        println!("id author Jakub Pietrzak");
    }

    fn option() {

    }
}