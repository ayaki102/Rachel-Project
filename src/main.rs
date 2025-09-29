use rachel_project::file_ops;

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

            match file_ops::make_template(filename) {
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
            file_ops::read_file(filename)
                .map(|c| println!("{:#?}", c))
                .unwrap_or_else(|e| {
                    eprintln!("{}", e);
                    std::process::exit(1);
                });
        }

        _ => {
            println!("No valid subcommand provided. Use 'gen' or 'parse'.");
        }
    }
}
