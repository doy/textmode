use std::os::fd::AsFd as _;

use futures::stream::StreamExt as _;
use textmode::Textmode as _;
use tokio::io::AsyncWriteExt as _;

#[derive(Debug)]
enum Command {
    NewWindow,
    NextWindow,
}

#[derive(Debug)]
enum Event {
    Input(textmode::Key),
    Output,
    WindowExit(usize),
    Command(Command),
    Notification,
}

struct Window {
    vt: std::sync::Arc<tokio::sync::Mutex<vt100::Parser>>,
    pty_w: pty_process::OwnedWritePty,
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
    wevents: tokio::sync::mpsc::UnboundedSender<Event>,
    revents: tokio::sync::mpsc::UnboundedReceiver<Event>,
}

impl State {
    fn new() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
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

    fn next_window(&mut self) {
        self.current_window = self
            .windows
            .keys()
            .copied()
            .cycle()
            .skip_while(|&id| id < self.current_window)
            .nth(1)
            .unwrap();
        self.notify(&format!("switched to window {}", self.current_window));
    }

    fn notify(&mut self, text: &str) {
        let now = std::time::Instant::now();
        let expiry = now + std::time::Duration::from_secs(5);
        let text = text.to_string();
        let notification = Notification { text, expiry };
        let id = self.next_notification_id;
        self.next_notification_id += 1;
        self.notifications.insert(id, notification);
        let notify = self.wevents.clone();
        tokio::task::spawn(async move {
            tokio::time::sleep_until(tokio::time::Instant::from_std(expiry))
                .await;
            notify.send(Event::Notification).unwrap();
        });
    }

    fn spawn_input_thread(&self, mut input: textmode::blocking::Input) {
        let notify = self.wevents.clone();
        std::thread::spawn(move || {
            let mut waiting_for_command = false;
            input.parse_utf8(false);
            input.parse_meta(false);
            input.parse_special_keys(false);
            loop {
                input.parse_single(waiting_for_command);
                match input.read_key() {
                    Ok(Some(key)) => {
                        if waiting_for_command {
                            waiting_for_command = false;
                            match key {
                                textmode::Key::Ctrl(b'n') => {
                                    notify.send(Event::Input(key)).unwrap();
                                }
                                textmode::Key::Byte(b'c') => {
                                    notify
                                        .send(Event::Command(
                                            Command::NewWindow,
                                        ))
                                        .unwrap();
                                }
                                textmode::Key::Byte(b'n') => {
                                    notify
                                        .send(Event::Command(
                                            Command::NextWindow,
                                        ))
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
                                    notify.send(Event::Input(key)).unwrap();
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
        });
    }

    fn new_window(
        &mut self,
        notify: tokio::sync::mpsc::UnboundedSender<Event>,
    ) {
        let (pty, pts) = pty_process::open().unwrap();
        let pts_clone = pts.as_fd().try_clone_to_owned().unwrap();
        pty.resize(pty_process::Size::new(24, 80)).unwrap();
        let cmd = pty_process::Command::new("zsh");
        let mut child = cmd.spawn(pts).unwrap();
        let (pty_r, pty_w) = pty.into_split();
        let vt = vt100::Parser::default();
        let screen = vt.screen().clone();
        let vt = std::sync::Arc::new(tokio::sync::Mutex::new(vt));
        let id = self.next_window_id;
        self.next_window_id += 1;
        let window = Window {
            pty_w,
            vt: vt.clone(),
            screen,
        };
        self.windows.insert(id, window);
        self.current_window = id;
        self.notify(&format!("created window {}", id));
        tokio::task::spawn(async move {
            enum Res {
                Bytes(tokio::io::Result<bytes::Bytes>),
                Done,
            }

            let _pts = pts_clone;

            let mut stream: futures::stream::SelectAll<_> = [
                tokio_util::io::ReaderStream::new(pty_r)
                    .map(Res::Bytes)
                    .boxed(),
                futures::stream::once(child.wait())
                    .map(|_| Res::Done)
                    .boxed(),
            ]
            .into_iter()
            .collect();
            while let Some(res) = stream.next().await {
                match res {
                    Res::Bytes(bytes) => match bytes {
                        Ok(bytes) => {
                            if bytes.is_empty() {
                                continue;
                            }
                            vt.clone().lock_owned().await.process(&bytes);
                            notify.send(Event::Output).unwrap();
                        }
                        Err(e) => {
                            eprintln!("pty read failed: {:?}", e);
                            break;
                        }
                    },
                    Res::Done => {
                        notify.send(Event::WindowExit(id)).unwrap();
                        break;
                    }
                }
            }
        });
    }

    async fn redraw_current_window(&mut self, tm: &mut textmode::Output) {
        let window = self.current_window();
        tm.clear();
        let new_screen =
            window.vt.clone().lock_owned().await.screen().clone();
        tm.write(&new_screen.state_formatted());
        self.draw_notifications(tm, &new_screen);
        tm.refresh().await.unwrap();
    }

    async fn update_current_window(&mut self, tm: &mut textmode::Output) {
        let window = self.current_window();
        let old_screen = window.screen.clone();
        let new_screen =
            window.vt.clone().lock_owned().await.screen().clone();
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
pub struct Tmux {
    input: textmode::blocking::Input,
    tm: textmode::Output,
    state: State,
}

impl Tmux {
    pub async fn new() -> Self {
        let input = textmode::blocking::Input::new().unwrap();
        let tm = textmode::Output::new().await.unwrap();
        let state = State::new();
        Self { input, tm, state }
    }

    pub async fn run(self) {
        let Self {
            mut input,
            mut tm,
            mut state,
        } = self;

        let _raw_guard = input.take_raw_guard();
        state.spawn_input_thread(input);

        state.new_window(state.wevents.clone());

        loop {
            match state.revents.recv().await {
                Some(Event::Output) => {
                    state.update_current_window(&mut tm).await;
                }
                Some(Event::Input(key)) => {
                    state
                        .current_window_mut()
                        .pty_w
                        .write_all(&key.into_bytes())
                        .await
                        .unwrap();
                }
                Some(Event::WindowExit(id)) => {
                    // do this first because next_window breaks if
                    // current_window is greater than all existing windows
                    if state.current_window == id {
                        state.next_window()
                    }
                    state.windows.remove(&id).unwrap();
                    if state.windows.is_empty() {
                        break;
                    }
                    state.notify(&format!("window {} exited", id));

                    state.redraw_current_window(&mut tm).await;
                }
                Some(Event::Command(c)) => match c {
                    Command::NewWindow => {
                        state.new_window(state.wevents.clone());
                        state.redraw_current_window(&mut tm).await;
                    }
                    Command::NextWindow => {
                        state.next_window();
                        state.redraw_current_window(&mut tm).await;
                    }
                },
                Some(Event::Notification) => {
                    state.update_current_window(&mut tm).await;
                }
                None => {
                    break;
                }
            }
        }
    }
}
