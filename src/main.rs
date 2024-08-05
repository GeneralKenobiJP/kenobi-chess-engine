mod board;
// use crate::board;

use std::io;
use scanner_rust::ScannerStr;

fn initiate_bot() {
    println!("initiating...");
}

fn main() {
    // Handling of UCI

    loop {
        let mut message = String::new();

        io::stdin().read_line(&mut message)
            .expect("Failed to read line");

        let mut scanner = ScannerStr::new(&message);

        let mut command = scanner.next().unwrap_or_default().unwrap_or_default();
        
        // match command.next().unwrap_or_default().unwrap_or_default() {
        match command {
            "ucinewgame" => initiate_bot(),
            "isready" => println!("readyok"),
            "position" => board::read_fen(scanner.next().unwrap_or_default().unwrap_or_default()),
            "quit" => break,
            _ => println!("unexpected command"),
        }
    }
}
