use futures_lite::io::AsyncReadExt as _;
use std::os::unix::io::AsRawFd as _;

use crate::private::Input as _;

/// Switches the terminal on `stdin` to raw mode, and restores it when this
/// object goes out of scope.
pub struct RawGuard {
    termios: Option<nix::sys::termios::Termios>,
}

impl RawGuard {
    /// Switches the terminal on `stdin` to raw mode and returns a guard
    /// object. This is typically called as part of
    /// [`Input::new`](Input::new).
    ///
    /// # Errors
    /// * `Error::SetTerminalMode`: failed to put the terminal into raw mode
    pub async fn new() -> crate::error::Result<Self> {
        let stdin = std::io::stdin().as_raw_fd();
        let termios = blocking::unblock(move || {
            nix::sys::termios::tcgetattr(stdin)
                .map_err(crate::error::Error::SetTerminalMode)
        })
        .await?;
        let mut termios_raw = termios.clone();
        nix::sys::termios::cfmakeraw(&mut termios_raw);
        blocking::unblock(move || {
            nix::sys::termios::tcsetattr(
                stdin,
                nix::sys::termios::SetArg::TCSANOW,
                &termios_raw,
            )
            .map_err(crate::error::Error::SetTerminalMode)
        })
        .await?;
        Ok(Self {
            termios: Some(termios),
        })
    }

    /// Switch back from raw mode early.
    ///
    /// # Errors
    /// * `Error::SetTerminalMode`: failed to return the terminal from raw mode
    pub async fn cleanup(&mut self) -> crate::error::Result<()> {
        if let Some(termios) = self.termios.take() {
            let stdin = std::io::stdin().as_raw_fd();
            blocking::unblock(move || {
                nix::sys::termios::tcsetattr(
                    stdin,
                    nix::sys::termios::SetArg::TCSANOW,
                    &termios,
                )
                .map_err(crate::error::Error::SetTerminalMode)
            })
            .await
        } else {
            Ok(())
        }
    }
}

impl Drop for RawGuard {
    /// Calls `cleanup`. Note that this may block, due to Rust's current lack
    /// of an async drop mechanism. If this could be a problem, you should
    /// call `cleanup` manually instead.
    fn drop(&mut self) {
        futures_lite::future::block_on(async {
            // https://github.com/rust-lang/rust-clippy/issues/8003
            #[allow(clippy::let_underscore_drop)]
            let _ = self.cleanup().await;
        });
    }
}

