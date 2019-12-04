extern crate reqwest;
extern crate serde;
extern crate serde_json;

use crate::util::fbh_known_hash_file;
use crate::util::prompt_until_allowed_val;
use std::fmt::Debug;
use core::str::FromStr;
use std::collections::HashMap;
use std::process::exit;
use std::fs::read;
use crate::util::{fbh_procedure_json_local_file ,fbh_procedure_json_master_file, Map, Mod};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopLevel {
    pub benchmark_sets: BTreeMap<String, BenchmarkSet>,
    pub meta_sets: BTreeMap<String, Vec<String>>,
}

impl TopLevel {
    pub fn print_summary(self, kinds: ProcedureKind) {
        if kinds == ProcedureKind::Benchmark || kinds == ProcedureKind::Both {
            println!("    Benchmark Sets:");
            for set in self.benchmark_sets.keys() {
                println!("\t{:?}", set);
            }
        }
        if kinds == ProcedureKind::Meta || kinds == ProcedureKind::Both {
            println!("    Meta Sets:");
            for set in self.meta_sets.keys() {
                println!("\t{:?}", set);
            }
        }
    }
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

impl PartialEq for BenchmarkSet {
    fn eq(&self, cmp: &Self) -> bool {
        let mut ret = true;
        if self.ticks == cmp.ticks && self.runs == cmp.runs && self.maps.len() == cmp.maps.len() && self.mods.len() == cmp.mods.len() {
            for i in 0..self.maps.len() {
                if self.maps[i] != cmp.maps[i] {
                    ret = false;
                }
            }
            for i in 0..self.mods.len() {
                if self.mods[i] != cmp.mods[i] {
                    ret = false;
                }
            }
        } else {
            ret = false;
        }
        ret
    }
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

#[derive(Debug, PartialEq)]
pub enum ProcedureKind {
    Benchmark,
    Meta,
    Both,
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
                    &read(fbh_procedure_json_local_file()).unwrap()
                ).unwrap_or_default();
                return json;
            }
        }
        ProcedureFileKind::Master => {
            if fbh_procedure_json_master_file().exists() {
                let json: Option<TopLevel> = serde_json::from_slice(
                    &read(fbh_procedure_json_master_file()).unwrap()
                ).unwrap_or_default();
                return json;
            }
        }
    }
    None
}

impl FromStr for ProcedureKind {
    type Err = String;
    fn from_str(s: &str) -> Result<ProcedureKind,Self::Err> {
        match s.to_lowercase().as_str() {
            "benchmark" => Ok(ProcedureKind::Benchmark),
            "meta" => Ok(ProcedureKind::Meta),
            _ => Err(String::from("Error: UnknownProcedureType")),
        }
    }
}

pub fn print_procedures(procedure_kind: ProcedureKind, file_kind: ProcedureFileKind) {
    let top_level = load_top_level_from_file(file_kind);
    if let Some(t) = top_level {
        t.print_summary(procedure_kind)
    }
}

pub fn print_all_procedures() {
    println!("Local: ");
    print_procedures(ProcedureKind::Both, ProcedureFileKind::Local);
    println!("Master:");
    print_procedures(ProcedureKind::Both, ProcedureFileKind::Master);
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

pub fn write_procedure_to_file(name: &str, set: BenchmarkSet, force: bool, file_kind: ProcedureFileKind, interactive: bool) {
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
        if interactive {
            println!("Procedure already exists, overwrite?");
            match prompt_until_allowed_val(&["y".to_string(), "n".to_string()]).as_str() {
                "y" => ({
                    top_level.benchmark_sets.insert(name.to_string(), set);
                    let j = serde_json::to_string_pretty(&top_level).unwrap();
                    std::fs::write(file_path, j).unwrap();
                }),
                "n" => (),
                _ => unreachable!("interactive answer not y or n, but how?"),
            }
        } else {
            eprintln!("Cannot write procedure to file, {:?} already exists! Maybe use --overwrite?", name);
            exit(1);
        }
    } else {
        top_level.benchmark_sets.insert(name.to_string(), set);
        let j = serde_json::to_string_pretty(&top_level).unwrap();
        std::fs::write(file_path, j).unwrap();
    }
}

