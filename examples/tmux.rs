use pty_process::Command as _;
use smol::io::{AsyncReadExt as _, AsyncWriteExt as _};
use textmode::Textmode as _;

enum Command {
    NewWindow,
    NextWindow,
}

enum Event {
    Input(textmode::Key),
    Output,
    WindowExit(usize),
    Command(Command),
    Notification,
}

struct Window {
    child: std::sync::Arc<pty_process::smol::Child>,
    vt: std::sync::Arc<smol::lock::Mutex<vt100::Parser>>,
    screen: vt100::Screen,
}

#[derive(Clone)]
struct Notification {
    text: String,
    expiry: std::time::Instant,
}

struct State {
    windows: std::collections::BTreeMap<usize, Window>,
    current_window: usize,
    next_window_id: usize,
    notifications: std::collections::BTreeMap<usize, Notification>,
    next_notification_id: usize,
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
            notifications: std::collections::BTreeMap::new(),
            next_notification_id: 0,
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

    fn next_window(&mut self, ex: &smol::Executor<'_>) {
        self.current_window = self
            .windows
            .keys()
            .copied()
            .cycle()
            .skip_while(|&id| id < self.current_window)
            .nth(1)
            .unwrap();
        self.notify(
            ex,
            &format!("switched to window {}", self.current_window),
        );
    }

    fn notify(&mut self, ex: &smol::Executor<'_>, text: &str) {
        let now = std::time::Instant::now();
        let expiry = now + std::time::Duration::from_secs(5);
        let text = text.to_string();
        let notification = Notification { text, expiry };
        let id = self.next_notification_id;
        self.next_notification_id += 1;
        self.notifications.insert(id, notification);
        let notify = self.wevents.clone();
        ex.spawn(async move {
            smol::Timer::at(expiry).await;
            notify.send(Event::Notification).await.unwrap();
        })
        .detach();
    }

