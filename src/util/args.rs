use crate::procedure_file::print_all_procedures;
use crate::procedure_file::ProcedureOverwrite;
use crate::util::common::FACTORIO_BENCHMARK_HELPER_NAME;
use crate::util::common::FACTORIO_BENCHMARK_HELPER_VERSION;
use crate::util::factorio_save_directory;
use crate::util::prompt_until_allowed_val;
use crate::util::ProcedureKind;
use clap::ArgMatches;
use clap::{App, AppSettings, Arg};
use std::path::PathBuf;
use std::process::exit;

#[derive(Debug)]
pub struct UserArgs {
    pub interactive: bool,
    pub overwrite: ProcedureOverwrite,

    pub run_benchmark: bool,
    pub create_benchmark: bool,

    pub benchmark_set_name: Option<String>,
    pub folder: Option<PathBuf>,
    pub ticks: Option<u32>,
    pub runs: Option<u32>,
    pub mods_dirty: Option<String>,

    pub run_meta: bool,
    pub create_meta: bool,
    pub meta_set_name: Option<String>,
    pub meta_set_members: Option<String>,

    pub commit_flag: bool,
    pub commit_name: Option<String>,
    pub commit_type: Option<ProcedureKind>,
    pub commit_recursive: bool,
}

impl Default for UserArgs {
    fn default() -> UserArgs {
        UserArgs {
            interactive: false,
            overwrite: false.into(),

            run_benchmark: false,
            create_benchmark: false,
            benchmark_set_name: None,

            ticks: None,
            runs: None,
            folder: None,
            mods_dirty: None,

            run_meta: false,
            create_meta: false,
            meta_set_name: None,
            meta_set_members: None,

            commit_flag: false,
            commit_name: None,
            commit_type: None,
            commit_recursive: false,
        }
    }
}

pub fn add_options_and_parse() -> UserArgs {
    let matches = App::new(FACTORIO_BENCHMARK_HELPER_NAME)
        .version(FACTORIO_BENCHMARK_HELPER_VERSION)
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::DeriveDisplayOrder)
        .setting(AppSettings::StrictUtf8)
        .version_short("v")
        .arg(
            Arg::with_name("list")
                .long("list")
                .help("List available benchmark/meta sets")
        )
        .arg(
            Arg::with_name("interactive")
                .short("-i")
                .long("interactive")
                .help("Run program interactively, prompting for actions")
        )
        .arg(
            Arg::with_name("overwrite")
                .long("overwrite")
                .help("If a benchmark/meta set already exists with NAME, overwrite")
        )
        .arg(
            Arg::with_name("recursive")
                .long("recursive")
                .short("r")
                .help("When committing a meta set, also recursively commit every meta/benchmark set contained within that set.")
        )
        .args(&[
            Arg::with_name("benchmark")
                .long("benchmark")
                .help("Run a benchmark of a named benchmark set")
                .value_name("NAME"),
            Arg::with_name("meta")
                .long("meta")
                .help("Runs benchmarks of all benchmark/meta sets found recusively within this meta set.")
                .value_name("NAME"),
            Arg::with_name("create-benchmark")
                .long("create-benchmark")
                .help("Creates a new benchmark, using NAME")
                .requires("folder")
                .value_name("NAME"),
            Arg::with_name("folder")
                .long("folder")
                .help("Restrict maps to those matching contained within FOLDER. This folder can be an absolute path, a relative path from the Factorio saves directory, or a relative path from the current directory. Priority is given in that order.")
                .min_values(1)
                .value_name("FOLDER"),
            Arg::with_name("ticks")
                .long("ticks")
                .help("The number of ticks each map should be benchmarked for per run")
                .value_name("TICKS"),
            Arg::with_name("runs")
                .long("runs")
                .help("How many times each map should be benchmarked")
                .value_name("RUNS"),
            Arg::with_name("mods")
                .long("mods")
                .help("A comma separated list of mods you wish to create this benchmark with.\
                    'region-cloner' specifies the latest version of region cloner, whereas\
                    'region-cloner_1.1.2' specifies that specific version.")
                .value_name("MODS..."),
            Arg::with_name("create-meta")
                .long("create-meta")
                .help("Creates a meta set with NAME, with provided MEMBERS. MEMBERS given as a comma separated list.")
                .value_names(&["NAME","MEMBERS..."])
                .min_values(2),
            Arg::with_name("commit")
                .long("commit")
                .help("Writes the benchmark or meta set TYPE with NAME to the master.json file. Types are \"benchmark\", \"meta\"")
                .conflicts_with_all(&[
                    "benchmark",
                    "run_meta",
                    "create-benchmark",
                    "pattern",
                    "ticks",
                    "runs",
                    "google-drive-folder",
                    "create-meta"
                ])
                .value_names(&["TYPE", "NAME"]),
            ])
        .get_matches();
    parse_matches(&matches)
}

