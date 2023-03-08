/// Type for errors returned by this crate.
#[derive(Debug)]
pub enum Error {
    /// error reading from stdin
    ReadStdin(std::io::Error),

    /// error setting terminal mode
    SetTerminalMode(nix::Error),

    /// error writing to stdout
    WriteStdout(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadStdin(e) => {
                write!(f, "error reading from stdin: {e}")
            }
            Self::SetTerminalMode(e) => {
                write!(f, "error setting terminal mode: {e}")
            }
            Self::WriteStdout(e) => {
                write!(f, "error writing to stdout: {e}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadStdin(e) | Self::WriteStdout(e) => Some(e),
            Self::SetTerminalMode(e) => Some(e),
        }
    }
}

/// Convenience wrapper for a `Result` using `textmode::Error`.
pub type Result<T> = std::result::Result<T, Error>;
