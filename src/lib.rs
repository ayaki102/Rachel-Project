// module for generating contents of template file for user to fill out
pub mod template_contents {
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

    use crate::template_contents;
    use std::io::Result;
    use std::io::prelude::*;
    use std::{fs::File, io::Write};

    pub fn make_template(file: &String) -> std::io::Result<()> {
        let mut file = File::create(file)?;
        file.write_all(template_contents::render())?;

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
        let file_path = file.clone();
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
                    eprintln!("{i}: Invalid keyword '{other}' in file {:?}", file_path);
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
    pub fn validate_me_senpai(contents: &Vec<Keywords>) -> Result<Vec<Keywords>> {
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
    use crate::tmpl_ops::Keywords;
    use futures::stream::{FuturesUnordered, StreamExt};
    use reqwest::redirect::Policy;
    use scraper::{Html, Selector};
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;
    use std::collections::{HashSet, VecDeque};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Semaphore;
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
        pub timeout: Option<Duration>,
        pub client: reqwest::Client,
        pub user_agent: Option<String>,
        pub follow_redirects: bool,
    }

    impl Scanner {
        /// Construct scanner; if endpoints is None, start with empty Vec.
        pub fn new(target: Url, endpoints: Option<Vec<Url>>, timeout: Option<Duration>) -> Self {
            Self {
                target,
                endpoints: endpoints.unwrap_or_default(),
                timeout,
                client: reqwest::Client::new(),
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

        /// Async run: crawl (or use provided endpoints) and scan pages.
        /// - If endpoints vector is non-empty, only scan those.
        /// - Otherwise, BFS-crawl from target up to `max_pages` and `max_depth`.
        /// Returns Vec<ScanResults>
        pub async fn run(&self) -> Vec<ScanResults> {
            // Configurable params (tune as needed or add to Scanner struct)
            let concurrency_limit = 10usize; // concurrent requests
            let max_pages = 500usize; // absolute limit
            let max_depth = 4usize; // how deep from start
            let snippet_len = 1024usize;

            // Build reqwest client honoring timeout, user agent, follow_redirects
            let mut client_builder = reqwest::Client::builder();
            if let Some(dur) = self.timeout {
                client_builder = client_builder.timeout(dur);
            }
            if self.follow_redirects {
                client_builder = client_builder.redirect(Policy::limited(10));
            } else {
                client_builder = client_builder.redirect(Policy::none());
            }
            if let Some(ua) = &self.user_agent {
                client_builder = client_builder.user_agent(ua.clone());
            }

            let client = match client_builder.build() {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    eprintln!("Failed to build HTTP client: {}", e);
                    return Vec::new();
                }
            };

            // If endpoints provided, scan only those
            let endpoints_to_scan: Vec<Url> = if !self.endpoints.is_empty() {
                self.endpoints.clone()
            } else {
                // Crawl and produce endpoints
                self.crawl_graph(client.clone(), max_pages, max_depth).await
            };

            // Limit concurrency
            let sem = Arc::new(Semaphore::new(concurrency_limit));
            let mut futs = FuturesUnordered::new();

            for url in endpoints_to_scan.into_iter() {
                let client = client.clone();
                let sem = sem.clone();
                // Acquire permit inside spawned future so concurrency bound applies
                futs.push(tokio::spawn(async move {
                    let _permit = sem.acquire().await;
                    scan_single(&client, &url, snippet_len).await
                }));
            }

            let mut results = Vec::new();
            while let Some(res) = futs.next().await {
                match res {
                    Ok(scan_res) => results.push(scan_res),
                    Err(e) => {
                        // join error
                        eprintln!("Task join error: {}", e);
                    }
                }
            }

            results
        }

        async fn crawl_graph(
            &self,
            client: Arc<reqwest::Client>,
            max_pages: usize,
            max_depth: usize,
        ) -> Vec<Url> {
            let mut discovered: Vec<Url> = Vec::new();
            let mut visited: HashSet<String> = HashSet::new();
            let mut q: VecDeque<(Url, usize)> = VecDeque::new();

            q.push_back((self.target.clone(), 0));
            visited.insert(self.target.as_str().to_string());

            while let Some((url, depth)) = q.pop_front() {
                // stop if reached limits
                if discovered.len() >= max_pages {
                    break;
                }
                // attempt fetch
                match client.get(url.clone()).send().await {
                    Ok(resp) => {
                        // collect url
                        discovered.push(url.clone());

                        // parse only html bodies for links if depth < max_depth
                        if depth < max_depth {
                            if let Ok(body) = resp.text().await {
                                let base = url.clone();
                                let links = extract_links(&body, &base);
                                for link in links.into_iter() {
                                    // normalization: remove fragment, query maybe keep? Keep query but canonicalize
                                    let mut link = link.clone();
                                    link.set_fragment(None);

                                    // scope decision: same origin (host+port+scheme)
                                    if !same_origin(&self.target, &link) {
                                        continue;
                                    }
                                    let key = link.as_str().to_string();
                                    if visited.contains(&key) {
                                        continue;
                                    }
                                    visited.insert(key.clone());
                                    q.push_back((link, depth + 1));
                                }
                            }
                        }
                    }
                    Err(_e) => {
                        // ignore fetch errors during crawl, but still continue
                    }
                }
            }

            discovered
        }
    }

    async fn scan_single(client: &reqwest::Client, url: &Url, snippet_len: usize) -> ScanResults {
        let mut res = ScanResults {
            url: url.clone(),
            status_code: 0,
            body_snippet: None,
            input_fields: Vec::new(),
            headers: HashMap::new(),
            errors: None,
        };

        let resp_res = client.get(url.clone()).send().await;
        match resp_res {
            Ok(resp) => {
                res.status_code = resp.status().as_u16();
                // headers
                for (k, v) in resp.headers().iter() {
                    if let Ok(s) = v.to_str() {
                        res.headers.insert(k.to_string(), s.to_string());
                    } else {
                        res.headers
                            .insert(k.to_string(), "<binary or non-utf8>".to_string());
                    }
                }
                // read body if text/html
                let maybe_ct = res.headers.get("content-type").cloned();
                let is_html = maybe_ct
                    .as_deref()
                    .map(|ct| ct.contains("text/html") || ct.contains("application/xhtml+xml"))
                    .unwrap_or(false);

                if is_html {
                    match resp.text().await {
                        Ok(body) => {
                            let snippet: String = body.chars().take(snippet_len).collect();
                            res.body_snippet = Some(snippet);
                            // parse input fields
                            res.input_fields = parse_input_fields(&body);
                        }
                        Err(e) => {
                            res.errors = Some(format!("Failed to read body: {}", e));
                        }
                    }
                } else {
                    // try to read some bytes as snippet (best-effort)
                    if let Ok(body) = resp.text().await {
                        let snippet: String = body.chars().take(snippet_len).collect();
                        res.body_snippet = Some(snippet);
                    }
                }
            }
            Err(e) => {
                res.errors = Some(e.to_string());
            }
        }

        res
    }

    /// Extract anchor links (hrefs) from HTML and resolve relative to base.
    /// Returns Vec<Url> for well-formed absolute urls.
    fn extract_links(html: &str, base: &Url) -> Vec<Url> {
        let mut links = Vec::new();
        let doc = Html::parse_document(html);

        if let Ok(sel) = Selector::parse("a[href], link[href], script[src], img[src], form[action]")
        {
            for el in doc.select(&sel) {
                let attr = if el.value().name() == "form" {
                    "action"
                } else {
                    "href"
                };
                // script/img use src; the selector included them but the attr might be src; try both
                let maybe = el
                    .value()
                    .attr("href")
                    .or_else(|| el.value().attr("src"))
                    .or_else(|| el.value().attr("action"));
                if let Some(href) = maybe {
                    if href.trim().is_empty() {
                        continue;
                    }
                    if let Ok(joined) = base.join(href) {
                        links.push(joined);
                    }
                }
            }
        }
        links
    }

    /// Check same-origin between two Urls (scheme, host, port)
    fn same_origin(a: &Url, b: &Url) -> bool {
        a.scheme() == b.scheme()
            && a.host_str() == b.host_str()
            && a.port_or_known_default() == b.port_or_known_default()
    }

    /// Parse input fields from HTML (input, textarea, select)
    fn parse_input_fields(html: &str) -> Vec<InputField> {
        let mut fields = Vec::new();
        let doc = Html::parse_document(html);
        let selector = Selector::parse("input,textarea,select").unwrap();

        for el in doc.select(&selector) {
            let val = el.value();
            let tag = val.name().to_string();
            let mut field = InputField::default();
            field.tag_name = tag.clone();

            // common attributes
            field.input_type = val.attr("type").map(|s| s.to_string());
            field.name = val.attr("name").map(|s| s.to_string());
            field.id = val.attr("id").map(|s| s.to_string());
            field.value = val.attr("value").map(|s| s.to_string());
            field.placeholder = val.attr("placeholder").map(|s| s.to_string());
            field.title = val.attr("title").map(|s| s.to_string());
            field.autocomplete = val.attr("autocomplete").map(|s| s.to_string());
            // classes
            field.classes = val
                .attr("class")
                .map(|s| s.split_whitespace().map(|x| x.to_string()).collect());

            // attributes map (capture everything)
            let mut attrs = HashMap::new();
            for (k, v) in val.attrs() {
                attrs.insert(k.to_string(), v.to_string());
            }
            field.attributes = Some(attrs);

            // select options
            if tag == "select" {
                let mut options = Vec::new();
                let opt_sel = Selector::parse("option").unwrap();
                // To get inner options we need to search within this element's HTML substring.
                // Simpler: parse full document and find options that have a parent select with matching attributes;
                // for brevity, just collect all options on document level and include text if `name` matches.
                for opt in el.select(&opt_sel) {
                    let text = opt.text().collect::<Vec<_>>().join("");
                    if let Some(v) = opt.value().attr("value") {
                        options.push(v.to_string());
                    } else {
                        options.push(text);
                    }
                }
                field.options = Some(options);
            }

            // basic flags
            field.required = val.attr("required").map(|_| true);
            field.readonly = val.attr("readonly").map(|_| true);
            field.disabled = val.attr("disabled").map(|_| true);
            // min/max/maxlength
            field.maxlength = val.attr("maxlength").and_then(|s| s.parse::<u64>().ok());
            field.minlength = val.attr("minlength").and_then(|s| s.parse::<u64>().ok());
            field.min = val.attr("min").map(|s| s.to_string());
            field.max = val.attr("max").map(|s| s.to_string());
            field.pattern = val.attr("pattern").map(|s| s.to_string());
            field.step = val.attr("step").map(|s| s.to_string());
            field.accept = val.attr("accept").map(|s| s.to_string());
            field.multiple = val.attr("multiple").map(|_| true);

            // outer_html: best-effort by rendering the element's HTML
            // Scraper doesn't have outer_html directly; grab element.html().
            field.outer_html = Some(el.html());

            // Evaluate sensitivity heuristics (name, id, value entropy)
            field.evaluate_sensitivity();

            // push the evaluated field
            fields.push(field);
        }

        fields
    }

    // Helper to check if a string looks like a full URL
    fn is_full_url(s: &str) -> bool {
        s.starts_with("http://") || s.starts_with("https://")
    }

    // i'm so proud of this
    pub fn build_scanner(contents: Vec<Keywords>) -> Scanner {
        let mut target_str: Option<String> = None;
        let mut endpoints_strs: Vec<String> = Vec::new();
        let mut timeout_secs: Option<i64> = None;

        for cont in contents {
            match cont {
                Keywords::Target(t) => target_str = Some(t),
                Keywords::ScopeVec(v) => endpoints_strs = v,
                Keywords::Timeout(i) => timeout_secs = Some(i),
                _ => {}
            }
        }

        let target = match target_str {
            Some(t) => match Url::parse(&t) {
                Ok(url) => url,
                Err(e) => {
                    eprintln!("Invalid target URL '{}': {}", t, e);
                    std::process::exit(1);
                }
            },
            None => {
                eprintln!("No target URL provided");
                std::process::exit(1);
            }
        };

        let endpoints: Option<Vec<Url>> = if endpoints_strs.is_empty() {
            None
        } else {
            Some(
                endpoints_strs
                    .iter()
                    .filter_map(|s| match Url::parse(s) {
                        Ok(url) => Some(url),
                        Err(e) => {
                            eprintln!("Skipping invalid endpoint URL '{}': {}", s, e);
                            None
                        }
                    })
                    .collect(),
            )
        };

        let timeout = Some(Duration::from_secs(timeout_secs.unwrap_or(0) as u64));

        Scanner::new(target, endpoints, timeout)
    }

    /// Results per request / page
    #[derive(Debug, Serialize, Deserialize)]
    pub struct ScanResults {
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

#[cfg(test)]
mod tests {

    use crate::{scanner::build_scanner, tmpl_ops::Keywords, tmpl_ops::validate_me_senpai};
    use std::{io, time::Duration};

    #[test]
    fn test_build_scanner_basic() {
        let target_kw = Keywords::Target("https://example.com".to_string());
        let timeout_kw = Keywords::Timeout(10);
        let endpoints_kw = Keywords::ScopeVec(vec![
            "https://example.com/x".to_string(),
            "https://example.com/y".to_string(),
        ]);

        let scanner = build_scanner(vec![target_kw, timeout_kw, endpoints_kw]);

        assert_eq!(scanner.target.as_str(), "https://example.com/");

        let endpoints = &scanner.endpoints;
        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[0].as_str(), "https://example.com/x");
        assert_eq!(endpoints[1].as_str(), "https://example.com/y");

        assert_eq!(scanner.timeout.unwrap(), Duration::from_secs(10));

        assert!(scanner.user_agent.is_none());
        assert!(scanner.follow_redirects);
    }

    #[test]
    fn test_build_scanner_defaults() {
        let target_kw = Keywords::Target("https://example.com".to_string());
        let scanner = build_scanner(vec![target_kw]);

        assert!(scanner.endpoints.is_empty());
        assert_eq!(scanner.timeout.unwrap(), Duration::from_secs(0));
    }

    #[test]
    fn test_validate_me_senpai_ok() {
        let contents = vec![
            Keywords::Target("https://example.com".to_string()),
            Keywords::ScopeVec(vec!["/x".to_string()]),
            Keywords::Timeout(10),
        ];

        let result = validate_me_senpai(&contents);
        assert!(result.is_ok());
        let validated = result.unwrap();

        assert_eq!(validated.len(), contents.len());
        assert!(matches!(validated[1], Keywords::ScopeVec(_)));
    }

    #[test]
    fn test_validate_me_senpai_fail() {
        let contents = vec![
            Keywords::ScopeVec(vec!["/x".to_string()]),
            Keywords::ScopeStr("/y".to_string()),
        ];

        let result = validate_me_senpai(&contents);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
        assert_eq!(err.to_string(), "Error: 'Scope' defined more than once");
    }

    #[test]
    fn test_validate_me_senpai_no_scope() {
        let contents = vec![
            Keywords::Target("https://example.com".to_string()),
            Keywords::Timeout(5),
        ];

        let result = validate_me_senpai(&contents);
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.len(), contents.len());
    }
}
