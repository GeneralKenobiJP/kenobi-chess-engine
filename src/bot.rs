use scanner_rust::ScannerStr;
use crate::initiate_bot;
use crate::search::Engine;

struct Bot {
    engine: Engine
}

impl Bot {
    pub fn new() -> Self {
        Bot {
            engine: Engine::new()
        }
    }

    pub fn message(message: &str) {
        let mut scanner = ScannerStr::new(message);

        let command = scanner.next().unwrap_or_default().unwrap_or_default();
        match command {
            "uci" => uci(),
            // "ucinewgame" => initiate_bot(&mut board),
            // "isready" => println!("readyok"),
            // "position" => board.read_fen(scanner.next().unwrap_or_default().unwrap_or_default()),
            // "quit" => break,
            _ => println!("unexpected command"),
    }

        fn uci() {
            id();
            option();
        }

        fn id() {
            println!("id name Kenobi {}", env!("CARGO_PKG_VERSION"));
            println!("id author Jakub Pietrzak");
        }

        fn option() {
            
        }
    }
}