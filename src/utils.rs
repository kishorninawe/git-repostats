use console::style;
use num_format::{Locale, ToFormattedString};
use std::{
    collections::HashSet,
    fs::File,
    io,
    path::{Path, PathBuf, absolute},
    process::Command,
};

use crate::{
    cli::{Args, Limit, Metrics, OutputFormat, SortKey as Sort},
    error::AppError,
    stats::AuthorStats,
};

/// Validate that a path:
/// - exists
/// - is a directory
/// - is a git repository
pub fn validate_git_dir(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);

    // 1. Canonicalize the path to resolve symlinks and get an absolute path
    let path = path
        .canonicalize()
        .map_err(|e| format!("invalid git directory '{}': {}", path.display(), e))?;

    // 2. Exists
    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }

    // 3. Is directory
    if !path.is_dir() {
        return Err(format!("path is not a directory: {}", path.display()));
    }

    // 4. Is git repository (.git directory or file)
    let git_dir = path.join(".git");
    if !git_dir.exists() {
        return Err(format!(
            "not a git repository (missing .git): {}",
            path.display()
        ));
    }

    // 5. Is valid git repository
    check_output(&[
        "git",
        "-C",
        path.to_string_lossy().as_ref(),
        "rev-parse",
        "--git-dir",
    ])
    .map_err(|e| {
        format!(
            "not a valid git repository: {} (Error: {})",
            path.display(),
            e
        )
    })?;

    Ok(path)
}

pub fn validate_ignore_revs_file(file: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(file);

    let path = absolute(&path).map_err(|e| format!("cannot resolve {}: {e}", path.display()))?;

    if !path.is_file() {
        return Err(format!("not a file: {}", path.display()));
    }

    File::open(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    Ok(path)
}

pub fn validate_output_extension(path: &Path, format: OutputFormat) -> Result<(), AppError> {
    let expected = format.extension();

    let actual = path
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| {
            AppError::InvalidOutputExtension(format!(
                "output filename must have a .{expected} extension"
            ))
        })?;

    if !actual.eq_ignore_ascii_case(expected) {
        return Err(AppError::InvalidOutputExtension(format!(
            "format {} requires a .{expected} extension, got .{actual}",
            format
        )));
    }

    Ok(())
}

pub fn parse_metrics(metrics: &[Metrics]) -> Vec<Metrics> {
    let unique: HashSet<_> = metrics.iter().copied().collect();

    if unique.contains(&Metrics::All) {
        return vec![
            Metrics::Commits,
            Metrics::Files,
            Metrics::History,
            Metrics::Current,
        ];
    }

    unique.into_iter().collect()
}

pub fn parse_limit(s: &str) -> Result<Limit, String> {
    match s {
        "all" => Ok(Limit::All),
        _ => {
            let n: usize = s
                .parse()
                .map_err(|_| "limit must be a positive number or `all`")?;
            if n == 0 {
                Err("limit must be greater than 0".into())
            } else {
                Ok(Limit::Count(n))
            }
        }
    }
}

pub fn push_opt_arg(cmd: &mut Command, flag: &'static str, value: Option<&str>) {
    if let Some(v) = value {
        cmd.arg(flag).arg(v);
    }
}

pub fn push_flag(cmd: &mut Command, flag: &'static str, enabled: bool) {
    if enabled {
        cmd.arg(flag);
    }
}

pub fn check_output(argv: &[&str]) -> io::Result<String> {
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "empty command"))?;

    let output = Command::new(program).args(args).output()?;

    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn is_sort_valid(sort: Sort, args: &Args) -> bool {
    args.metrics.iter().any(|m| match m {
        Metrics::Commits => matches!(sort, Sort::Commits),
        Metrics::Files => matches!(sort, Sort::Files),
        Metrics::History => matches!(
            sort,
            Sort::Insertions | Sort::Deletions | Sort::Net | Sort::Churn
        ),
        Metrics::Current => matches!(sort, Sort::Surviving),
        Metrics::All => matches!(
            sort,
            Sort::Commits
                | Sort::Files
                | Sort::Insertions
                | Sort::Deletions
                | Sort::Net
                | Sort::Churn
                | Sort::Surviving
        ),
    })
}

