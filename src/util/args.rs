use crate::procedure_file::write_procedure_to_file;
use crate::procedure_file::read_procedure_from_file;
use crate::procedure_file::print_all_procedures;
use crate::ProcedureFileKind;
use std::process::exit;
use crate::FACTORIO_BENCHMARK_HELPER_VERSION;
use crate::FACTORIO_BENCHMARK_HELPER_NAME;
use clap::ArgMatches;
use clap::{Arg,App,AppSettings};

#[derive(Debug)]
pub struct UserArgs {
    pub interactive: bool,
    pub overwrite: bool,

    pub run_benchmark: bool,
    pub create_benchmark: bool,

    pub benchmark_set_name: Option<String>,
    pub pattern: Option<String>,
    pub ticks: Option<u32>,
    pub runs: Option<u32>,
    pub google_drive_folder: Option<String>,

    pub run_meta: bool,
    pub create_meta: bool,
    pub meta_set_name: Option<String>,
    pub meta_set_members: Option<Vec<String>>,

    pub commit_flag: bool,
    pub commit_name: Option<String>,
    pub commit_type: Option<String>,
}


impl Default for UserArgs {
    fn default() -> UserArgs {
        UserArgs {
            interactive: false,
            overwrite: false,

            run_benchmark: false,
            create_benchmark: false,
            benchmark_set_name: None,

            ticks: None,
            runs: None,
            pattern: None,
            google_drive_folder: None,

            run_meta: false,
            create_meta: false,
            meta_set_name: None,
            meta_set_members: None,

            commit_flag: false,
            commit_name: None,
            commit_type: None,
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
        .args(&[
            Arg::with_name("benchmark")
                .long("benchmark")
                .help("Run a benchmark of a named benchmark set")
                .value_name("NAME"),
            Arg::with_name("meta")
                .long("meta")
                .help("Runs benchmarks of all benchmark/meta sets found recusively within this meta set.")
                .value_name("NAME"),
            Arg::with_name("pattern")
                .long("pattern")
                .help("Restrict maps to those matching PATTERN")
                .min_values(1)
                .value_name("PATTERN"),
            Arg::with_name("ticks")
                .long("ticks")
                .help("The number of ticks each map should be benchmarked for per run")
                .value_name("TICKS"),
            Arg::with_name("runs")
                .long("runs")
                .help("How many times each map should be benchmarked")
                .value_name("RUNS"),
            Arg::with_name("google-drive-folder")
                .long("google-drive-folder")
                .help("A link to a publicly shared google drive folder so that individual download links can be automatically filled")
                .value_name("LINK"),
            Arg::with_name("create-benchmark")
                .long("create-benchmark")
                .help("Creates a new benchmark, using NAME")
                .value_name("NAME"),
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
        arguments.overwrite = true;
    }
    if args.contains_key("list") {
        print_all_procedures();
        exit(0);
    }
    if args.contains_key("benchmark") {
        arguments.run_benchmark = true;
        arguments.benchmark_set_name = Some(args["benchmark"].vals[0].to_str().unwrap().to_string());
    }
    if args.contains_key("meta") {
        arguments.run_meta = true;
        arguments.meta_set_name = Some(args["meta"].vals[0].to_str().unwrap().to_string());
    }
    if args.contains_key("create-benchmark") {
        arguments.create_benchmark = true;
        arguments.benchmark_set_name = Some(args["create-benchmark"].vals[0].to_str().unwrap().to_string());
    }
    if args.contains_key("pattern") {
        let mut pattern = String::new();
        for v in &args["pattern"].vals {
            pattern.push_str(&v.clone().into_string().unwrap());
            pattern.push_str(" ");
        }
        pattern.pop();
        arguments.pattern = Some(pattern);
    }
    if args.contains_key("ticks") {
        arguments.ticks = match args["ticks"].vals[0].clone().into_string() {
            Ok(m) =>  match m.parse::<u32>() {
                Ok(u) => if u != 0 {
                    Some(u)
                } else {
                    eprintln!("Ticks not allowed to be 0!");
                    exit(1);
                }
                _ => {
                    eprintln!("Failed to process --ticks as u32");
                    exit(1);
                },
            },
            _ => {
                eprintln!("Failed to process --ticks");
                exit(1);
            },
        };
    }
    if args.contains_key("runs") {
        arguments.ticks = match args["runs"].vals[0].clone().into_string() {
            Ok(m) =>  match m.parse::<u32>() {
                Ok(u) => if u != 0 {
                    Some(u)
                } else {
                    eprintln!("Runs not allowed to be 0!");
                    exit(1);
                }
                _ => {
                    eprintln!("Failed to process --runs as u32");
                    exit(1);
                },
            },
            _ => {
                eprintln!("Failed to process --runs");
                exit(1);
            },
        };
    }
    if args.contains_key("google-drive-folder") {
        let url = args["google-drive-folder"].vals[0].to_str().unwrap().to_string();
        if url.contains("drive.google.com") {

        }
    }
    if args.contains_key("create-meta") {
        arguments.create_meta = true;
        let mut vals: Vec<String> = Vec::new();
        for v in args["create-meta"].vals[1..].iter() {
            for k in v.to_str().unwrap().split(',') {
                vals.push(k.to_string());
            };
        };
        let name = args["create-meta"].vals[0].to_str().unwrap().replace(',', "");
        arguments.meta_set_name = Some(name);
        arguments.meta_set_members = Some(vals);
    }
    if args.contains_key("commit") {
        arguments.commit_flag = true;
        let commit_name = args["commit"].vals[0].to_str().unwrap().to_string();
        arguments.commit_name = Some(commit_name);
        let commit_type = args["commit"].vals[1].to_str().unwrap().to_string().to_lowercase();
        if !(commit_type == "benchmark" || commit_type == "meta") {
            eprintln!("Invalid type supplied for commit! Expected {:?} or {:?}", "benchmark", "meta");
            if arguments.interactive {
                println!("Stub for interactively fetching commit type");
                exit(1);
            } else {
                exit(1);
            }
        }
        arguments.commit_type = Some(commit_type);
    }
    println!("{:?}", arguments);
    arguments
}
