use csv::Writer;
use serde::Serialize;
use serde_json::{Map, Value, json};
use tabled::{
    Table, Tabled,
    settings::{
        Alignment, Format, Modify, Panel, Remove, Style,
        object::Columns,
        object::Rows,
        style::{HorizontalLine, LineChar},
        themes::BorderCorrection,
    },
};

use crate::{
    cli::{Args, Limit, Metrics, OutputFormat},
    error::AppError,
    stats::AuthorStats,
    utils::fmt_number,
};

#[derive(Tabled, Serialize)]
#[serde(rename_all = "PascalCase")]
#[tabled(rename_all = "Upper Title Case")]
struct AllRow {
    author: String,

    #[tabled(display = "fmt_number")]
    commits: Option<usize>,

    #[tabled(display = "fmt_number")]
    files: Option<usize>,

    #[tabled(display = "fmt_number")]
    surviving: Option<usize>,

    #[tabled(display = "display_ins")]
    insertions: Option<usize>,

    #[tabled(display = "display_del")]
    deletions: Option<usize>,

    #[tabled(display = "display_net")]
    net: Option<isize>,

    #[tabled(display = "fmt_number")]
    churn: Option<usize>,
}

const METRIC_COLUMNS: &[(Metrics, &[usize])] = &[
    (Metrics::Commits, &[1]),
    (Metrics::Files, &[2]),
    (Metrics::Current, &[3]),
    (Metrics::History, &[4, 5, 6, 7]),
];

pub fn format_repo(
    args: &Args,
    repo_name: &str,
    rows: Vec<(String, AuthorStats)>,
) -> Result<String, AppError> {
    let (visible, remaining) = split_rows_by_limit(args, &rows);

    match args.format {
        OutputFormat::Table => format_table(args, repo_name, &rows, visible, remaining),
        OutputFormat::Json => format_json(args, repo_name, &rows, visible, remaining),
        OutputFormat::Csv => format_csv(args, repo_name, &rows, visible, remaining),
        OutputFormat::Yaml => format_yaml(args, repo_name, &rows, visible, remaining),
        OutputFormat::Markdown => format_markdown(args, repo_name, &rows, visible, remaining),
    }
}

fn split_rows_by_limit<'a>(
    args: &Args,
    rows: &'a [(String, AuthorStats)],
) -> (&'a [(String, AuthorStats)], &'a [(String, AuthorStats)]) {
    let limit = match args.limit {
        Limit::All => rows.len(),
        Limit::Count(n) => n,
    };

    let visible_count = rows.len().min(limit);
    rows.split_at(visible_count)
}

fn aggregate_stats(rows: &[(String, AuthorStats)]) -> Result<AuthorStats, AppError> {
    rows.iter()
        .map(|(_, s)| s)
        .cloned()
        .reduce(|a, b| a + b)
        .ok_or_else(|| AppError::Format("cannot aggregate empty rows".into()))
}

// -------------------------------------------------------------------------
// Table
// -------------------------------------------------------------------------

pub fn format_table(
    args: &Args,
    repo_name: &str,
    rows: &[(String, AuthorStats)],
    visible: &[(String, AuthorStats)],
    remaining: &[(String, AuthorStats)],
) -> Result<String, AppError> {
    let total_rows = rows.len();
    let is_other_row = !remaining.is_empty();
    let mut final_rows: Vec<(String, AuthorStats)> = visible.to_vec();

    if !remaining.is_empty() {
        let others = aggregate_stats(remaining)?;
        final_rows.push((
            format!(
                "OTHER ({} author{})",
                fmt_number(&Some(remaining.len())),
                if remaining.len() == 1 { "" } else { "s" }
            ),
            others,
        ));
    }

    if total_rows > 1 {
        let total = aggregate_stats(&rows)?;
        final_rows.push((
            format!(
                "TOTAL ({} author{})",
                fmt_number(&Some(total_rows)),
                if total_rows == 1 { "" } else { "s" }
            ),
            total,
        ))
    };

    if final_rows.is_empty() {
        return Ok(format!("No data to display for repository: {repo_name}").to_owned());
    }

    let table_rows: Vec<AllRow> = final_rows
        .into_iter()
        .map(|(author, stats)| AllRow {
            author,
            commits: stats.commits,
            files: stats.files,
            surviving: stats.surviving,
            insertions: stats.ins,
            deletions: stats.del,
            net: stats.net,
            churn: stats.churn,
        })
        .collect();

    let mut table = Table::new(table_rows);

    let arrow = if args.reverse { " ↑" } else { " ↓" };
    let sort_key = args.sort.as_ref().map(ToString::to_string);

    let other_sep = if is_other_row {
        table.count_rows() - 1
    } else {
        table.count_rows() // disabled separator
    };

    let horizontals = [
        (0, HorizontalLine::new('=')),
        (1, HorizontalLine::new('=')),
        (2, HorizontalLine::new('-')),
        (other_sep, HorizontalLine::new('-')),
        (table.count_rows(), HorizontalLine::new('-')),
    ];

    let style = Style::empty().horizontals(horizontals);

    let mut remove_cols = Vec::new();

    for (metric, cols) in METRIC_COLUMNS {
        if !args.metrics.contains(metric) {
            remove_cols.extend_from_slice(cols);
        }
    }

    remove_cols.sort_unstable_by(|a, b| b.cmp(a));

    for idx in remove_cols {
        table.with(Remove::column(Columns::one(idx)));
    }

    table
        .with(style)
        .with(Panel::header(format!("Repository: {repo_name}")))
        .with(BorderCorrection::span())
        .with(Modify::new(Columns::new(1..)).with(Alignment::right()))
        .with(Modify::new(Rows::one(1)).with(Format::content(move |s| {
            if sort_key.as_deref() == Some(s) {
                format!("{s}{arrow}")
            } else {
                s.to_owned()
            }
        })));

    Ok(table.to_string())
}

