pub mod color;

pub mod blocking;

#[cfg(feature = "async")]
pub mod r#async;
#[cfg(feature = "async")]
pub use r#async::Textmode;

mod private {
    pub trait TextmodeImpl {
        fn cur(&self) -> &vt100::Parser;
        fn cur_mut(&mut self) -> &mut vt100::Parser;
        fn next(&self) -> &vt100::Parser;
        fn next_mut(&mut self) -> &mut vt100::Parser;

        fn write_u16(&mut self, i: u16) {
            // unwrap is fine because vt100::Parser::write can never fail
            itoa::write(self.next_mut(), i).unwrap();
        }

        fn write_u8(&mut self, i: u8) {
            // unwrap is fine because vt100::Parser::write can never fail
            itoa::write(self.next_mut(), i).unwrap();
        }
    }
}

pub trait TextmodeExt: private::TextmodeImpl {
    fn cursor_position(&self) -> (u16, u16) {
        self.next().screen().cursor_position()
    }

    fn write(&mut self, buf: &[u8]) {
        self.next_mut().process(buf);
    }

    fn set_size(&mut self, rows: u16, cols: u16) {
        self.cur_mut().set_size(rows, cols);
        self.next_mut().set_size(rows, cols);
    }

    fn write_str(&mut self, text: &str) {
        self.write(text.as_bytes());
    }

    fn move_to(&mut self, row: u16, col: u16) {
        self.write(b"\x1b[");
        self.write_u16(row);
        self.write(b";");
        self.write_u16(col);
        self.write(b"H");
    }

    fn clear(&mut self) {
        self.write(b"\x1b[2J");
    }

    fn set_fgcolor(&mut self, color: vt100::Color) {
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

    fn set_bgcolor(&mut self, color: vt100::Color) {
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
}
