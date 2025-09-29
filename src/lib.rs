pub mod tmpl_cont {
    // this will groowwwww :3
    pub fn render() -> &'static [u8] {
        let template: &'static [u8] = b"target='http://target.com'
scope=['/endpoint1', '/endpoint2']
//scope=crawl 
// ^ pick one you need
timeout=10 //10 seconds
";
        template
    }
}

pub mod file_ops {

    use crate::tmpl_cont;
    use std::io::Result;
    use std::io::prelude::*;
    use std::{fs::File, io::Write};

    pub fn make_template(file: &String) -> std::io::Result<()> {
        let mut file = File::create(file)?;
        file.write_all(tmpl_cont::render())?;

        Ok(())
    }

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    pub enum Keywords {
        Target(String),
        ScopeVec(Vec<String>),
        ScopeStr(String),
        Timeout(i64),
        Comment,
    }

    pub fn read_file(file: &String) -> Result<Vec<Keywords>> {
        let mut syntax_vec: Vec<Keywords> = Vec::new();
        let mut file = File::open(file)?;
        let mut contents = String::new();

        file.read_to_string(&mut contents)?;
        let lines = contents.split_terminator('\n');

        for (i, raw_line) in lines.enumerate() {
            // separate code from comment
            let mut parts = raw_line.splitn(2, "//");
            let code = parts.next().unwrap().trim();
            let has_comment = parts.next().is_some();

            // full-line comment
            if code.is_empty() {
                syntax_vec.push(Keywords::Comment);
                continue;
            }

            // split keyword and value
            let line_conts: Vec<&str> = code.splitn(2, '=').collect();
            let keyword = line_conts.get(0).map(|s| s.trim()).unwrap_or("");
            let value = line_conts.get(1).map(|s| s.trim());

            match keyword {
                "target" => {
                    if let Some(v) = value {
                        syntax_vec.push(Keywords::Target(v.to_string()));
                    } else {
                        eprintln!("{i}: Missing value for 'target'");
                    }
                }
                "scope" => {
                    if let Some(v) = value {
                        if v.starts_with('[') {
                            let items: Vec<String> = v
                                .trim_matches(&['[', ']'][..])
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            syntax_vec.push(Keywords::ScopeVec(items));
                        } else {
                            syntax_vec.push(Keywords::ScopeStr(v.to_string()));
                        }
                    } else {
                        eprintln!("{i}: Missing value for 'scope'");
                    }
                }
                "timeout" => {
                    if let Some(v) = value {
                        match v.parse::<i64>() {
                            Ok(num) => syntax_vec.push(Keywords::Timeout(num)),
                            Err(_) => eprintln!("{i}: Invalid integer for 'timeout': {v}"),
                        }
                    } else {
                        eprintln!("{i}: Missing value for 'timeout'");
                    }
                }
                "" => continue, // empty line
                other => {
                    eprintln!("{i}: Invalid keyword '{other}' in file {:?}", file);
                }
            }

            // record comment if line had both code + comment
            if has_comment {
                syntax_vec.push(Keywords::Comment);
            }
        }

        validate_me_senpai(&syntax_vec)?;

        Ok(syntax_vec)
    }

    // if user specified scope more than once.. kill them
    fn validate_me_senpai(contents: &Vec<Keywords>) -> Result<Vec<Keywords>> {
        let mut counter = 0;
        for content in contents.iter() {
            match content {
                Keywords::ScopeVec(_) | Keywords::ScopeStr(_) => counter += 1,
                _ => (),
            }
        }

        if counter > 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Error: 'Scope' defined more than once",
            ));
        } else {
            Ok(contents.to_vec())
        }
    }
}
