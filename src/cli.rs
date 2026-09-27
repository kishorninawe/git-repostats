use clap::{Parser, ValueEnum};
use std::{fmt, path::PathBuf};

use crate::utils::{parse_limit, validate_git_dir};

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Args {
    #[arg(
        default_value = ".", 
        num_args = 1..,
        value_parser = validate_git_dir,
        help = "Git repository directory or directories to analyze."
    )]
    pub gitdir: Vec<PathBuf>,

    #[arg(
        long,
        default_value = "HEAD",
        help = "Branch or tag up to which to check."
    )]
    pub branch: String,

    #[arg(
        long,
        value_enum,
        num_args = 1..,
        default_values_t =vec![ Metrics::Commits],
        help = "Select which metrics to compute."
    )]
    pub metrics: Vec<Metrics>,

    #[arg(
        long,
        value_enum,
        help = "Sort output by the specified metric.",
        long_help = "Sort output by the specified metric.\nSorting is descending by default."
    )]
    pub sort: Option<SortKey>,

    #[arg(long, help = "Reverse the sort order from descending to ascending.")]
    pub reverse: bool,

    #[arg(
        short,
        long,
        default_value = "10",
        value_parser = parse_limit,
        value_name = "N|all",
        help = "Limit number of authors displayed in the output. Use `all` for no limit."
    )]
    pub limit: Limit,

    #[arg(
        long,
        value_name = "DATE",
        help = "Include only commits after the specified date.",
        long_help = "Include only commits after the specified date.\nAccepts any date format supported by Git."
    )]
    pub since: Option<String>,

    #[arg(
        long,
        value_name = "DATE",
        help = "Include only commits before the specified date.",
        long_help = "Include only commits before the specified date.\nAccepts any date format supported by Git."
    )]
    pub until: Option<String>,

    #[arg(
        long,
        value_enum,
        default_value_t = ShowAuthor::Both,
        help = "Author information to show."
    )]
    pub show: ShowAuthor,

    #[arg(
        short = 'M',
        long,
        help = "Detect moved lines when computing metrics.",
        long_help = "Detect moved lines when computing metrics.\nImproves accuracy for refactors at the cost of performance."
    )]
    pub detect_moves: bool,

    #[arg(
        short = 'C',
        long,
        help = "Detect copied lines across files when computing metrics.",
        long_help = "Detect copied lines across files when computing metrics."
    )]
    pub detect_copies: bool,

    #[arg(
        short = 'w',
        long,
        help = "Ignore whitespace-only changes when computing current metrics."
    )]
    pub ignore_whitespace: bool,

    #[arg(
        long,
        default_value_t = 0,
        help = "Number of parallel git operations. 0 = auto (min(CPU cores, 4))."
    )]
    pub threads: u32,
}

#[derive(Copy, Clone, Debug, PartialEq, ValueEnum, Hash, Eq)]
pub enum Metrics {
    /// Commit count per author.
    Commits,

    /// Number of unique files modified by the author.
    Files,

    /// Commit count, files, insertions, deletions, net change, and churn.
    History,

    /// Commit count, files, and surviving lines in the current code state.
    Current,

    /// Combine history and current metrics.
    All,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum SortKey {
    /// Number of commits authored by the user.
    Commits,

    /// Number of unique files modified by the user across all commits.
    Files,

    /// Total number of lines added by the user.
    Insertions,

    /// Total number of lines removed by the user.
    Deletions,

    /// Net line change introduced by the user (insertions minus deletions).
    Net,

    /// Total lines of code modified by the user (insertions plus deletions).
    Churn,

    /// Number of lines currently present in the codebase that are attributed to the user.
    Surviving,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ShowAuthor {
    /// Author name only.
    Name,

    /// Author email only.
    Email,

    /// Author name and email.
    Both,
}

#[derive(Debug, Clone)]
pub enum Limit {
    /// No limit; show all authors.
    All,

    /// Limit to a specific number of authors.
    Count(usize),
}

impl fmt::Display for SortKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{:?}", self);

        let first = &s[..1].to_uppercase();
        let rest = &s[1..].to_lowercase();

        write!(f, "{first}{rest}")
    }
}
