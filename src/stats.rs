use std::{collections::HashSet, ops::Add};

use crate::cli::Metrics;

#[derive(Default, Debug)]
pub struct AuthorHistoryStats {
    pub files: HashSet<String>,
    pub ins: usize,
    pub del: usize,
    pub net: isize,
    pub churn: usize,
}

#[derive(Default, Debug)]
pub struct AuthorCurrentStats {
    pub files: HashSet<String>,
    pub surviving: usize,
}

#[derive(Default, Debug, Clone)]
pub struct AuthorStats {
    pub commits: Option<usize>,
    pub files: Option<usize>,
    pub surviving: Option<usize>,
    pub ins: Option<usize>,
    pub del: Option<usize>,
    pub net: Option<isize>,
    pub churn: Option<usize>,
}

impl AuthorStats {
    pub fn for_metrics(metrics: &[Metrics]) -> Self {
        let mut stats = Self::default();

        for metric in metrics {
            match metric {
                Metrics::Commits => stats.commits = Some(0),
                Metrics::Files => stats.files = Some(0),
                Metrics::Current => stats.surviving = Some(0),
                Metrics::History => {
                    stats.ins = Some(0);
                    stats.del = Some(0);
                    stats.net = Some(0);
                    stats.churn = Some(0);
                }
                Metrics::All => {
                    stats.commits = Some(0);
                    stats.files = Some(0);
                    stats.surviving = Some(0);
                    stats.ins = Some(0);
                    stats.del = Some(0);
                    stats.net = Some(0);
                    stats.churn = Some(0);
                }
            }
        }

        stats
    }
}

impl Add for AuthorStats {
    type Output = AuthorStats;

    fn add(self, rhs: AuthorStats) -> AuthorStats {
        AuthorStats {
            commits: add_opt(self.commits, rhs.commits),
            files: add_opt(self.files, rhs.files),
            surviving: add_opt(self.surviving, rhs.surviving),
            ins: add_opt(self.ins, rhs.ins),
            del: add_opt(self.del, rhs.del),
            net: add_opt(self.net, rhs.net),
            churn: add_opt(self.churn, rhs.churn),
        }
    }
}

fn add_opt<T: Add<Output = T>>(a: Option<T>, b: Option<T>) -> Option<T> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x + y),
        (Some(x), None) => Some(x),
        (None, Some(y)) => Some(y),
        (None, None) => None,
    }
}
