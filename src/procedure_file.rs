extern crate reqwest;
extern crate serde;
extern crate serde_json;

use crate::util::{Map,Mod};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopLevel {
    pub benchmark_sets: Vec<BenchmarkSet>,
    pub meta_sets: Vec<MetaSet>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetaSet {
    pub name: String,
    pub members: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BenchmarkSet {
    pub name: String,
    pub pattern: String,
    pub mod_groups: Vec<ModSet>,
    pub maps: Vec<Map>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModSet {
    pub mods: Vec<Mod>
}

pub fn load_top_level_json() -> Option<TopLevel> {
    

    None
}

pub fn create_procedure_interactively() {
    let single_map = Map::new("a single map", "a hash", "a download link", 260, 5);
    let single_mod = Mod::new("this mod", "this hash", "this version");
    let modset: ModSet = ModSet {mods: vec!(single_mod)};
    //let foo = BenchmarkSet{name: String::from("asdf"), pattern: String::from("asf"), mod_groups: vec!(modset), maps: vec!(single_map)};
    let single_benchmark_set =
        BenchmarkSet{
            name: String::from("something describing all these maps like test-000041"),
            pattern: String::from("The regex pattern used to autoselect maps"),
            mod_groups: vec!(modset),
            maps: vec!(single_map)};
    let single_meta_set = MetaSet{name: String::from("mulark.github.io maps"), members: vec!(String::from("test-000041"), String::from("test-000025"))};
    let mut another_meta_set = single_meta_set.clone();
    another_meta_set.name = String::from("another meta set");
    another_meta_set.members[0] = String::from("another thingy");
    another_meta_set.members[1] = String::from("something other than that");
    let mut benchmark_sets = Vec::new();
    let mut meta_sets = Vec::new();
    benchmark_sets.push(single_benchmark_set);
    meta_sets.push(single_meta_set);
    meta_sets.push(another_meta_set);

    let top_level = TopLevel{meta_sets, benchmark_sets};
    let j = serde_json::to_string_pretty(&top_level).unwrap();
    println!("{}", j);
}
