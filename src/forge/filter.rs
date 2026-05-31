#[derive(Debug, Default, Clone)]
pub struct TestFilter {
    pub match_contract: Option<String>,
    pub match_test: Option<String>,
    pub match_path: Option<String>,
    pub no_match_contract: Option<String>,
    pub no_match_test: Option<String>,
}

impl TestFilter {
    pub fn new(
        match_contract: Option<String>,
        match_test: Option<String>,
        match_path: Option<String>,
        no_match_contract: Option<String>,
        no_match_test: Option<String>,
    ) -> Self {
        TestFilter { match_contract, match_test, match_path, no_match_contract, no_match_test }
    }

    pub fn match_contract(&self, name: &str) -> bool {
        let matches = self.match_contract.as_deref()
            .map_or(true, |pat| name.contains(pat));
        let excluded = self.no_match_contract.as_deref()
            .map_or(false, |pat| name.contains(pat));
        matches && !excluded
    }

    pub fn match_test(&self, name: &str) -> bool {
        let matches = self.match_test.as_deref()
            .map_or(true, |pat| name.contains(pat));
        let excluded = self.no_match_test.as_deref()
            .map_or(false, |pat| name.contains(pat));
        matches && !excluded
    }

    pub fn match_path_str(&self, path: &str) -> bool {
        self.match_path.as_deref().map_or(true, |pat| path.contains(pat))
    }
}
