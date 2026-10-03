use indicatif::ProgressBar;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use regex::Regex;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    process::Command,
    sync::LazyLock,
};

use crate::{
    cli::{Args, ShowAuthor},
    error::GitError,
    stats::{AuthorCurrentStats, AuthorHistoryStats},
    utils::{push_flag, push_opt_arg},
};

static SHORTLOG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\s*(?P<commit>\d+)\s+(?P<name>.*)\s+<(?P<email>[^>]+)>$").unwrap()
});

static LOG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^aN:(?P<name>.+?) aE:(?P<email>.*?)$").unwrap());

static BLAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?mi)^[0-9a-f]{40} \d+ \d+ (?P<num_lines>\d+)\nauthor (?P<name>.+)\nauthor-mail <(?P<email>.+)>$"
    )
    .unwrap()
});

pub fn git_list_files(dir: &Path, args: &Args) -> Result<HashSet<String>, GitError> {
    let mut cmd = Command::new("git");

    cmd.arg("-C")
        .arg(dir)
        .args(["grep", "--no-color", "-I", "--name-only", "."])
        .arg(&args.branch);

    // Git executable couldn't be started.
    let out = cmd.output().map_err(|source| GitError::Io {
        command: format!("git grep"),
        source,
    })?;

    // Git started but returned an error.
    if !out.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git grep"),
            status: out.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        });
    }

    let stdout = String::from_utf8_lossy(&out.stdout);

    let prefix = format!("{}:", args.branch);

    let result = stdout
        .lines()
        .map(|line| line.strip_prefix(&prefix).unwrap_or(line)) // Remove "<branch>:" prefix if present
        .map(str::trim) // strip whitespace
        .filter(|line| !line.is_empty()) // ignore empty lines
        .map(str::to_owned)
        .collect();

    Ok(result)
}

pub fn git_shortlog(
    dir: &Path,
    args: &Args,
    repo_pb: &ProgressBar,
) -> Result<HashMap<String, usize>, GitError> {
    let mut cmd = Command::new("git");

    cmd.arg("-C").arg(dir).args(["shortlog", "-s", "-e"]);

    push_opt_arg(&mut cmd, "--since", args.since.as_deref());
    push_opt_arg(&mut cmd, "--until", args.until.as_deref());

    push_flag(&mut cmd, "--merges", args.merges);
    push_flag(&mut cmd, "--no-merges", args.no_merges);

    cmd.arg(&args.branch);

    // Git executable couldn't be started.
    let out = cmd.output().map_err(|source| GitError::Io {
        command: format!("git shortlog"),
        source,
    })?;

    // Git started but returned an error.
    if !out.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git shortlog"),
            status: out.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        });
    }

    let stdout = String::from_utf8_lossy(&out.stdout);

    let mut result: HashMap<String, usize> = HashMap::new();

    for caps in SHORTLOG_RE.captures_iter(&stdout) {
        let commit_count = caps["commit"].parse::<usize>().unwrap_or(0);
        let name = caps["name"].trim();
        let email = caps["email"].trim();

        let author = match args.show {
            ShowAuthor::Name => name.to_owned(),
            ShowAuthor::Email => email.to_owned(),
            ShowAuthor::Both => format!("{} <{}>", name, email),
        };

        *result.entry(author).or_default() += commit_count;
    }

    repo_pb.inc(1);

    Ok(result)
}

