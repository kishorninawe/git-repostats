pub mod cli;
pub mod git;
pub mod process;
pub mod render;
pub mod stats;
pub mod utils;

use crate::cli::Args;
use crate::process::process_repo;
use crate::render::render_all;
use crate::stats::AuthorStats;
use std::collections::HashMap;
use std::io;

pub fn run(args: Args) -> io::Result<()> {
    let mut result: HashMap<String, Vec<(String, AuthorStats)>> = HashMap::new();

    for dir in &args.gitdir {
        let rows = process_repo(dir, &args);
        result.insert(dir.clone(), rows);
    }

    render_all(&args, result);

    Ok(())
}
