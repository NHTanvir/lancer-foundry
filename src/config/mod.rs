//! foundry.toml parsing and profile resolution — implemented in week 5.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Config {
    pub src: Option<String>,
    pub out: Option<String>,
    pub libs: Option<Vec<String>>,
    pub remappings: Option<Vec<String>>,
    pub solc_version: Option<String>,
    pub optimizer: Option<bool>,
    pub optimizer_runs: Option<u32>,
    pub fuzz_runs: Option<u32>,
    pub invariant_runs: Option<u32>,
}
