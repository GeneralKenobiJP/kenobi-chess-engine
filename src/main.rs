mod board;
mod piece;
mod move_generator;

use std::io;
use scanner_rust::ScannerStr;

fn initiate_bot(board: &mut board::Board) {
    *board = board::Board::new();
    println!("initiating...");
}

fn main() {

    let mut board = board::Board::new();

    // Handling of UCI

    loop {
        let mut message = String::new();

        io::stdin().read_line(&mut message)
            .expect("Failed to read line");

        let mut scanner = ScannerStr::new(&message);

        let command = scanner.next().unwrap_or_default().unwrap_or_default();
        match command {
            "ucinewgame" => initiate_bot(&mut board),
            "isready" => println!("readyok"),
            "position" => board.read_fen(scanner.next().unwrap_or_default().unwrap_or_default()),
            "quit" => break,
            _ => println!("unexpected command"),
        }
    }
}
