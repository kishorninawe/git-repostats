use std::{
    collections::HashMap,
    io::{self, BufWriter, Write},
};
use tabled::{
    Table, Tabled,
    settings::{
        Alignment, Format, Modify, Panel, Remove, Style, object::Columns, object::Rows,
        style::HorizontalLine, themes::BorderCorrection,
    },
};

use crate::{
    cli::{Args, Limit, Metrics},
    stats::AuthorStats,
    utils::fmt_number,
};

#[derive(Tabled)]
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

fn aggregate_stats(rows: &[(String, AuthorStats)]) -> AuthorStats {
    rows.iter()
        .map(|(_, s)| s)
        .cloned()
        .reduce(|a, b| a + b)
        .unwrap()
}

pub fn render_all(args: &Args, result: HashMap<String, Vec<(String, AuthorStats)>>) {
    for (repo_name, rows) in result {
        let limit = match args.limit {
            Limit::All => rows.len(),
            Limit::Count(n) => n,
        };

        let total_rows = rows.len();

        let (visible, remaining) = if total_rows > limit {
            rows.split_at(limit)
        } else {
            (rows.as_slice(), &[][..])
        };

        let is_other_row = !remaining.is_empty();
        let mut final_rows: Vec<(String, AuthorStats)> = visible.to_vec();

        if !remaining.is_empty() {
            let others = aggregate_stats(remaining);
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
            let total = aggregate_stats(&rows);
            final_rows.push((
                format!(
                    "TOTAL ({} author{})",
                    fmt_number(&Some(total_rows)),
                    if total_rows == 1 { "" } else { "s" }
                ),
                total,
            ))
        };

        render_table(&repo_name, args, final_rows, is_other_row);
    }
}

pub fn render_table(
    repo_name: &str,
    args: &Args,
    rows: Vec<(String, AuthorStats)>,
    is_other_row: bool,
) {
    if rows.is_empty() {
        eprintln!("No data to display for repository: {}", repo_name);
        return;
    }

    let table_rows: Vec<AllRow> = rows
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

    let stdout = io::stdout().lock();
    let mut out = BufWriter::new(stdout);
    writeln!(out, "{table}").unwrap();
    out.flush().unwrap();
}