pub fn git_log(
    dir: &Path,
    args: &Args,
    allowed_files: &HashSet<String>,
    repo_pb: &ProgressBar,
) -> Result<HashMap<String, AuthorHistoryStats>, GitError> {
    let mut cmd = Command::new("git");

    cmd.arg("-C")
        .arg(dir)
        .args(["log", "--no-color", "--format=aN:%aN aE:%aE", "--numstat"]);

    push_opt_arg(&mut cmd, "--since", args.since.as_deref());
    push_opt_arg(&mut cmd, "--until", args.until.as_deref());

    push_flag(&mut cmd, "-w", args.ignore_whitespace);
    push_flag(&mut cmd, "-M", args.detect_moves);
    push_flag(&mut cmd, "-C", args.detect_copies);

    push_flag(&mut cmd, "--merges", args.merges);
    push_flag(&mut cmd, "--no-merges", args.no_merges);

    cmd.arg(&args.branch);

    // Git executable couldn't be started.
    let out = cmd.output().map_err(|source| GitError::Io {
        command: format!("git log"),
        source,
    })?;

    // Git started but returned an error.
    if !out.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git log"),
            status: out.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        });
    }

    let stdout = String::from_utf8_lossy(&out.stdout);

    let mut result: HashMap<String, AuthorHistoryStats> = HashMap::new();

    let mut current_author: Option<String> = None;

    for line in stdout.lines() {
        let line = line.trim();

        if line.is_empty() {
            continue;
        }

        if let Some(caps) = LOG_RE.captures(line) {
            let name = &caps["name"];
            let email = &caps["email"];

            let author = match args.show {
                ShowAuthor::Name => name.to_owned(),
                ShowAuthor::Email => email.to_owned(),
                ShowAuthor::Both => format!("{name} <{email}>"),
            };

            result.entry(author.clone()).or_default();
            current_author = Some(author);

            continue;
        }

        let Some(author) = &current_author else {
            continue;
        };

        let Some((ins_str, rest)) = line.split_once('\t') else {
            continue;
        };

        let Some((del_str, file)) = rest.split_once('\t') else {
            continue;
        };

        // Skip binary files.
        if ins_str == "-" || del_str == "-" {
            continue;
        }

        // Skip files not in allowed list.
        if !allowed_files.contains(file) {
            continue;
        }

        let ins = ins_str.parse::<usize>().unwrap_or(0);
        let del = del_str.parse::<usize>().unwrap_or(0);

        let entry = result.get_mut(author).unwrap();

        entry.ins += ins;
        entry.del += del;
        entry.churn += ins + del;
        entry.net += ins as isize - del as isize;
        entry.files.insert(file.to_owned());
    }

    repo_pb.inc(1);

    Ok(result)
}

pub fn git_blame(
    dir: &Path,
    args: &Args,
    files: &HashSet<String>,
    repo_pb: &ProgressBar,
) -> Result<HashMap<String, AuthorCurrentStats>, GitError> {
    files
        .par_iter()
        .try_fold(
            HashMap::new,
            |mut local: HashMap<String, AuthorCurrentStats>, file| {
                let file_stats = git_blame_file(dir, args, file)?;

                repo_pb.inc(1);

                for (author, stats) in file_stats {
                    let entry = local.entry(author).or_default();

                    entry.files.insert(file.clone());
                    entry.surviving += stats.surviving;
                }

                Ok(local)
            },
        )
        .try_reduce(HashMap::new, |mut acc, local| {
            for (author, stats) in local {
                let entry = acc.entry(author).or_default();

                entry.files.extend(stats.files);
                entry.surviving += stats.surviving;
            }

            Ok(acc)
        })
}

