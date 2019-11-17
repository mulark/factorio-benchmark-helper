extern crate reqwest;
extern crate serde;
extern crate serde_json;

use std::path::PathBuf;
use std::sync::Mutex;
use std::fs::read;
use crate::util::{fbh_procedure_json_local_file ,fbh_procedure_json_master_file, Map, Mod};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File};
use std::io::BufReader;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopLevel {
    pub benchmark_sets: BTreeMap<String, BenchmarkSet>,
    pub meta_sets: BTreeMap<String, Vec<String>>,
}

impl Default for TopLevel {
    fn default() -> TopLevel {
        TopLevel {benchmark_sets: BTreeMap::new(), meta_sets: BTreeMap::new()}
    }
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

pub enum ProcedureFileKind {
    Local,
    Master,
}

fn load_top_level_from_file(file_type: ProcedureFileKind) -> Option<TopLevel> {
    match file_type {
        ProcedureFileKind::Local => {
            if fbh_procedure_json_local_file().exists() {
                let json: Option<TopLevel> = serde_json::from_slice(
                    &read(fbh_procedure_json_local_file()).expect("")
                ).unwrap_or_default();
                return json;
            }
        }
        ProcedureFileKind::Master => {
            if fbh_procedure_json_master_file().exists() {
                let json: Option<TopLevel> = serde_json::from_slice(
                    &read(fbh_procedure_json_master_file()).expect("")
                ).unwrap_or_default();
                return json;
            }
        }
    }
    None
}

pub fn read_procedure_from_file(name: &str, file_kind: ProcedureFileKind) -> Option<BenchmarkSet> {
    match load_top_level_from_file(file_kind) {
        Some(m) => {
            if m.benchmark_sets.contains_key(name) {
                return Some(m.benchmark_sets[name].clone())
            }
        }
        _ => return None,
    }
    None
}

pub fn write_procedure_to_file(name: &str, set: BenchmarkSet, force: bool, file_kind: ProcedureFileKind) {
    let mut top_level;
    let file_path = match file_kind {
        ProcedureFileKind::Local => fbh_procedure_json_local_file(),
        ProcedureFileKind::Master => fbh_procedure_json_master_file(),
    };
    match load_top_level_from_file(file_kind) {
        Some(m) => {
            top_level = m;
        }
        _ => {
            top_level = TopLevel::default();
        }
    }
    if top_level.benchmark_sets.contains_key(name) && !force {
        eprintln!("Cannot write procedure to file, {:?} already exists!", name);
        eprintln!("Maybe use --overwrite?");
        std::process::exit(1);
    } else {
        top_level.benchmark_sets.insert(name.to_string(), set);
        let j = serde_json::to_string_pretty(&top_level).unwrap();
        std::fs::write(file_path, j).unwrap();
    }
}

fn write_meta_to_file(name: &str, members: Vec<String>, force: bool, file_kind: ProcedureFileKind) {
    let mut top_level;
    let file_path = match file_kind {
        ProcedureFileKind::Local => fbh_procedure_json_local_file(),
        ProcedureFileKind::Master => fbh_procedure_json_master_file(),
    };
    match load_top_level_from_file(file_kind) {
        Some(m) => top_level = m,
        _ => top_level = TopLevel::default(),
    }

    if top_level.meta_sets.contains_key(name) && !force {
        eprintln!("Cannot write procedure to master file, the key {:?} already exists!", name);
        std::process::exit(1);
    } else {
        top_level.meta_sets.insert(name.to_string(), members);
        let j = serde_json::to_string_pretty(&top_level).unwrap();
        std::fs::write(file_path, j).unwrap();
    }
}

pub fn create_procedure_interactively() {
    let mut set = BenchmarkSet::default();
    set.maps = vec!(Map::new("name","hash","dl"));
    set.runs = 100;
    set.ticks = 40;
    write_procedure_to_file("test-000041-1", set, true, ProcedureFileKind::Local);
    write_procedure_to_file("test-000041-2", BenchmarkSet::default(), true, ProcedureFileKind::Local);
    write_meta_to_file("mulark.github.io maps", vec!("test-000041-1".to_string(), "test-000041-2".to_string()), true, ProcedureFileKind::Local);
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
    let reserialize: TopLevel = serde_json::from_str(&j).unwrap();
    println!("{:?}", reserialize);
}