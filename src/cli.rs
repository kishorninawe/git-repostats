use clap::{Parser, ValueEnum};
use serde::Serialize;
use std::{fmt, path::PathBuf};

use crate::utils::{parse_limit, validate_git_dir, validate_ignore_revs_file};

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
        default_values_t =vec![Metrics::Commits],
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
        short = 's',
        long,
        help = "Suppress progress output.",
        long_help = "Suppress progress bars and progress messages while processing repositories."
    )]
    pub silent_progress: bool,

    #[arg(
    long,
    value_enum,
    default_value_t = OutputFormat::Table,
    help = "Set the output format (table, json, csv, yaml, markdown).",
    long_help = "Set the output format for repository statistics.\nSupported formats: table, json, csv, yaml and markdown.\nDefaults to table."
)]
    pub format: OutputFormat,

    #[arg(
        long,
        value_name = "FILE",
        help = "Write output to a file instead of stdout.",
        long_help = "Write repository statistics to the specified file instead of stdout.\nThe output format is determined by --format.\nIf the file already exists, it will be overwritten."
    )]
    pub output: Option<PathBuf>,

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
        help = "Include only merge commits.",
        long_help = "Include only merge commits (commits with two or more parents) for commits, files, and history metrics. Equivalent to Git's --min-parents=2."
    )]
    pub merges: bool,

    #[arg(
        long,
        help = "Include only non-merge commits.",
        long_help = "Include only non-merge commits (commits with at most one parent) for commits, files, and history metrics. Equivalent to Git's --max-parents=1.",
        conflicts_with = "merges"
    )]
    pub no_merges: bool,

    #[arg(
        long,
        value_name = "REV",
        help = "Ignore the specified revision for current metrics.",
        long_help = "Ignore the specified revision for current metrics.\nEquivalent to Git blame's --ignore-rev option."
    )]
    pub ignore_rev: Vec<String>,

    #[arg(
        long,
        value_name = "FILE",
        value_parser = validate_ignore_revs_file,
        help = "Ignore revisions listed in a file for current metrics.",
        long_help = "Ignore revisions listed in the specified file for current metrics.\nEquivalent to Git blame's --ignore-revs-file option."
    )]
    pub ignore_revs_file: Vec<PathBuf>,

    #[arg(
        long,
        default_value_t = 0,
        help = "Number of parallel git operations. 0 = auto (min(CPU cores, 4))."
    )]
    pub threads: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum, Hash)]
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
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

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum ShowAuthor {
    /// Author name only.
    Name,

    /// Author email only.
    Email,

    /// Author name and email.
    Both,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize)]
pub enum Limit {
    /// No limit; show all authors.
    All,

    /// Limit to a specific number of authors.
    Count(usize),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    /// Output results as formatted table.
    Table,

    /// Output results as JSON.
    Json,

    /// Output results as CSV.
    Csv,

    /// Output results as YAML.
    Yaml,

    /// Output results as Markdown.
    Markdown,
}

impl fmt::Display for SortKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = format!("{:?}", self);

        let first = &s[..1].to_uppercase();
        let rest = &s[1..].to_lowercase();

        write!(f, "{first}{rest}")
    }
}

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::All => write!(f, "All"),
            Self::Count(n) => write!(f, "{n}"),
        }
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Table => "Table",
            Self::Json => "JSON",
            Self::Csv => "CSV",
            Self::Yaml => "YAML",
            Self::Markdown => "Markdown",
        };

        write!(f, "{name}")
    }
}

