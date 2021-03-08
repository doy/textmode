use std::io::Read as _;
use std::os::unix::io::AsRawFd as _;

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum Key {
    String(String),
    Char(char),
    Bytes(Vec<u8>),
    Byte(u8),
    Ctrl(u8),
    Meta(u8),
    Backspace,
    Escape,
    Up,
    Down,
    Right,
    Left,
    KeypadUp,
    KeypadDown,
    KeypadRight,
    KeypadLeft,
    Home,
    End,
    Insert,
    Delete,
    PageUp,
    PageDown,
    F(u8),
}

impl Key {
    pub fn into_bytes(self) -> Vec<u8> {
        use Key::*;
        match self {
            String(s) => s.into_bytes(),
            Char(c) => c.to_string().into_bytes(),
            Bytes(s) => s,
            Byte(c) => vec![c],
            Ctrl(c) => vec![c - b'a' + 1],
            Meta(c) => vec![b'\x1b', c],
            Backspace => b"\x7f".to_vec(),
            Escape => b"\x1b".to_vec(),
            Up => b"\x1b[A".to_vec(),
            Down => b"\x1b[B".to_vec(),
            Right => b"\x1b[C".to_vec(),
            Left => b"\x1b[D".to_vec(),
            KeypadUp => b"\x1bOA".to_vec(),
            KeypadDown => b"\x1bOB".to_vec(),
            KeypadRight => b"\x1bOC".to_vec(),
            KeypadLeft => b"\x1bOD".to_vec(),
            Home => b"\x1b[H".to_vec(),
            End => b"\x1b[F".to_vec(),
            Insert => b"\x1b[2~".to_vec(),
            Delete => b"\x1b[3~".to_vec(),
            PageUp => b"\x1b[5~".to_vec(),
            PageDown => b"\x1b[6~".to_vec(),
            F(c) => match c {
                1 => b"\x1bOP".to_vec(),
                2 => b"\x1bOQ".to_vec(),
                3 => b"\x1bOR".to_vec(),
                4 => b"\x1bOS".to_vec(),
                5 => b"\x1b[15~".to_vec(),
                6 => b"\x1b[17~".to_vec(),
                7 => b"\x1b[18~".to_vec(),
                8 => b"\x1b[19~".to_vec(),
                9 => b"\x1b[20~".to_vec(),
                10 => b"\x1b[21~".to_vec(),
                11 => b"\x1b[23~".to_vec(),
                12 => b"\x1b[24~".to_vec(),
                13 => b"\x1b[25~".to_vec(),
                14 => b"\x1b[26~".to_vec(),
                15 => b"\x1b[28~".to_vec(),
                16 => b"\x1b[29~".to_vec(),
                17 => b"\x1b[31~".to_vec(),
                18 => b"\x1b[32~".to_vec(),
                19 => b"\x1b[33~".to_vec(),
                20 => b"\x1b[34~".to_vec(),
                _ => vec![],
            },
        }
    }
}

pub struct RawGuard {
    termios: nix::sys::termios::Termios,
    cleaned_up: bool,
}

