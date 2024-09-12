use scanner_rust::ScannerStr;
use crate::board::{Board, START_POSITION};
use crate::initiate_bot;
use crate::move_generator::{Move, MoveList};
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

    pub fn message(&mut self, message: &str) {
        let mut scanner = ScannerStr::new(message);

        let command = scanner.next().unwrap_or_default().unwrap_or_default();
        match command {
            "uci" => Bot::uci(),
            "ucinewgame" => self.new_game(),
            "isready" => println!("readyok"),
            "position" => self.input_position(&mut scanner),
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
        self.move_list.clear();
    }

    fn input_position(&mut self, scanner: &mut ScannerStr) {
        let mode = scanner.next().unwrap_or_default().unwrap_or_default();

        let mut board = self.move_list.get_mutable_board();

        if mode == "startpos" {
            board.read_fen(START_POSITION);
        }
        else /*fen*/ {
            let fen = scanner.next().unwrap_or_default().unwrap_or_default();
            board.read_fen(fen);
        }

        self.input_moves(scanner);
    }

    fn input_moves(&mut self, scanner: &mut ScannerStr) {
        while let Some(input) = scanner.next().unwrap_or_default() {
            self.move_list.make_move(&Move::from_algebraic_notation(input, self.move_list.get_board()));
        }
    }

    fn id() {
        println!("id name Kenobi {}", env!("CARGO_PKG_VERSION"));
        println!("id author Jakub Pietrzak");
    }

    fn option() {

    }
}