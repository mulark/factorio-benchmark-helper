extern crate reqwest;
extern crate serde;
extern crate serde_json;

use std::ops::Not;
use crate::util::fbh_save_dl_dir;
use crate::backblaze::upload_files_to_backblaze;
use crate::util::download_benchmark_deps_parallel;
use crate::util::fbh_cache_path;
use crate::util::prompt_until_allowed_val;
use crate::util::{fbh_procedure_json_local_file, fbh_procedure_json_master_file, Map, Mod};
use core::str::FromStr;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::read;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::exit;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TopLevel {
    pub benchmark_sets: BTreeMap<String, BenchmarkSet>,
    pub meta_sets: BTreeMap<String, BTreeSet<String>>,
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct BenchmarkSet {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_subdirectory: Option<PathBuf>,
    pub mods: BTreeSet<Mod>,
    pub maps: BTreeSet<Map>,
    pub ticks: u32,
    pub runs: u32,
}

impl Default for BenchmarkSet {
    fn default() -> BenchmarkSet {
        BenchmarkSet {
            save_subdirectory: None,
            mods: BTreeSet::new(),
            maps: BTreeSet::new(),
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

#[derive(Debug)]
pub enum ProcedureFileKind {
    Local,
    Master,
    Custom(PathBuf),
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ProcedureOverwrite {
    True,
    False,
}

impl From<bool> for ProcedureOverwrite {
    fn from(b: bool) -> ProcedureOverwrite {
        if b { ProcedureOverwrite::True } else { ProcedureOverwrite::False }
    }
}

impl Not for ProcedureOverwrite {
    type Output = ProcedureOverwrite;
    fn not(self) -> Self::Output {
        match self {
            ProcedureOverwrite::True => ProcedureOverwrite::False,
            ProcedureOverwrite::False => ProcedureOverwrite::True,
        }
    }
}


#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ProcedureInteractive {
    True,
    False,
}

impl From<bool> for ProcedureInteractive {
    fn from(b: bool) -> ProcedureInteractive {
        if b { ProcedureInteractive::True } else { ProcedureInteractive::False }
    }
}

impl Not for ProcedureInteractive {
    type Output = ProcedureInteractive;
    fn not(self) -> Self::Output {
        match self {
            ProcedureInteractive::True => ProcedureInteractive::False,
            ProcedureInteractive::False => ProcedureInteractive::True,
        }
    }
}

pub fn update_master_json() {
    if let Some(orig_top_level) = load_top_level_from_file(&ProcedureFileKind::Master) {
        let new = fbh_cache_path().join(".new.json");
        perform_master_json_dl(&new);
        if let Some(new_top_level) = load_top_level_from_file(&ProcedureFileKind::Custom(new)) {
            for (k, v) in new_top_level.benchmark_sets {
                if orig_top_level.benchmark_sets.contains_key(&k) {
                    //println!("Automatically updating benchmark set {:?}", &k);
                }
                write_benchmark_set_to_file(&k, v, ProcedureOverwrite::True, ProcedureFileKind::Master, ProcedureInteractive::True);
            }
            for (k, v) in new_top_level.meta_sets {
                if orig_top_level.meta_sets.contains_key(&k) {
                    //println!("Automatically updating metaset {:?}", &k);
                }
                write_meta_to_file(&k, v, true.into(), ProcedureFileKind::Master);
            }
        }
    } else {
        perform_master_json_dl(&fbh_procedure_json_master_file());
    }
}

fn perform_master_json_dl(file_to_write: &PathBuf) {
    if let Ok(mut resp) = reqwest::get(
        "https://raw.githubusercontent.com/mulark/factorio-benchmark-helper/master/master.json",
    ) {
        if resp.status() == 200 {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .open(file_to_write)
                .unwrap();
            match resp.copy_to(&mut file) {
                Ok(_) => (),
                Err(e) => {
                    println!("Failed to write file to {:?}!", file);
                    panic!(e);
                }
            }
        }
    }
}

fn load_top_level_from_file(file_type: &ProcedureFileKind) -> Option<TopLevel> {
    match file_type {
        ProcedureFileKind::Local => {
            if fbh_procedure_json_local_file().exists() {
                let json: Option<TopLevel> =
                    serde_json::from_slice(&read(fbh_procedure_json_local_file()).unwrap())
                        .unwrap_or_default();
                return json;
            }
        }
        ProcedureFileKind::Master => {
            if fbh_procedure_json_master_file().exists() {
                let json: Option<TopLevel> =
                    serde_json::from_slice(&read(fbh_procedure_json_master_file()).unwrap())
                        .unwrap_or_default();
                return json;
            }
        }
        ProcedureFileKind::Custom(p) => {
            if p.exists() {
                let json: Option<TopLevel> =
                    serde_json::from_slice(&read(p).unwrap()).unwrap_or_default();
                return json;
            }
        }
    }
    None
}

impl FromStr for ProcedureKind {
    type Err = String;
    fn from_str(s: &str) -> Result<ProcedureKind, Self::Err> {
        match s.to_lowercase().as_str() {
            "benchmark" => Ok(ProcedureKind::Benchmark),
            "meta" => Ok(ProcedureKind::Meta),
            _ => Err(String::from("Error: UnknownProcedureType")),
        }
    }
}

pub fn print_procedures(procedure_kind: ProcedureKind, file_kind: ProcedureFileKind) {
    let top_level = load_top_level_from_file(&file_kind);
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

pub fn read_benchmark_set_from_file(
    name: &str,
    file_kind: ProcedureFileKind,
) -> Option<BenchmarkSet> {
    match load_top_level_from_file(&file_kind) {
        Some(m) => {
            if m.benchmark_sets.contains_key(name) {
                return Some(m.benchmark_sets[name].clone());
            }
        }
        _ => return None,
    }
    None
}

pub fn write_benchmark_set_to_file(
    name: &str,
    set: BenchmarkSet,
    force: ProcedureOverwrite,
    file_kind: ProcedureFileKind,
    interactive: ProcedureInteractive,
) {
    let mut top_level;
    match load_top_level_from_file(&file_kind) {
        Some(m) => {
            top_level = m;
        }
        _ => {
            top_level = TopLevel::default();
        }
    }
    let procedure_file_path = match file_kind {
        ProcedureFileKind::Local => fbh_procedure_json_local_file(),
        ProcedureFileKind::Master => fbh_procedure_json_master_file(),
        ProcedureFileKind::Custom(p) => p,
    };
    if top_level.benchmark_sets.contains_key(name) && force == false.into() {
        if interactive == ProcedureInteractive::True {
            println!("Procedure already exists, overwrite?");
            match prompt_until_allowed_val(&["y".to_string(), "n".to_string()]).as_str() {
                "y" => {
                    ({
                        top_level.benchmark_sets.insert(name.to_string(), set);
                        let j = serde_json::to_string_pretty(&top_level).unwrap();
                        std::fs::write(procedure_file_path, j).unwrap();
                    })
                }
                "n" => (),
                _ => unreachable!("interactive answer not y or n, but how?"),
            }
        } else {
            eprintln!(
                "Cannot write procedure to file, {:?} already exists! Maybe use --overwrite?",
                name
            );
            exit(1);
        }
    } else {
        top_level.benchmark_sets.insert(name.to_string(), set);
        let j = serde_json::to_string_pretty(&top_level).unwrap();
        std::fs::write(procedure_file_path, j).unwrap();
    }
}

pub fn read_meta_from_file(name: &str, file_kind: ProcedureFileKind) -> Option<BTreeSet<String>> {
    match load_top_level_from_file(&file_kind) {
        Some(m) => {
            if m.meta_sets.contains_key(name) {
                return Some(m.meta_sets[name].clone());
            }
        }
        _ => return None,
    }
    None
}

pub fn write_meta_to_file(
    name: &str,
    members: BTreeSet<String>,
    force: ProcedureOverwrite,
    file_kind: ProcedureFileKind,
) {
    let mut top_level;
    match load_top_level_from_file(&file_kind) {
        Some(m) => top_level = m,
        _ => top_level = TopLevel::default(),
    }
    let procedure_file_path = match file_kind {
        ProcedureFileKind::Local => fbh_procedure_json_local_file(),
        ProcedureFileKind::Master => fbh_procedure_json_master_file(),
        ProcedureFileKind::Custom(p) => p,
    };

    if top_level.meta_sets.contains_key(name) && force == false.into() {
        eprintln!("Cannot write procedure to master file, meta set {:?} already exists! Maybe use --overwrite?", name);
        exit(1);
    } else {
        top_level.meta_sets.insert(name.to_string(), members);
        let j = serde_json::to_string_pretty(&top_level).unwrap();
        std::fs::write(procedure_file_path, j).unwrap();
    }
}

// Returns a hashmap of all benchmark sets contained within this meta set, as well as the meta sets
// found recursively within meta sets contained within this meta set.
pub fn get_sets_from_meta(
    meta_set_key: String,
    source: ProcedureFileKind,
) -> HashMap<String, BenchmarkSet> {
    let mut current_sets = HashMap::new();
    let mut seen_keys = Vec::new();
    let top_level = load_top_level_from_file(&source).unwrap();
    walk_meta_recursive_for_benchmarks(meta_set_key, &top_level, &mut seen_keys, &mut current_sets);
    current_sets
}

fn walk_meta_recursive_for_benchmarks(
    key: String,
    top_level: &TopLevel,
    seen_keys: &mut Vec<String>,
    current_benchmark_sets: &mut HashMap<String, BenchmarkSet>,
) {
    if !seen_keys.contains(&key) {
        if top_level.meta_sets.contains_key(&key) {
            seen_keys.push(key.clone());
            for k in &top_level.meta_sets[&key] {
                walk_meta_recursive_for_benchmarks(
                    k.to_string(),
                    &top_level,
                    seen_keys,
                    current_benchmark_sets,
                );
            }
        }
        if top_level.benchmark_sets.contains_key(&key) {
            current_benchmark_sets.insert(key.clone(), top_level.benchmark_sets[&key].to_owned());
        }
    }
}

pub fn get_metas_from_meta(
    meta_set_key: String,
    file_source_type: ProcedureFileKind,
) -> Vec<String> {
    let mut seen_keys = Vec::new();
    let mut current_meta_sets = Vec::new();
    let top_level = load_top_level_from_file(&file_source_type).unwrap();
    walk_meta_recursive_for_metas(
        meta_set_key,
        &top_level,
        &mut seen_keys,
        &mut current_meta_sets,
    );
    current_meta_sets
}

fn walk_meta_recursive_for_metas(
    key: String,
    top_level: &TopLevel,
    seen_keys: &mut Vec<String>,
    current_meta_sets: &mut Vec<String>,
) {
    if !seen_keys.contains(&key) && top_level.meta_sets.contains_key(&key) {
        seen_keys.push(key.clone());
        for k in &top_level.meta_sets[&key] {
            walk_meta_recursive_for_metas(k.to_string(), &top_level, seen_keys, current_meta_sets);
        }
        current_meta_sets.push(key);
    }
}
