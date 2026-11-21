//! X11 (and XWayland). One thread owns the X connection and blocks on it;
//! `wake` reaches it with a ClientMessage sent over a second connection, which
//! also serves `list_windows`.

mod describe;
mod input;
mod overlay;

use super::{Command, Event, Platform, Unsupported};
use crate::core::focus::WindowInfo;
use describe::Atoms;
use input::Input;
use overlay::Overlay;
use std::error::Error;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt as _, EventMask, Window};
use x11rb::protocol::Event as XEvent;
use x11rb::rust_connection::RustConnection;

struct State {
    conn: RustConnection,
    atoms: Atoms,
    root: Window,
    overlay: Overlay,
    input: Input,
    active: Option<Window>,
    last: Option<Option<WindowInfo>>,
    events: Sender<Event>,
}

pub fn start(events: Sender<Event>) -> Result<Platform, Unsupported> {
    if std::env::var_os("DISPLAY").is_none() {
        return Err(Unsupported("Scorpio needs X11 or XWayland, and DISPLAY is not set (Wayland without XWayland is not supported)".into()));
    }
    let connect = || RustConnection::connect(None).map_err(|e| Unsupported(format!("cannot connect to the X server: {e}")));
    let (conn, screen_num) = connect()?;
    let (helper, _) = connect()?;
    let state = setup(conn, screen_num, events).map_err(|e| Unsupported(format!("X11 setup failed: {e}")))?;
    let (atoms, root, target) = (state.atoms, state.root, state.overlay.window);

    let (commands, receiver) = channel();
    std::thread::Builder::new()
        .name("scorpio-x11".into())
        .spawn(move || run(state, receiver))
        .map_err(|e| Unsupported(format!("cannot start the X11 thread: {e}")))?;

    let helper = Arc::new(helper);
    let lister = Arc::clone(&helper);
    Ok(Platform {
        commands,
        wake: Box::new(move || {
            // An empty event mask delivers the event to the window's creator: our thread.
            let message = ClientMessageEvent::new(32, target, atoms._SCORPIO_WAKE, [0u32; 5]);
            let _ = helper.send_event(false, target, EventMask::NO_EVENT, message);
            let _ = helper.flush();
        }),
        list_windows: Box::new(move || describe::list(&*lister, &atoms, root)),
    })
}

fn setup(conn: RustConnection, screen_num: usize, events: Sender<Event>) -> Result<State, Box<dyn Error>> {
    let atoms = Atoms::new(&conn)?.reply()?;
    let root = conn.setup().roots[screen_num].root;
    let overlay = Overlay::create(&conn, screen_num)?;
    let input = Input::new(&conn);
    conn.change_window_attributes(root, &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE))?;
    // A resolution or monitor change moves the centre; without RandR it is
    // simply not seen.
    let _ = conn.randr_select_input(root, x11rb::protocol::randr::NotifyMask::SCREEN_CHANGE);
    conn.flush()?;
    Ok(State { conn, atoms, root, overlay, input, active: None, last: None, events })
}

fn run(mut state: State, commands: Receiver<Command>) {
    state.refresh_foreground();
    loop {
        // Commands are drained on every wake-up, so a lost wake only delays one.
        loop {
            match commands.try_recv() {
                Ok(Command::Quit) | Err(TryRecvError::Disconnected) => return,
                Ok(command) => state.apply(command),
                Err(TryRecvError::Empty) => break,
            }
        }
        if state.conn.flush().is_err() {
            return;
        }
        match state.conn.wait_for_event() {
            Ok(event) => state.handle(event),
            Err(e) => {
                log::error!("X connection lost: {e}");
                return;
            }
        }
    }
}

impl State {
    fn apply(&mut self, command: Command) {
        let result = match command {
            Command::Show { bitmap, x, y, unlocked, button } => self.overlay.show(&self.conn, &bitmap, x, y, unlocked, button),
            Command::Hide => self.overlay.hide(&self.conn),
            Command::ListenTo { mouse, keys } => {
                self.input.listen(&self.conn, self.root, mouse, keys);
                Ok(())
            }
            Command::Raise => self.overlay.raise(&self.conn),
            // X11 has no way to keep a window out of captures.
            Command::ExcludeFromCapture(_) | Command::Quit => Ok(()),
        };
        if let Err(e) = result {
            log::warn!("overlay command failed: {e}");
        }
    }

    fn handle(&mut self, event: XEvent) {
        let reply = match &event {
            XEvent::PropertyNotify(e) if e.window == self.root && e.atom == self.atoms._NET_ACTIVE_WINDOW => {
                self.refresh_foreground();
                None
            }
            // Title and fullscreen changes of the window in front.
            XEvent::PropertyNotify(e)
                if Some(e.window) == self.active
                    && [self.atoms._NET_WM_NAME, self.atoms._NET_WM_STATE, u32::from(AtomEnum::WM_NAME)].contains(&e.atom) =>
            {
                self.refresh_foreground();
                None
            }
            XEvent::RandrScreenChangeNotify(_) => {
                self.last = None;
                self.refresh_foreground();
                None
            }
            XEvent::MappingNotify(_) => {
                self.input.load_keymap(&self.conn);
                None
            }
            XEvent::Error(e) => {
                // Mostly BadWindow from a window that closed while we read it.
                log::debug!("X error: {e:?}");
                None
            }
            _ => self.input.handle(&event).or_else(|| self.overlay.handle(&self.conn, &event).unwrap_or_else(|e| {
                log::warn!("overlay event failed: {e}");
                None
            })),
        };
        if let Some(reply) = reply {
            let _ = self.events.send(reply);
        }
    }

    fn refresh_foreground(&mut self) {
        // Some window managers name the root when the desktop is in front.
        let active = describe::active_window(&self.conn, &self.atoms, self.root).filter(|&w| w != self.root);
        if active != self.active {
            // Watch the window in front for its own property changes only.
            let watch = |w, mask| self.conn.change_window_attributes(w, &ChangeWindowAttributesAux::new().event_mask(mask)).map(|_| ());
            if let Some(old) = self.active {
                let _ = watch(old, EventMask::NO_EVENT);
            }
            if let Some(new) = active {
                let _ = watch(new, EventMask::PROPERTY_CHANGE);
            }
            self.active = active;
        }
        let info = active.and_then(|w| describe::describe(&self.conn, &self.atoms, w));
        if self.last.as_ref() != Some(&info) {
            self.last = Some(info.clone());
            let _ = self.events.send(Event::Foreground(info));
        }
    }
}