// -------------------------------------------------------------------------
// CSV
// -------------------------------------------------------------------------

fn format_csv(
    args: &Args,
    repo_name: &str,
    rows: &[(String, AuthorStats)],
    visible: &[(String, AuthorStats)],
    remaining: &[(String, AuthorStats)],
) -> Result<String, AppError> {
    let mut writer = Writer::from_writer(Vec::new());

    let mut headers = vec!["Author"];

    for (metric, _) in METRIC_COLUMNS {
        if args.metrics.contains(metric) {
            match metric {
                Metrics::Commits => headers.push("Commits"),
                Metrics::Files => headers.push("Files"),
                Metrics::Current => headers.push("Surviving"),
                Metrics::History => {
                    headers.extend(["Insertions", "Deletions", "Net", "Churn"]);
                }
                Metrics::All => {}
            }
        }
    }

    let metadata = [
        format!("# Repository: {repo_name}"),
        format!("# Authors: {} of {} shown", visible.len(), rows.len()),
        format!("# Limit: {}", args.limit),
        format!("# Truncated: {}", !remaining.is_empty()),
    ];

    for line in metadata {
        let mut record = vec![line];
        record.resize(headers.len(), String::new());
        writer.write_record(&record)?;
    }

    writer.write_record(&headers)?;

    for (author, stats) in visible {
        let mut record = vec![author.clone()];

        if args.metrics.contains(&Metrics::Commits) {
            record.push(stats.commits.map_or_default(|v| v.to_string()));
        }

        if args.metrics.contains(&Metrics::Files) {
            record.push(stats.files.map_or_default(|v| v.to_string()));
        }

        if args.metrics.contains(&Metrics::Current) {
            record.push(stats.surviving.map_or_default(|v| v.to_string()));
        }

        if args.metrics.contains(&Metrics::History) {
            record.push(stats.ins.map_or_default(|v| v.to_string()));
            record.push(stats.del.map_or_default(|v| v.to_string()));
            record.push(stats.net.map_or_default(|v| v.to_string()));
            record.push(stats.churn.map_or_default(|v| v.to_string()));
        }

        writer.write_record(&record)?;
    }

    let bytes = writer
        .into_inner()
        .map_err(|e| AppError::Io(e.into_error()))?;

    let output = String::from_utf8(bytes).map_err(|e| AppError::Format(e.to_string()))?;

    Ok(output)
}

// -------------------------------------------------------------------------
// JSON
// -------------------------------------------------------------------------

pub fn format_json(
    args: &Args,
    repo_name: &str,
    rows: &[(String, AuthorStats)],
    visible: &[(String, AuthorStats)],
    remaining: &[(String, AuthorStats)],
) -> Result<String, AppError> {
    let output = build_output_value(args, repo_name, rows, visible, remaining);

    serde_json::to_string_pretty(&output).map_err(|e| AppError::Format(e.to_string()))
}

// -------------------------------------------------------------------------
// YAML
// -------------------------------------------------------------------------

