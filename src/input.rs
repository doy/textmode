use std::io::Read as _;
use std::os::unix::io::AsRawFd as _;

#[derive(Eq, PartialEq, Debug)]
pub enum Key {
    String(String),
    Char(char),
    Bytes(Vec<u8>),
    Byte(u8),
    Ctrl(u8),
    Meta(u8),
    Backspace,
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

#[derive(Clone)]
pub struct Input {
    termios: nix::sys::termios::Termios,
    buf: Vec<u8>,
}

#[allow(clippy::new_without_default)]
impl Input {
    pub fn new() -> Self {
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
        Self {
            termios,
            buf: Vec::with_capacity(4096),
        }
    }

    pub fn read_keys(&mut self) -> std::io::Result<Option<Key>> {
        self.real_read_key(true, false)
    }

    pub fn read_keys_utf8(&mut self) -> std::io::Result<Option<Key>> {
        self.real_read_key(true, true)
    }

    pub fn read_key(&mut self) -> std::io::Result<Option<Key>> {
        self.real_read_key(false, false)
    }

    pub fn read_key_char(&mut self) -> std::io::Result<Option<Key>> {
        self.real_read_key(false, true)
    }

    pub fn cleanup(&mut self) {
        let stdin = std::io::stdin().as_raw_fd();
        let _ = nix::sys::termios::tcsetattr(
            stdin,
            nix::sys::termios::SetArg::TCSANOW,
            &self.termios,
        );
    }

    fn real_read_key(
        &mut self,
        combine: bool,
        utf8: bool,
    ) -> std::io::Result<Option<Key>> {
        match self.next_byte(true)? {
            Some(c @ 32..=126) | Some(c @ 128..=255) => {
                self.parse_text(c, combine, utf8)
            }
            Some(c @ 1..=26) => Ok(Some(Key::Ctrl(b'a' + c - 1))),
            Some(27) => self.parse_escape_sequence(),
            Some(c @ 0) | Some(c @ 28..=31) => {
                self.parse_unknown_char(c, combine)
            }
            Some(127) => Ok(Some(Key::Backspace)),
            None => Ok(None),
        }
    }

