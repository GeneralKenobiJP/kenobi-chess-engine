mod board;
mod piece;
mod move_generator;
mod magic_hasher;
mod perft;
mod evaluation;
mod search;
mod zobrist;
mod transposition_table;
mod bot;
mod string_builder;

use std::io;
use scanner_rust::ScannerStr;
use crate::bot::Bot;

fn main() {

    let mut board = board::Board::new();
    let mut bot = Bot::new(&mut board);

    // Handling of UCI <=> the game loop

    loop {
        let mut message = String::new();

        io::stdin().read_line(&mut message)
            .expect("Failed to read line");

        if !bot.message(&message) { break; }
    }
}
