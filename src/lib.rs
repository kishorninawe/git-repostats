pub mod cli;
pub mod error;
pub mod git;
pub mod process;
pub mod render;
pub mod stats;
pub mod utils;

use std::collections::HashMap;

use crate::{cli::Args, error::AppError, process::process_repo, render::render_all};

pub fn run(args: Args) -> Result<(), AppError> {
    let mut result = HashMap::with_capacity(args.gitdir.len());

    for dir in &args.gitdir {
        let (repo_name, rows) = process_repo(dir, &args)?;
        result.insert(repo_name, rows);
    }

    render_all(&args, result);

    Ok(())
}
