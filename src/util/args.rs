use getopts::Matches;

pub struct UserSuppliedArgs {
    pub benchmark_set_name: Option<String>,
    pub ticks: Option<u32>,
    pub runs: Option<u32>,
    pub pattern: Option<String>,
    pub help_target: Option<String>,
    pub overwrite_existing_procedure: bool,
    pub google_drive_folder: Option<String>,
}


impl Default for UserSuppliedArgs {
    fn default() -> UserSuppliedArgs {
        UserSuppliedArgs {
            benchmark_set_name: None,
            ticks: None,
            runs: None,
            pattern: None,
            help_target: None,
            overwrite_existing_procedure: false,
            google_drive_folder: None,
        }
    }
}

pub fn add_options(options: &mut getopts::Options) {
    options.parsing_style(getopts::ParsingStyle::FloatingFrees);
    options.optflag(
        "h",
        "help",
        "Prints this general help",
    );
    options.optflag("v", "version", "Print program version, then exits");
    options.optflag(
        "",
        "list",
        "Lists available benchmark/meta sets",
    );
    options.optflag("", "interactive", "Runs program interactively");
    options.optflag("", "overwrite", "Overwrite existing procedure if NAME supplied already exists.");
    options.opt(
        "",
        "pattern",
        "Limit benchmarks to maps that match PATTERN",
        "PATTERN",
        getopts::HasArg::Yes,
        getopts::Occur::Optional,
    );
    options.optopt(
        "",
        "ticks",
        "Runs benchmarks for TICKS duration per run",
        "TICKS",
    );
    options.optopt(
        "",
        "runs",
        "How many times should each map be benchmarked?",
        "TIMES",
    );
    options.opt(
        "",
        "create-benchmark-procedure",
        "Create a benchmark procedure named NAME",
        "NAME",
        getopts::HasArg::Yes,
        getopts::Occur::Optional,
    );
    options.opt(
        "",
        "google-drive-folder",
        "A link to a publicly shared folder that contains the maps of the benchmark set you are creating.",
        "LINK",
        getopts::HasArg::Yes,
        getopts::Occur::Optional,
    );
    options.optopt(
        "",
        "commit",
        "Commits a benchmark to the master json file, staging it for upload to the git repository.",
        "NAME",
    );
    options.optopt(
        "",
        "benchmark",
        "Runs a benchmark from local/master json files",
        "NAME",
    );
}


pub fn fetch_user_supplied_optargs(options: &Matches, user_args: &mut UserSuppliedArgs) {
    if let Ok(new_set_name) = options.opt_get::<String>("create-benchmark-procedure") {
        user_args.benchmark_set_name = new_set_name;
    }
    if let Ok(ticks) = options.opt_get::<u32>("ticks") {
        user_args.ticks = ticks;
    }
    if let Ok(runs) = options.opt_get::<u32>("runs") {
        user_args.runs = runs;
    }
    if let Ok(pattern) = options.opt_get::<String>("pattern") {
        user_args.pattern = pattern;
    }
    if let Ok(help_target) = options.opt_get::<String>("help") {
        user_args.help_target = help_target;
    }
    if options.opt_present("overwrite") {
        user_args.overwrite_existing_procedure = true;
    }
    if let Ok(drive_url) = options.opt_get::<String>("google-drive-folder") {
        user_args.google_drive_folder = drive_url;
    }
    if let Ok(commit_name) = options.opt_get::<String>("commit") {
        if user_args.benchmark_set_name.is_none() {
            user_args.benchmark_set_name = commit_name;
        }
    }
    if let Ok(bench_name) = options.opt_get::<String>("benchmark") {
        if user_args.benchmark_set_name.is_none() {
            user_args.benchmark_set_name = bench_name;
        }
    }
}
