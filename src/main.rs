use rachel_project::{
    scanner::build_scanner,
    tmpl_ops::{self},
};

use clap::{Arg, Command};

fn main() {
    let matches = Command::new("rachel")
        .subcommand(
            Command::new("gen")
                .about("Generate a file for manual input")
                .arg(
                    Arg::new("file")
                        .help("File to generate")
                        .required(true) // must provide a file
                        .index(1), // positional argument
                ),
        )
        .subcommand(
            Command::new("parse").about("Parse an existing file").arg(
                Arg::new("file")
                    .help("File to parse")
                    .required(true)
                    .index(1),
            ),
        )
        .get_matches();

    match matches.subcommand() {
        // ------------ handle generation
        Some(("gen", sub_m)) => {
            let filename = sub_m.get_one::<String>("file").unwrap();

            if !filename.ends_with(".rchl") {
                eprintln!("Invalid file type. it must end .rchl suffix");
                std::process::exit(1);
            }

            match tmpl_ops::make_template(filename) {
                Ok(_) => println!("Template file generated: {}", filename),
                Err(e) => {
                    println!("error {}", e);
                    std::process::exit(2)
                }
            }
        }
        // ------------ handle parsin
        Some(("parse", sub_m)) => {
            let filename = sub_m.get_one::<String>("file").unwrap();
            println!("Parsing file: {}", filename);
            // don't know how i'll use this cont variable yet
            let contents = match tmpl_ops::read_file(filename) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to read file '{}': {}", filename, e);
                    std::process::exit(1);
                }
            };

            // this will save me from hell i created
            let _scanner = build_scanner(contents);
        }

        _ => {
            println!("No valid subcommand provided. Use 'gen' or 'parse'.");
        }
    }
}