impl OutputFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Table => "txt",
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Yaml => "yaml",
            Self::Markdown => "md",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_clap_command_validity() {
        // Verifies clap configuration has no internal conflicts or misconfigurations.
        Args::command().debug_assert();
    }

    #[test]
    fn test_default_args() {
        let args = Args::try_parse_from(["git-repostats"]).expect("Failed to parse default args");

        assert_eq!(args.branch, "HEAD");
        assert_eq!(args.metrics, vec![Metrics::Commits]);
        assert_eq!(args.sort, None);
        assert!(!args.reverse);
        assert_eq!(args.limit, Limit::Count(10));
        assert!(!args.silent_progress);
        assert_eq!(args.format, OutputFormat::Table);
        assert_eq!(args.output, None);
        assert_eq!(args.since, None);
        assert_eq!(args.until, None);
        assert_eq!(args.show, ShowAuthor::Both);
        assert!(!args.detect_moves);
        assert!(!args.detect_copies);
        assert!(!args.ignore_whitespace);
        assert!(!args.merges);
        assert!(!args.no_merges);
        assert_eq!(args.threads, 0);
        assert!(args.ignore_rev.is_empty());
        assert!(args.ignore_revs_file.is_empty());
        assert!(!args.gitdir.is_empty());
    }

    #[test]
    fn test_gitdir_arg() {
        let args = Args::try_parse_from(["git-repostats", "."]).expect("Failed to parse gitdir");
        assert_eq!(args.gitdir.len(), 1);

        let err = Args::try_parse_from(["git-repostats", "this_path_should_not_exist"]);
        assert!(err.is_err());
    }

    #[test]
    fn test_branch_arg() {
        let args = Args::try_parse_from(["git-repostats", "--branch", "feature-branch"])
            .expect("Failed to parse branch");
        assert_eq!(args.branch, "feature-branch");
    }

    #[test]
    fn test_metrics_arg() {
        let args = Args::try_parse_from([
            "git-repostats",
            "--metrics",
            "commits",
            "files",
            "history",
            "current",
            "all",
        ])
        .expect("Failed to parse metrics");

        assert_eq!(
            args.metrics,
            vec![
                Metrics::Commits,
                Metrics::Files,
                Metrics::History,
                Metrics::Current,
                Metrics::All
            ]
        );

        let err = Args::try_parse_from(["git-repostats", "--metrics", "invalid_metric"]);
        assert!(err.is_err());
    }

    #[test]
    fn test_sort_arg() {
        let sort_keys = [
            ("commits", SortKey::Commits),
            ("files", SortKey::Files),
            ("insertions", SortKey::Insertions),
            ("deletions", SortKey::Deletions),
            ("net", SortKey::Net),
            ("churn", SortKey::Churn),
            ("surviving", SortKey::Surviving),
        ];

        for (arg_val, expected_key) in sort_keys {
            let args = Args::try_parse_from(["git-repostats", "--sort", arg_val])
                .unwrap_or_else(|_| panic!("Failed to parse --sort {arg_val}"));
            assert_eq!(args.sort, Some(expected_key));
        }

        let err = Args::try_parse_from(["git-repostats", "--sort", "unknown_key"]);
        assert!(err.is_err());
    }

    #[test]
    fn test_reverse_flag() {
        let args = Args::try_parse_from(["git-repostats", "--reverse"])
            .expect("Failed to parse --reverse");
        assert!(args.reverse);
    }

    #[test]
    fn test_limit_arg() {
        let args_all = Args::try_parse_from(["git-repostats", "--limit", "all"])
            .expect("Failed to parse --limit all");
        assert_eq!(args_all.limit, Limit::All);

        let args_count =
            Args::try_parse_from(["git-repostats", "-l", "25"]).expect("Failed to parse -l 25");
        assert_eq!(args_count.limit, Limit::Count(25));

        assert!(Args::try_parse_from(["git-repostats", "--limit", "0"]).is_err());
        assert!(Args::try_parse_from(["git-repostats", "--limit", "-10"]).is_err());
        assert!(Args::try_parse_from(["git-repostats", "--limit", "invalid"]).is_err());
    }

    #[test]
    fn test_silent_progress_flag() {
        let args_short = Args::try_parse_from(["git-repostats", "-s"]).expect("Failed to parse -s");
        assert!(args_short.silent_progress);

        let args_long = Args::try_parse_from(["git-repostats", "--silent-progress"])
            .expect("Failed to parse --silent-progress");
        assert!(args_long.silent_progress);
    }

    #[test]
    fn test_format_arg() {
        let formats = [
            ("table", OutputFormat::Table),
            ("json", OutputFormat::Json),
            ("csv", OutputFormat::Csv),
            ("yaml", OutputFormat::Yaml),
            ("markdown", OutputFormat::Markdown),
        ];

        for (name, expected_fmt) in formats {
            let args = Args::try_parse_from(["git-repostats", "--format", name])
                .unwrap_or_else(|_| panic!("Failed to parse --format {name}"));
            assert_eq!(args.format, expected_fmt);
        }

        let err = Args::try_parse_from(["git-repostats", "--format", "xml"]);
        assert!(err.is_err());
    }

    #[test]
    fn test_output_arg() {
        let args = Args::try_parse_from(["git-repostats", "--output", "stats.json"])
            .expect("Failed to parse --output");
        assert_eq!(args.output, Some(PathBuf::from("stats.json")));
    }

    #[test]
    fn test_since_until_args() {
        let args = Args::try_parse_from([
            "git-repostats",
            "--since",
            "2024-01-01",
            "--until",
            "2024-12-31",
        ])
        .expect("Failed to parse --since and --until");

        assert_eq!(args.since, Some("2024-01-01".to_string()));
        assert_eq!(args.until, Some("2024-12-31".to_string()));
    }

    #[test]
    fn test_show_author_arg() {
        let show_options = [
            ("name", ShowAuthor::Name),
            ("email", ShowAuthor::Email),
            ("both", ShowAuthor::Both),
        ];

        for (val, expected_show) in show_options {
            let args = Args::try_parse_from(["git-repostats", "--show", val])
                .unwrap_or_else(|_| panic!("Failed to parse --show {val}"));
            assert_eq!(args.show, expected_show);
        }

        let err = Args::try_parse_from(["git-repostats", "--show", "other"]);
        assert!(err.is_err());
    }

    #[test]
    fn test_detection_and_whitespace_flags() {
        let args = Args::try_parse_from(["git-repostats", "-M", "-C", "-w"])
            .expect("Failed to parse short flags");
        assert!(args.detect_moves);
        assert!(args.detect_copies);
        assert!(args.ignore_whitespace);

        let args_long = Args::try_parse_from([
            "git-repostats",
            "--detect-moves",
            "--detect-copies",
            "--ignore-whitespace",
        ])
        .expect("Failed to parse long flags");
        assert!(args_long.detect_moves);
        assert!(args_long.detect_copies);
        assert!(args_long.ignore_whitespace);
    }

    #[test]
    fn test_merges_and_no_merges() {
        let merges =
            Args::try_parse_from(["git-repostats", "--merges"]).expect("Failed to parse --merges");
        assert!(merges.merges);
        assert!(!merges.no_merges);

        let no_merges = Args::try_parse_from(["git-repostats", "--no-merges"])
            .expect("Failed to parse --no-merges");
        assert!(!no_merges.merges);
        assert!(no_merges.no_merges);

        // Conflicting flags
        let conflict = Args::try_parse_from(["git-repostats", "--merges", "--no-merges"]);
        assert!(conflict.is_err());
    }

    #[test]
    fn test_ignore_rev_args() {
        let args = Args::try_parse_from([
            "git-repostats",
            "--ignore-rev",
            "abc1234",
            "--ignore-rev",
            "def5678",
        ])
        .expect("Failed to parse --ignore-rev");
        assert_eq!(args.ignore_rev, vec!["abc1234", "def5678"]);
    }

    #[test]
    fn test_ignore_revs_file_arg() {
        let args = Args::try_parse_from(["git-repostats", "--ignore-revs-file", "Cargo.toml"])
            .expect("Failed to parse valid ignore-revs-file");
        assert_eq!(args.ignore_revs_file.len(), 1);

        let err = Args::try_parse_from([
            "git-repostats",
            "--ignore-revs-file",
            "nonexistent_revs.txt",
        ]);
        assert!(err.is_err());
    }

    #[test]
    fn test_threads_arg() {
        let args = Args::try_parse_from(["git-repostats", "--threads", "4"])
            .expect("Failed to parse --threads");
        assert_eq!(args.threads, 4);
    }

    #[test]
    fn test_output_format_display() {
        assert_eq!(OutputFormat::Table.to_string(), "Table");
        assert_eq!(OutputFormat::Json.to_string(), "JSON");
        assert_eq!(OutputFormat::Csv.to_string(), "CSV");
        assert_eq!(OutputFormat::Yaml.to_string(), "YAML");
        assert_eq!(OutputFormat::Markdown.to_string(), "Markdown");
    }

    #[test]
    fn test_output_format_extension() {
        assert_eq!(OutputFormat::Table.extension(), "txt");
        assert_eq!(OutputFormat::Json.extension(), "json");
        assert_eq!(OutputFormat::Csv.extension(), "csv");
        assert_eq!(OutputFormat::Yaml.extension(), "yaml");
        assert_eq!(OutputFormat::Markdown.extension(), "md");
    }

    #[test]
    fn test_sort_key_display() {
        assert_eq!(SortKey::Commits.to_string(), "Commits");
        assert_eq!(SortKey::Files.to_string(), "Files");
        assert_eq!(SortKey::Insertions.to_string(), "Insertions");
        assert_eq!(SortKey::Deletions.to_string(), "Deletions");
        assert_eq!(SortKey::Net.to_string(), "Net");
        assert_eq!(SortKey::Churn.to_string(), "Churn");
        assert_eq!(SortKey::Surviving.to_string(), "Surviving");
    }

    #[test]
    fn test_limit_display() {
        assert_eq!(Limit::All.to_string(), "All");
        assert_eq!(Limit::Count(10).to_string(), "10");
        assert_eq!(Limit::Count(0).to_string(), "0");
    }
}
