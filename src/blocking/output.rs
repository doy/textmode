use crate::error::*;

use std::io::Write as _;

use crate::private::TextmodeImpl as _;

pub struct ScreenGuard {
    cleaned_up: bool,
}

impl ScreenGuard {
    pub fn new() -> Result<Self> {
        write_stdout(crate::INIT)?;
        Ok(Self { cleaned_up: false })
    }

    pub fn cleanup(&mut self) -> Result<()> {
        if self.cleaned_up {
            return Ok(());
        }
        self.cleaned_up = true;
        write_stdout(crate::DEINIT)
    }
}

impl Drop for ScreenGuard {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

pub struct Output {
    cur: vt100::Parser,
    next: vt100::Parser,
}

impl crate::private::TextmodeImpl for Output {
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
    pub fn new() -> Result<(Self, ScreenGuard)> {
        Ok((Self::new_without_screen(), ScreenGuard::new()?))
    }

    pub fn new_without_screen() -> Self {
        let (rows, cols) = match terminal_size::terminal_size() {
            Some((terminal_size::Width(w), terminal_size::Height(h))) => {
                (h, w)
            }
            _ => (24, 80),
        };
        let cur = vt100::Parser::new(rows, cols, 0);
        let next = vt100::Parser::new(rows, cols, 0);

        Self { cur, next }
    }

    pub fn refresh(&mut self) -> Result<()> {
        let diff = self.next().screen().state_diff(self.cur().screen());
        write_stdout(&diff)?;
        self.cur_mut().process(&diff);
        Ok(())
    }
}

fn write_stdout(buf: &[u8]) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(buf).map_err(Error::WriteStdout)?;
    stdout.flush().map_err(Error::WriteStdout)?;
    Ok(())
}
