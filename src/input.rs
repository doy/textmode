use crate::error::*;

use futures_lite::io::AsyncReadExt as _;

pub struct Input {
    buf: Vec<u8>,
    pos: usize,

    parse_utf8: bool,
    parse_ctrl: bool,
    parse_meta: bool,
    parse_special_keys: bool,
    parse_single: bool,
}

#[allow(clippy::new_without_default)]
impl Input {
    pub fn new() -> Result<(Self, crate::RawGuard)> {
        Ok((Self::new_without_raw(), crate::RawGuard::new()?))
    }

    pub fn new_without_raw() -> Self {
        Self {
            buf: Vec::with_capacity(4096),
            pos: 0,
            parse_utf8: true,
            parse_ctrl: true,
            parse_meta: true,
            parse_special_keys: true,
            parse_single: true,
        }
    }

    pub fn parse_utf8(&mut self, parse: bool) {
        self.parse_utf8 = parse;
    }

    pub fn parse_ctrl(&mut self, parse: bool) {
        self.parse_ctrl = parse;
    }

    pub fn parse_meta(&mut self, parse: bool) {
        self.parse_meta = parse;
    }

    pub fn parse_special_keys(&mut self, parse: bool) {
        self.parse_special_keys = parse;
    }

    pub fn parse_single(&mut self, parse: bool) {
        self.parse_single = parse;
    }

    pub async fn read_key(&mut self) -> Result<Option<crate::Key>> {
        if self.parse_single {
            self.read_single_key().await
        } else {
            self.maybe_fill_buf().await?;
            if self.parse_utf8 {
                let prefix: Vec<_> = self
                    .buf
                    .iter()
                    .copied()
                    .skip(self.pos)
                    .take_while(|&c| matches!(c, 32..=126 | 128..=255))
                    .collect();
                if !prefix.is_empty() {
                    self.pos += prefix.len();
                    match std::string::String::from_utf8(prefix) {
                        Ok(s) => return Ok(Some(crate::Key::String(s))),
                        Err(e) => {
                            return Ok(Some(crate::Key::Bytes(
                                e.into_bytes(),
                            )))
                        }
                    }
                }
            }

            let prefix: Vec<_> = self
                .buf
                .iter()
                .copied()
                .skip(self.pos)
                .take_while(|&c| match c {
                    0 => true,
                    1..=26 => !self.parse_ctrl,
                    27 => !self.parse_meta && !self.parse_special_keys,
                    28..=31 => true,
                    32..=126 => true,
                    127 => !self.parse_special_keys,
                    128..=255 => true,
                })
                .collect();
            if !prefix.is_empty() {
                self.pos += prefix.len();
                return Ok(Some(crate::Key::Bytes(prefix)));
            }

            self.read_single_key().await.map(|key| {
                if let Some(crate::Key::Byte(c)) = key {
                    Some(crate::Key::Bytes(vec![c]))
                } else {
                    key
                }
            })
        }
    }

    async fn read_single_key(&mut self) -> Result<Option<crate::Key>> {
        match self.getc(true).await? {
            Some(0) => Ok(Some(crate::Key::Byte(0))),
            Some(c @ 1..=26) => {
                if self.parse_ctrl {
                    Ok(Some(crate::Key::Ctrl(b'a' + c - 1)))
                } else {
                    Ok(Some(crate::Key::Byte(c)))
                }
            }
            Some(27) => {
                if self.parse_meta || self.parse_special_keys {
                    self.read_escape_sequence().await
                } else {
                    Ok(Some(crate::Key::Byte(27)))
                }
            }
            Some(c @ 28..=31) => Ok(Some(crate::Key::Byte(c))),
            Some(c @ 32..=126) => {
                if self.parse_utf8 {
                    Ok(Some(crate::Key::Char(c as char)))
                } else {
                    Ok(Some(crate::Key::Byte(c)))
                }
            }
            Some(127) => {
                if self.parse_special_keys {
                    Ok(Some(crate::Key::Backspace))
                } else {
                    Ok(Some(crate::Key::Byte(127)))
                }
            }
            Some(c @ 128..=255) => {
                if self.parse_utf8 {
                    self.read_utf8_char(c).await
                } else {
                    Ok(Some(crate::Key::Byte(c)))
                }
            }
            None => Ok(None),
        }
    }

