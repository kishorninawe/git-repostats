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

    cmd.arg("-C")
        .arg(dir)
        .args(["shortlog", "-s", "-e", "--no-merges"]);

    push_opt_arg(&mut cmd, "--since", args.since.as_deref());
    push_opt_arg(&mut cmd, "--until", args.until.as_deref());

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

    cmd.arg("-C").arg(dir).args([
        "log",
        "--no-color",
        "--format=aN:%aN aE:%aE",
        "--no-merges",
        "--numstat",
    ]);

    push_opt_arg(&mut cmd, "--since", args.since.as_deref());
    push_opt_arg(&mut cmd, "--until", args.until.as_deref());

    push_flag(&mut cmd, "-w", args.ignore_whitespace);
    push_flag(&mut cmd, "-M", args.detect_moves);
    push_flag(&mut cmd, "-C", args.detect_copies);

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
