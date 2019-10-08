#![warn(unreachable_code)]

#[macro_use]
extern crate lazy_static;

extern crate regex;
extern crate directories;
extern crate getopts;
extern crate glob;

use std::path::PathBuf;
use std::env;
use benchmark_utility::{BenchmarkParams, run_benchmarks};

mod benchmark_utility;
mod common;
mod help;
mod database;

const FACTORIO_BENCHMARK_HELPER_VERSION: &str = "0.0.1";

fn main() {
    println!("{:?}", common::get_saves_directory());
    common::setup_config_file(false);
    database::setup_database(false);
    let mut params = BenchmarkParams::default();
    parse_args(&mut params);
    get_maps_and_append_to_params(common::get_saves_directory(), &mut params);
    run_benchmarks(&params, &common::get_executable_path());
}

fn parse_args(params: &mut BenchmarkParams) {
    let args: Vec<String> = env::args().collect();
    let mut options = getopts::Options::new();
    options.parsing_style(getopts::ParsingStyle::FloatingFrees);
    options.opt("h","help","Prints general help, or help about OPTION if supplied","OPTION",getopts::HasArg::Maybe,getopts::Occur::Optional);
    options.optflag("v", "version", "Print program version");
    options.optflag("i", "interactive", "Runs program in interactive mode");
    options.opt("p","pattern","Limit benchmarks to maps that match PATTERN","PATTERN",getopts::HasArg::Yes,getopts::Occur::Optional);
    options.optopt("t", "ticks", "Runs benchmarks for TICKS duration per run", "TICKS");
    options.optopt("r", "runs", "How many times should each map be benchmarked?", "TIMES");
    options.optflag("a", "auto-analysis", "Runs program in auto-analysis mode");
    options.optflag("", "regen-config-file", "Regenerates the config.ini file from defaults");
    options.optflag("", "set-executable-path", "Sets the path of the Factorio executable, and writes it to the config file");
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
        std::process::exit(0);
    }
    if matched_options.opt_present("version") {
        print_version();
        std::process::exit(0);
    }
    if matched_options.opt_present("pattern") {
        if let Some(matching_string) = matched_options.opt_str("pattern") {
            params.match_pattern = matching_string;
        }
    }
    if matched_options.opt_present("ticks") {
        if let Some(matching_string) = matched_options.opt_str("ticks") {
            match matching_string.parse::<u32>() {
                Ok(uint) => {
                    if uint != 0 {
                        params.ticks = uint;
                    } else {
                        println!("Ticks must be greater than 0!");
                    }
                },
                Err(e) => {
                    println!("Could not parse ticks as a u32! Value supplied was: {:?}", matching_string);
                    panic!("{}",e.to_string());
                },
            }
        }
    }
    if matched_options.opt_present("runs") {
        if let Some(matching_string) = matched_options.opt_str("runs") {
            match matching_string.parse::<u32>() {
                Ok(uint) => {
                    if uint != 0 {
                        params.runs = uint
                    } else {
                        println!("Runs must be greater than 0!");
                    }
                },
                Err(e) => {
                    println!("Could not parse runs as a u32! Value supplied was: {:?}", matching_string);
                    panic!("{}",e.to_string());
                },
            }
        }
    }
    if matched_options.opt_present("auto-analysis") {

    }
    if matched_options.opt_present("regen-config-file") {
        println!("Regenerating config file from defaults!");
        common::setup_config_file(true);
        std::process::exit(0);
    }
    if matched_options.opt_present("interactive") && !matched_options.opt_present("auto-analysis") {
        run_interactive(params);
    }
}

fn print_version() {
    println!("factorio_rust {}",FACTORIO_BENCHMARK_HELPER_VERSION);
    std::process::exit(0);
}

fn run_interactive(params: &mut BenchmarkParams) {
    println!("Selected interactive mode");
    if params.match_pattern == "" || params.match_pattern == "*" {
        params.match_pattern = "".to_string();
        println!("Enter a map pattern to match (implied leading and trailing wildcard)... [\"\"]");
    } else {
        println!("Pattern supplied from arguments... {:?}", params.match_pattern);
    }
    if params.ticks == 0 {
        println!("Enter the number of ticks to test per run... [1000]");
        params.ticks = 1000;
    } else {
        println!("Ticks supplied from arguments... {}", params.ticks);
    }
    if params.runs == 0 {
        println!("Enter the number of times to benchmark each map... [1]");
        params.runs = 1;
    } else {
        println!("Runs supplied from arguments... {}", params.runs);
    }
}

fn get_maps_and_append_to_params(save_directory: PathBuf, params: &mut BenchmarkParams) {
    assert!(save_directory.is_dir());
    if params.match_pattern.len() != 0 {
        params.match_pattern.push_str("*");
    }
    let combined_pattern = &format!("{}*{}",save_directory.to_string_lossy(),params.match_pattern);
    for item in glob::glob(combined_pattern).unwrap().filter_map(Result::ok) {
        if item.is_file() {
            match item.extension() {
                Some(extension) => match extension.to_str() {
                    Some("zip") => params.maps.push(item),
                    _ => (),
                },
                _ => (),
            }

        }
    }
}
