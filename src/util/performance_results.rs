use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};
use crate::util::Mod;

#[derive(Debug, Serialize, Deserialize, Default)]
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
    pub map_hash: String,
    pub verbose_data: Vec<String>,
}
