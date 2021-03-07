use pty_process::Command as _;
use smol::io::{AsyncReadExt as _, AsyncWriteExt as _};
use std::os::unix::io::AsRawFd as _;
use textmode::TextmodeExt as _;

pub struct RawGuard {
    termios: nix::sys::termios::Termios,
}

#[allow(clippy::new_without_default)]
impl RawGuard {
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
        Self { termios }
    }
}

impl Drop for RawGuard {
    fn drop(&mut self) {
        let stdin = std::io::stdin().as_raw_fd();
        let _ = nix::sys::termios::tcsetattr(
            stdin,
            nix::sys::termios::SetArg::TCSANOW,
            &self.termios,
        );
    }
}

enum Command {
    NewWindow,
    NextWindow,
}

enum Event {
    Input(Vec<u8>),
    Output,
    WindowExit(usize),
    Command(Command),
}

struct Window {
    child: std::sync::Arc<pty_process::smol::Child>,
    vt: std::sync::Arc<smol::lock::Mutex<vt100::Parser>>,
    screen: vt100::Screen,
}

struct State {
    windows: std::collections::BTreeMap<usize, Window>,
    current_window: usize,
    next_window_id: usize,
    wevents: smol::channel::Sender<Event>,
    revents: smol::channel::Receiver<Event>,
}

impl State {
    fn new() -> Self {
        let (sender, receiver) = smol::channel::unbounded();
        Self {
            windows: std::collections::BTreeMap::new(),
            current_window: 0,
            next_window_id: 0,
            wevents: sender,
            revents: receiver,
        }
    }

    fn current_window(&self) -> &Window {
        &self.windows[&self.current_window]
    }

    fn current_window_mut(&mut self) -> &mut Window {
        self.windows.get_mut(&self.current_window).unwrap()
    }

    fn next_window(&mut self) {
        self.current_window = self
            .windows
            .keys()
            .copied()
            .cycle()
            .skip_while(|&id| id < self.current_window)
            .nth(1)
            .unwrap();
    }

    fn spawn_input_task(&self, ex: &smol::Executor<'_>) {
        let notify = self.wevents.clone();
        ex.spawn(async move {
            let mut waiting_for_command = false;
            let mut stdin = smol::Unblock::new(std::io::stdin());
            let mut buf = [0u8; 4096];
            loop {
                match stdin.read(&mut buf).await {
                    Ok(bytes) => {
                        waiting_for_command = Self::handle_input(
                            &buf[..bytes],
                            notify.clone(),
                            waiting_for_command,
                        )
                        .await;
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                        break;
                    }
                }
            }
        })
        .detach();
    }

    async fn new_window(
        &mut self,
        ex: &smol::Executor<'_>,
        notify: smol::channel::Sender<Event>,
    ) {
        let child = smol::process::Command::new("zsh")
            .spawn_pty(Some(&pty_process::Size::new(24, 80)))
            .unwrap();
        let child = std::sync::Arc::new(child);
        let vt = vt100::Parser::new(24, 80, 0);
        let screen = vt.screen().clone();
        let vt = std::sync::Arc::new(smol::lock::Mutex::new(vt));
        let id = self.next_window_id;
        self.next_window_id += 1;
        let window = Window {
            child: child.clone(),
            vt: vt.clone(),
            screen,
        };
        self.windows.insert(id, window);
        self.current_window = id;
        ex.spawn(async move {
            let mut buf = [0_u8; 4096];
            loop {
                match child.pty().read(&mut buf).await {
                    Ok(bytes) => {
                        vt.lock_arc().await.process(&buf[..bytes]);
                        notify.send(Event::Output).await.unwrap();
                    }
                    Err(e) => {
                        // EIO means that the process closed the other
                        // end of the pty
                        if e.raw_os_error() != Some(libc::EIO) {
                            eprintln!("pty read failed: {:?}", e);
                        }
                        notify.send(Event::WindowExit(id)).await.unwrap();
                        break;
                    }
                }
            }
        })
        .detach();
    }

