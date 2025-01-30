use std::{
    io::{BufRead as _, Read as _},
    os::fd::AsFd as _,
};

pub struct Fixture {
    name: String,
    features: String,
    screenguard: bool,

    tempdir: assert_fs::TempDir,
}

impl Fixture {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            features: String::new(),
            screenguard: true,
            tempdir: assert_fs::TempDir::new().unwrap(),
        }
    }

    #[allow(dead_code)]
    pub fn features(&mut self, features: &str) {
        self.features = features.to_string();
    }

    #[allow(dead_code)]
    pub fn screenguard(&mut self, screenguard: bool) {
        self.screenguard = screenguard;
    }

    pub fn build(self) -> BuiltFixture {
        let Self {
            name,
            features,
            screenguard,
            tempdir,
        } = self;
        let run = escargot::CargoBuild::new()
            .bin(name)
            .current_release()
            .current_target()
            .manifest_path("tests/fixtures/bin/Cargo.toml")
            .target_dir(tempdir.path())
            .features(features)
            .run()
            .unwrap();

        BuiltFixture {
            _tempdir: tempdir,
            run,
            screenguard,
        }
    }
}

pub struct BuiltFixture {
    _tempdir: assert_fs::TempDir,
    run: escargot::CargoRun,
    screenguard: bool,
}

impl BuiltFixture {
    pub fn run<F: FnOnce(&mut pty_process::blocking::Pty)>(
        &mut self,
        args: &[&str],
        f: F,
    ) {
        let (mut pty, pts) = pty_process::blocking::open().unwrap();
        let _pts_clone = pts.as_fd().try_clone_to_owned().unwrap();
        pty.resize(pty_process::Size::new(24, 80)).unwrap();
        let cmd =
            pty_process::blocking::Command::new(self.run.path()).args(args);
        let mut child = cmd.spawn(pts).unwrap();

        if self.screenguard {
            assert!(read_ready(&pty));
            let mut buf = vec![0u8; 1024];
            let bytes = pty.read(&mut buf).unwrap();
            buf.truncate(bytes);
            assert_eq!(&buf[..], b"\x1b7\x1b[?47h\x1b[2J\x1b[H\x1b[?25h");
        } else {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        f(&mut pty);

        if self.screenguard {
            assert!(read_ready(&pty));
            let mut buf = vec![0u8; 1024];
            let bytes = pty.read(&mut buf).unwrap();
            buf.truncate(bytes);
            assert_eq!(&buf[..], b"\x1b[?47l\x1b8\x1b[?25h");
        }

        let status = child.wait().unwrap();
        assert!(status.success());
    }
}

#[allow(dead_code)]
#[track_caller]
pub fn read(f: &mut pty_process::blocking::Pty) -> Vec<u8> {
    assert!(read_ready(&f));
    let mut buf = vec![0u8; 1024];
    let bytes = f.read(&mut buf).unwrap();
    buf.truncate(bytes);
    buf
}

#[allow(dead_code)]
#[track_caller]
pub fn read_line(
    f: &mut std::io::BufReader<&mut pty_process::blocking::Pty>,
) -> Vec<u8> {
    assert!(!f.buffer().is_empty() || read_ready(f.get_ref()));
    let mut buf = vec![];
    f.read_until(b'\n', &mut buf).unwrap();
    buf
}

#[allow(dead_code)]
pub fn read_ready<Fd: std::os::fd::AsFd>(fd: Fd) -> bool {
    let mut set = nix::sys::select::FdSet::new();
    let raw_fd = fd.as_fd();
    set.insert(raw_fd);
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
                set.contains(raw_fd)
            } else {
                false
            }
        }
        Err(_) => false,
    }
}
