#![allow(clippy::collapsible_if)]

//! `textmode` is a library for terminal interaction built on top of a real
//! terminal parsing library. It allows you to do arbitrary drawing operations
//! on an in-memory screen, and then update the visible terminal output to
//! reflect the in-memory screen via an optimized diff algorithm when you are
//! finished. Being built on a real terminal parsing library means that while
//! normal curses-like operations are available:
//!
//! ```no_run
//! use textmode::Textmode;
//! # #[cfg(feature = "async")]
//! # fn main() -> textmode::Result<()> {
//! #     futures_lite::future::block_on(async { run().await })
//! # }
//! # #[cfg(feature = "async")]
//! # async fn run() -> textmode::Result<()> {
//! let mut tm = textmode::Output::new().await?;
//! tm.clear();
//! tm.move_to(5, 5);
//! tm.set_fgcolor(textmode::color::RED);
//! tm.write_str("foo");
//! tm.refresh().await?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "async"))]
//! # fn main() -> textmode::Result<()> {
//! # let mut tm = textmode::blocking::Output::new()?;
//! # tm.clear();
//! # tm.move_to(5, 5);
//! # tm.set_fgcolor(textmode::color::RED);
//! # tm.write_str("foo");
//! # tm.refresh()?;
//! # Ok(())
//! # }
//! ```
//!
//! you can also write data containing arbitrary terminal escape codes to the
//! output and they will also do the right thing:
//!
//! ```no_run
//! # use textmode::Textmode;
//! # #[cfg(feature = "async")]
//! # fn main() -> textmode::Result<()> {
//! #     futures_lite::future::block_on(async { run().await })
//! # }
//! # #[cfg(feature = "async")]
//! # async fn run() -> textmode::Result<()> {
//! # let mut tm = textmode::Output::new().await?;
//! tm.write(b"\x1b[34m\x1b[3;9Hbar\x1b[m");
//! tm.refresh().await?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "async"))]
//! # fn main() -> textmode::Result<()> {
//! # let mut tm = textmode::blocking::Output::new()?;
//! # tm.write(b"\x1b[34m\x1b[3;9Hbar\x1b[m");
//! # tm.refresh()?;
//! # Ok(())
//! # }
//! ```
//!
//! This module is split into two main parts: [`Output`](Output) and
//! [`Input`](Input). See the documentation for those types for more details.
//! Additionally, the [`blocking`] module provides an equivalent interface
//! with blocking calls instead of async.

/// Blocking interface.
pub mod blocking;

pub mod color;
pub use vt100::Color;
mod error;
pub use error::{Error, Result};
mod key;
pub use key::Key;
mod private;

#[cfg(feature = "async")]
mod output;
#[cfg(feature = "async")]
pub use output::{Output, ScreenGuard};
#[cfg(feature = "async")]
mod input;
#[cfg(feature = "async")]
pub use input::{Input, RawGuard};

const INIT: &[u8] = b"\x1b7\x1b[?47h\x1b[2J\x1b[H\x1b[?25h";
const DEINIT: &[u8] = b"\x1b[?47l\x1b8\x1b[?25h";

/// Provides the methods used to manipulate the in-memory screen.
pub trait Textmode: private::Output {
    /// Returns the in-memory screen itself. This is the screen that will be
    /// drawn on the next call to `refresh`.
    fn screen(&self) -> &vt100::Screen {
        self.next().screen()
    }

    /// Writes a sequence of bytes, potentially containing terminal escape
    /// sequences, to the in-memory screen.
    fn write(&mut self, buf: &[u8]) {
        self.next_mut().process(buf);
    }

    /// Sets the terminal size for the in-memory screen.
    fn set_size(&mut self, rows: u16, cols: u16) {
        self.cur_mut().set_size(rows, cols);
        self.next_mut().set_size(rows, cols);
    }

    /// Writes a string of printable characters to the in-memory screen.
    fn write_str(&mut self, text: &str) {
        self.write(text.as_bytes());
    }

    /// Moves the in-memory screen's cursor.
    fn move_to(&mut self, row: u16, col: u16) {
        self.write(b"\x1b[");
        self.write_u16(row + 1);
        self.write(b";");
        self.write_u16(col + 1);
        self.write(b"H");
    }

    fn move_relative(&mut self, row_offset: i16, col_offset: i16) {
        let abs_row_offset = row_offset.unsigned_abs();
        let abs_col_offset = col_offset.unsigned_abs();
        if row_offset > 0 {
            self.write(b"\x1b[");
            self.write_u16(abs_row_offset);
            self.write(b"B")
        }
        if row_offset < 0 {
            self.write(b"\x1b[");
            self.write_u16(abs_row_offset);
            self.write(b"A")
        }
        if col_offset > 0 {
            self.write(b"\x1b[");
            self.write_u16(abs_col_offset);
            self.write(b"C")
        }
        if col_offset < 0 {
            self.write(b"\x1b[");
            self.write_u16(abs_col_offset);
            self.write(b"D")
        }
    }

    /// Clears the in-memory screen.
    fn clear(&mut self) {
        self.write(b"\x1b[2J");
    }

    /// Clears the line containing the cursor on the in-memory screen.
    fn clear_line(&mut self) {
        self.write(b"\x1b[K");
    }

    /// Clears the in-memory screen's currently active drawing attributes.
    fn reset_attributes(&mut self) {
        self.write(b"\x1b[m");
    }

    /// Sets the foreground color for subsequent drawing operations to the
    /// in-memory screen.
    fn set_fgcolor(&mut self, color: vt100::Color) {
        match color {
            vt100::Color::Default => {
                self.write(b"\x1b[39m");
            }
            vt100::Color::Idx(i) => {
                if i < 8 {
                    self.write(b"\x1b[");
                    self.write_u8(30 + i);
                } else if i < 16 {
                    self.write(b"\x1b[");
                    self.write_u8(82 + i);
                } else {
                    self.write(b"\x1b[38;5;");
                    self.write_u8(i);
                }
                self.write(b"m");
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

    /// Sets the background color for subsequent drawing operations to the
    /// in-memory screen.
    fn set_bgcolor(&mut self, color: vt100::Color) {
        match color {
            vt100::Color::Default => {
                self.write(b"\x1b[49m");
            }
            vt100::Color::Idx(i) => {
                if i < 8 {
                    self.write(b"\x1b[");
                    self.write_u8(40 + i);
                } else if i < 16 {
                    self.write(b"\x1b[");
                    self.write_u8(92 + i);
                } else {
                    self.write(b"\x1b[48;5;");
                    self.write_u8(i);
                }
                self.write(b"m");
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

    /// Sets whether subsequent text drawn to the in-memory screen should be
    /// bold.
    fn set_bold(&mut self, bold: bool) {
        if bold {
            self.write(b"\x1b[1m");
        } else {
            self.write(b"\x1b[22m");
        }
    }

    /// Sets whether subsequent text drawn to the in-memory screen should be
    /// italic.
    fn set_italic(&mut self, italic: bool) {
        if italic {
            self.write(b"\x1b[3m");
        } else {
            self.write(b"\x1b[23m");
        }
    }

    /// Sets whether subsequent text drawn to the in-memory screen should be
    /// underlined.
    fn set_underline(&mut self, underline: bool) {
        if underline {
            self.write(b"\x1b[4m");
        } else {
            self.write(b"\x1b[24m");
        }
    }

    /// Sets whether subsequent text drawn to the in-memory screen should have
    /// its colors inverted.
    fn set_inverse(&mut self, inverse: bool) {
        if inverse {
            self.write(b"\x1b[7m");
        } else {
            self.write(b"\x1b[27m");
        }
    }
}