    fn parse_text(
        &mut self,
        c: u8,
        combine: bool,
        utf8: bool,
    ) -> std::io::Result<Option<Key>> {
        if combine {
            let idx = self
                .buf
                .iter()
                .take_while(|&c| {
                    (32..=126).contains(c) || (128..=255).contains(c)
                })
                .count();
            let mut rest = self.buf.split_off(idx);
            std::mem::swap(&mut self.buf, &mut rest);
            rest.insert(0, c);
            if utf8 {
                match std::string::String::from_utf8(rest) {
                    Ok(s) => Ok(Some(Key::String(s))),
                    Err(e) => Ok(Some(Key::Bytes(e.into_bytes()))),
                }
            } else {
                Ok(Some(Key::Bytes(rest)))
            }
        } else {
            if utf8 {
                self.parse_utf8_char(c)
            } else {
                Ok(Some(Key::Byte(c)))
            }
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    fn parse_unknown_char(
        &mut self,
        c: u8,
        combine: bool,
    ) -> std::io::Result<Option<Key>> {
        if combine {
            let idx = self
                .buf
                .iter()
                .take_while(|&c| *c == 0 || (28..=31).contains(c))
                .count();
            let mut rest = self.buf.split_off(idx);
            std::mem::swap(&mut self.buf, &mut rest);
            rest.insert(0, c);
            Ok(Some(Key::Bytes(rest)))
        } else {
            Ok(Some(Key::Byte(c)))
        }
    }

    fn parse_escape_sequence(&mut self) -> std::io::Result<Option<Key>> {
        let mut seen = vec![b'\x1b'];
        macro_rules! next_byte {
            () => {
                match self.next_byte(false)? {
                    Some(c) => c,
                    None => return Ok(Some(Key::Bytes(seen))),
                }
            };
        }
        enum EscapeState {
            Escape,
            CSI(Vec<u8>),
            CKM(Vec<u8>),
        }
        let mut state = EscapeState::Escape;
        loop {
            let c = next_byte!();
            seen.push(c);
            match state {
                EscapeState::Escape => match c {
                    b'[' => {
                        state = EscapeState::CSI(vec![]);
                    }
                    b'O' => {
                        state = EscapeState::CKM(vec![]);
                    }
                    _ => {
                        return Ok(Some(Key::Meta(c)));
                    }
                },
                EscapeState::CSI(ref mut param) => match c {
                    b'A' => return Ok(Some(Key::Up)),
                    b'B' => return Ok(Some(Key::Down)),
                    b'C' => return Ok(Some(Key::Right)),
                    b'D' => return Ok(Some(Key::Left)),
                    b'H' => return Ok(Some(Key::Home)),
                    b'F' => return Ok(Some(Key::End)),
                    b'0'..=b'9' => {
                        param.push(c);
                        state = EscapeState::CSI(param.to_vec());
                    }
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
                        _ => {
                            let mut seq = vec![b'\x1b', b'['];
                            seq.extend(param.iter());
                            seq.push(b'~');
                            return Ok(Some(Key::Bytes(seq)));
                        }
                    },
                    _ => {
                        let mut seq = vec![b'\x1b', b'['];
                        seq.extend(param.iter());
                        seq.push(c);
                        return Ok(Some(Key::Bytes(seq)));
                    }
                },
                EscapeState::CKM(ref mut param) => match c {
                    b'A' => return Ok(Some(Key::KeypadUp)),
                    b'B' => return Ok(Some(Key::KeypadDown)),
                    b'C' => return Ok(Some(Key::KeypadRight)),
                    b'D' => return Ok(Some(Key::KeypadLeft)),
                    b'P' => return Ok(Some(Key::F(1))),
                    b'Q' => return Ok(Some(Key::F(2))),
                    b'R' => return Ok(Some(Key::F(3))),
                    b'S' => return Ok(Some(Key::F(4))),
                    _ => {
                        let mut seq = vec![b'\x1b', b'O'];
                        seq.extend(param.iter());
                        seq.push(c);
                        return Ok(Some(Key::Bytes(seq)));
                    }
                },
            }
        }
    }

    fn parse_utf8_char(
        &mut self,
        initial: u8,
    ) -> std::io::Result<Option<Key>> {
        let mut buf = vec![initial];

        macro_rules! next_byte {
            () => {
                match self.next_byte(true)? {
                    Some(c) => {
                        if (0b1000_0000..=0b1011_1111).contains(&c) {
                            c
                        } else {
                            self.buf = buf
                                .iter()
                                .skip(1)
                                .copied()
                                .chain(self.buf.iter().copied())
                                .collect();
                            return Ok(Some(Key::Byte(initial)));
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
            _ => {
                return Ok(Some(Key::Bytes(buf)));
            }
        }
        match std::string::String::from_utf8(buf) {
            Ok(s) => Ok(Some(Key::Char(s.chars().next().unwrap()))),
            Err(e) => {
                let buf = e.into_bytes();
                self.buf = buf
                    .iter()
                    .skip(1)
                    .copied()
                    .chain(self.buf.iter().copied())
                    .collect();
                Ok(Some(Key::Byte(initial)))
            }
        }
    }

    fn next_byte(&mut self, fill: bool) -> std::io::Result<Option<u8>> {
        if self.buf.is_empty() {
            if !fill || !self.fill_buf()? {
                return Ok(None);
            }
        }
        let c = self.buf.remove(0);
        Ok(Some(c))
    }

    fn fill_buf(&mut self) -> std::io::Result<bool> {
        self.buf.resize(4096, 0);
        // can't use self.read here because the borrow checker can't tell
        // that our read implementation doesn't actually need to mutably
        // borrow self
        let bytes = std::io::stdin().read(&mut self.buf)?;
        if bytes == 0 {
            return Ok(false);
        }
        self.buf.truncate(bytes);
        Ok(true)
    }
}

impl std::io::Read for Input {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        std::io::stdin().read(buf)
    }
}

impl Drop for Input {
    fn drop(&mut self) {
        self.cleanup();
    }
}
