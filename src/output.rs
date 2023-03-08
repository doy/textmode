use tokio::io::AsyncWriteExt as _;

use crate::private::Output as _;

/// Switches the terminal on `stdout` to alternate screen mode, and restores
/// it when this object goes out of scope.
pub struct ScreenGuard {
    cleaned_up: bool,
}

impl ScreenGuard {
    /// Switches the terminal on `stdout` to alternate screen mode and returns
    /// a guard object. This is typically called as part of
    /// [`Output::new`](Output::new).
    ///
    /// # Errors
    /// * `Error::WriteStdout`: failed to write initialization to stdout
    pub async fn new() -> crate::error::Result<Self> {
        write_stdout(&mut tokio::io::stdout(), crate::INIT).await?;
        Ok(Self { cleaned_up: false })
    }

    /// Switch back from alternate screen mode early.
    ///
    /// # Errors
    /// * `Error::WriteStdout`: failed to write deinitialization to stdout
    pub async fn cleanup(&mut self) -> crate::error::Result<()> {
        if self.cleaned_up {
            return Ok(());
        }
        self.cleaned_up = true;
        write_stdout(&mut tokio::io::stdout(), crate::DEINIT).await
    }
}

impl Drop for ScreenGuard {
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
        use std::io::Write as _;

        if !self.cleaned_up {
            let mut stdout = std::io::stdout();
            let _ = stdout.write_all(crate::DEINIT);
            let _ = stdout.flush();
        }
    }
}

/// Manages drawing to the terminal on `stdout`.
///
/// Most functionality is provided by the [`Textmode`](crate::Textmode) trait.
/// You should call those trait methods to draw to the in-memory screen, and
/// then call [`refresh`](Output::refresh) when you want to update the
/// terminal on `stdout`.
pub struct Output {
    stdout: tokio::io::Stdout,
    screen: Option<ScreenGuard>,

    cur: vt100::Parser,
    next: vt100::Parser,
}

impl crate::private::Output for Output {
    fn cur(&self) -> &vt100::Parser {
        &self.cur
    }

    fn cur_mut(&mut self) -> &mut vt100::Parser {
        &mut self.cur
    }

    fn next(&self) -> &vt100::Parser {
        &self.next
    }

    fn next_mut(&mut self) -> &mut vt100::Parser {
        &mut self.next
    }
}

impl crate::Textmode for Output {}

impl Output {
    /// Creates a new `Output` instance containing a
    /// [`ScreenGuard`](ScreenGuard) instance.
    ///
    /// # Errors
    /// * `Error::WriteStdout`: failed to write initialization to stdout
    pub async fn new() -> crate::error::Result<Self> {
        let mut self_ = Self::new_without_screen();
        self_.screen = Some(ScreenGuard::new().await?);
        Ok(self_)
    }

    /// Creates a new `Output` instance without creating a
    /// [`ScreenGuard`](ScreenGuard) instance.
    #[must_use]
    pub fn new_without_screen() -> Self {
        let (rows, cols) = match terminal_size::terminal_size() {
            Some((terminal_size::Width(w), terminal_size::Height(h))) => {
                (h, w)
            }
            _ => (24, 80),
        };
        let cur = vt100::Parser::new(rows, cols, 0);
        let next = vt100::Parser::new(rows, cols, 0);

        Self {
            stdout: tokio::io::stdout(),
            screen: None,
            cur,
            next,
        }
    }

    /// Removes the [`ScreenGuard`](ScreenGuard) instance stored in this
    /// `Output` instance and returns it. This can be useful if you need to
    /// manage the lifetime of the [`ScreenGuard`](ScreenGuard) instance
    /// separately.
    pub fn take_screen_guard(&mut self) -> Option<ScreenGuard> {
        self.screen.take()
    }

    /// Draws the in-memory screen to the terminal on `stdout`. This is done
    /// using a diff mechanism to only update the parts of the terminal which
    /// are different from the in-memory screen.
    ///
    /// # Errors
    /// * `Error::WriteStdout`: failed to write screen state to stdout
    pub async fn refresh(&mut self) -> crate::error::Result<()> {
        let diff = self.next().screen().state_diff(self.cur().screen());
        write_stdout(&mut self.stdout, &diff).await?;
        self.cur_mut().process(&diff);
        Ok(())
    }

    /// Draws the in-memory screen to the terminal on `stdout`. This clears
    /// the screen and redraws it from scratch, rather than using a diff
    /// mechanism like `refresh`. This can be useful when the current state of
    /// the terminal screen is unknown, such as after the terminal has been
    /// resized.
    ///
    /// # Errors
    /// * `Error::WriteStdout`: failed to write screen state to stdout
    pub async fn hard_refresh(&mut self) -> crate::error::Result<()> {
        let contents = self.next().screen().state_formatted();
        write_stdout(&mut self.stdout, &contents).await?;
        self.cur_mut().process(&contents);
        Ok(())
    }
}

async fn write_stdout(
    stdout: &mut tokio::io::Stdout,
    buf: &[u8],
) -> crate::error::Result<()> {
    stdout
        .write_all(buf)
        .await
        .map_err(crate::error::Error::WriteStdout)?;
    stdout
        .flush()
        .await
        .map_err(crate::error::Error::WriteStdout)?;
    Ok(())
}
