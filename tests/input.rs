#![allow(clippy::collapsible_if)]

use std::io::Write as _;
use std::os::unix::io::AsRawFd as _;

mod fixtures;

#[test]
fn test_basic() {
    let mut fixture = fixtures::Fixture::new("input");
    fixture.screenguard(false);
    let mut run = fixture.build();

    for utf8 in &[true, false] {
        for ctrl in &[true, false] {
            for meta in &[true, false] {
                for special_keys in &[true, false] {
                    for single in &[true, false] {
                        run_input_test(
                            &mut run,
                            *utf8,
                            *ctrl,
                            *meta,
                            *special_keys,
                            *single,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn test_async() {
    let mut fixture = fixtures::Fixture::new("input");
    fixture.screenguard(false);
    fixture.features("async");
    let mut run = fixture.build();

    for utf8 in &[true, false] {
        for ctrl in &[true, false] {
            for meta in &[true, false] {
                for special_keys in &[true, false] {
                    for single in &[true, false] {
                        run_input_test(
                            &mut run,
                            *utf8,
                            *ctrl,
                            *meta,
                            *special_keys,
                            *single,
                        );
                    }
                }
            }
        }
    }
}

fn run_input_test(
    fixture: &mut fixtures::BuiltFixture,
    utf8: bool,
    ctrl: bool,
    meta: bool,
    special_keys: bool,
    single: bool,
) {
    let mut args = vec![];
    if !utf8 {
        args.push("--disable-utf8")
    }
    if !ctrl {
        args.push("--disable-ctrl")
    }
    if !meta {
        args.push("--disable-meta")
    }
    if !special_keys {
        args.push("--disable-special-keys")
    }
    if !single {
        args.push("--disable-single")
    }

    fixture.run(&args, |pty| {
        let mut r = std::io::BufReader::new(pty);

        assert_no_more_lines(&mut r);

        write(r.get_mut(), textmode::Key::Up);
        if special_keys {
            assert_line(&mut r, "Up: [27, 91, 65]");
        } else {
            if single {
                if utf8 {
                    assert_line(&mut r, "Byte(27): [27]");
                    assert_line(&mut r, "Char('['): [91]");
                    assert_line(&mut r, "Char('A'): [65]");
                } else {
                    assert_line(&mut r, "Byte(27): [27]");
                    assert_line(&mut r, "Byte(91): [91]");
                    assert_line(&mut r, "Byte(65): [65]");
                }
            } else {
                if utf8 {
                    assert_line(&mut r, "Bytes([27]): [27]");
                    assert_line(&mut r, "String(\"[A\"): [91, 65]");
                } else {
                    // TODO: ideally this wouldn't make a difference
                    if meta {
                        assert_line(&mut r, "Bytes([27]): [27]");
                        assert_line(&mut r, "Bytes([91, 65]): [91, 65]");
                    } else {
                        assert_line(
                            &mut r,
                            "Bytes([27, 91, 65]): [27, 91, 65]",
                        );
                    }
                }
            }
        }
        assert_no_more_lines(&mut r);

        write(r.get_mut(), textmode::Key::Meta(b'c'));
        if meta {
            assert_line(&mut r, "Meta(99): [27, 99]");
        } else {
            if special_keys {
                assert_line(&mut r, "Escape: [27]");
                if utf8 {
                    if single {
                        assert_line(&mut r, "Char('c'): [99]");
                    } else {
                        assert_line(&mut r, "String(\"c\"): [99]");
                    }
                } else {
                    if single {
                        assert_line(&mut r, "Byte(99): [99]");
                    } else {
                        assert_line(&mut r, "Bytes([99]): [99]");
                    }
                }
            } else {
                if single {
                    assert_line(&mut r, "Byte(27): [27]");
                    if utf8 {
                        assert_line(&mut r, "Char('c'): [99]");
                    } else {
                        assert_line(&mut r, "Byte(99): [99]");
                    }
                } else {
                    if utf8 {
                        assert_line(&mut r, "Bytes([27]): [27]");
                        assert_line(&mut r, "String(\"c\"): [99]");
                    } else {
                        assert_line(&mut r, "Bytes([27, 99]): [27, 99]");
                    }
                }
            }
        }
        assert_no_more_lines(&mut r);

        write(r.get_mut(), textmode::Key::String("foo".to_string()));
        if single {
            if utf8 {
                assert_line(&mut r, "Char('f'): [102]");
                assert_line(&mut r, "Char('o'): [111]");
                assert_line(&mut r, "Char('o'): [111]");
            } else {
                assert_line(&mut r, "Byte(102): [102]");
                assert_line(&mut r, "Byte(111): [111]");
                assert_line(&mut r, "Byte(111): [111]");
            }
        } else {
            if utf8 {
                assert_line(&mut r, "String(\"foo\"): [102, 111, 111]");
            } else {
                assert_line(
                    &mut r,
                    "Bytes([102, 111, 111]): [102, 111, 111]",
                );
            }
        }
        assert_no_more_lines(&mut r);

        write(r.get_mut(), textmode::Key::Ctrl(b'c'));
        if ctrl {
            assert_line(&mut r, "Ctrl(99): [3]");
        } else {
            if single {
                assert_line(&mut r, "Byte(3): [3]");
            } else {
                assert_line(&mut r, "Bytes([3]): [3]");
            }
        }
        assert_no_more_lines(&mut r);
    });
}

#[track_caller]
fn write(f: &mut std::fs::File, key: textmode::Key) {
    f.write_all(&key.into_bytes()).unwrap();
}

#[track_caller]
fn read(f: &mut std::io::BufReader<&mut std::fs::File>) -> String {
    std::string::String::from_utf8(fixtures::read_line(f)).unwrap()
}

#[track_caller]
fn assert_line(
    f: &mut std::io::BufReader<&mut std::fs::File>,
    expected: &str,
) {
    assert_eq!(read(f), format!("{}\r\n", expected));
}

#[track_caller]
fn assert_no_more_lines(f: &mut std::io::BufReader<&mut std::fs::File>) {
    if fixtures::read_ready(f.get_ref().as_raw_fd()) || !f.buffer().is_empty()
    {
        use std::io::Read as _;
        let mut buf = vec![0; 4096];
        let bytes = f.read(&mut buf).unwrap();
        buf.truncate(bytes);
        panic!(
            "got bytes: \"{}\"({:?})",
            std::string::String::from_utf8_lossy(&buf),
            buf
        );
    }
}
