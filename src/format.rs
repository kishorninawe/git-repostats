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
