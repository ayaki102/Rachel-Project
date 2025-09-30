use clap::{Arg, Command};
use rachel_project::{scanner::build_scanner, tmpl_ops};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("rachel")
        .about("Blue-team scanner & template tool")
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
            Command::new("parse")
                .about("Parse an existing file and run scanner")
                .arg(
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
                eprintln!("Invalid file type. it must end with the .rchl suffix");
                std::process::exit(1);
            }

            match tmpl_ops::make_template(filename) {
                Ok(_) => {
                    println!("Template file generated: {}", filename);
                }
                Err(e) => {
                    eprintln!("error creating template: {}", e);
                    std::process::exit(2);
                }
            }
        }

        // ------------ handle parsing + scanning
        Some(("parse", sub_m)) => {
            let filename = sub_m.get_one::<String>("file").unwrap();
            println!("Parsing file: {}", filename);

            let contents = match tmpl_ops::read_file(filename) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to read file '{}': {}", filename, e);
                    std::process::exit(1);
                }
            };

            // build scanner from parsed contents
            let scanner = build_scanner(contents);

            println!("Starting scan for target: {}", scanner.target);
            let results = scanner.run().await;

            for r in results.iter() {
                println!("=== URL: {} ===", r.url);
                println!("Status: {}", r.status_code);
                if let Some(snippet) = &r.body_snippet {
                    println!("Snippet ({} chars):", snippet.chars().count());
                    let s: String = snippet.chars().take(400).collect();
                    println!("{}", s);
                }
                if !r.headers.is_empty() {
                    println!("Headers:");
                    for (k, v) in &r.headers {
                        println!("  {}: {}", k, v);
                    }
                }
                if !r.input_fields.is_empty() {
                    println!("Discovered input fields:");
                    for f in &r.input_fields {
                        // uses Display impl for InputField
                        println!("  {}", f);
                        // optionally show some more details:
                        if let Some(name) = &f.name {
                            println!("    name: {}", name);
                        }
                        if let Some(id) = &f.id {
                            println!("    id: {}", id);
                        }
                        if let Some(val) = &f.value {
                            let display_len = std::cmp::min(val.len(), 80);
                            println!("    value (len={}): {}", val.len(), &val[..display_len]);
                        }
                        if let Some(prob) = f.probable_secret {
                            println!("    probable_secret: {}", prob);
                        }
                        if let Some(entropy) = f.secret_entropy {
                            println!("    entropy: {:.2}", entropy);
                        }
                    }
                }
                if let Some(err) = &r.errors {
                    println!("Errors: {}", err);
                }
                println!("---------------------------");
            }

            Ok(())
        }

        _ => {
            println!("No valid subcommand provided. Use 'gen' or 'parse'.");
            Ok(())
        }
    }
}