pub fn get_sort_key(args: &Args) -> Sort {
    // user provided sort
    if let Some(s) = args.sort {
        if is_sort_valid(s, args) {
            return s;
        } else {
            eprintln!(
                "{} sort key {:?} is not valid for the selected metrics. Using default sort key.",
                style("Warning:").yellow().bold(),
                s
            );
        }
    }

    // fallback based on selected metrics
    for m in &args.metrics {
        let default = match m {
            Metrics::Commits => Some(Sort::Commits),
            Metrics::Files => Some(Sort::Files),
            Metrics::History => Some(Sort::Churn),
            Metrics::Current => Some(Sort::Surviving),
            Metrics::All => Some(Sort::Surviving),
        };

        if let Some(sort) = default {
            return sort;
        }
    }

    // final safe fallback (should never really hit)
    Sort::Commits
}

fn opt_usize(v: Option<usize>) -> isize {
    v.map(|n| n as isize).unwrap_or(0)
}

pub fn sort_value(stats: &AuthorStats, sort: Sort) -> isize {
    match sort {
        Sort::Commits => opt_usize(stats.commits),
        Sort::Files => opt_usize(stats.files),
        Sort::Insertions => opt_usize(stats.ins),
        Sort::Deletions => opt_usize(stats.del),
        Sort::Net => stats.net.unwrap_or(0),
        Sort::Churn => opt_usize(stats.churn),
        Sort::Surviving => opt_usize(stats.surviving),
    }
}

pub fn fmt_number<T: ToFormattedString>(n: &Option<T>) -> String {
    match n {
        Some(v) => v.to_formatted_string(&Locale::en),
        None => "".to_owned(),
    }
}

