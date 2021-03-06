use std::io::Write as _;

use super::private::TextmodeImpl as _;

pub struct Textmode {
    cur: vt100::Parser,
    next: vt100::Parser,
}

impl super::private::TextmodeImpl for Textmode {
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

impl super::Textmode for Textmode {}

impl Textmode {
    pub fn new() -> std::io::Result<Self> {
        let (rows, cols) = match terminal_size::terminal_size() {
            Some((terminal_size::Width(w), terminal_size::Height(h))) => {
                (h, w)
            }
            _ => (24, 80),
        };
        let cur = vt100::Parser::new(rows, cols, 0);
        let next = vt100::Parser::new(rows, cols, 0);

        let self_ = Self { cur, next };
        self_.write_stdout(b"\x1b7\x1b[?47h\x1b[2J\x1b[H\x1b[?25h")?;
        Ok(self_)
    }

    pub fn refresh(&mut self) -> std::io::Result<()> {
        let diff = self.next().screen().contents_diff(self.cur().screen());
        self.write_stdout(&diff)?;
        self.cur_mut().process(&diff);
        Ok(())
    }

    fn write_stdout(&self, buf: &[u8]) -> std::io::Result<()> {
        let mut stdout = std::io::stdout();
        stdout.write_all(buf)?;
        stdout.flush()?;
        Ok(())
    }
}

impl Drop for Textmode {
    fn drop(&mut self) {
        let _ = self.write_stdout(b"\x1b[?47l\x1b8\x1b[?25h");
    }
}
