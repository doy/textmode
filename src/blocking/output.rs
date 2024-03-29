use std::io::Write as _;

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
    pub fn new() -> crate::error::Result<Self> {
        write_stdout(crate::INIT)?;
        Ok(Self { cleaned_up: false })
    }

    /// Switch back from alternate screen mode early.
    ///
    /// # Errors
    /// * `Error::WriteStdout`: failed to write deinitialization to stdout
    pub fn cleanup(&mut self) -> crate::error::Result<()> {
        if self.cleaned_up {
            return Ok(());
        }
        self.cleaned_up = true;
        write_stdout(crate::DEINIT)
    }
}

impl Drop for ScreenGuard {
    /// Calls `cleanup`.
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// Manages drawing to the terminal on `stdout`.
///
/// Most functionality is provided by the [`Textmode`](crate::Textmode) trait.
/// You should call those trait methods to draw to the in-memory screen, and
/// then call [`refresh`](Output::refresh) when you want to update the
/// terminal on `stdout`.
pub struct Output {
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
    pub fn new() -> crate::error::Result<Self> {
        let mut self_ = Self::new_without_screen();
        self_.screen = Some(ScreenGuard::new()?);
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
    pub fn refresh(&mut self) -> crate::error::Result<()> {
        let diff = self.next().screen().state_diff(self.cur().screen());
        write_stdout(&diff)?;
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
    pub fn hard_refresh(&mut self) -> crate::error::Result<()> {
        let contents = self.next().screen().state_formatted();
        write_stdout(&contents)?;
        self.cur_mut().process(&contents);
        Ok(())
    }
}

fn write_stdout(buf: &[u8]) -> crate::error::Result<()> {
    let mut stdout = std::io::stdout();
    stdout
        .write_all(buf)
        .map_err(crate::error::Error::WriteStdout)?;
    stdout.flush().map_err(crate::error::Error::WriteStdout)?;
    Ok(())
}
