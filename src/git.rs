use crate::cli::{Args, ShowAuthor};
use crate::stats::{AuthorCurrentStats, AuthorHistoryStats};
use crate::utils::{push_flag, push_opt_arg};
use indicatif::ProgressBar;
use rayon::prelude::*;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::process::Command;

pub fn git_list_files(dir: &str, args: &Args) -> HashSet<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["grep", "-I", "--name-only", ".", &args.branch])
        .output()
        .expect("git grep failed");

    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim) // strip whitespace
        .filter(|l| !l.is_empty()) // ignore empty lines
        .map(|l| {
            // Remove "<branch>:" prefix if present
            l.strip_prefix(&format!("{}:", args.branch))
                .unwrap_or(l)
                .to_string()
        })
        .collect()
}

pub fn git_shortlog(
    dir: &str,
    args: &Args,
    repo_pb: &ProgressBar,
    git_pb: &ProgressBar,
) -> HashMap<String, usize> {
    git_pb.set_message("git shortlog …");

    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(dir)
        .args(["shortlog", "-s", "-e", "--no-merges"]);
    push_opt_arg(&mut cmd, "--since", args.since.as_deref());
    push_opt_arg(&mut cmd, "--until", args.until.as_deref());
    cmd.arg(&args.branch);

    let out = cmd.output().expect("git shortlog failed");
    let stdout = String::from_utf8_lossy(&out.stdout);

    let re = Regex::new(r"(?m)^\s*(?P<commit>\d+)\s+(?P<name>.*)\s+<(?P<email>[^>]+)>$").unwrap();
    let mut result = HashMap::new();

    for caps in re.captures_iter(&stdout) {
        let commit_count: usize = caps["commit"].parse().unwrap_or(0);
        let name = caps["name"].trim();
        let email = caps["email"].trim();

        let author = match args.show {
            ShowAuthor::Name => name.to_string(),
            ShowAuthor::Email => email.to_string(),
            ShowAuthor::Both => format!("{} <{}>", name, email),
        };

        result.insert(author, commit_count);
    }

    repo_pb.inc(1);

    result
}

pub fn git_log(
    dir: &str,
    args: &Args,
    allowed_files: &HashSet<String>,
    repo_pb: &ProgressBar,
    git_pb: &ProgressBar,
) -> HashMap<String, AuthorHistoryStats> {
    git_pb.set_message("git log …");

    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args([
        "log",
        "--format=aN:%aN aE:%aE ct:%ct",
        "--no-merges",
        "--numstat",
    ]);
    push_opt_arg(&mut cmd, "--since", args.since.as_deref());
    push_opt_arg(&mut cmd, "--until", args.until.as_deref());
    push_flag(&mut cmd, "-w", args.ignore_whitespace);
    push_flag(&mut cmd, "-M", args.detect_moves);
    push_flag(&mut cmd, "-C", args.detect_copies);
    cmd.arg(&args.branch);

    let out = cmd.output().expect("git log failed");
    let header_re = Regex::new(r"^aN:(?P<name>.+?) aE:(?P<email>.*?) ct:(?P<ts>\d+)$").unwrap();

    let mut result: HashMap<String, AuthorHistoryStats> = HashMap::new();
    let mut current_author: Option<String> = None;

    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Commit header
        if let Some(caps) = header_re.captures(line) {
            let name = &caps["name"];
            let email = &caps["email"];
            let author = match args.show {
                ShowAuthor::Name => name.to_string(),
                ShowAuthor::Email => email.to_string(),
                ShowAuthor::Both => format!("{} <{}>", name, email),
            };

            let _entry = result.entry(author.clone()).or_default();

            current_author = Some(author);
            continue;
        }

        // Numstat line
        if let Some(author) = &current_author {
            let parts: Vec<&str> = line.split('\t').collect();
            let file = parts[2];

            if parts.len() == 3 {
                // Skip binary files
                if parts[0] == "-" || parts[1] == "-" {
                    continue;
                }

                // Skip files not in allowed list
                if !allowed_files.contains(file) {
                    continue;
                }

                let ins = parts[0].parse::<usize>().unwrap_or(0);
                let del = parts[1].parse::<usize>().unwrap_or(0);

                let entry = result.get_mut(author).unwrap();
                entry.ins += ins;
                entry.del += del;
                entry.churn += ins + del;
                entry.net += ins as isize - del as isize;
                entry.files.insert(file.to_string());
            }
        }
    }

    repo_pb.inc(1);

    result
}

pub fn git_blame(
    dir: &str,
    args: &Args,
    files: &HashSet<String>,
    repo_pb: &ProgressBar,
    git_pb: &ProgressBar,
) -> HashMap<String, AuthorCurrentStats> {
    files
        .par_iter()
        .map(|file| {
            // Progress message (thread-safe)
            git_pb.set_message(format!("git blame {} …", file));

            // Run blame for ONE file
            let file_stats = git_blame_file(dir, args, file);

            // Update repo progress
            repo_pb.inc(1);

            // Build a LOCAL HashMap
            let mut local: HashMap<String, AuthorCurrentStats> = HashMap::new();

            for (author, stats) in file_stats {
                let entry = local.entry(author).or_default();
                entry.files.insert(file.clone());
                entry.surviving += stats.surviving;
            }

            local
        })
        // Reduce: merge all local HashMaps
        .reduce(HashMap::new, |mut acc, local| {
            for (author, stats) in local {
                let entry = acc.entry(author).or_default();
                entry.files.extend(stats.files);
                entry.surviving += stats.surviving;
            }
            acc
        })
}

fn git_blame_file(dir: &str, args: &Args, file: &str) -> HashMap<String, AuthorCurrentStats> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(["blame", "--line-porcelain"]);
    push_opt_arg(&mut cmd, "--since", args.since.as_deref());
    push_opt_arg(&mut cmd, "--until", args.until.as_deref());
    push_flag(&mut cmd, "-w", args.ignore_whitespace);
    push_flag(&mut cmd, "-M", args.detect_moves);
    push_flag(&mut cmd, "-C", args.detect_copies);

    cmd.arg(&args.branch).arg(file);

    let out = cmd.output().expect("git blame failed");

    let re = Regex::new(
    r"(?mi)^[0-9a-f]{40} \d+ \d+ (?P<num_lines>\d+)\nauthor (?P<name>.+)\nauthor-mail <(?P<email>.+)>$"
    ).unwrap();

    let mut result: HashMap<String, AuthorCurrentStats> = HashMap::new();

    for caps in re.captures_iter(&String::from_utf8_lossy(&out.stdout)) {
        let num_lines: usize = caps["num_lines"].parse().unwrap();
        let name = &caps["name"];
        let email = &caps["email"];
        let author = match args.show {
            ShowAuthor::Name => name.to_string(),
            ShowAuthor::Email => email.to_string(),
            ShowAuthor::Both => format!("{} <{}>", name, email),
        };

        let entry = result.entry(author).or_default();
        entry.surviving += num_lines;
    }

    result
}
