use crate::error::*;

use futures_lite::io::AsyncWriteExt as _;

use super::private::TextmodeImpl as _;

pub struct ScreenGuard {
    cleaned_up: bool,
}

impl ScreenGuard {
    pub async fn cleanup(&mut self) -> Result<()> {
        if self.cleaned_up {
            return Ok(());
        }
        self.cleaned_up = true;
        write_stdout(super::DEINIT).await
    }
}

impl Drop for ScreenGuard {
    fn drop(&mut self) {
        futures_lite::future::block_on(async {
            let _ = self.cleanup().await;
        });
    }
}

pub struct Output {
    cur: vt100::Parser,
    next: vt100::Parser,
}

impl super::private::TextmodeImpl for Output {
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

impl super::Textmode for Output {}

impl Output {
    pub async fn new() -> Result<(Self, ScreenGuard)> {
        write_stdout(super::INIT).await?;

        Ok((
            Self::new_without_screen(),
            ScreenGuard { cleaned_up: false },
        ))
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

    pub async fn refresh(&mut self) -> Result<()> {
        let diffs = &[
            self.next().screen().contents_diff(self.cur().screen()),
            self.next().screen().input_mode_diff(self.cur().screen()),
            self.next().screen().title_diff(self.cur().screen()),
            self.next().screen().bells_diff(self.cur().screen()),
        ];
        for diff in diffs {
            write_stdout(&diff).await?;
            self.cur_mut().process(&diff);
        }
        Ok(())
    }
}

async fn write_stdout(buf: &[u8]) -> Result<()> {
    let mut stdout = blocking::Unblock::new(std::io::stdout());
    stdout.write_all(buf).await.map_err(Error::WriteStdout)?;
    stdout.flush().await.map_err(Error::WriteStdout)?;
    Ok(())
}
