/// Type for errors returned by this crate.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// error reading from stdin
    #[error("error reading from stdin")]
    ReadStdin(#[source] std::io::Error),

    /// error enabling terminal raw mode
    #[error("error enabling terminal raw mode")]
    SetRaw(#[source] nix::Error),

    /// error restoring terminal from raw mode
    #[error("error restoring terminal from raw mode")]
    UnsetRaw(#[source] nix::Error),

    /// error writing to stdout
    #[error("error writing to stdout")]
    WriteStdout(#[source] std::io::Error),
}

/// Convenience wrapper for a `Result` using `textmode::Error`.
pub type Result<T> = std::result::Result<T, Error>;
