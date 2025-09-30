// module for generating contents of template file for user to fill out
pub mod template_conetnts {
    // this will groowwwww :3
    pub fn render() -> &'static [u8] {
        let template: &'static [u8] = b"target=http://target.com
scope=[/endpoint1, /endpoint2]
#scope=crawl 
# ^ pick one you need
# timeout=10 #10 seconds
";
        template
    }
}

pub mod tmpl_ops {

    use crate::template_conetnts;
    use std::io::Result;
    use std::io::prelude::*;
    use std::{fs::File, io::Write};

    pub fn make_template(file: &String) -> std::io::Result<()> {
        let mut file = File::create(file)?;
        file.write_all(template_conetnts::render())?;

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

    // change this to be just file contents to test this lol
    pub fn read_file(file: &String) -> Result<Vec<Keywords>> {
        let mut syntax_vec: Vec<Keywords> = Vec::new();
        let mut file = File::open(file)?;
        let mut contents = String::new();

        file.read_to_string(&mut contents)?;
        let lines = contents.split_terminator('\n');

        for (i, raw_line) in lines.enumerate() {
            // separate code from comment
            let mut parts = raw_line.splitn(2, "#");
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

pub mod scanner {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::time::Duration;
    use url::Url;

    /// Whether we've visited an endpoint
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum EndpointState {
        Seen,
        NotSeen,
    }

    /// More robust scanner config & runtime object
    #[derive(Debug, Clone)]
    pub struct Scanner {
        pub target: Url,
        pub endpoints: Vec<Url>,
        pub timeout: Duration,
        // you can add a reqwest::Client here when integrating network code
        // pub client: reqwest::Client,
        pub user_agent: Option<String>,
        pub follow_redirects: bool,
    }

    impl Scanner {
        /// Construct scanner; if endpoints is None, start with empty Vec.
        pub fn new(target: Url, endpoints: Option<Vec<Url>>, timeout: Duration) -> Self {
            Self {
                target,
                endpoints: endpoints.unwrap_or_default(),
                timeout,
                user_agent: None,
                follow_redirects: true,
            }
        }

        /// Helper to append path segment to base (preserves Url encoding)
        pub fn new_endpoint(base: &Url, append: &str) -> Option<Url> {
            let mut new = base.clone();
            // push path properly
            let mut path = new.path().to_owned();
            if !path.ends_with('/') {
                path.push('/');
            }
            path.push_str(append);
            new.set_path(&path);
            Some(new)
        }
    }

    /// Results per request / page
    #[derive(Debug, Serialize, Deserialize)]
    pub struct Results {
        pub url: Url,
        pub status_code: u16,
        pub body_snippet: Option<String>, // trimmed outer HTML or snippet
        pub input_fields: Vec<InputField>,
        pub headers: HashMap<String, String>,
        pub errors: Option<String>,
    }

    // ---- InputField: practical & security-focused ----
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct InputField {
        // Basic element identity/metadata
        pub tag_name: String,           // "input", "textarea", "select"
        pub input_type: Option<String>, // for <input type="...">
        pub name: Option<String>,
        pub id: Option<String>,
        pub classes: Option<Vec<String>>,
        pub css_selector: Option<String>, // helpful for auto-fix/manual review
        pub xpath: Option<String>,
        pub outer_html: Option<String>, // the raw element (trimmed)
        pub inner_html: Option<String>, // for select/textarea

        // Value & user-facing hints
        pub value: Option<String>,
        pub placeholder: Option<String>,
        pub title: Option<String>,

        // Validation & constraints
        pub required: Option<bool>,
        pub readonly: Option<bool>,
        pub disabled: Option<bool>,
        pub maxlength: Option<u64>,
        pub minlength: Option<u64>,
        pub pattern: Option<String>,
        pub step: Option<String>,
        pub min: Option<String>,
        pub max: Option<String>,

        // Browser hints & accessibility
        pub autocomplete: Option<String>,
        pub inputmode: Option<String>,
        pub spellcheck: Option<bool>,
        pub aria: Option<HashMap<String, String>>,

        // Dataset & custom attributes
        pub data_attributes: Option<HashMap<String, String>>,
        pub attributes: Option<HashMap<String, String>>, // capture all remaining attrs

        // Select/file specific
        pub options: Option<Vec<String>>, // for <select>
        pub multiple: Option<bool>,       // select or file multiple
        pub accept: Option<String>,       // for file input `accept` attribute

        // Visibility / style hints (may need computed styles from headless browser)
        pub is_hidden: Option<bool>, // true if input is visually hidden (display:none, hidden, aria-hidden, offscreen)
        pub is_visible: Option<bool>,

        // Form context
        pub form_action: Option<String>,
        pub form_method: Option<String>,
        pub form_id: Option<String>,
        pub enctype: Option<String>,

        // Security heuristics & scoring (computed)
        pub probable_secret: Option<bool>, // heuristics: looks like API key/token
        pub secret_entropy: Option<f64>,   // Shannon entropy of default value if present
        pub likely_csrf_token: Option<bool>,
        pub xss_candidate: Option<bool>, // field reflected without sanitization in responses
        pub notes: Option<Vec<String>>,  // custom results / findings
    }

    impl InputField {
        /// Quick heuristic: sensitive if name/id contains common secret words or autocomplete suggests it
        pub fn is_sensitive_name(&self) -> bool {
            let sensitive_terms = [
                "token",
                "apikey",
                "api_key",
                "secret",
                "passwd",
                "password",
                "auth",
                "access_token",
                "jwt",
            ];
            let check = |s: &str| {
                let s = s.to_lowercase();
                sensitive_terms.iter().any(|t| s.contains(t))
            };
            if let Some(n) = &self.name {
                if check(n) {
                    return true;
                }
            }
            if let Some(id) = &self.id {
                if check(id) {
                    return true;
                }
            }
            if let Some(attrs) = &self.attributes {
                if let Some(v) = attrs.get("type") {
                    if v.to_lowercase() == "password" {
                        return true;
                    }
                }
            }
            if let Some(ac) = &self.autocomplete {
                let ac_l = ac.to_lowercase();
                if ac_l.contains("cc-")
                    || ac_l.contains("password")
                    || ac_l.contains("one-time-code")
                {
                    return true;
                }
            }
            false
        }

        /// Compute rough Shannon entropy for a string; used to detect tokens/secrets leaked in value attribute
        pub fn shannon_entropy(s: &str) -> f64 {
            if s.is_empty() {
                return 0.0;
            }
            let mut counts = std::collections::HashMap::new();
            for b in s.bytes() {
                *counts.entry(b).or_insert(0usize) += 1;
            }
            let len = s.len() as f64;
            let mut entropy = 0.0f64;
            for &c in counts.values() {
                let p = (c as f64) / len;
                entropy -= p * p.log2();
            }
            entropy
        }

        /// Convenience: mark field as likely sensitive if name suggests it or value entropy is high
        pub fn evaluate_sensitivity(&mut self) {
            if self.probable_secret.unwrap_or(false) {
                return;
            }
            if self.is_sensitive_name() {
                self.probable_secret = Some(true);
            } else if let Some(val) = &self.value {
                let ent = Self::shannon_entropy(val);
                self.secret_entropy = Some(ent);
                if ent > 4.0 || val.len() > 20 {
                    self.probable_secret = Some(true)
                }
            }
        }
    }

    // Example: small helper to convert an InputField to a concise summary string
    impl std::fmt::Display for InputField {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "<{} name={:?} id={:?} type={:?} sensitive={:?}>",
                self.tag_name, self.name, self.id, self.input_type, self.probable_secret
            )
        }
    }
}
