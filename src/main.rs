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

use std::io::{self, BufRead, Write};

use crate::bot::{Bot, Command};

fn send_response(response: &str) -> io::Result<()> {
    if response.is_empty() {
        return Ok(());
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "{response}")?;
    out.flush()
}

fn run_uci_loop() -> io::Result<()> {
    // The control loop is the sole owner of Bot. Bot::go must start search in
    // the background
    let mut bot = Bot::new();
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut line = String::new();

    loop {
        line.clear();

        // EOF is equivalent to a shutdown. Calling Quit first leaves one place
        // for Bot to stop and join its search worker during cleanup.
        if input.read_line(&mut line)? == 0 {
            let _ = bot.process(Command::Quit);
            break;
        }

        // Blank lines and comments are intentionally ignored. They are not
        // parse errors and must not produce protocol noise on stdout.
        let Some(command) = Bot::parse(&line) else {
            continue;
        };

        if matches!(&command, Command::Quit) {
            let _ = bot.process(command);
            break;
        }

        if let Some(response) = bot.process(command) {
            send_response(&response)?;
        }
    }

    Ok(())
}

fn main() {
    if let Err(error) = run_uci_loop() {
        // Diagnostics belong on stderr; stdout is reserved for UCI traffic.
        eprintln!("UCI I/O error: {error}");
    }
}