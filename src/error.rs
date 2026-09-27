use thiserror;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("failed to execute `{command}`: {source}")]
    Io {
        command: String,
        #[source]
        source: std::io::Error,
    },

    #[error("`{command}` failed with exit code {status}: {stderr}")]
    CommandFailed {
        command: String,
        status: i32,
        stderr: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Git(#[from] GitError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
