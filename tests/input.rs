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

        r.get_mut()
            .write_all(&textmode::Key::Up.into_bytes())
            .unwrap();
        if special_keys {
            assert_eq!(
                std::string::String::from_utf8(fixtures::read_line(&mut r))
                    .unwrap(),
                "Up: [27, 91, 65]\r\n"
            );
        } else {
            if single {
                if utf8 {
                    assert_eq!(
                        std::string::String::from_utf8(fixtures::read_line(
                            &mut r
                        ))
                        .unwrap(),
                        "Byte(27): [27]\r\n"
                    );
                    assert_eq!(
                        std::string::String::from_utf8(fixtures::read_line(
                            &mut r
                        ))
                        .unwrap(),
                        "Char('['): [91]\r\n"
                    );
                    assert_eq!(
                        std::string::String::from_utf8(fixtures::read_line(
                            &mut r
                        ))
                        .unwrap(),
                        "Char('A'): [65]\r\n"
                    );
                } else {
                    assert_eq!(
                        std::string::String::from_utf8(fixtures::read_line(
                            &mut r
                        ))
                        .unwrap(),
                        "Byte(27): [27]\r\n"
                    );
                    assert_eq!(
                        std::string::String::from_utf8(fixtures::read_line(
                            &mut r
                        ))
                        .unwrap(),
                        "Byte(91): [91]\r\n"
                    );
                    assert_eq!(
                        std::string::String::from_utf8(fixtures::read_line(
                            &mut r
                        ))
                        .unwrap(),
                        "Byte(65): [65]\r\n"
                    );
                }
            } else {
                if utf8 {
                    // assert_eq!(
                    //     std::string::String::from_utf8(fixtures::read_line(
                    //         &mut r
                    //     ))
                    //     .unwrap(),
                    //     "Bytes([27]): [27]\r\n"
                    // );
                    // assert_eq!(
                    //     std::string::String::from_utf8(fixtures::read_line(
                    //         &mut r
                    //     ))
                    //     .unwrap(),
                    //     "String(\"[A\"): [91, 65]\r\n"
                    // );
                    if meta {
                        assert_eq!(
                            std::string::String::from_utf8(
                                fixtures::read_line(&mut r)
                            )
                            .unwrap(),
                            "Bytes([27]): [27]\r\n"
                        );
                        assert_eq!(
                            std::string::String::from_utf8(
                                fixtures::read_line(&mut r)
                            )
                            .unwrap(),
                            "String(\"[A\"): [91, 65]\r\n"
                        );
                    } else {
                        assert_eq!(
                            std::string::String::from_utf8(
                                fixtures::read_line(&mut r)
                            )
                            .unwrap(),
                            "Bytes([27, 91, 65]): [27, 91, 65]\r\n"
                        );
                    }
                } else {
                    if meta {
                        assert_eq!(
                            std::string::String::from_utf8(
                                fixtures::read_line(&mut r)
                            )
                            .unwrap(),
                            "Bytes([27]): [27]\r\n"
                        );
                        assert_eq!(
                            std::string::String::from_utf8(
                                fixtures::read_line(&mut r)
                            )
                            .unwrap(),
                            "Bytes([91, 65]): [91, 65]\r\n"
                        );
                    } else {
                        assert_eq!(
                            std::string::String::from_utf8(
                                fixtures::read_line(&mut r)
                            )
                            .unwrap(),
                            "Bytes([27, 91, 65]): [27, 91, 65]\r\n"
                        );
                    }
                }
            }
        }
        assert!(!fixtures::read_ready(r.get_ref().as_raw_fd()));
        assert!(r.buffer().is_empty());

        r.get_mut()
            .write_all(&textmode::Key::Ctrl(b'c').into_bytes())
            .unwrap();
        if ctrl {
            assert_eq!(
                std::string::String::from_utf8(fixtures::read_line(&mut r))
                    .unwrap(),
                "Ctrl(99): [3]\r\n"
            );
        } else {
            if single {
                assert_eq!(
                    std::string::String::from_utf8(fixtures::read_line(
                        &mut r
                    ))
                    .unwrap(),
                    "Byte(3): [3]\r\n"
                );
            } else {
                assert_eq!(
                    std::string::String::from_utf8(fixtures::read_line(
                        &mut r
                    ))
                    .unwrap(),
                    "Bytes([3]): [3]\r\n"
                );
            }
        }
        assert!(!fixtures::read_ready(r.get_ref().as_raw_fd()));
        assert!(r.buffer().is_empty());
    });
}