    async fn handle_input(
        buf: &[u8],
        notify: smol::channel::Sender<Event>,
        mut waiting_for_command: bool,
    ) -> bool {
        let bytes = buf.len();
        let mut real_buf = Vec::with_capacity(bytes);
        for &c in buf {
            if waiting_for_command {
                match c {
                    // ^N
                    14 => {
                        real_buf.push(c);
                    }
                    // c
                    99 => {
                        notify
                            .send(Event::Command(Command::NewWindow))
                            .await
                            .unwrap();
                    }
                    // n
                    110 => {
                        notify
                            .send(Event::Command(Command::NextWindow))
                            .await
                            .unwrap();
                    }
                    _ => {}
                }
                waiting_for_command = false;
            } else {
                match c {
                    // ^N
                    14 => {
                        if !real_buf.is_empty() {
                            notify
                                .send(Event::Input(real_buf.clone()))
                                .await
                                .unwrap();
                            real_buf.clear();
                        }
                        waiting_for_command = true;
                    }
                    _ => {
                        real_buf.push(c);
                    }
                }
            }
        }
        if !real_buf.is_empty() {
            notify.send(Event::Input(real_buf.clone())).await.unwrap();
        }
        return waiting_for_command;
    }

    async fn redraw_current_window(&self, tm: &mut textmode::Textmode) {
        let window = self.current_window();
        tm.clear();
        tm.write(&window.vt.lock_arc().await.screen().contents_formatted());
        tm.refresh().await.unwrap();
    }

    async fn update_current_window(&mut self, tm: &mut textmode::Textmode) {
        let window = self.current_window_mut();
        let new_screen = window.vt.lock_arc().await.screen().clone();
        let diff = new_screen.contents_diff(&window.screen);
        tm.write(&diff);
        tm.refresh().await.unwrap();
        window.screen = new_screen;
    }
}

#[must_use]
struct Tmux {
    _raw: RawGuard,
    tm: textmode::Textmode,
    state: State,
}

impl Tmux {
    async fn new() -> Self {
        let _raw = RawGuard::new();
        let tm = textmode::Textmode::new().await.unwrap();
        let state = State::new();
        Self { _raw, tm, state }
    }

    async fn run(self, ex: &smol::Executor<'_>) {
        let Self {
            _raw,
            mut tm,
            mut state,
        } = self;

        state.new_window(ex, state.wevents.clone()).await;
        state.spawn_input_task(ex);

        ex.run(async {
            loop {
                match state.revents.recv().await {
                    Ok(Event::Output) => {
                        state.update_current_window(&mut tm).await;
                    }
                    Ok(Event::Input(buf)) => {
                        state
                            .current_window()
                            .child
                            .pty()
                            .write_all(&buf)
                            .await
                            .unwrap();
                    }
                    Ok(Event::WindowExit(id)) => {
                        let mut dropped_window =
                            state.windows.remove(&id).unwrap();
                        // i can get_mut because at this point the future
                        // holding the other copy of child has already been
                        // dropped
                        std::sync::Arc::get_mut(&mut dropped_window.child)
                            .unwrap()
                            .status()
                            .await
                            .unwrap();
                        if state.windows.is_empty() {
                            break;
                        }
                        if state.current_window == id {
                            state.next_window()
                        }

                        state.redraw_current_window(&mut tm).await;
                    }
                    Ok(Event::Command(c)) => match c {
                        Command::NewWindow => {
                            state
                                .new_window(&ex, state.wevents.clone())
                                .await;
                            state.redraw_current_window(&mut tm).await;
                        }
                        Command::NextWindow => {
                            state.next_window();
                            state.redraw_current_window(&mut tm).await;
                        }
                    },
                    Err(e) => {
                        eprintln!("{}", e);
                        break;
                    }
                }
            }
        })
        .await;

        tm.cleanup().await.unwrap();
    }
}

async fn async_main(ex: &smol::Executor<'_>) {
    let tmux = Tmux::new().await;
    tmux.run(&ex).await;
}

fn main() {
    let ex = smol::Executor::new();
    smol::block_on(async { async_main(&ex).await })
}
