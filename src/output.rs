use crate::error::*;

use futures_lite::io::AsyncWriteExt as _;

use crate::private::Output as _;

pub struct ScreenGuard {
    cleaned_up: bool,
}

impl ScreenGuard {
    pub async fn new() -> Result<Self> {
        write_stdout(
            &mut blocking::Unblock::new(std::io::stdout()),
            crate::INIT,
        )
        .await?;
        Ok(Self { cleaned_up: false })
    }

    pub async fn cleanup(&mut self) -> Result<()> {
        if self.cleaned_up {
            return Ok(());
        }
        self.cleaned_up = true;
        write_stdout(
            &mut blocking::Unblock::new(std::io::stdout()),
            crate::DEINIT,
        )
        .await
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
    stdout: blocking::Unblock<std::io::Stdout>,
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
    pub async fn new() -> Result<(Self, ScreenGuard)> {
        Ok((Self::new_without_screen(), ScreenGuard::new().await?))
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
        Self {
            stdout: blocking::Unblock::new(std::io::stdout()),
            cur,
            next,
        }
    }

    pub async fn refresh(&mut self) -> Result<()> {
        let diff = self.next().screen().state_diff(self.cur().screen());
        write_stdout(&mut self.stdout, &diff).await?;
        self.cur_mut().process(&diff);
        Ok(())
    }
}

async fn write_stdout(
    stdout: &mut blocking::Unblock<std::io::Stdout>,
    buf: &[u8],
) -> Result<()> {
    stdout.write_all(buf).await.map_err(Error::WriteStdout)?;
    stdout.flush().await.map_err(Error::WriteStdout)?;
    Ok(())
}
