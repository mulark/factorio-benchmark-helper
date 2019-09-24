extern crate regex;
extern crate directories;
extern crate getopts;
extern crate glob;

use core::fmt::Error;
use std::process::Command;
use std::path::PathBuf;
use regex::Regex;
use std::env;
use std::fs;
use glob::glob;

mod common;
mod config;
mod help;
mod database;

const FACTORIO_BENCHMARK_HELPER_VERSION: &str = "0.0.1";

fn main() {
    fetch_maps_from_pattern(".","asd");
    config::setup_config_file(false);
    database::setup_database(false);
    read_args();
    //setup_directories();
    run_benchmark("test-000044.480MW", 1000, 3, "./factorio");
//    print(x);
}

fn read_args() {
    let args: Vec<String> = env::args().collect();
    for (i,arg) in args.iter().enumerate() {
        println!("{}, {}",i, arg);
    }
    let mut options = getopts::Options::new();
    options.parsing_style(getopts::ParsingStyle::FloatingFrees);
    options.opt("h","help","Prints general help, or help about OPTION if supplied","OPTION",getopts::HasArg::Maybe,getopts::Occur::Optional);
    options.optflag("v", "version", "Print program version");
    options.optflag("i", "interactive", "Runs program in interactive mode");
    options.opt("p","pattern","Limit benchmarks to maps that match PATTERN","PATTERN",getopts::HasArg::Yes,getopts::Occur::Optional);
    options.optflag("a", "auto-analysis", "Runs program in auto-analysis mode");
    options.optflag("", "regen-config-file", "Regenerates the config.ini file from defaults");
    let matched_options = match options.parse(&args[1..]) {
        Ok (m) => { m }
        Err (e) => {
            println!("{}", options.short_usage("factorio_rust"));
            panic!(e.to_string());
        }
    };
    if matched_options.opt_present("help") {
        let help_arg = matched_options.opt_get_default::<String>("help","help".to_string()).unwrap();
        if help_arg == "help" {
            println!("{}", options.usage("factorio_rust"));
        }
        else
        {
            help::print_help(&help_arg);
        }
        return;
    }
    if matched_options.opt_present("version") {
        print_version();
        return;
    }
    if matched_options.opt_present("pattern") {
        let restrict_pattern = matched_options.opt_strs("pattern");
        println!("{:?}",restrict_pattern);
        for x in restrict_pattern.iter() {
            println!("{:?}",x);
        }
    }
    if matched_options.opt_present("interactive") {

        run_interactive();
    }
    if matched_options.opt_present("auto-analysis") {

    }
    if matched_options.opt_present("regen-config-file") {
        println!("Regenerating config file from defaults!");
        config::setup_config_file(true);
        std::process::exit(0);
    }
}

fn print_version() {
    println!("factorio_rust {}",FACTORIO_BENCHMARK_HELPER_VERSION);
    std::process::exit(0);
}

fn run_interactive() {
    println!("Selected interactive mode")
}

fn fetch_maps_from_pattern(factorio_saves_directory: &str, match_pattern: &str) -> i32 {
    let save_path = PathBuf::from(factorio_saves_directory);
    assert!(save_path.is_dir());

    for path in glob::glob(match_pattern).unwrap().filter_map(Result::ok) {
        if path.is_file() {
            let extension = path.extension().unwrap().to_str().expect("");
            if extension == "zip" {

            }
        }
    }


    return 3;
}

fn run_benchmark(map: &str,ticks: u32, runs: u32, executable_path: &str) {
    println!("You doofus, you haven't setup an executable path");
    let bench_data = Command::new(executable_path)
        .arg("--benchmark")
        .arg(map.to_string())
        .arg("--benchmark-ticks")
        .arg(ticks.to_string())
        .arg("--benchmark-runs")
        .arg(runs.to_string())
        .arg("--benchmark-verbose")
        .arg("all")
        .output()
        .expect("");
    let bench_data_refined = String::from_utf8_lossy(&bench_data.stdout);
    let verbose_line_match_pattern = Regex::new("t[0-9]*[0-9],[0-9]").unwrap();
    let mut run_index = 0;
    let mut line_index = 0;
    assert_eq!(bench_data_refined.lines().count(), (ticks * runs) as usize);
    for line in bench_data_refined.lines() {
        if verbose_line_match_pattern.is_match(line) {
            if line_index % 1000 == 0 {
                run_index = run_index + 1;
            }
            println!("{}{}",line,run_index);
            line_index = line_index + 1;
            assert!(run_index > 0);
        }

    }
    //  println!("stdout: {}", String::from_utf8_lossy(&bench_data.stdout));
}
