pub mod cli;
pub mod error;
pub mod format;
pub mod git;
pub mod process;
pub mod stats;
pub mod utils;

use std::{
    collections::HashMap,
    fs,
    io::{self, BufWriter, Write},
};

use crate::{
    cli::Args, error::AppError, format::format_repo, process::process_repo,
    utils::output_path_for_repo,
};

pub fn run(args: Args) -> Result<(), AppError> {
    let mut result = HashMap::with_capacity(args.gitdir.len());

    for dir in &args.gitdir {
        let (repo_name, rows) = process_repo(dir, &args)?;
        result.insert(repo_name, rows);
    }

    let multiple_repos = result.len() > 1;

    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    for (repo_name, rows) in result {
        let output = format_repo(&args, &repo_name, rows)?;

        if let Some(path) = &args.output {
            let output_path = if multiple_repos {
                output_path_for_repo(path, &repo_name)
            } else {
                path.to_path_buf()
            };

            fs::write(output_path, output)?;
        } else {
            writeln!(out, "{output}")?;
        }
    }

    if args.output.is_none() {
        out.flush()?;
    }

    Ok(())
}