impl RawGuard {
    pub fn cleanup(&mut self) {
        if self.cleaned_up {
            return;
        }
        self.cleaned_up = true;
        let stdin = std::io::stdin().as_raw_fd();
        let _ = nix::sys::termios::tcsetattr(
            stdin,
            nix::sys::termios::SetArg::TCSANOW,
            &self.termios,
        );
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        self.cleanup();
    }
}

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
    pub fn new() -> (Self, RawGuard) {
        let stdin = std::io::stdin().as_raw_fd();
        let termios = nix::sys::termios::tcgetattr(stdin).unwrap();
        let mut termios_raw = termios.clone();
        nix::sys::termios::cfmakeraw(&mut termios_raw);
        nix::sys::termios::tcsetattr(
            stdin,
            nix::sys::termios::SetArg::TCSANOW,
            &termios_raw,
        )
        .unwrap();
        (
            Self::new_without_raw(),
            RawGuard {
                termios,
                cleaned_up: false,
            },
        )
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

    pub fn read_key(&mut self) -> std::io::Result<Option<Key>> {
        if self.parse_single {
            self.read_single_key()
        } else {
            self.maybe_fill_buf()?;
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
                        Ok(s) => return Ok(Some(Key::String(s))),
                        Err(e) => {
                            return Ok(Some(Key::Bytes(e.into_bytes())))
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
                return Ok(Some(Key::Bytes(prefix)));
            }

            self.read_single_key().map(|key| {
                if let Some(Key::Byte(c)) = key {
                    Some(Key::Bytes(vec![c]))
                } else {
                    key
                }
            })
        }
    }

    fn read_single_key(&mut self) -> std::io::Result<Option<Key>> {
        match self.getc(true)? {
            Some(0) => Ok(Some(Key::Byte(0))),
            Some(c @ 1..=26) => {
                if self.parse_ctrl {
                    Ok(Some(Key::Ctrl(b'a' + c - 1)))
                } else {
                    Ok(Some(Key::Byte(c)))
                }
            }
            Some(27) => {
                if self.parse_meta || self.parse_special_keys {
                    self.read_escape_sequence()
                } else {
                    Ok(Some(Key::Byte(27)))
                }
            }
            Some(c @ 28..=31) => Ok(Some(Key::Byte(c))),
            Some(c @ 32..=126) => {
                if self.parse_utf8 {
                    Ok(Some(Key::Char(c as char)))
                } else {
                    Ok(Some(Key::Byte(c)))
                }
            }
            Some(127) => {
                if self.parse_special_keys {
                    Ok(Some(Key::Backspace))
                } else {
                    Ok(Some(Key::Byte(127)))
                }
            }
            Some(c @ 128..=255) => {
                if self.parse_utf8 {
                    self.read_utf8_char(c)
                } else {
                    Ok(Some(Key::Byte(c)))
                }
            }
            None => Ok(None),
        }
    }

    fn read_escape_sequence(&mut self) -> std::io::Result<Option<Key>> {
        let mut seen = vec![b'\x1b'];

        macro_rules! fail {
            () => {{
                for &c in seen.iter().skip(1).rev() {
                    self.ungetc(c);
                }
                if self.parse_special_keys {
                    return Ok(Some(Key::Escape));
                } else {
                    return Ok(Some(Key::Byte(27)));
                }
            }};
        }
        macro_rules! next_byte {
            () => {
                match self.getc(false)? {
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
                            return Ok(Some(Key::Meta(c)));
                        } else {
                            fail!()
                        }
                    }
                    _ => fail!(),
                },
                EscapeState::CSI(ref mut param) => match c {
                    b'A' => return Ok(Some(Key::Up)),
                    b'B' => return Ok(Some(Key::Down)),
                    b'C' => return Ok(Some(Key::Right)),
                    b'D' => return Ok(Some(Key::Left)),
                    b'H' => return Ok(Some(Key::Home)),
                    b'F' => return Ok(Some(Key::End)),
                    b'0'..=b'9' => param.push(c),
                    b'~' => match param.as_slice() {
                        [b'2'] => return Ok(Some(Key::Insert)),
                        [b'3'] => return Ok(Some(Key::Delete)),
                        [b'5'] => return Ok(Some(Key::PageUp)),
                        [b'6'] => return Ok(Some(Key::PageDown)),
                        [b'1', b'5'] => return Ok(Some(Key::F(5))),
                        [b'1', b'7'] => return Ok(Some(Key::F(6))),
                        [b'1', b'8'] => return Ok(Some(Key::F(7))),
                        [b'1', b'9'] => return Ok(Some(Key::F(8))),
                        [b'2', b'0'] => return Ok(Some(Key::F(9))),
                        [b'2', b'1'] => return Ok(Some(Key::F(10))),
                        [b'2', b'3'] => return Ok(Some(Key::F(11))),
                        [b'2', b'4'] => return Ok(Some(Key::F(12))),
                        [b'2', b'5'] => return Ok(Some(Key::F(13))),
                        [b'2', b'6'] => return Ok(Some(Key::F(14))),
                        [b'2', b'8'] => return Ok(Some(Key::F(15))),
                        [b'2', b'9'] => return Ok(Some(Key::F(16))),
                        [b'3', b'1'] => return Ok(Some(Key::F(17))),
                        [b'3', b'2'] => return Ok(Some(Key::F(18))),
                        [b'3', b'3'] => return Ok(Some(Key::F(19))),
                        [b'3', b'4'] => return Ok(Some(Key::F(20))),
                        _ => fail!(),
                    },
                    _ => fail!(),
                },
                EscapeState::CKM => match c {
                    b'A' => return Ok(Some(Key::KeypadUp)),
                    b'B' => return Ok(Some(Key::KeypadDown)),
                    b'C' => return Ok(Some(Key::KeypadRight)),
                    b'D' => return Ok(Some(Key::KeypadLeft)),
                    b'P' => return Ok(Some(Key::F(1))),
                    b'Q' => return Ok(Some(Key::F(2))),
                    b'R' => return Ok(Some(Key::F(3))),
                    b'S' => return Ok(Some(Key::F(4))),
                    _ => fail!(),
                },
            }
        }
    }

    fn read_utf8_char(
        &mut self,
        initial: u8,
    ) -> std::io::Result<Option<Key>> {
        let mut buf = vec![initial];

        macro_rules! fail {
            () => {{
                for &c in buf.iter().skip(1).rev() {
                    self.ungetc(c);
                }
                return Ok(Some(Key::Byte(initial)));
            }};
        }
        macro_rules! next_byte {
            () => {
                match self.getc(true)? {
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
            Ok(s) => Ok(Some(Key::Char(s.chars().next().unwrap()))),
            Err(e) => {
                buf = e.into_bytes();
                fail!()
            }
        }
    }

    fn getc(&mut self, fill: bool) -> std::io::Result<Option<u8>> {
        if fill {
            if !self.maybe_fill_buf()? {
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

    fn maybe_fill_buf(&mut self) -> std::io::Result<bool> {
        if self.buf_is_empty() {
            self.fill_buf()
        } else {
            Ok(true)
        }
    }

    fn buf_is_empty(&self) -> bool {
        self.pos >= self.buf.len()
    }

    fn fill_buf(&mut self) -> std::io::Result<bool> {
        self.buf.resize(4096, 0);
        self.pos = 0;
        let bytes = read_stdin(&mut self.buf)?;
        if bytes == 0 {
            return Ok(false);
        }
        self.buf.truncate(bytes);
        Ok(true)
    }
}

fn read_stdin(buf: &mut [u8]) -> std::io::Result<usize> {
    std::io::stdin().read(buf)
}
