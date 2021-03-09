#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("error reading from stdin")]
    ReadStdin(#[source] std::io::Error),

    #[error("error enabling terminal raw mode")]
    SetRaw(#[source] nix::Error),

    #[error("error restoring terminal from raw mode")]
    UnsetRaw(#[source] nix::Error),

    #[error("error writing to stdout")]
    WriteStdout(#[source] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