fn git_blame_file(
    dir: &Path,
    args: &Args,
    file: &str,
) -> Result<HashMap<String, AuthorCurrentStats>, GitError> {
    let mut cmd = Command::new("git");

    cmd.arg("-C").arg(dir).args(["blame", "--line-porcelain"]);

    push_opt_arg(&mut cmd, "--since", args.since.as_deref());
    push_opt_arg(&mut cmd, "--until", args.until.as_deref());

    push_flag(&mut cmd, "-w", args.ignore_whitespace);
    push_flag(&mut cmd, "-M", args.detect_moves);
    push_flag(&mut cmd, "-C", args.detect_copies);

    for rev in &args.ignore_rev {
        cmd.arg("--ignore-rev").arg(rev);
    }

    for rev_file in &args.ignore_revs_file {
        cmd.arg("--ignore-revs-file").arg(rev_file);
    }

    cmd.arg(&args.branch).arg(file);

    // Git executable couldn't be started.
    let out = cmd.output().map_err(|source| GitError::Io {
        command: format!("git blame"),
        source,
    })?;

    // Git started but returned an error.
    if !out.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git blame"),
            status: out.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        });
    }

    let stdout = String::from_utf8_lossy(&out.stdout);

    let mut result: HashMap<String, AuthorCurrentStats> = HashMap::new();

    for caps in BLAME_RE.captures_iter(&stdout) {
        let num_lines = caps["num_lines"].parse::<usize>().unwrap_or(0);
        let name = &caps["name"];
        let email = &caps["email"];

        let author = match args.show {
            ShowAuthor::Name => name.to_owned(),
            ShowAuthor::Email => email.to_owned(),
            ShowAuthor::Both => format!("{name} <{email}>"),
        };

        result.entry(author).or_default().surviving += num_lines;
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_shortlog_regex_matching() {
        let sample = "    42  John Doe  <john@example.com>";
        let caps = SHORTLOG_RE
            .captures(sample)
            .expect("Should match shortlog line");
        assert_eq!(&caps["commit"], "42");
        assert_eq!(caps["name"].trim(), "John Doe");
        assert_eq!(&caps["email"], "john@example.com");

        let tab_sample = "\t5\tJane Smith\t<jane@test.org>";
        let caps_tab = SHORTLOG_RE
            .captures(tab_sample)
            .expect("Should match tabbed shortlog line");
        assert_eq!(&caps_tab["commit"], "5");
        assert_eq!(caps_tab["name"].trim(), "Jane Smith");
        assert_eq!(&caps_tab["email"], "jane@test.org");

        assert!(SHORTLOG_RE.captures("invalid line without email").is_none());
        assert!(SHORTLOG_RE.captures("").is_none());
    }

    #[test]
    fn test_log_regex_matching() {
        let header = "aN:Alice Wonder aE:alice@wonderland.com";
        let caps = LOG_RE.captures(header).expect("Should match log header");
        assert_eq!(&caps["name"], "Alice Wonder");
        assert_eq!(&caps["email"], "alice@wonderland.com");

        let empty_email = "aN:Bob aE:";
        let caps_empty = LOG_RE
            .captures(empty_email)
            .expect("Should match log header with empty email");
        assert_eq!(&caps_empty["name"], "Bob");
        assert_eq!(&caps_empty["email"], "");

        assert!(LOG_RE.captures("12\t4\tsrc/git.rs").is_none());
        assert!(LOG_RE.captures("commit abcdef1234567890").is_none());
    }

    #[test]
    fn test_blame_regex_matching() {
        let porcelain = "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678 10 20 15\nauthor Carol King\nauthor-mail <carol@music.com>";
        let caps = BLAME_RE
            .captures(porcelain)
            .expect("Should match blame porcelain header");
        assert_eq!(&caps["num_lines"], "15");
        assert_eq!(&caps["name"], "Carol King");
        assert_eq!(&caps["email"], "carol@music.com");

        let invalid = "invalid blame content";
        assert!(BLAME_RE.captures(invalid).is_none());
    }

    #[test]
    fn test_git_list_files_success() {
        let args = Args::try_parse_from(["git-repostats"]).expect("Failed to parse default args");
        let res = git_list_files(Path::new("."), &args);
        assert!(res.is_ok());

        let files = res.unwrap();
        assert!(!files.is_empty());
        assert!(files.contains("Cargo.toml"));
        assert!(files.contains("src/git.rs"));
    }

    #[test]
    fn test_git_list_files_invalid_branch() {
        let args =
            Args::try_parse_from(["git-repostats", "--branch", "non_existent_branch_xyz123"])
                .expect("Failed to parse args");
        let res = git_list_files(Path::new("."), &args);
        assert!(matches!(res, Err(GitError::CommandFailed { .. })));
    }

    #[test]
    fn test_git_shortlog_success() {
        let args = Args::try_parse_from(["git-repostats"]).expect("Failed to parse default args");
        let pb = ProgressBar::hidden();
        let res = git_shortlog(Path::new("."), &args, &pb);
        assert!(res.is_ok());

        let stats = res.unwrap();
        assert!(!stats.is_empty());
        let total_commits: usize = stats.values().sum();
        assert!(total_commits > 0);
    }

    #[test]
    fn test_git_shortlog_show_author_modes() {
        let pb = ProgressBar::hidden();
        let modes = [
            ("name", ShowAuthor::Name),
            ("email", ShowAuthor::Email),
            ("both", ShowAuthor::Both),
        ];

        for (val, mode) in modes {
            let args = Args::try_parse_from(["git-repostats", "--show", val])
                .unwrap_or_else(|_| panic!("Failed to parse --show {val}"));
            let res = git_shortlog(Path::new("."), &args, &pb);
            assert!(res.is_ok());

            let stats = res.unwrap();
            assert!(!stats.is_empty());
            for author in stats.keys() {
                match mode {
                    ShowAuthor::Name | ShowAuthor::Email => {
                        assert!(!author.contains('<') && !author.contains('>'));
                    }
                    ShowAuthor::Both => {
                        assert!(author.contains('<') && author.contains('>'));
                    }
                }
            }
        }
    }

    #[test]
    fn test_git_shortlog_invalid_branch() {
        let args =
            Args::try_parse_from(["git-repostats", "--branch", "non_existent_branch_xyz123"])
                .expect("Failed to parse args");
        let pb = ProgressBar::hidden();
        let res = git_shortlog(Path::new("."), &args, &pb);
        assert!(matches!(res, Err(GitError::CommandFailed { .. })));
    }

    #[test]
    fn test_git_log_success() {
        let args = Args::try_parse_from(["git-repostats"]).expect("Failed to parse default args");
        let mut allowed_files = HashSet::new();
        allowed_files.insert("Cargo.toml".to_string());
        let pb = ProgressBar::hidden();

        let res = git_log(Path::new("."), &args, &allowed_files, &pb);
        assert!(res.is_ok());

        let history = res.unwrap();
        assert!(!history.is_empty());

        for stats in history.values() {
            assert_eq!(stats.churn, stats.ins + stats.del);
            assert_eq!(stats.net, stats.ins as isize - stats.del as isize);
        }
    }

    #[test]
    fn test_git_log_empty_allowed_files() {
        let args = Args::try_parse_from(["git-repostats"]).expect("Failed to parse default args");
        let allowed_files = HashSet::new();
        let pb = ProgressBar::hidden();

        let res = git_log(Path::new("."), &args, &allowed_files, &pb);
        assert!(res.is_ok());

        let history = res.unwrap();
        for stats in history.values() {
            assert!(stats.files.is_empty());
            assert_eq!(stats.ins, 0);
            assert_eq!(stats.del, 0);
            assert_eq!(stats.churn, 0);
            assert_eq!(stats.net, 0);
        }
    }

    #[test]
    fn test_git_log_invalid_branch() {
        let args =
            Args::try_parse_from(["git-repostats", "--branch", "non_existent_branch_xyz123"])
                .expect("Failed to parse args");
        let allowed = HashSet::new();
        let pb = ProgressBar::hidden();
        let res = git_log(Path::new("."), &args, &allowed, &pb);
        assert!(matches!(res, Err(GitError::CommandFailed { .. })));
    }

    #[test]
    fn test_git_blame_file_success() {
        let args = Args::try_parse_from(["git-repostats"]).expect("Failed to parse default args");
        let res = git_blame_file(Path::new("."), &args, "Cargo.toml");
        assert!(res.is_ok());

        let stats = res.unwrap();
        assert!(!stats.is_empty());
        let total_lines: usize = stats.values().map(|s| s.surviving).sum();
        assert!(total_lines > 0);
    }

    #[test]
    fn test_git_blame_file_nonexistent() {
        let args = Args::try_parse_from(["git-repostats"]).expect("Failed to parse default args");
        let res = git_blame_file(Path::new("."), &args, "non_existent_file_xyz123.txt");
        assert!(matches!(res, Err(GitError::CommandFailed { .. })));
    }

    #[test]
    fn test_git_blame_success() {
        let args = Args::try_parse_from(["git-repostats"]).expect("Failed to parse default args");
        let mut files = HashSet::new();
        files.insert("Cargo.toml".to_string());
        let pb = ProgressBar::hidden();

        let res = git_blame(Path::new("."), &args, &files, &pb);
        assert!(res.is_ok());

        let stats = res.unwrap();
        assert!(!stats.is_empty());
        let total_lines: usize = stats.values().map(|s| s.surviving).sum();
        assert!(total_lines > 0);
    }

    #[test]
    fn test_git_blame_empty_files() {
        let args = Args::try_parse_from(["git-repostats"]).expect("Failed to parse default args");
        let files = HashSet::new();
        let pb = ProgressBar::hidden();

        let res = git_blame(Path::new("."), &args, &files, &pb);
        assert!(res.is_ok());

        let stats = res.unwrap();
        assert!(stats.is_empty());
    }

    #[test]
    fn test_git_error_display() {
        let cmd_err = GitError::CommandFailed {
            command: "git grep".to_string(),
            status: 128,
            stderr: "fatal: not a git repo".to_string(),
        };
        assert_eq!(
            format!("{cmd_err}"),
            "`git grep` failed with exit code 128: fatal: not a git repo"
        );

        let io_err = GitError::Io {
            command: "git log".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "binary not found"),
        };
        assert!(format!("{io_err}").contains("failed to execute `git log`"));
    }
}
