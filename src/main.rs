use std::io;

fn main() {
    let mut command;

    // Handling of UCI

    loop {
        io::stdin().read_line(&mut command)
            .expect("Failed to read line");
        
        match command.as_str() {
            "ucinewgame" => println!("x"),
            "isready" => println!("readyok"),
            "position" => println!("pos"),
            "quit" => break,
            _ => println!("unexpected command")
        }
    }
}
