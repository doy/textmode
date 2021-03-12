use crate::error::*;

use std::io::Read as _;
use std::os::unix::io::AsRawFd as _;

use crate::private::Input as _;

pub struct RawGuard {
    termios: Option<nix::sys::termios::Termios>,
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
            termios: Some(termios),
        })
    }

    pub fn cleanup(&mut self) -> Result<()> {
        if let Some(termios) = self.termios.take() {
            let stdin = std::io::stdin().as_raw_fd();
            nix::sys::termios::tcsetattr(
                stdin,
                nix::sys::termios::SetArg::TCSANOW,
                &termios,
            )
            .map_err(Error::UnsetRaw)
        } else {
            Ok(())
        }
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

pub struct Input {
    raw: Option<RawGuard>,

    buf: Vec<u8>,
    pos: usize,

    parse_utf8: bool,
    parse_ctrl: bool,
    parse_meta: bool,
    parse_special_keys: bool,
    parse_single: bool,
}

impl crate::private::Input for Input {
    fn buf(&self) -> &[u8] {
        &self.buf[self.pos..]
    }

    fn buf_mut(&mut self) -> &mut [u8] {
        &mut self.buf[self.pos..]
    }

    fn buf_mut_vec(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }

    fn consume(&mut self, n: usize) {
        self.pos += n;
    }

    fn unconsume(&mut self, n: usize) {
        self.pos -= n;
    }

    fn buf_is_empty(&self) -> bool {
        self.pos >= self.buf.len()
    }

    fn buf_at_beginning(&self) -> bool {
        self.pos == 0
    }

    fn should_parse_utf8(&self) -> bool {
        self.parse_utf8
    }

    fn should_parse_ctrl(&self) -> bool {
        self.parse_ctrl
    }

    fn should_parse_meta(&self) -> bool {
        self.parse_meta
    }

    fn should_parse_special_keys(&self) -> bool {
        self.parse_special_keys
    }

    fn should_parse_single(&self) -> bool {
        self.parse_single
    }
}

#[allow(clippy::new_without_default)]
impl Input {
    pub fn new() -> Result<Self> {
        let mut self_ = Self::new_without_raw();
        self_.raw = Some(RawGuard::new()?);
        Ok(self_)
    }

    pub fn new_without_raw() -> Self {
        Self {
            raw: None,
            buf: Vec::with_capacity(4096),
            pos: 0,
            parse_utf8: true,
            parse_ctrl: true,
            parse_meta: true,
            parse_special_keys: true,
            parse_single: true,
        }
    }

    pub fn parse_utf8(&mut self, parse: bool) {
        self.parse_utf8 = parse;
    }

    pub fn parse_ctrl(&mut self, parse: bool) {
        self.parse_ctrl = parse;
    }

    pub fn parse_meta(&mut self, parse: bool) {
        self.parse_meta = parse;
    }

    pub fn parse_special_keys(&mut self, parse: bool) {
        self.parse_special_keys = parse;
    }

    pub fn parse_single(&mut self, parse: bool) {
        self.parse_single = parse;
    }

    pub fn take_raw_guard(&mut self) -> Option<RawGuard> {
        self.raw.take()
    }

    pub fn read_key(&mut self) -> Result<Option<crate::Key>> {
        self.fill_buf()?;

        if self.parse_single {
            self.read_single_key()
        } else {
            if let Some(s) = self.try_read_string()? {
                return Ok(Some(s));
            }

            if let Some(s) = self.try_read_bytes()? {
                return Ok(Some(s));
            }

            if let Some(key) = self.read_single_key()? {
                return Ok(Some(self.normalize_to_bytes(key)));
            }

            Ok(None)
        }
    }

    fn fill_buf(&mut self) -> Result<bool> {
        if !self.buf_is_empty() {
            return Ok(true);
        }

        self.buf.resize(4096, 0);
        self.pos = 0;
        let bytes = read_stdin(&mut self.buf)?;
        if bytes == 0 {
            return Ok(false);
        }
        self.buf.truncate(bytes);

        if self.parse_utf8 {
            let mut extra = self.find_truncated_utf8();
            if extra > 0 {
                let mut cur = self.buf.len();
                self.buf.resize(4096 + extra, 0);
                while extra > 0 {
                    let bytes = read_stdin(&mut self.buf[cur..])?;
                    if bytes == 0 {
                        return Ok(false);
                    }
                    cur += bytes;
                    extra = extra.saturating_sub(bytes);
                }
                self.buf.truncate(cur);
            }
        }

        Ok(true)
    }
}

fn read_stdin(buf: &mut [u8]) -> Result<usize> {
    std::io::stdin().read(buf).map_err(Error::ReadStdin)
}
