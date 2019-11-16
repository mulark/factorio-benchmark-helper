extern crate reqwest;
extern crate serde;
extern crate serde_json;

use std::collections::BTreeMap;
use crate::util::{
    Map,
    Mod,
    fbh_procedure_json_local_file,
    fbh_procedure_json_master_file,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopLevel {
    pub benchmark_sets: BTreeMap<String, BenchmarkSet>,
    pub meta_sets: BTreeMap<String, MetaSet>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetaSet {
    pub members: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BenchmarkSet {
    pub mod_groups: Vec<ModSet>,
    pub maps: Vec<Map>,
    pub ticks: u32,
    pub runs: u32,
}

impl Default for BenchmarkSet {
    fn default() -> BenchmarkSet {
        BenchmarkSet{
            mod_groups: Vec::new(),
            maps: Vec::new(),
            ticks: 0,
            runs: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModSet {
    pub mods: Vec<Mod>
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
    let single_map = Map::new("a single map", "a hash", "a download link");
    let single_mod = Mod::new("this mod", "this hash", "this version");
    let mut another_mod = single_mod.clone();
    another_mod.name = "something else".to_string();
    let modset: ModSet = ModSet {mods: vec!(single_mod, another_mod)};
    let modset2 = ModSet {mods: vec!(Mod::new("base", "", ""))};
    //let foo = BenchmarkSet{name: String::from("asdf"), pattern: String::from("asf"), mod_groups: vec!(modset), maps: vec!(single_map)};
    let single_benchmark_set =
        BenchmarkSet{
            mod_groups: vec!(modset, modset2),
            maps: vec!(single_map),
            ticks: 100,
            runs: 2,
        };
    let single_meta_set = MetaSet{members: vec!(String::from("test-000041"), String::from("test-000025"))};
    let single_meta_set_name = "mulark.github.io maps".to_string();
    let mut another_meta_set = single_meta_set.clone();
    another_meta_set.members[0] = String::from("another thingy");
    another_meta_set.members[1] = String::from("something other than that");
    let mut benchmark_sets = BTreeMap::new();
    let mut meta_sets = BTreeMap::new();
    benchmark_sets.insert("test-000041".to_string(), single_benchmark_set);
    meta_sets.insert(single_meta_set_name, single_meta_set);
    meta_sets.insert("name2".to_string(), another_meta_set);

    let top_level = TopLevel{meta_sets, benchmark_sets};
    let j = serde_json::to_string_pretty(&top_level).unwrap();
    println!("{}", j);
}
