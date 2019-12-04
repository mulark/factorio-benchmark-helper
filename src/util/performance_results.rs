use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CollectionData {
    pub benchmark_name: String,
    pub factorio_version: String,
    pub os: String,
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
    pub verbose_data: Vec<String>,
}
