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

pub trait InputImpl {
    fn buf(&self) -> &[u8];
    fn buf_mut(&mut self) -> &mut [u8];
    fn buf_mut_vec(&mut self) -> &mut Vec<u8>;
    fn consume(&mut self, n: usize);
    fn unconsume(&mut self, n: usize);
    fn buf_is_empty(&self) -> bool;
    fn buf_at_beginning(&self) -> bool;

    fn should_parse_utf8(&self) -> bool;
    fn should_parse_ctrl(&self) -> bool;
    fn should_parse_meta(&self) -> bool;
    fn should_parse_special_keys(&self) -> bool;
    fn should_parse_single(&self) -> bool;

    fn try_read_string(&mut self) -> crate::Result<Option<crate::Key>> {
        if !self.should_parse_utf8() {
            return Ok(None);
        }

        let prefix: Vec<_> = self
            .buf()
            .iter()
            .copied()
            .take_while(|&c| matches!(c, 32..=126 | 128..=255))
            .collect();
        if !prefix.is_empty() {
            self.consume(prefix.len());
            match std::string::String::from_utf8(prefix) {
                Ok(s) => return Ok(Some(crate::Key::String(s))),
                Err(e) => return Ok(Some(crate::Key::Bytes(e.into_bytes()))),
            }
        }

        Ok(None)
    }

    fn try_read_bytes(&mut self) -> crate::Result<Option<crate::Key>> {
        let prefix: Vec<_> = self
            .buf()
            .iter()
            .copied()
            .take_while(|&c| match c {
                0 => true,
                1..=26 => !self.should_parse_ctrl(),
                27 => {
                    !self.should_parse_meta()
                        && !self.should_parse_special_keys()
                }
                28..=31 => true,
                32..=126 => true,
                127 => !self.should_parse_special_keys(),
                128..=255 => true,
            })
            .collect();
        if !prefix.is_empty() {
            self.consume(prefix.len());
            return Ok(Some(crate::Key::Bytes(prefix)));
        }

        Ok(None)
    }

    fn normalize_to_bytes(&self, key: crate::Key) -> crate::Key {
        if let crate::Key::Byte(c) = key {
            crate::Key::Bytes(vec![c])
        } else {
            key
        }
    }

    fn read_single_key(&mut self) -> crate::Result<Option<crate::Key>> {
        match self.getc() {
            Some(0) => Ok(Some(crate::Key::Byte(0))),
            Some(c @ 1..=26) => {
                if self.should_parse_ctrl() {
                    Ok(Some(crate::Key::Ctrl(b'a' + c - 1)))
                } else {
                    Ok(Some(crate::Key::Byte(c)))
                }
            }
            Some(27) => {
                if self.should_parse_meta()
                    || self.should_parse_special_keys()
                {
                    self.read_escape_sequence()
                } else {
                    Ok(Some(crate::Key::Byte(27)))
                }
            }
            Some(c @ 28..=31) => Ok(Some(crate::Key::Byte(c))),
            Some(c @ 32..=126) => {
                if self.should_parse_utf8() {
                    Ok(Some(crate::Key::Char(c as char)))
                } else {
                    Ok(Some(crate::Key::Byte(c)))
                }
            }
            Some(127) => {
                if self.should_parse_special_keys() {
                    Ok(Some(crate::Key::Backspace))
                } else {
                    Ok(Some(crate::Key::Byte(127)))
                }
            }
            Some(c @ 128..=255) => {
                if self.should_parse_utf8() {
                    self.read_utf8_char(c)
                } else {
                    Ok(Some(crate::Key::Byte(c)))
                }
            }
            None => Ok(None),
        }
    }

    fn read_escape_sequence(&mut self) -> crate::Result<Option<crate::Key>> {
        let mut seen = vec![b'\x1b'];

        macro_rules! fail {
            () => {{
                for &c in seen.iter().skip(1).rev() {
                    self.ungetc(c);
                }
                if self.should_parse_special_keys() {
                    return Ok(Some(crate::Key::Escape));
                } else {
                    return Ok(Some(crate::Key::Byte(27)));
                }
            }};
        }
        macro_rules! next_byte {
            () => {
                match self.getc() {
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
                        if self.should_parse_special_keys() {
                            state = EscapeState::CSI(vec![]);
                        } else {
                            fail!()
                        }
                    }
                    b'O' => {
                        if self.should_parse_special_keys() {
                            state = EscapeState::CKM;
                        } else {
                            fail!()
                        }
                    }
                    b' '..=b'N' | b'P'..=b'Z' | b'\\'..=b'~' => {
                        if self.should_parse_meta() {
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

    fn read_utf8_char(
        &mut self,
        initial: u8,
    ) -> crate::Result<Option<crate::Key>> {
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
                match self.getc() {
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
            // unwrap is fine because buf always contains at least the
            // initial character, and we have already done the parsing to
            // ensure that it contains a valid utf8 character before
            // getting here
            Ok(s) => Ok(Some(crate::Key::Char(s.chars().next().unwrap()))),
            Err(e) => {
                buf = e.into_bytes();
                fail!()
            }
        }
    }

    fn getc(&mut self) -> Option<u8> {
        if self.buf_is_empty() {
            return None;
        }
        let c = self.buf()[0];
        self.consume(1);
        Some(c)
    }

    fn ungetc(&mut self, c: u8) {
        if self.buf_at_beginning() {
            self.buf_mut_vec().insert(0, c);
        } else {
            self.unconsume(1);
            self.buf_mut()[0] = c;
        }
    }

    fn find_truncated_utf8(&self) -> usize {
        for i in 0..4 {
            match self.buf()[self.buf().len() - 1 - i] {
                0b0000_0000..=0b0111_1111 => return 0,
                0b1100_0000..=0b1101_1111 => {
                    return 1usize.saturating_sub(i);
                }
                0b1110_0000..=0b1110_1111 => {
                    return 2usize.saturating_sub(i);
                }
                0b1111_0000..=0b1111_0111 => {
                    return 3usize.saturating_sub(i);
                }
                0b1000_0000..=0b1011_1111 => {}
                _ => return 0,
            }
        }
        0
    }
}
