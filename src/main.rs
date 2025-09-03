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
use std::io::Write;
use std::sync::{Arc, mpsc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::spawn;
use crate::bot::Bot;
use crate::evaluation::MainEvaluator;

fn main() {
    let loop_flag = Arc::new(AtomicBool::new(true));
    let loop_flag_worker = Arc::clone(&loop_flag);
    let (tx, rx) = mpsc::channel::<String>();

    // The thread for the handling of UCI messages

    let worker = spawn(move || {
        let mut bot = Bot::new();

        while loop_flag_worker.load(Ordering::SeqCst) {
            let message = rx.recv();

            if message.is_err() {
                break;
            }

            let message = message.unwrap();
            let message = message.as_str();

            let responses = bot.message(&message);
            let mut responses = responses.iter();
            while let Some(response) = responses.next() {
                match response {
                    Some(output) => {
                        if !output.is_empty() {
                            println!("{}", output);
                            io::stdout().flush().unwrap();
                        }
                    },
                    // Exit the engine
                    None => { loop_flag_worker.store(false, Ordering::SeqCst); }
                }
            }
        }
    });

    // Main thread: read stdin, send messages to worker <=> the game loop
    while loop_flag.load(Ordering::SeqCst) {
        let mut message = String::new();

        io::stdin().read_line(&mut message)
            .expect("Failed to read line");

        // If user typed "quit", send it so the worker can clean up, then stop immediately.
        if message.trim() == "quit" {
            let _ = tx.send(message);
            break; // stop main loop immediately
        }

        if tx.send(message).is_err() { break; } // worker gone
    }

    // Close the channel and wait for worker to finish.
    drop(tx);
    let _ = worker.join();
}