pub fn read_meta_from_file(name: &str, file_kind: ProcedureFileKind) -> Option<Vec<String>> {
    match load_top_level_from_file(file_kind) {
        Some(m) => {
            if m.meta_sets.contains_key(name) {
                return Some(m.meta_sets[name].clone())
            }
        }
        _ => return None,
    }
    None
}

pub fn write_meta_to_file(name: &str, members: Vec<String>, force: bool, file_kind: ProcedureFileKind) {
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
        eprintln!("Cannot write procedure to master file, meta set {:?} already exists! Maybe use --overwrite?", name);
        exit(1);
    } else {
        top_level.meta_sets.insert(name.to_string(), members);
        let j = serde_json::to_string_pretty(&top_level).unwrap();
        std::fs::write(file_path, j).unwrap();
    }
}


// Returns a hashmap of all benchmark sets contained within this meta set, as well as the meta sets
// found recursively within meta sets contained within this meta set.
pub fn get_sets_from_meta(meta_set_key: String, source: ProcedureFileKind) -> HashMap<String, BenchmarkSet> {
    let mut current_sets = HashMap::new();
    let mut seen_keys = Vec::new();
    let top_level = load_top_level_from_file(source).unwrap();
    walk_meta_recursive_for_benchmarks(meta_set_key, &top_level, &mut seen_keys, &mut current_sets);
    current_sets
}

fn walk_meta_recursive_for_benchmarks(key: String, top_level: &TopLevel, seen_keys: &mut Vec<String>, current_benchmark_sets: &mut HashMap<String, BenchmarkSet>) {
    if !seen_keys.contains(&key) {
        if top_level.meta_sets.contains_key(&key) {
            seen_keys.push(key.clone());
            for k in &top_level.meta_sets[&key] {
                walk_meta_recursive_for_benchmarks(k.to_string(), &top_level, seen_keys, current_benchmark_sets);
            }
        }
        if top_level.benchmark_sets.contains_key(&key) {
            current_benchmark_sets.insert(key.clone(), top_level.benchmark_sets[&key].to_owned());
        }
    }
}

pub fn get_metas_from_meta(meta_set_key: String, file_source_type: ProcedureFileKind) -> Vec<String> {
    let mut seen_keys = Vec::new();
    let mut current_meta_sets = Vec::new();
    let top_level = load_top_level_from_file(file_source_type).unwrap();
    walk_meta_recursive_for_metas(meta_set_key, &top_level, &mut seen_keys, &mut current_meta_sets);
    current_meta_sets
}

fn walk_meta_recursive_for_metas(key: String, top_level: &TopLevel, seen_keys: &mut Vec<String>, current_meta_sets: &mut Vec<String>) {
    if !seen_keys.contains(&key) && top_level.meta_sets.contains_key(&key) {
        seen_keys.push(key.clone());
        for k in &top_level.meta_sets[&key] {
            walk_meta_recursive_for_metas(k.to_string(), &top_level, seen_keys, current_meta_sets);
        }
        current_meta_sets.push(key);
    }
}

pub fn write_known_map_hash(s: String) {
    let mut hashes = load_known_map_hashes();
    hashes.push(s);
    hashes.sort();
    hashes.dedup();
    let j = serde_json::to_string_pretty(&hashes).unwrap();
    std::fs::write(fbh_known_hash_file(), j).unwrap();
}

pub fn is_known_map_hash(s: String) -> bool {
    let hashes = load_known_map_hashes();
    hashes.contains(&s)
}

fn load_known_map_hashes() -> Vec<String> {
    serde_json::from_slice(
        &read(fbh_known_hash_file()).expect("")
    ).unwrap_or_default()
}

#[allow(dead_code)]
pub fn create_procedure_example() {
    let mut set = BenchmarkSet::default();
    set.maps = vec!(Map::new("name","hash","dl"));
    set.runs = 100;
    set.ticks = 40;
    write_procedure_to_file("test-000041-1", set, true, ProcedureFileKind::Local, false);
    write_procedure_to_file("test-000041-2", BenchmarkSet::default(), true, ProcedureFileKind::Local, true);
    write_meta_to_file("mulark.github.io maps", vec!("test-000041-1".to_string(), "test-000041-2".to_string()), true, ProcedureFileKind::Local);
    let single_map = Map::new("test-000046.dummy_load", "a hash", "a download link");
    let single_mod = Mod::new("this mod", "this mod.zip", "this hash", "this version");
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
