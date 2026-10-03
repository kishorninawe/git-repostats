# git-repostats

A fast CLI tool and library to analyze Git repositories and generate detailed contribution statistics.

## Features

- Author-wise contribution stats
- High performance (Rust-based)
- Metrics:
  - Commits
  - Files
  - Surviving lines
  - Insertions / Deletions
  - Net lines
  - Churn
- Limit output (`--limit`)
- Smart aggregation (`OTHER` bucket)

## Installation

### Using PIP

```bash
pip install git-repostats
```

### Using Docker

1. Build the Docker image

Run this command from the directory containing the `Dockerfile`:

```bash
docker build -t git-repostats .
```

2. Run Git Repostats

Mount your Git repository into the container:

```bash
docker run --rm -it -v "/path/to/your/repo:/repo" git-repostats
```

OR

2. Run with CLI options

```bash
docker run --rm -it -v "/path/to/your/repo:/repo" git-repostats --metrics all
```

**Note:** Replace `/path/to/your/repo` with your repository's absolute path. `--rm` automatically removes the container after execution; the Docker image remains available for reuse.

## Usage

```bash
git-repostats --metrics <metrics> --limit <N> <repo_path>
```

```bash
python -m git_repostats --metrics <metrics> --limit <N> <repo_path>
```

## Documentation

```
Usage: git-repostats [OPTIONS] [GITDIR]...

Arguments:
  [GITDIR]...
          Git repository directory or directories to analyze.

          [default: .]

Options:
      --branch <BRANCH>
          Branch or tag up to which to check.

          [default: HEAD]

      --metrics <METRICS>...
          Select which metrics to compute.

          Possible values:
          - commits: Commit count per author
          - files:   Number of unique files modified by the author
          - history: Commit count, files, insertions, deletions, net change, and churn
          - current: Commit count, files, and surviving lines in the current code state
          - all:     Combine history and current metrics

          [default: commits]

      --sort <SORT>
          Sort output by the specified metric.
          Sorting is descending by default.

          Possible values:
          - commits:    Number of commits authored by the user
          - files:      Number of unique files modified by the user across all commits
          - insertions: Total number of lines added by the user
          - deletions:  Total number of lines removed by the user
          - net:        Net line change introduced by the user (insertions minus deletions)
          - churn:      Total lines of code modified by the user (insertions plus deletions)
          - surviving:  Number of lines currently present in the codebase that are attributed to the user

      --reverse
          Reverse the sort order from descending to ascending.

  -l, --limit <N|all>
          Limit number of authors displayed in the output. Use `all` for no limit.

          [default: 10]

  -s, --silent-progress
          Suppress progress bars and progress messages while processing repositories.

      --format <FORMAT>
          Set the output format for repository statistics.
          Supported formats: table, json, csv, yaml and markdown.
          Defaults to table.

          Possible values:
          - table:    Output results as formatted table
          - json:     Output results as JSON
          - csv:      Output results as CSV
          - yaml:     Output results as YAML
          - markdown: Output results as Markdown

          [default: table]

      --output <FILE>
          Write repository statistics to the specified file instead of stdout.
          The output format is determined by --format.
          If the file already exists, it will be overwritten.

      --since <DATE>
          Include only commits after the specified date.
          Accepts any date format supported by Git.

      --until <DATE>
          Include only commits before the specified date.
          Accepts any date format supported by Git.

      --show <SHOW>
          Author information to show.

          Possible values:
          - name:  Author name only
          - email: Author email only
          - both:  Author name and email

          [default: both]

  -M, --detect-moves
          Detect moved lines when computing metrics.
          Improves accuracy for refactors at the cost of performance.

  -C, --detect-copies
          Detect copied lines across files when computing metrics.

  -w, --ignore-whitespace
          Ignore whitespace-only changes when computing current metrics.

      --merges
          Include only merge commits (commits with two or more parents) for commits, files, and history metrics. Equivalent to Git's --min-parents=2.

      --no-merges
          Include only non-merge commits (commits with at most one parent) for commits, files, and history metrics. Equivalent to Git's --max-parents=1.

      --threads <THREADS>
          Number of parallel git operations. 0 = auto (min(CPU cores, 4)).

          [default: 0]

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## Example

```bash
git-repostats --metrics all --limit=5 "/path/to/sample-repo"
```

```text
Done sample-repo in 2 seconds
==========================================================================================================================
 Repository: sample-repo
==========================================================================================================================
 Author                                                Commits ↓  Files  Surviving  Insertions  Deletions      Net   Churn
--------------------------------------------------------------------------------------------------------------------------
 Alice <alice@example.com>                                    50    120      8,500      +5,000     -1,200   +3,800   6,200
 Bob <bob@example.com>                                        30     80      5,200      +3,000       -900   +2,100   3,900
 Charlie <charlie@example.com>                                20     40      2,800      +1,500       -300   +1,200   1,800
 David <david@example.com>                                    15     35      2,100      +1,200       -200   +1,000   1,400
 Eve <eve@example.com>                                        10     25      1,400        +900       -150     +750   1,050
--------------------------------------------------------------------------------------------------------------------------
 OTHER (3 authors)                                            12     30      1,800      +1,000       -250     +750   1,250
--------------------------------------------------------------------------------------------------------------------------
 TOTAL (8 authors)                                           137    330     23,800     +13,600     -3,000  +10,600  15,600
```

## Contributing

Contributions are welcome! Feel free to open issues or PRs.

## License

The `git-repostats` is licensed under the terms of the MIT license.
