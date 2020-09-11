use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Serialize, Deserialize, Clone, PartialOrd, Ord, Eq)]
pub struct Mod {
    pub name: String,
    #[serde(skip)]
    pub file_name: String,
    pub version: String,
    pub sha1: String,
}

impl Mod {
    #[allow(dead_code)]
    pub fn new(name: &str, file_name: &str, version: &str, hash: &str) -> Mod {
        Mod {
            name: name.to_string(),
            file_name: file_name.to_string(),
            version: version.to_string(),
            sha1: hash.to_string(),
        }
    }
}

impl PartialEq for Mod {
    fn eq(&self, cmp: &Self) -> bool {
        if self.sha1 == cmp.sha1 && !self.sha1.is_empty() {
            return true;
        }
        false
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct CollectionData {
    pub benchmark_name: String,
    pub factorio_version: String,
    pub os: String,
    pub executable_type: String,
    pub cpuid: String,
    pub benchmarks: Vec<BenchmarkData>,
    pub mods: BTreeSet<Mod>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BenchmarkData {
    pub map_name: String,
    pub runs: u32,
    pub ticks: u32,
    /// The sha256 hash of the map
    pub map_hash: String,
    /// A vec of CSV rows
    ///     tick,wholeUpdate,gameUpdate,circuitNetworkUpdate,transportLinesUpdate,
    ///     fluidsUpdate,entityUpdate,mapGenerator,electricNetworkUpdate,logisticManagerUpdate,
    ///     constructionManagerUpdate,pathFinder,trains,trainPathFinder,commander,chartRefresh,
    ///     luaGarbageIncremental,chartUpdate,scriptUpdate,run_index
    pub verbose_data: Vec<String>,
}
