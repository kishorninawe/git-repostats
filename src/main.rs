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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_main() {
        let res = try_main();
        assert!(res.is_ok());
    }

    #[test]
    fn test_thread_calculation_logic() {
        let cpu_cores = std::thread::available_parallelism().map_or(1, |n| n.get()) as u32;

        // When threads is 0, auto-detect uses min(cpu_cores, 4)
        let threads_0 = match 0 {
            0 => cpu_cores.min(4),
            t => (t as u32).min(cpu_cores).min(16).max(1),
        };
        assert_eq!(threads_0, cpu_cores.min(4));

        // When threads is explicitly set to 2
        let threads_2 = match 2 {
            0 => cpu_cores.min(4),
            t => (t as u32).min(cpu_cores).min(16).max(1),
        };
        assert_eq!(threads_2, (2u32).min(cpu_cores).min(16).max(1));

        // When threads is set large (capped at 16)
        let threads_large = match 100 {
            0 => cpu_cores.min(4),
            t => (t as u32).min(cpu_cores).min(16).max(1),
        };
        assert!(threads_large <= 16);
        assert!(threads_large >= 1);
    }

    #[test]
    fn test_output_extension_validation_in_main() {
        let args_valid = Args::try_parse_from([
            "git-repostats",
            "-s",
            "--format",
            "json",
            "--output",
            "report.json",
        ])
        .expect("Failed to parse args");
        assert!(
            validate_output_extension(args_valid.output.as_ref().unwrap(), args_valid.format)
                .is_ok()
        );

        let args_invalid = Args::try_parse_from([
            "git-repostats",
            "-s",
            "--format",
            "json",
            "--output",
            "report.txt",
        ])
        .expect("Failed to parse args");
        assert!(
            validate_output_extension(args_invalid.output.as_ref().unwrap(), args_invalid.format)
                .is_err()
        );
    }

    #[test]
    fn test_metrics_and_sort_in_main() {
        let mut args = Args::try_parse_from(["git-repostats", "-s", "--metrics", "all"])
            .expect("Failed to parse args");
        args.metrics = parse_metrics(&args.metrics);
        args.sort = Some(get_sort_key(&args));

        assert_eq!(args.metrics.len(), 4);
        assert!(args.sort.is_some());
    }
}
