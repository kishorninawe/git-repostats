use clap::Parser;
use git_repostats::cli::Args;
use git_repostats::run;
use git_repostats::utils::{get_sort_key, parse_metrics};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = Args::parse();
    args.metrics = parse_metrics(args.metrics);
    args.sort = Some(get_sort_key(&args));

    let cpu_cores = std::thread::available_parallelism().map_or(1, |n| n.get()) as u32;
    args.threads = match args.threads {
        0 => cpu_cores.min(4),
        t => (t as u32).min(cpu_cores).min(16).max(1),
    };

    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads as usize)
        .build_global()
        .expect("Failed to build thread pool");

    let _ = run(args);
    Ok(())
}
