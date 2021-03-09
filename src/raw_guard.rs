use crate::error::*;

use std::os::unix::io::AsRawFd as _;

pub struct RawGuard {
    termios: nix::sys::termios::Termios,
    cleaned_up: bool,
}

impl RawGuard {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Result<Self> {
        let stdin = std::io::stdin().as_raw_fd();
        let termios =
            nix::sys::termios::tcgetattr(stdin).map_err(Error::SetRaw)?;
        let mut termios_raw = termios.clone();
        nix::sys::termios::cfmakeraw(&mut termios_raw);
        nix::sys::termios::tcsetattr(
            stdin,
            nix::sys::termios::SetArg::TCSANOW,
            &termios_raw,
        )
        .map_err(Error::SetRaw)?;
        Ok(Self {
            termios,
            cleaned_up: false,
        })
    }

    pub fn cleanup(&mut self) -> Result<()> {
        if self.cleaned_up {
            return Ok(());
        }
        self.cleaned_up = true;
        let stdin = std::io::stdin().as_raw_fd();
        nix::sys::termios::tcsetattr(
            stdin,
            nix::sys::termios::SetArg::TCSANOW,
            &self.termios,
        )
        .map_err(Error::UnsetRaw)
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