pub fn output_path_for_repo(path: &Path, repo_name: &str) -> PathBuf {
    let parent = path.parent().unwrap_or(Path::new(""));
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path.extension().map(|ext| ext.to_string_lossy());

    let filename = match extension {
        Some(ext) => format!("{stem}-{repo_name}.{ext}"),
        None => format!("{stem}-{repo_name}"),
    };

    parent.join(filename)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_validate_git_dir_valid() {
        let res = validate_git_dir(".");
        assert!(res.is_ok());
        let path = res.unwrap();
        assert!(path.is_dir());
        assert!(path.join(".git").exists());
    }

    #[test]
    fn test_validate_git_dir_nonexistent() {
        let res = validate_git_dir("non_existent_directory_for_tests");
        assert!(res.is_err());
    }

    #[test]
    fn test_validate_git_dir_not_a_directory() {
        let res = validate_git_dir("Cargo.toml");
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.contains("not a directory"));
    }

    #[test]
    fn test_validate_git_dir_not_a_git_repo() {
        let temp_dir = std::env::temp_dir().join("test_repostats_not_git_repo");
        let _ = std::fs::create_dir_all(&temp_dir);

        let res = validate_git_dir(temp_dir.to_str().unwrap());
        let _ = std::fs::remove_dir_all(&temp_dir);

        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.contains("not a git repository") || err.contains("missing .git"));
    }

    #[test]
    fn test_validate_ignore_revs_file_valid() {
        let res = validate_ignore_revs_file("Cargo.toml");
        assert!(res.is_ok());
        let path = res.unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn test_validate_ignore_revs_file_nonexistent() {
        let res = validate_ignore_revs_file("non_existent_file_for_tests.txt");
        assert!(res.is_err());
    }

    #[test]
    fn test_validate_ignore_revs_file_directory() {
        let res = validate_ignore_revs_file("src");
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert!(err.contains("not a file"));
    }

    #[test]
    fn test_validate_output_extension_valid() {
        let cases = [
            ("out.txt", OutputFormat::Table),
            ("out.json", OutputFormat::Json),
            ("out.csv", OutputFormat::Csv),
            ("out.yaml", OutputFormat::Yaml),
            ("out.md", OutputFormat::Markdown),
        ];

        for (filename, format) in cases {
            let res = validate_output_extension(Path::new(filename), format);
            assert!(
                res.is_ok(),
                "Expected Ok for {filename} with format {format:?}"
            );
        }
    }

    #[test]
    fn test_validate_output_extension_case_insensitive() {
        assert!(validate_output_extension(Path::new("out.JSON"), OutputFormat::Json).is_ok());
        assert!(validate_output_extension(Path::new("out.CSV"), OutputFormat::Csv).is_ok());
    }

    #[test]
    fn test_validate_output_extension_missing_extension() {
        let res = validate_output_extension(Path::new("out"), OutputFormat::Json);
        assert!(matches!(res, Err(AppError::InvalidOutputExtension(_))));
    }

    #[test]
    fn test_validate_output_extension_mismatched() {
        let res = validate_output_extension(Path::new("out.txt"), OutputFormat::Json);
        assert!(matches!(res, Err(AppError::InvalidOutputExtension(_))));
    }

    #[test]
    fn test_parse_metrics() {
        // Deduplication
        let parsed = parse_metrics(&[Metrics::Commits, Metrics::Commits, Metrics::Files]);
        assert_eq!(parsed.len(), 2);
        assert!(parsed.contains(&Metrics::Commits));
        assert!(parsed.contains(&Metrics::Files));

        // Metrics::All expansion
        let parsed_all = parse_metrics(&[Metrics::All]);
        assert_eq!(
            parsed_all,
            vec![
                Metrics::Commits,
                Metrics::Files,
                Metrics::History,
                Metrics::Current,
            ]
        );

        let parsed_with_all = parse_metrics(&[Metrics::Commits, Metrics::All]);
        assert_eq!(
            parsed_with_all,
            vec![
                Metrics::Commits,
                Metrics::Files,
                Metrics::History,
                Metrics::Current,
            ]
        );

        // Empty
        let empty = parse_metrics(&[]);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_parse_limit() {
        assert_eq!(parse_limit("all").unwrap(), Limit::All);
        assert_eq!(parse_limit("10").unwrap(), Limit::Count(10));
        assert_eq!(parse_limit("1").unwrap(), Limit::Count(1));

        assert!(parse_limit("0").is_err());
        assert!(parse_limit("-5").is_err());
        assert!(parse_limit("abc").is_err());
    }

    #[test]
    fn test_push_opt_arg() {
        let mut cmd = Command::new("git");
        push_opt_arg(&mut cmd, "--branch", Some("main"));
        let args: Vec<&str> = cmd.get_args().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(args, vec!["--branch", "main"]);

        let mut cmd_none = Command::new("git");
        push_opt_arg(&mut cmd_none, "--branch", None);
        assert_eq!(cmd_none.get_args().count(), 0);
    }

    #[test]
    fn test_push_flag() {
        let mut cmd_enabled = Command::new("git");
        push_flag(&mut cmd_enabled, "-M", true);
        let args: Vec<&str> = cmd_enabled
            .get_args()
            .map(|s| s.to_str().unwrap())
            .collect();
        assert_eq!(args, vec!["-M"]);

        let mut cmd_disabled = Command::new("git");
        push_flag(&mut cmd_disabled, "-M", false);
        assert_eq!(cmd_disabled.get_args().count(), 0);
    }

    #[test]
    fn test_check_output() {
        // Empty argv error
        let empty_res = check_output(&[]);
        assert!(empty_res.is_err());

        // Successful execution
        let ok_res = check_output(&["git", "--version"]);
        assert!(ok_res.is_ok());
        let stdout = ok_res.unwrap();
        assert!(stdout.contains("git version"));

        // Failing execution
        let fail_res = check_output(&["git", "definitely-not-a-valid-command-12345"]);
        assert!(fail_res.is_err());
    }

    #[test]
    fn test_get_sort_key_valid_user_sort() {
        let args =
            Args::try_parse_from(["git-repostats", "--metrics", "history", "--sort", "churn"])
                .expect("Failed to parse args");
        assert_eq!(get_sort_key(&args), Sort::Churn);

        let args = Args::try_parse_from([
            "git-repostats",
            "--metrics",
            "history",
            "--sort",
            "insertions",
        ])
        .expect("Failed to parse args");
        assert_eq!(get_sort_key(&args), Sort::Insertions);
    }

    #[test]
    fn test_get_sort_key_invalid_user_sort_fallback() {
        // Insertions is not valid when only Commits metric is selected; should fallback to Commits
        let args = Args::try_parse_from([
            "git-repostats",
            "--metrics",
            "commits",
            "--sort",
            "insertions",
        ])
        .expect("Failed to parse args");
        assert_eq!(get_sort_key(&args), Sort::Commits);
    }

    #[test]
    fn test_get_sort_key_defaults() {
        let defaults = [
            ("commits", Sort::Commits),
            ("files", Sort::Files),
            ("history", Sort::Churn),
            ("current", Sort::Surviving),
            ("all", Sort::Surviving),
        ];

        for (metric, expected_sort) in defaults {
            let args = Args::try_parse_from(["git-repostats", "--metrics", metric])
                .unwrap_or_else(|_| panic!("Failed to parse metric {metric}"));
            assert_eq!(get_sort_key(&args), expected_sort);
        }
    }

    #[test]
    fn test_sort_value() {
        let stats = AuthorStats {
            commits: Some(15),
            files: Some(7),
            surviving: Some(120),
            ins: Some(250),
            del: Some(50),
            net: Some(200),
            churn: Some(300),
        };

        assert_eq!(sort_value(&stats, Sort::Commits), 15);
        assert_eq!(sort_value(&stats, Sort::Files), 7);
        assert_eq!(sort_value(&stats, Sort::Surviving), 120);
        assert_eq!(sort_value(&stats, Sort::Insertions), 250);
        assert_eq!(sort_value(&stats, Sort::Deletions), 50);
        assert_eq!(sort_value(&stats, Sort::Net), 200);
        assert_eq!(sort_value(&stats, Sort::Churn), 300);

        // Negative net change
        let neg_stats = AuthorStats {
            net: Some(-45),
            ..AuthorStats::default()
        };
        assert_eq!(sort_value(&neg_stats, Sort::Net), -45);

        // Fallbacks for None values
        let empty_stats = AuthorStats::default();
        assert_eq!(sort_value(&empty_stats, Sort::Commits), 0);
        assert_eq!(sort_value(&empty_stats, Sort::Files), 0);
        assert_eq!(sort_value(&empty_stats, Sort::Surviving), 0);
        assert_eq!(sort_value(&empty_stats, Sort::Insertions), 0);
        assert_eq!(sort_value(&empty_stats, Sort::Deletions), 0);
        assert_eq!(sort_value(&empty_stats, Sort::Net), 0);
        assert_eq!(sort_value(&empty_stats, Sort::Churn), 0);
    }

    #[test]
    fn test_fmt_number() {
        assert_eq!(fmt_number(&Some(1234567usize)), "1,234,567");
        assert_eq!(fmt_number(&Some(0usize)), "0");
        assert_eq!(fmt_number::<usize>(&None), "");
        assert_eq!(fmt_number(&Some(-12345i64)), "-12,345");
    }

    #[test]
    fn test_output_path_for_repo() {
        let path = Path::new("reports/summary.json");
        assert_eq!(
            output_path_for_repo(path, "my-repo"),
            PathBuf::from("reports/summary-my-repo.json")
        );

        let path_no_ext = Path::new("reports/summary");
        assert_eq!(
            output_path_for_repo(path_no_ext, "my-repo"),
            PathBuf::from("reports/summary-my-repo")
        );

        let path_root = Path::new("summary.csv");
        assert_eq!(
            output_path_for_repo(path_root, "project"),
            PathBuf::from("summary-project.csv")
        );
    }
}