/// Manages handling terminal input from `stdin`.
///
/// The primary interface provided is [`read_key`](Input::read_key). You can
/// additionally configure the types of keypresses you are interested in
/// through the `parse_*` methods. This configuration can be changed between
/// any two calls to [`read_key`](Input::read_key).
pub struct Input {
    stdin: blocking::Unblock<std::io::Stdin>,
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
        self.buf.get(self.pos..).unwrap()
    }

    fn buf_mut(&mut self) -> &mut [u8] {
        self.buf.get_mut(self.pos..).unwrap()
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

impl Input {
    /// Creates a new `Input` instance containing a [`RawGuard`](RawGuard)
    /// instance.
    ///
    /// # Errors
    /// * `Error::SetTerminalMode`: failed to put the terminal into raw mode
    pub async fn new() -> crate::error::Result<Self> {
        let mut self_ = Self::new_without_raw();
        self_.raw = Some(RawGuard::new().await?);
        Ok(self_)
    }

    /// Creates a new `Input` instance without creating a
    /// [`RawGuard`](RawGuard) instance.
    #[must_use]
    pub fn new_without_raw() -> Self {
        Self {
            stdin: blocking::Unblock::new(std::io::stdin()),
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

    /// Removes the [`RawGuard`](RawGuard) instance stored in this `Input`
    /// instance and returns it. This can be useful if you need to manage the
    /// lifetime of the [`RawGuard`](RawGuard) instance separately.
    pub fn take_raw_guard(&mut self) -> Option<RawGuard> {
        self.raw.take()
    }

    /// Sets whether `read_key` should try to produce
    /// [`String`](crate::Key::String) or [`Char`](crate::Key::Char) keys when
    /// possible, rather than [`Bytes`](crate::Key::Bytes) or
    /// [`Byte`](crate::Key::Byte) keys. Note that
    /// [`Bytes`](crate::Key::Bytes) or [`Byte`](crate::Key::Byte) keys may
    /// still be produced if the input fails to be parsed as UTF-8. Defaults
    /// to true.
    pub fn parse_utf8(&mut self, parse: bool) {
        self.parse_utf8 = parse;
    }

    /// Sets whether `read_key` should produce [`Ctrl`](crate::Key::Ctrl) keys
    /// when possible, rather than [`Bytes`](crate::Key::Bytes) or
    /// [`Byte`](crate::Key::Byte) keys. Defaults to true.
    pub fn parse_ctrl(&mut self, parse: bool) {
        self.parse_ctrl = parse;
    }

    /// Sets whether `read_key` should produce [`Meta`](crate::Key::Meta) keys
    /// when possible, rather than producing the
    /// [`Escape`](crate::Key::Escape) key separately. Defaults to true.
    pub fn parse_meta(&mut self, parse: bool) {
        self.parse_meta = parse;
    }

    /// Sets whether `read_key` should produce keys other than
    /// [`String`](crate::Key::String), [`Char`](crate::Key::Char),
    /// [`Bytes`](crate::Key::Bytes), [`Byte`](crate::Key::Byte),
    /// [`Ctrl`](crate::Key::Ctrl), or [`Meta`](crate::Key::Meta). Defaults to
    /// true.
    pub fn parse_special_keys(&mut self, parse: bool) {
        self.parse_special_keys = parse;
    }

    /// Sets whether `read_key` should produce individual
    /// [`Char`](crate::Key::Char) or [`Byte`](crate::Key::Byte) keys, rather
    /// than combining them into [`String`](crate::Key::String) or
    /// [`Bytes`](crate::Key::Bytes) keys when possible. When this is true,
    /// [`String`](crate::Key::String) and [`Bytes`](crate::Key::Bytes) will
    /// never be returned, and when this is false, [`Char`](crate::Key::Char)
    /// and [`Byte`](crate::Key::Byte) will never be returned. Defaults to
    /// true.
    pub fn parse_single(&mut self, parse: bool) {
        self.parse_single = parse;
    }

    /// Reads a keypress from the terminal on `stdin`. Returns `Ok(None)` on
    /// EOF.
    ///
    /// # Errors
    /// * `Error::ReadStdin`: failed to read data from stdin
    pub async fn read_key(
        &mut self,
    ) -> crate::error::Result<Option<crate::Key>> {
        self.fill_buf().await?;

        if self.parse_single {
            Ok(self.read_single_key())
        } else {
            if let Some(key) = self.try_read_string() {
                return Ok(Some(key));
            }

            if let Some(key) = self.try_read_bytes() {
                return Ok(Some(key));
            }

            if let Some(key) = self.read_single_key() {
                return Ok(Some(self.normalize_to_bytes(key)));
            }

            Ok(None)
        }
    }

    async fn fill_buf(&mut self) -> crate::error::Result<()> {
        if self.buf_is_empty() {
            self.buf.resize(4096, 0);
            self.pos = 0;
            let bytes = read_stdin(&mut self.stdin, &mut self.buf).await?;
            if bytes == 0 {
                return Ok(());
            }
            self.buf.truncate(bytes);
        }

        if self.parse_utf8 {
            let expected_bytes =
                self.expected_leading_utf8_bytes(*self.buf().get(0).unwrap());
            if self.buf.len() < self.pos + expected_bytes {
                let mut cur = self.buf.len();
                self.buf.resize(4096 + expected_bytes, 0);
                while cur < self.pos + expected_bytes {
                    let bytes = read_stdin(
                        &mut self.stdin,
                        self.buf.get_mut(cur..).unwrap(),
                    )
                    .await?;
                    if bytes == 0 {
                        return Ok(());
                    }
                    cur += bytes;
                }
                self.buf.truncate(cur);
            }
        }

        Ok(())
    }
}

async fn read_stdin(
    stdin: &mut blocking::Unblock<std::io::Stdin>,
    buf: &mut [u8],
) -> crate::error::Result<usize> {
    stdin
        .read(buf)
        .await
        .map_err(crate::error::Error::ReadStdin)
}