    async fn read_escape_sequence(&mut self) -> Result<Option<crate::Key>> {
        let mut seen = vec![b'\x1b'];

        macro_rules! fail {
            () => {{
                for &c in seen.iter().skip(1).rev() {
                    self.ungetc(c);
                }
                if self.parse_special_keys {
                    return Ok(Some(crate::Key::Escape));
                } else {
                    return Ok(Some(crate::Key::Byte(27)));
                }
            }};
        }
        macro_rules! next_byte {
            () => {
                match self.getc(false).await? {
                    Some(c) => c,
                    None => {
                        fail!()
                    }
                }
            };
        }

        enum EscapeState {
            Escape,
            CSI(Vec<u8>),
            CKM,
        }

        let mut state = EscapeState::Escape;
        loop {
            let c = next_byte!();
            seen.push(c);
            match state {
                EscapeState::Escape => match c {
                    b'[' => {
                        if self.parse_special_keys {
                            state = EscapeState::CSI(vec![]);
                        } else {
                            fail!()
                        }
                    }
                    b'O' => {
                        if self.parse_special_keys {
                            state = EscapeState::CKM;
                        } else {
                            fail!()
                        }
                    }
                    b' '..=b'N' | b'P'..=b'Z' | b'\\'..=b'~' => {
                        if self.parse_meta {
                            return Ok(Some(crate::Key::Meta(c)));
                        } else {
                            fail!()
                        }
                    }
                    _ => fail!(),
                },
                EscapeState::CSI(ref mut param) => match c {
                    b'A' => return Ok(Some(crate::Key::Up)),
                    b'B' => return Ok(Some(crate::Key::Down)),
                    b'C' => return Ok(Some(crate::Key::Right)),
                    b'D' => return Ok(Some(crate::Key::Left)),
                    b'H' => return Ok(Some(crate::Key::Home)),
                    b'F' => return Ok(Some(crate::Key::End)),
                    b'0'..=b'9' => param.push(c),
                    b'~' => match param.as_slice() {
                        [b'2'] => return Ok(Some(crate::Key::Insert)),
                        [b'3'] => return Ok(Some(crate::Key::Delete)),
                        [b'5'] => return Ok(Some(crate::Key::PageUp)),
                        [b'6'] => return Ok(Some(crate::Key::PageDown)),
                        [b'1', b'5'] => return Ok(Some(crate::Key::F(5))),
                        [b'1', b'7'] => return Ok(Some(crate::Key::F(6))),
                        [b'1', b'8'] => return Ok(Some(crate::Key::F(7))),
                        [b'1', b'9'] => return Ok(Some(crate::Key::F(8))),
                        [b'2', b'0'] => return Ok(Some(crate::Key::F(9))),
                        [b'2', b'1'] => return Ok(Some(crate::Key::F(10))),
                        [b'2', b'3'] => return Ok(Some(crate::Key::F(11))),
                        [b'2', b'4'] => return Ok(Some(crate::Key::F(12))),
                        [b'2', b'5'] => return Ok(Some(crate::Key::F(13))),
                        [b'2', b'6'] => return Ok(Some(crate::Key::F(14))),
                        [b'2', b'8'] => return Ok(Some(crate::Key::F(15))),
                        [b'2', b'9'] => return Ok(Some(crate::Key::F(16))),
                        [b'3', b'1'] => return Ok(Some(crate::Key::F(17))),
                        [b'3', b'2'] => return Ok(Some(crate::Key::F(18))),
                        [b'3', b'3'] => return Ok(Some(crate::Key::F(19))),
                        [b'3', b'4'] => return Ok(Some(crate::Key::F(20))),
                        _ => fail!(),
                    },
                    _ => fail!(),
                },
                EscapeState::CKM => match c {
                    b'A' => return Ok(Some(crate::Key::KeypadUp)),
                    b'B' => return Ok(Some(crate::Key::KeypadDown)),
                    b'C' => return Ok(Some(crate::Key::KeypadRight)),
                    b'D' => return Ok(Some(crate::Key::KeypadLeft)),
                    b'P' => return Ok(Some(crate::Key::F(1))),
                    b'Q' => return Ok(Some(crate::Key::F(2))),
                    b'R' => return Ok(Some(crate::Key::F(3))),
                    b'S' => return Ok(Some(crate::Key::F(4))),
                    _ => fail!(),
                },
            }
        }
    }

    async fn read_utf8_char(
        &mut self,
        initial: u8,
    ) -> Result<Option<crate::Key>> {
        let mut buf = vec![initial];

        macro_rules! fail {
            () => {{
                for &c in buf.iter().skip(1).rev() {
                    self.ungetc(c);
                }
                return Ok(Some(crate::Key::Byte(initial)));
            }};
        }
        macro_rules! next_byte {
            () => {
                match self.getc(true).await? {
                    Some(c) => {
                        if (0b1000_0000..=0b1011_1111).contains(&c) {
                            c
                        } else {
                            fail!()
                        }
                    }
                    None => return Ok(None),
                }
            };
        }

        match initial {
            0b0000_0000..=0b0111_1111 => {}
            0b1100_0000..=0b1101_1111 => {
                buf.push(next_byte!());
            }
            0b1110_0000..=0b1110_1111 => {
                buf.push(next_byte!());
                buf.push(next_byte!());
            }
            0b1111_0000..=0b1111_0111 => {
                buf.push(next_byte!());
                buf.push(next_byte!());
                buf.push(next_byte!());
            }
            _ => fail!(),
        }

        match std::string::String::from_utf8(buf) {
            Ok(s) => Ok(Some(crate::Key::Char(s.chars().next().unwrap()))),
            Err(e) => {
                buf = e.into_bytes();
                fail!()
            }
        }
    }

    async fn getc(&mut self, fill: bool) -> Result<Option<u8>> {
        if fill {
            if !self.maybe_fill_buf().await? {
                return Ok(None);
            }
        } else {
            if self.buf_is_empty() {
                return Ok(None);
            }
        }
        let c = self.buf[self.pos];
        self.pos += 1;
        Ok(Some(c))
    }

    fn ungetc(&mut self, c: u8) {
        if self.pos == 0 {
            self.buf.insert(0, c);
        } else {
            self.pos -= 1;
            self.buf[self.pos] = c;
        }
    }

    async fn maybe_fill_buf(&mut self) -> Result<bool> {
        if self.buf_is_empty() {
            self.fill_buf().await
        } else {
            Ok(true)
        }
    }

    fn buf_is_empty(&self) -> bool {
        self.pos >= self.buf.len()
    }

    async fn fill_buf(&mut self) -> Result<bool> {
        self.buf.resize(4096, 0);
        self.pos = 0;
        let bytes = read_stdin(&mut self.buf).await?;
        if bytes == 0 {
            return Ok(false);
        }
        self.buf.truncate(bytes);
        Ok(true)
    }
}

async fn read_stdin(buf: &mut [u8]) -> Result<usize> {
    blocking::Unblock::new(std::io::stdin())
        .read(buf)
        .await
        .map_err(Error::ReadStdin)
}
