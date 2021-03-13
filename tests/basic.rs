use std::io::Write as _;
use std::os::unix::io::AsRawFd as _;

mod fixtures;

#[test]
fn test_basic() {
    let fixture = fixtures::Fixture::new("basic");
    fixture.build().run(&[], |pty| {
        pty.write_all(b"a").unwrap();
        assert_eq!(fixtures::read(pty), b"\x1b[6;6Hfoo");

        pty.write_all(b"a").unwrap();
        assert!(!fixtures::read_ready(pty.as_raw_fd()));

        pty.write_all(b"a").unwrap();
        assert_eq!(
            fixtures::read(pty),
            b"\x1b[9;9H\x1b[32mbar\x1b[12;12H\x1b[mbaz"
        );

        pty.write_all(b"a").unwrap();
    });
}

#[test]
fn test_async() {
    let mut fixture = fixtures::Fixture::new("basic");
    fixture.features("async");
    fixture.build().run(&[], |pty| {
        pty.write_all(b"a").unwrap();
        assert_eq!(fixtures::read(pty), b"\x1b[6;6Hfoo");

        pty.write_all(b"a").unwrap();
        assert!(!fixtures::read_ready(pty.as_raw_fd()));

        pty.write_all(b"a").unwrap();
        assert_eq!(
            fixtures::read(pty),
            b"\x1b[9;9H\x1b[32mbar\x1b[12;12H\x1b[mbaz"
        );

        pty.write_all(b"a").unwrap();
    });
}