fn parse_matches(matches: &ArgMatches) -> UserArgs {
    let args = &matches.args;
    let mut arguments = UserArgs::default();
    if args.contains_key("interactive") {
        arguments.interactive = true;
    }
    if args.contains_key("overwrite") {
        arguments.overwrite = true.into();
    }
    if args.contains_key("list") {
        print_all_procedures();
        exit(0);
    }

    if args.contains_key("benchmark") {
        arguments.run_benchmark = true;
        arguments.benchmark_set_name = Some(
            args["benchmark"].vals[0]
                .to_str()
                .unwrap()
                .trim()
                .to_string(),
        );
    }

    if args.contains_key("meta") {
        arguments.run_meta = true;
        arguments.meta_set_name =
            Some(args["meta"].vals[0].to_str().unwrap().trim().to_string());
    }

    if args.contains_key("create-benchmark") {
        arguments.create_benchmark = true;
        arguments.benchmark_set_name = Some(
            args["create-benchmark"].vals[0]
                .to_str()
                .unwrap()
                .trim()
                .to_string(),
        );
    }

    if args.contains_key("folder") {
        let path =
            PathBuf::from(args["folder"].vals[0].to_str().unwrap().trim());
        if path.is_absolute() {
            arguments.folder = Some(path);
        } else if factorio_save_directory().join(&path).is_dir() {
            arguments.folder = Some(factorio_save_directory().join(&path));
        } else if path.is_dir() {
            if let Ok(path) = path.canonicalize() {
                arguments.folder = Some(path);
            } else {
                eprintln!(
                    "Could not resolve path {:?} to a valid folder",
                    path
                );
                exit(1);
            }
        } else {
            eprintln!("Could not resolve path {:?} to a valid folder", path);
            exit(1);
        }
    }

    if args.contains_key("ticks") {
        arguments.ticks =
            try_parse_nonzero_u32(args["ticks"].vals[0].to_str().unwrap_or(""));
    }

    if args.contains_key("runs") {
        arguments.runs =
            try_parse_nonzero_u32(args["runs"].vals[0].to_str().unwrap_or(""));
    }

    if args.contains_key("mods") {
        let collect_as_csv: String = args["mods"]
            .vals
            .iter()
            .map(|x| x.to_str().unwrap().trim())
            .collect();
        arguments.mods_dirty = Some(collect_as_csv);
    }

    if args.contains_key("create-meta") {
        arguments.create_meta = true;
        let collect_as_csv: String = args["create-meta"].vals[1..]
            .iter()
            .map(|x| x.to_str().unwrap().trim())
            .collect();
        arguments.meta_set_members = Some(collect_as_csv);

        let name = args["create-meta"].vals[0]
            .to_str()
            .unwrap()
            .replace(',', "");
        arguments.meta_set_name = Some(name);
    }

    if args.contains_key("commit") {
        arguments.commit_flag = true;
        let commit_name =
            args["commit"].vals[1].to_str().unwrap().trim().to_string();
        arguments.commit_name = Some(commit_name);
        if let Ok(commit_type) =
            args["commit"].vals[0].to_str().unwrap().trim().parse()
        {
            arguments.commit_type = Some(commit_type);
        } else if arguments.interactive {
            println!("You failed to set a valid type from args, and are running interactively, enter a commit type. types: benchmark, meta");
            arguments.commit_type = Some(prompt_until_allowed_val(&[
                ProcedureKind::Benchmark,
                ProcedureKind::Meta,
            ]));
        } else {
            unreachable!("Illegal commit type found!");
        }
    }

    if args.contains_key("recursive") {
        arguments.commit_recursive = true;
    }

    arguments
}

fn try_parse_nonzero_u32(s: &str) -> Option<u32> {
    match s.parse::<u32>() {
        Ok(u) => {
            if u != 0 {
                Some(u)
            } else {
                eprintln!("Parsed arg not allowd to be 0!");
                exit(1);
            }
        }
        _ => {
            eprintln!("Failed to process --runs as u32");
            exit(1);
        }
    }
}

#[cfg(test)]
mod test {
    extern crate assert_cmd;
    use crate::util::common::FACTORIO_BENCHMARK_HELPER_NAME;
    use crate::util::common::FACTORIO_BENCHMARK_HELPER_VERSION;
    use assert_cmd::Command;
    #[test]
    fn test_version() {
        let output = Command::cargo_bin(FACTORIO_BENCHMARK_HELPER_NAME)
            .unwrap()
            .arg("--version")
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!(
                "{} {}\n",
                FACTORIO_BENCHMARK_HELPER_NAME,
                FACTORIO_BENCHMARK_HELPER_VERSION
            )
        );
    }
}
