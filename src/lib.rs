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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_run_stdout() {
        let args = Args::try_parse_from(["git-repostats", "-s"]).expect("Failed to parse args");
        let res = run(args);
        assert!(res.is_ok());
    }

    #[test]
    fn test_run_file_output_json() {
        let temp_output = std::env::temp_dir().join("test_repostats_output.json");
        let args = Args::try_parse_from([
            "git-repostats",
            "-s",
            "--format",
            "json",
            "--output",
            temp_output.to_str().unwrap(),
        ])
        .expect("Failed to parse args");

        let res = run(args);
        assert!(res.is_ok());

        assert!(temp_output.exists());
        let content = fs::read_to_string(&temp_output).expect("Failed to read output file");
        assert!(content.contains("git-repostats"));

        let _ = fs::remove_file(temp_output);
    }

    #[test]
    fn test_run_invalid_branch() {
        let args = Args::try_parse_from([
            "git-repostats",
            "-s",
            "--branch",
            "non_existent_branch_xyz123",
        ])
        .expect("Failed to parse args");

        let res = run(args);
        assert!(res.is_err());
    }
}