pub fn format_yaml(
    args: &Args,
    repo_name: &str,
    rows: &[(String, AuthorStats)],
    visible: &[(String, AuthorStats)],
    remaining: &[(String, AuthorStats)],
) -> Result<String, AppError> {
    let output = build_output_value(args, repo_name, rows, visible, remaining);

    let options = serde_saphyr::ser_options! {
        compact_list_indent:false
    };

    serde_saphyr::to_string_with_options(&output, options)
        .map_err(|e| AppError::Format(e.to_string()))
}

// -------------------------------------------------------------------------
// Markdown
// -------------------------------------------------------------------------

pub fn format_markdown(
    args: &Args,
    repo_name: &str,
    rows: &[(String, AuthorStats)],
    visible: &[(String, AuthorStats)],
    remaining: &[(String, AuthorStats)],
) -> Result<String, AppError> {
    let total_rows = rows.len();
    let mut final_rows: Vec<(String, AuthorStats)> = visible.to_vec();

    if !remaining.is_empty() {
        let others = aggregate_stats(remaining)?;
        final_rows.push((
            format!(
                "OTHER ({} author{})",
                fmt_number(&Some(remaining.len())),
                if remaining.len() == 1 { "" } else { "s" }
            ),
            others,
        ));
    }

    if total_rows > 1 {
        let total = aggregate_stats(&rows)?;
        final_rows.push((
            format!(
                "TOTAL ({} author{})",
                fmt_number(&Some(total_rows)),
                if total_rows == 1 { "" } else { "s" }
            ),
            total,
        ))
    };

    if final_rows.is_empty() {
        return Ok(format!("No data to display for repository: {repo_name}").to_owned());
    }

    let table_rows: Vec<AllRow> = final_rows
        .into_iter()
        .map(|(author, stats)| AllRow {
            author,
            commits: stats.commits,
            files: stats.files,
            surviving: stats.surviving,
            insertions: stats.ins,
            deletions: stats.del,
            net: stats.net,
            churn: stats.churn,
        })
        .collect();

    let mut table = Table::new(table_rows);

    let mut remove_cols = Vec::new();

    for (metric, cols) in METRIC_COLUMNS {
        if !args.metrics.contains(metric) {
            remove_cols.extend_from_slice(cols);
        }
    }

    remove_cols.sort_unstable_by(|a, b| b.cmp(a));

    for idx in remove_cols {
        table.with(Remove::column(Columns::one(idx)));
    }

    table
        .with(Style::markdown())
        .with(Modify::new(Columns::first()).with(Alignment::left()))
        .with(Modify::new(Columns::first()).with(LineChar::horizontal(':', 0 as usize)))
        .with(Modify::new(Columns::new(1..)).with(Alignment::right()))
        .with(Modify::new(Columns::new(1..)).with(LineChar::horizontal(':', 0)));

    if !remaining.is_empty() {
        table.with(
            Modify::new(Rows::one(table.count_rows() - 2))
                .with(Format::content(move |s| format!("**{s}**"))),
        );
    }

    if total_rows > 1 {
        table.with(
            Modify::new(Rows::one(table.count_rows() - 1))
                .with(Format::content(move |s| format!("**{s}**"))),
        );
    }

    Ok(format!(
        "# Repository: {repo_name}\n\n{}",
        table.to_string()
    ))
}

fn build_output_value(
    args: &Args,
    repo_name: &str,
    rows: &[(String, AuthorStats)],
    visible: &[(String, AuthorStats)],
    remaining: &[(String, AuthorStats)],
) -> Value {
    let mut authors = Vec::with_capacity(visible.len());

    for (author, stats) in visible {
        let mut record = Map::new();

        record.insert("Author".to_owned(), json!(author));

        if args.metrics.contains(&Metrics::Commits) {
            record.insert("Commits".to_owned(), json!(stats.commits));
        }

        if args.metrics.contains(&Metrics::Files) {
            record.insert("Files".to_owned(), json!(stats.files));
        }

        if args.metrics.contains(&Metrics::Current) {
            record.insert("Surviving".to_owned(), json!(stats.surviving));
        }

        if args.metrics.contains(&Metrics::History) {
            record.insert("Insertions".to_owned(), json!(stats.ins));
            record.insert("Deletions".to_owned(), json!(stats.del));
            record.insert("Net".to_owned(), json!(stats.net));
            record.insert("Churn".to_owned(), json!(stats.churn));
        }

        authors.push(Value::Object(record));
    }

    json!({
        "metadata": {
            "repository": repo_name,
            "total_authors": rows.len(),
            "returned_authors": visible.len(),
            "limit": match args.limit {
                Limit::All => json!("All"),
                Limit::Count(n) => json!(n),
            },
            "truncated": !remaining.is_empty()
        },
        "authors": authors
    })
}