    fn spawn_input_task(
        &self,
        ex: &smol::Executor<'_>,
        mut input: textmode::Input,
    ) {
        let notify = self.wevents.clone();
        ex.spawn(async move {
            let mut waiting_for_command = false;
            input.parse_utf8(false);
            input.parse_meta(false);
            input.parse_special_keys(false);
            loop {
                input.parse_single(waiting_for_command);
                match input.read_key().await {
                    Ok(Some(key)) => {
                        if waiting_for_command {
                            waiting_for_command = false;
                            match key {
                                textmode::Key::Ctrl(b'n') => {
                                    notify
                                        .send(Event::Input(key))
                                        .await
                                        .unwrap();
                                }
                                textmode::Key::Byte(b'c') => {
                                    notify
                                        .send(Event::Command(
                                            Command::NewWindow,
                                        ))
                                        .await
                                        .unwrap();
                                }
                                textmode::Key::Byte(b'n') => {
                                    notify
                                        .send(Event::Command(
                                            Command::NextWindow,
                                        ))
                                        .await
                                        .unwrap();
                                }
                                _ => {} // ignore
                            }
                        } else {
                            match key {
                                textmode::Key::Ctrl(b'n') => {
                                    waiting_for_command = true;
                                }
                                _ => {
                                    notify
                                        .send(Event::Input(key))
                                        .await
                                        .unwrap();
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        break;
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

    fn new_window(
        &mut self,
        ex: &smol::Executor<'_>,
        notify: smol::channel::Sender<Event>,
    ) {
        let child = smol::process::Command::new("zsh")
            .spawn_pty(Some(&pty_process::Size::new(24, 80)))
            .unwrap();
        let child = std::sync::Arc::new(child);
        let vt = vt100::Parser::default();
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
        self.notify(ex, &format!("created window {}", id));
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

    async fn redraw_current_window(&mut self, tm: &mut textmode::Output) {
        let window = self.current_window();
        tm.clear();
        let new_screen = window.vt.lock_arc().await.screen().clone();
        tm.write(&new_screen.state_formatted());
        self.draw_notifications(tm, &new_screen);
        tm.refresh().await.unwrap();
    }

    async fn update_current_window(&mut self, tm: &mut textmode::Output) {
        let window = self.current_window();
        let old_screen = window.screen.clone();
        let new_screen = window.vt.lock_arc().await.screen().clone();
        let diff = new_screen.state_diff(&old_screen);
        self.clear_notifications(tm, &old_screen);
        tm.write(&diff);
        self.draw_notifications(tm, &new_screen);
        tm.refresh().await.unwrap();
        self.current_window_mut().screen = new_screen;
    }

    fn clear_notifications(
        &mut self,
        tm: &mut textmode::Output,
        screen: &vt100::Screen,
    ) {
        if self.notifications.is_empty() {
            return;
        }

        let reset_attrs = screen.attributes_formatted();
        let pos = screen.cursor_position();
        for (i, row) in screen
            .rows_formatted(0, 80)
            .enumerate()
            .take(self.notifications.len())
        {
            tm.move_to(i as u16, 0);
            tm.reset_attributes();
            tm.clear_line();
            tm.write(&row);
        }
        tm.move_to(pos.0, pos.1);
        tm.write(&reset_attrs);
    }

    fn draw_notifications(
        &mut self,
        tm: &mut textmode::Output,
        screen: &vt100::Screen,
    ) {
        if self.notifications.is_empty() {
            return;
        }

        let now = std::time::Instant::now();
        self.notifications = self
            .notifications
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .filter(|(_, v)| v.expiry >= now)
            .collect();

        if self.notifications.is_empty() {
            return;
        }

        let reset_attrs = screen.attributes_formatted();
        let pos = screen.cursor_position();
        tm.reset_attributes();
        tm.set_bgcolor(textmode::color::CYAN);
        tm.set_fgcolor(textmode::color::WHITE);
        for (i, notification) in self.notifications.values().enumerate() {
            tm.move_to(i as u16, 0);
            tm.clear_line();
            let str_len = notification.text.len();
            let spaces = 80 - str_len;
            let prefix_spaces = spaces / 2;
            tm.write(&vec![b' '; prefix_spaces]);
            tm.write_str(&notification.text);
        }
        tm.move_to(pos.0, pos.1);
        tm.write(&reset_attrs);
    }
}

#[must_use]
struct Tmux {
    input: textmode::Input,
    _raw: textmode::RawGuard,
    tm: textmode::Output,
    _screen: textmode::ScreenGuard,
    state: State,
}

impl Tmux {
    async fn new() -> Self {
        let (input, _raw) = textmode::Input::new().await.unwrap();
        let (tm, _screen) = textmode::Output::new().await.unwrap();
        let state = State::new();
        Self {
            input,
            _raw,
            tm,
            _screen,
            state,
        }
    }

    async fn run(self, ex: &smol::Executor<'_>) {
        let Self {
            input,
            _raw,
            mut tm,
            _screen,
            mut state,
        } = self;

        state.new_window(ex, state.wevents.clone());
        state.spawn_input_task(ex, input);

        ex.run(async {
            loop {
                match state.revents.recv().await {
                    Ok(Event::Output) => {
                        state.update_current_window(&mut tm).await;
                    }
                    Ok(Event::Input(key)) => {
                        state
                            .current_window()
                            .child
                            .pty()
                            .write_all(&key.into_bytes())
                            .await
                            .unwrap();
                    }
                    Ok(Event::WindowExit(id)) => {
                        // do this first because next_window breaks if
                        // current_window is greater than all existing windows
                        if state.current_window == id {
                            state.next_window(ex)
                        }
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
                        state.notify(ex, &format!("window {} exited", id));

                        state.redraw_current_window(&mut tm).await;
                    }
                    Ok(Event::Command(c)) => match c {
                        Command::NewWindow => {
                            state.new_window(ex, state.wevents.clone());
                            state.redraw_current_window(&mut tm).await;
                        }
                        Command::NextWindow => {
                            state.next_window(ex);
                            state.redraw_current_window(&mut tm).await;
                        }
                    },
                    Ok(Event::Notification) => {
                        state.update_current_window(&mut tm).await;
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                        break;
                    }
                }
            }
        })
        .await;
    }
}

async fn async_main(ex: &smol::Executor<'_>) {
    let tmux = Tmux::new().await;
    tmux.run(ex).await;
}

fn main() {
    let ex = smol::Executor::new();
    smol::block_on(async { async_main(&ex).await })
}
