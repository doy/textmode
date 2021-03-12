use pty_process::Command as _;
use std::io::Read as _;
use std::os::unix::io::AsRawFd as _;

pub fn run_fixture<F>(name: &str, screenguard: bool, f: F)
where
    F: FnOnce(&mut std::fs::File),
{
    let temp = assert_fs::TempDir::new().unwrap();
    let run = escargot::CargoBuild::new()
        .bin(name)
        .current_release()
        .current_target()
        .manifest_path("tests/fixtures/bin/Cargo.toml")
        .target_dir(temp.path())
        .run()
        .unwrap();
    let mut cmd = run.command();
    let mut child = cmd
        .spawn_pty(Some(&pty_process::Size::new(24, 80)))
        .unwrap();

    if screenguard {
        assert!(read_ready(child.pty().as_raw_fd()));
        let mut buf = vec![0u8; 1024];
        let bytes = child.pty().read(&mut buf).unwrap();
        buf.truncate(bytes);
        assert_eq!(&buf[..], b"\x1b7\x1b[?47h\x1b[2J\x1b[H\x1b[?25h");
    }

    f(child.pty_mut());

    if screenguard {
        assert!(read_ready(child.pty().as_raw_fd()));
        let mut buf = vec![0u8; 1024];
        let bytes = child.pty().read(&mut buf).unwrap();
        buf.truncate(bytes);
        assert_eq!(&buf[..], b"\x1b[?47l\x1b8\x1b[?25h");
    }

    let status = child.wait().unwrap();
    assert!(status.success());
}

pub fn read(f: &mut std::fs::File) -> Vec<u8> {
    assert!(read_ready(f.as_raw_fd()));
    let mut buf = vec![0u8; 1024];
    let bytes = f.read(&mut buf).unwrap();
    buf.truncate(bytes);
    buf
}

pub fn read_ready(fd: std::os::unix::io::RawFd) -> bool {
    let mut set = nix::sys::select::FdSet::new();
    set.insert(fd);
    let timeout = libc::timeval {
        tv_sec: 0,
        tv_usec: 100_000,
    };
    let timeout = &mut nix::sys::time::TimeVal::from(timeout);
    match nix::sys::select::select(
        None,
        Some(&mut set),
        None,
        None,
        Some(timeout),
    ) {
        Ok(n) => {
            if n > 0 {
                set.contains(fd)
            } else {
                false
            }
        }
        Err(_) => false,
    }
}