fn display_ins(v: &Option<usize>) -> String {
    match *v {
        None => String::new(),
        Some(0) => "0".to_owned(),
        Some(n) => format!("+{}", fmt_number(&Some(n))),
    }
}

fn display_del(v: &Option<usize>) -> String {
    match *v {
        None => String::new(),
        Some(0) => "0".to_owned(),
        Some(n) => format!("-{}", fmt_number(&Some(n))),
    }
}

fn display_net(v: &Option<isize>) -> String {
    match *v {
        None => String::new(),
        Some(0) => "0".to_owned(),
        Some(n) if n > 0 => format!("+{}", fmt_number(&Some(n))),
        Some(n) => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn sample_stats() -> Vec<(String, AuthorStats)> {
        vec![
            (
                "Alice <alice@test.com>".to_string(),
                AuthorStats {
                    commits: Some(10),
                    files: Some(5),
                    surviving: Some(100),
                    ins: Some(150),
                    del: Some(30),
                    net: Some(120),
                    churn: Some(180),
                },
            ),
            (
                "Bob <bob@test.com>".to_string(),
                AuthorStats {
                    commits: Some(5),
                    files: Some(2),
                    surviving: Some(50),
                    ins: Some(60),
                    del: Some(10),
                    net: Some(50),
                    churn: Some(70),
                },
            ),
            (
                "Carol <carol@test.com>".to_string(),
                AuthorStats {
                    commits: Some(2),
                    files: Some(1),
                    surviving: Some(20),
                    ins: Some(30),
                    del: Some(5),
                    net: Some(25),
                    churn: Some(35),
                },
            ),
        ]
    }

    #[test]
    fn test_display_helpers() {
        // display_ins
        assert_eq!(display_ins(&None), "");
        assert_eq!(display_ins(&Some(0)), "0");
        assert_eq!(display_ins(&Some(100)), "+100");
        assert_eq!(display_ins(&Some(1234567)), "+1,234,567");

        // display_del
        assert_eq!(display_del(&None), "");
        assert_eq!(display_del(&Some(0)), "0");
        assert_eq!(display_del(&Some(50)), "-50");
        assert_eq!(display_del(&Some(987654)), "-987,654");

        // display_net
        assert_eq!(display_net(&None), "");
        assert_eq!(display_net(&Some(0)), "0");
        assert_eq!(display_net(&Some(250)), "+250");
        assert_eq!(display_net(&Some(1000000)), "+1,000,000");
        assert_eq!(display_net(&Some(-42)), "-42");
    }

    #[test]
    fn test_split_rows_by_limit() {
        let rows = sample_stats();

        // Limit::All
        let args_all = Args::try_parse_from(["git-repostats", "--limit", "all"])
            .expect("Failed to parse args");
        let (visible, remaining) = split_rows_by_limit(&args_all, &rows);
        assert_eq!(visible.len(), 3);
        assert!(remaining.is_empty());

        // Limit::Count(2)
        let args_count =
            Args::try_parse_from(["git-repostats", "-l", "2"]).expect("Failed to parse args");
        let (visible, remaining) = split_rows_by_limit(&args_count, &rows);
        assert_eq!(visible.len(), 2);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0, "Carol <carol@test.com>");

        // Limit larger than row count
        let args_large =
            Args::try_parse_from(["git-repostats", "-l", "10"]).expect("Failed to parse args");
        let (visible, remaining) = split_rows_by_limit(&args_large, &rows);
        assert_eq!(visible.len(), 3);
        assert!(remaining.is_empty());

        // Empty rows
        let empty_rows: Vec<(String, AuthorStats)> = vec![];
        let (visible, remaining) = split_rows_by_limit(&args_count, &empty_rows);
        assert!(visible.is_empty());
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_aggregate_stats_success() {
        let rows = sample_stats();
        let aggregated = aggregate_stats(&rows).expect("Failed to aggregate stats");

        assert_eq!(aggregated.commits, Some(17));
        assert_eq!(aggregated.files, Some(8));
        assert_eq!(aggregated.surviving, Some(170));
        assert_eq!(aggregated.ins, Some(240));
        assert_eq!(aggregated.del, Some(45));
        assert_eq!(aggregated.net, Some(195));
        assert_eq!(aggregated.churn, Some(285));
    }

    #[test]
    fn test_aggregate_stats_empty() {
        let empty_rows: Vec<(String, AuthorStats)> = vec![];
        let err = aggregate_stats(&empty_rows);
        assert!(matches!(err, Err(AppError::Format(_))));
    }

    #[test]
    fn test_format_table_single_author() {
        let rows = vec![(
            "Alice".to_string(),
            AuthorStats {
                commits: Some(10),
                files: Some(5),
                surviving: Some(100),
                ins: Some(150),
                del: Some(30),
                net: Some(120),
                churn: Some(180),
            },
        )];

        let args = Args::try_parse_from(["git-repostats"]).expect("Failed to parse args");
        let (visible, remaining) = split_rows_by_limit(&args, &rows);
        let table = format_table(&args, "my-repo", &rows, visible, remaining)
            .expect("Failed to format table");

        assert!(table.contains("Repository: my-repo"));
        assert!(table.contains("Alice"));
        assert!(!table.contains("TOTAL"));
        assert!(!table.contains("OTHER"));
    }

    #[test]
    fn test_format_table_multiple_with_limit() {
        let rows = sample_stats();
        let args =
            Args::try_parse_from(["git-repostats", "-l", "2"]).expect("Failed to parse args");
        let (visible, remaining) = split_rows_by_limit(&args, &rows);
        let table = format_table(&args, "my-repo", &rows, visible, remaining)
            .expect("Failed to format table");

        assert!(table.contains("Repository: my-repo"));
        assert!(table.contains("Alice"));
        assert!(table.contains("Bob"));
        assert!(table.contains("OTHER (1 author)"));
        assert!(table.contains("TOTAL (3 authors)"));
    }

    #[test]
    fn test_format_table_empty() {
        let rows: Vec<(String, AuthorStats)> = vec![];
        let args = Args::try_parse_from(["git-repostats"]).expect("Failed to parse args");
        let (visible, remaining) = split_rows_by_limit(&args, &rows);
        let output = format_table(&args, "my-repo", &rows, visible, remaining)
            .expect("Failed to format table");

        assert_eq!(output, "No data to display for repository: my-repo");
    }

    #[test]
    fn test_format_table_sort_indicators() {
        let rows = sample_stats();

        // Descending arrow (default reverse=false)
        let args_desc = Args::try_parse_from(["git-repostats", "--sort", "commits"])
            .expect("Failed to parse args");
        let (visible, remaining) = split_rows_by_limit(&args_desc, &rows);
        let table_desc = format_table(&args_desc, "repo", &rows, visible, remaining).unwrap();
        assert!(table_desc.contains("Commits ↓"));

        // Ascending arrow (reverse=true)
        let args_asc = Args::try_parse_from(["git-repostats", "--sort", "commits", "--reverse"])
            .expect("Failed to parse args");
        let (visible, remaining) = split_rows_by_limit(&args_asc, &rows);
        let table_asc = format_table(&args_asc, "repo", &rows, visible, remaining).unwrap();
        assert!(table_asc.contains("Commits ↑"));
    }

    #[test]
    fn test_format_csv_valid() {
        let rows = sample_stats();
        let args = Args::try_parse_from([
            "git-repostats",
            "--format",
            "csv",
            "--metrics",
            "commits",
            "files",
            "current",
            "history",
            "all",
            "-l",
            "2",
        ])
        .expect("Failed to parse args");

        let (visible, remaining) = split_rows_by_limit(&args, &rows);
        let csv_output =
            format_csv(&args, "csv-repo", &rows, visible, remaining).expect("Failed to format csv");

        assert!(csv_output.contains("# Repository: csv-repo"));
        assert!(csv_output.contains("# Authors: 2 of 3 shown"));
        assert!(csv_output.contains("# Limit: 2"));
        assert!(csv_output.contains("# Truncated: true"));
        assert!(
            csv_output.contains("Author,Commits,Files,Surviving,Insertions,Deletions,Net,Churn")
        );
        assert!(csv_output.contains("Alice <alice@test.com>,10,5,100,150,30,120,180"));
        assert!(csv_output.contains("Bob <bob@test.com>,5,2,50,60,10,50,70"));
    }

    #[test]
    fn test_format_json_valid() {
        let rows = sample_stats();
        let args = Args::try_parse_from([
            "git-repostats",
            "--format",
            "json",
            "--metrics",
            "commits",
            "files",
            "current",
            "history",
            "all",
            "-l",
            "2",
        ])
        .expect("Failed to parse args");

        let (visible, remaining) = split_rows_by_limit(&args, &rows);
        let json_str = format_json(&args, "json-repo", &rows, visible, remaining)
            .expect("Failed to format json");

        let v: Value = serde_json::from_str(&json_str).expect("Failed to deserialize json");
        assert_eq!(v["metadata"]["repository"], "json-repo");
        assert_eq!(v["metadata"]["total_authors"], 3);
        assert_eq!(v["metadata"]["returned_authors"], 2);
        assert_eq!(v["metadata"]["limit"], 2);
        assert_eq!(v["metadata"]["truncated"], true);

        let authors = v["authors"].as_array().expect("authors should be array");
        assert_eq!(authors.len(), 2);
        assert_eq!(authors[0]["Author"], "Alice <alice@test.com>");
        assert_eq!(authors[0]["Commits"], 10);
        assert_eq!(authors[0]["Files"], 5);
        assert_eq!(authors[0]["Surviving"], 100);
        assert_eq!(authors[0]["Insertions"], 150);
        assert_eq!(authors[0]["Deletions"], 30);
        assert_eq!(authors[0]["Net"], 120);
        assert_eq!(authors[0]["Churn"], 180);
        assert_eq!(authors[1]["Author"], "Bob <bob@test.com>");
        assert_eq!(authors[1]["Commits"], 5);
        assert_eq!(authors[1]["Files"], 2);
        assert_eq!(authors[1]["Surviving"], 50);
        assert_eq!(authors[1]["Insertions"], 60);
        assert_eq!(authors[1]["Deletions"], 10);
        assert_eq!(authors[1]["Net"], 50);
        assert_eq!(authors[1]["Churn"], 70);
    }

    #[test]
    fn test_format_yaml_valid() {
        let rows = sample_stats();
        let args = Args::try_parse_from(["git-repostats", "--format", "yaml", "-l", "2"])
            .expect("Failed to parse args");

        let (visible, remaining) = split_rows_by_limit(&args, &rows);
        let yaml_str = format_yaml(&args, "yaml-repo", &rows, visible, remaining)
            .expect("Failed to format yaml");

        assert!(yaml_str.contains("repository: yaml-repo"));
        assert!(yaml_str.contains("total_authors: 3"));
        assert!(yaml_str.contains("returned_authors: 2"));
        assert!(yaml_str.contains("Alice <alice@test.com>"));
    }

    #[test]
    fn test_format_markdown_valid() {
        let rows = sample_stats();
        let args = Args::try_parse_from(["git-repostats", "--format", "markdown", "-l", "2"])
            .expect("Failed to parse args");

        let (visible, remaining) = split_rows_by_limit(&args, &rows);
        let md = format_markdown(&args, "md-repo", &rows, visible, remaining)
            .expect("Failed to format markdown");

        assert!(md.contains("# Repository: md-repo"));
        assert!(md.contains("| Author"));
        assert!(md.contains("Alice <alice@test.com>"));
        assert!(md.contains("Bob <bob@test.com>"));
        assert!(md.contains("**OTHER (1 author)**"));
        assert!(md.contains("**TOTAL (3 authors)**"));
    }

    #[test]
    fn test_format_markdown_empty() {
        let rows: Vec<(String, AuthorStats)> = vec![];
        let args = Args::try_parse_from(["git-repostats", "--format", "markdown"])
            .expect("Failed to parse args");

        let (visible, remaining) = split_rows_by_limit(&args, &rows);
        let output = format_markdown(&args, "md-repo", &rows, visible, remaining)
            .expect("Failed to format markdown");

        assert_eq!(output, "No data to display for repository: md-repo");
    }

    #[test]
    fn test_format_repo_dispatch() {
        let rows = sample_stats();
        let formats = [
            ("table", OutputFormat::Table),
            ("json", OutputFormat::Json),
            ("csv", OutputFormat::Csv),
            ("yaml", OutputFormat::Yaml),
            ("markdown", OutputFormat::Markdown),
        ];

        for (fmt_str, _) in formats {
            let args = Args::try_parse_from(["git-repostats", "--format", fmt_str])
                .unwrap_or_else(|_| panic!("Failed to parse format {fmt_str}"));
            let res = format_repo(&args, "test-repo", rows.clone());
            assert!(res.is_ok(), "format_repo failed for {fmt_str}");
            let text = res.unwrap();
            assert!(!text.is_empty());
        }
    }
}
