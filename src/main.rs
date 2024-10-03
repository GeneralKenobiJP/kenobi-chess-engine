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

use std::{io, thread};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::bot::Bot;

fn main() {
    let mut bot = Bot::new();
    let loop_flag = Arc::new(AtomicBool::new(true));

    // Handling of UCI <=> the game loop

    while loop_flag.load(Ordering::SeqCst) {
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
            None => { loop_flag.store(false, Ordering::SeqCst); }
        }
    }

    // drop(bot);
}
