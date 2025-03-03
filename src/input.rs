use tokio::io::AsyncReadExt as _;

use crate::private::Input as _;

/// Switches the terminal on `stdin` to raw mode, and restores it when this
/// object goes out of scope.
pub struct RawGuard {
    termios: Option<rustix::termios::Termios>,
}

impl RawGuard {
    /// Switches the terminal on `stdin` to raw mode and returns a guard
    /// object. This is typically called as part of
    /// [`Input::new`](Input::new).
    ///
    /// # Errors
    /// * `Error::SetTerminalMode`: failed to put the terminal into raw mode
    // spawn_blocking is uncancellable, and the tcgetattr/tcsetattr calls
    // can't panic, so unwrap is safe here
    #[allow(clippy::missing_panics_doc)]
    pub async fn new() -> crate::error::Result<Self> {
        let termios = tokio::task::spawn_blocking(move || {
            rustix::termios::tcgetattr(std::io::stdin())
                .map_err(crate::error::Error::SetTerminalMode)
        })
        .await
        .unwrap()?;
        let mut termios_raw = termios.clone();
        termios_raw.make_raw();
        tokio::task::spawn_blocking(move || {
            rustix::termios::tcsetattr(
                std::io::stdin(),
                rustix::termios::OptionalActions::Now,
                &termios_raw,
            )
            .map_err(crate::error::Error::SetTerminalMode)
        })
        .await
        .unwrap()?;
        Ok(Self {
            termios: Some(termios),
        })
    }

    /// Switch back from raw mode early.
    ///
    /// # Errors
    /// * `Error::SetTerminalMode`: failed to return the terminal from raw
    ///   mode
    // spawn_blocking is uncancellable, and the tcsetattr call can't panic, so
    // unwrap is safe here
    #[allow(clippy::missing_panics_doc)]
    pub async fn cleanup(&mut self) -> crate::error::Result<()> {
        if let Some(termios) = self.termios.take() {
            tokio::task::spawn_blocking(move || {
                rustix::termios::tcsetattr(
                    std::io::stdin(),
                    rustix::termios::OptionalActions::Now,
                    &termios,
                )
                .map_err(crate::error::Error::SetTerminalMode)
            })
            .await
            .unwrap()
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
        // doesn't literally call `cleanup`, because calling spawn_blocking
        // while the tokio runtime is in the process of shutting down doesn't
        // work (spawn_blocking tasks are cancelled if the runtime starts
        // shutting down before the task body starts running), and using
        // block_in_place/block_on doesn't work on the current_thread runtime,
        // but should be kept in sync with the actual things that `cleanup`
        // does.
        if let Some(termios) = self.termios.take() {
            let _ = rustix::termios::tcsetattr(
                std::io::stdin(),
                rustix::termios::OptionalActions::Now,
                &termios,
            );
        }
    }
}

/// Manages handling terminal input from `stdin`.
///
/// The primary interface provided is [`read_key`](Input::read_key). You can
/// additionally configure the types of keypresses you are interested in
/// through the `parse_*` methods. This configuration can be changed between
/// any two calls to [`read_key`](Input::read_key).
///
/// # Note
///
/// This is built on [`tokio::io::Stdin`], and inherits its caveats. In
/// particular, it will likely cause a hang until one more newline is received
/// when the tokio runtime shuts down. Because of this, it is generally
/// recommended to spawn a thread and use
/// [`textmode::blocking::Input`](crate::blocking::Input) instead.
pub struct Input {
    stdin: tokio::io::Stdin,
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
            stdin: tokio::io::stdin(),
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
                self.expected_leading_utf8_bytes(self.buf()[0]);
            if self.buf.len() < self.pos + expected_bytes {
                let mut cur = self.buf.len();
                self.buf.resize(4096 + expected_bytes, 0);
                while cur < self.pos + expected_bytes {
                    let bytes =
                        read_stdin(&mut self.stdin, &mut self.buf[cur..])
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
    stdin: &mut tokio::io::Stdin,
    buf: &mut [u8],
) -> crate::error::Result<usize> {
    stdin
        .read(buf)
        .await
        .map_err(crate::error::Error::ReadStdin)
}
