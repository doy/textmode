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
