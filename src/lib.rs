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
