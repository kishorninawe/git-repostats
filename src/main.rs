use clap::Parser;

use git_repostats::{
    cli::Args,
    error::AppError,
    run,
    utils::{get_sort_key, parse_metrics, validate_output_extension},
};

fn main() {
    if let Err(err) = try_main() {
        eprintln!("Error: {err}");
    }
}

fn try_main() -> Result<(), AppError> {
    let mut args = Args::parse();

    if let Some(path) = &args.output {
        validate_output_extension(path, args.format)?;
    }

    args.metrics = parse_metrics(&args.metrics);
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

    run(args)?;

    Ok(())
}
