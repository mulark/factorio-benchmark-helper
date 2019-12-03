use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CollectionData {
    pub benchmark_name: String,
    pub factorio_version: String,
    pub platform: String,
    pub executable_type: String,
    pub cpuid: String,
    pub benchmarks: Vec<BenchmarkData>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BenchmarkData {
    pub map_name: String,
    pub runs: u32,
    pub ticks: u32,
    pub map_hash: String,
    pub verbose_data: Vec<VerboseData>,
}

// Allow non_snake_case because timing list from Factorio itself is this way.
#[derive(Debug, Serialize, Deserialize, Default)]
#[allow(non_snake_case)]
pub struct VerboseData {
    pub tick_number: u32,
    pub wholeUpdate: u64,
    pub gameUpdate: u64,
    pub circuitNetworkUpdate: u64,
    pub transportLinesUpdate: u64,
    pub fluidsUpdate: u64,
    pub entityUpdate: u64,
    pub mapGenerator: u64,
    pub electricNetworkUpdate: u64,
    pub logisticManagerUpdate: u64,
    pub constructionManagerUpdate: u64,
    pub pathFinder: u64,
    pub trains: u64,
    pub trainPathFinder: u64,
    pub commander: u64,
    pub chartRefresh: u64,
    pub luaGarbageIncremental: u64,
    pub chartUpdate: u64,
    pub scriptUpdate: u64,
    pub run_index: u32,
}
