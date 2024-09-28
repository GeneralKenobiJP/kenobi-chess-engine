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
use crate::bot::Bot;

fn main() {
    let mut bot = Bot::new();

    // Handling of UCI <=> the game loop

    loop {
        let mut message = String::new();

        io::stdin().read_line(&mut message)
            .expect("Failed to read line");

        let response = bot.message(&message);
        match response {
            Some(output) => {
                if !output.is_empty() {
                    println!("{}", output);
                }
            },
            None => break
        }
    }
}
