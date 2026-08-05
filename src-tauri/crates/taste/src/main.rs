//! Main entry point for the taste CLI tool
#![allow(dead_code)]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let str_args: Vec<&str> = args.iter().map(String::as_str).collect();

    if let Err(e) = taste::cli::run(&str_args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
