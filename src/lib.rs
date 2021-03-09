#![allow(clippy::collapsible_if)]

pub mod blocking;

pub mod color;
mod error;
pub use error::{Error, Result};
mod key;
pub use key::Key;
mod raw_guard;
pub use raw_guard::RawGuard;

#[cfg(feature = "async")]
mod output;
#[cfg(feature = "async")]
pub use output::{Output, ScreenGuard};
#[cfg(feature = "async")]
mod input;
#[cfg(feature = "async")]
pub use input::Input;

const INIT: &[u8] = b"\x1b7\x1b[?47h\x1b[2J\x1b[H\x1b[?25h";
const DEINIT: &[u8] = b"\x1b[?47l\x1b8\x1b[?25h";

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

pub trait Textmode: private::TextmodeImpl {
    fn screen(&self) -> &vt100::Screen {
        self.next().screen()
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
        self.write_u16(row + 1);
        self.write(b";");
        self.write_u16(col + 1);
        self.write(b"H");
    }

    fn clear(&mut self) {
        self.write(b"\x1b[2J");
    }

    fn clear_line(&mut self) {
        self.write(b"\x1b[K");
    }

    fn reset_attributes(&mut self) {
        self.write(b"\x1b[m");
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
                } else if i < 16 {
                    self.write(b"\x1b[");
                    self.write_u8(82 + i);
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
                } else if i < 16 {
                    self.write(b"\x1b[");
                    self.write_u8(92 + i);
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

    fn set_bold(&mut self, bold: bool) {
        if bold {
            self.write(b"\x1b[1m");
        } else {
            self.write(b"\x1b[22m");
        }
    }

    fn set_italic(&mut self, italic: bool) {
        if italic {
            self.write(b"\x1b[3m");
        } else {
            self.write(b"\x1b[23m");
        }
    }

    fn set_underline(&mut self, underline: bool) {
        if underline {
            self.write(b"\x1b[4m");
        } else {
            self.write(b"\x1b[24m");
        }
    }

    fn set_inverse(&mut self, inverse: bool) {
        if inverse {
            self.write(b"\x1b[7m");
        } else {
            self.write(b"\x1b[27m");
        }
    }
}
