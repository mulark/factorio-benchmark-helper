extern crate reqwest;
extern crate serde;
extern crate serde_json;

use crate::util::{fbh_procedure_json_local_file, fbh_procedure_json_master_file, Map, Mod};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopLevel {
    pub benchmark_sets: BTreeMap<String, BenchmarkSet>,
    pub meta_sets: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BenchmarkSet {
    pub mods: Vec<Mod>,
    pub maps: Vec<Map>,
    pub ticks: u32,
    pub runs: u32,
}

impl Default for BenchmarkSet {
    fn default() -> BenchmarkSet {
        BenchmarkSet {
            mods: Vec::new(),
            maps: Vec::new(),
            ticks: 0,
            runs: 0,
        }
    }
}

pub fn load_top_level_json() -> Option<TopLevel> {
    if fbh_procedure_json_local_file().exists() {
        let mut something = Vec::new();
        something.push("foo");
        something;

    }
    if fbh_procedure_json_master_file().exists() {

    }
    None
}

pub fn create_procedure_interactively() {
    let single_map = Map::new("test-000046.dummy_load", "a hash", "a download link");
    let single_mod = Mod::new("this mod", "this hash", "this version");
    let mut another_mod = single_mod.clone();
    another_mod.name = "something else".to_string();

    //let foo = BenchmarkSet{name: String::from("asdf"), pattern: String::from("asf"), mod_groups: vec!(modset), maps: vec!(single_map)};
    let single_benchmark_set = BenchmarkSet {
        mods: vec![single_mod, another_mod],
        maps: vec![single_map],
        ticks: 100,
        runs: 2,
    };
    let single_meta_set_name = "mulark.github.io maps".to_string();
    let mut single_meta_set = Vec::new();
    single_meta_set.push(String::from("test-000046"));
    single_meta_set.push(String::from("test-000025"));
    let mut benchmark_sets = BTreeMap::new();
    let mut meta_sets = BTreeMap::new();
    meta_sets.insert(single_meta_set_name, single_meta_set);
    benchmark_sets.insert("test-000041".to_string(), single_benchmark_set);

    let top_level = TopLevel {
        meta_sets,
        benchmark_sets,
    };
    let j = serde_json::to_string_pretty(&top_level).unwrap();
    println!("{}", j);
}
