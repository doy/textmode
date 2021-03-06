use std::io::Write as _;

pub mod color;

pub struct Textmode {
    cur: vt100::Parser,
    next: vt100::Parser,
}

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

    pub fn cursor_position(&self) -> (u16, u16) {
        self.next.screen().cursor_position()
    }

    pub fn write(&mut self, buf: &[u8]) {
        self.next.process(buf);
    }

    pub fn refresh(&mut self) -> std::io::Result<()> {
        let diff = self.next.screen().contents_diff(self.cur.screen());
        self.write_stdout(&diff)?;
        self.cur.process(&diff);
        Ok(())
    }

    pub fn set_size(&mut self, rows: u16, cols: u16) {
        self.cur.set_size(rows, cols);
        self.next.set_size(rows, cols);
    }

    pub fn write_str(&mut self, text: &str) {
        self.write(text.as_bytes());
    }

    pub fn move_to(&mut self, row: u16, col: u16) {
        self.write(b"\x1b[");
        self.write_u16(row);
        self.write(b";");
        self.write_u16(col);
        self.write(b"H");
    }

    pub fn clear(&mut self) {
        self.write(b"\x1b[2J");
    }

    pub fn set_fgcolor(&mut self, color: vt100::Color) {
        match color {
            vt100::Color::Default => {
                self.write(b"\x1b[39m");
            }
            vt100::Color::Idx(i) => {
                if i < 8 {
                    self.write(b"\x1b[");
                    self.write_u8(30 + i);
                    self.write(b"m");
                } else {
                    self.write(b"\x1b[38;5;");
                    self.write_u8(i);
                    self.write(b"m");
                }
            }
            vt100::Color::Rgb(r, g, b) => {
                self.write(b"\x1b[38;2;");
                self.write_u8(r);
                self.write(b";");
                self.write_u8(g);
                self.write(b";");
                self.write_u8(b);
                self.write(b"m");
            }
        }
    }

    pub fn set_bgcolor(&mut self, color: vt100::Color) {
        match color {
            vt100::Color::Default => {
                self.write(b"\x1b[49m");
            }
            vt100::Color::Idx(i) => {
                if i < 8 {
                    self.write(b"\x1b[");
                    self.write_u8(40 + i);
                    self.write(b"m");
                } else {
                    self.write(b"\x1b[48;5;");
                    self.write_u8(i);
                    self.write(b"m");
                }
            }
            vt100::Color::Rgb(r, g, b) => {
                self.write(b"\x1b[48;2;");
                self.write_u8(r);
                self.write(b";");
                self.write_u8(g);
                self.write(b";");
                self.write_u8(b);
                self.write(b"m");
            }
        }
    }

    fn write_u16(&mut self, i: u16) {
        // unwrap is fine because vt100::Parser::write can never fail
        itoa::write(&mut self.next, i).unwrap();
    }

    fn write_u8(&mut self, i: u8) {
        // unwrap is fine because vt100::Parser::write can never fail
        itoa::write(&mut self.next, i).unwrap();
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
