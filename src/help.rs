extern crate textwrap;

use textwrap::{fill,indent,wrap};

pub fn print_help(help_arg: &str) {
    println!("Usage: factorio_rust [OPTIONS ...]");
    match help_arg {
        "help" => print_help_arg_help(),
        "version" => println!("No additional help available for {}",help_arg),
        "dump" => print_help_arg_dump(),
        "pattern" => print_help_arg_pattern(),
        "interactive" => print_help_arg_interactive(),
        "auto-analysis" => print_help_arg_auto_analysis(),
        _ => println!("No help available for option: {}", help_arg),
    }

}


fn print_formatted_arg(arg_opt: &str, arg_opt_explain: &str) {
    println!("\t{}",arg_opt);
    println!("{}", indent(&fill(arg_opt_explain,72),"\t\t"));
}

fn print_help_arg_help(){
    print_formatted_arg("-h, --help [OPTION]", "Print additional help for OPTION");
    print_formatted_arg("-v, --version", "Print the version");
    print_formatted_arg("-d, --dump OPTION", "Dump data to csv file. dump OPTIONS: min, avg, all");
    print_formatted_arg("-p, --pattern PATTERN", "Restrict benchmarked maps to those that match PATTERN.\nIf not specified, all maps in the save directory are benchmarked.");
    print_formatted_arg("-i, --interactive", "Run program in interactive mode");
    print_formatted_arg("-a, --auto-analysis", "Run program in auto-analysis mode");
    print_formatted_arg("-t, --ticks TICKS", "Run benchmarks for TICKS duration");
    print_formatted_arg("-r, --runs RUNS", "Perform benchmarks for each map selected RUNS many times");
}

fn print_help_arg_dump() {
    print_formatted_arg("-d, --dump OPTION", "Dump data to csv file. OPTIONS: min, avg, all");
    println!("\tAllowed OPTIONS: min, avg, all");
    print_formatted_arg("min:","Returns the tick that took the least duration for this tick, among all benchmark runs.");
    print_formatted_arg("","This effectively returns an idealized perfect run, the idea is making the assumption that results slower than the fastest one were caused by OS interference.");
    print_formatted_arg("","This metric is recommended for comparing designs, but not hardware or software changes.");
    print_formatted_arg("avg:","Returns the average time taken per tick.");
    print_formatted_arg("all:","Returns all data without aggregation.");

}

fn print_help_arg_pattern() {
    print_formatted_arg("-p, --pattern PATTERN", "Restrict benchmarked maps to those that match PATTERN");

}

fn print_help_arg_interactive() {

}

fn print_help_arg_auto_analysis() {

}
