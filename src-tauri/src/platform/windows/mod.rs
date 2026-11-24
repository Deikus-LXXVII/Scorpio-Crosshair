//! Windows: one thread owns the overlay window, the foreground event hook and
//! Raw Input, and sleeps in `GetMessageW` until the system or the app has
//! something for it. Nothing polls.
//!
//! Anti-cheat rules (docs/plans/scorpio.md) as they apply here:
//! - no process is ever opened: the executable path comes from the kernel's
//!   process list by pid (`process.rs`), everything else from the window manager;
//! - no hooks that load into or intercept other processes: the foreground hook
//!   is `WINEVENT_OUTOFCONTEXT` (a notification posted to this thread's queue),
//!   input is Raw Input on our own hidden window (`input.rs`);
//! - no input is ever sent, nothing is injected, nothing is drawn into a game.

mod input;
mod overlay;
mod process;
pub mod system;
mod window;

pub use process::image as process_image;

use super::{Command, Event, Geometry, Platform, Unsupported};
use std::cell::{Cell, OnceCell, RefCell};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_RUNNING_D3D_FULL_SCREEN};
use windows::Win32::UI::HiDpi::{SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, KillTimer, SetTimer, WM_TIMER, DestroyWindow, DispatchMessageW, GetForegroundWindow, GetMessageW, LoadCursorW,
    GetWindowThreadProcessId, PostMessageW, PostQuitMessage, RegisterClassW, EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND, HWND_MESSAGE, IDC_ARROW, MSG,
    WINDOW_EX_STYLE, WINDOW_STYLE, WINEVENT_OUTOFCONTEXT, WM_APP, WM_INPUT, WNDCLASSW, WNDPROC,
};

/// Posted to the hidden window by `Platform::wake`: commands are waiting.
const WM_WAKE: u32 = WM_APP + 1;

struct State {
    events: Sender<Event>,
    commands: Receiver<Command>,
    /// Hidden message-only window: the target of wake-ups and of Raw Input.
    sink: HWND,
    listener: RefCell<input::Listener>,
    follow: RefCell<Follow>,
    /// The next of `EXCLUSIVE_CHECKS` to run.
    exclusive_check: Cell<usize>,
}

/// The sink's one timer: asking whether the window in front is in exclusive fullscreen.
const EXCLUSIVE_TIMER: usize = 1;
/// When to ask, in ms after the previous ask, from when a fullscreen window came
/// to the front: a game may switch to exclusive fullscreen while it loads.
/// Three asks at most, then nothing until the next change of window.
const EXCLUSIVE_CHECKS: [u32; 3] = [2_000, 8_000, 20_000];

/// Arms the next exclusive-fullscreen check, or stops the checks.
fn check_exclusive(state: &State, next: Option<usize>) {
    match next.filter(|next| *next < EXCLUSIVE_CHECKS.len()) {
        Some(next) => {
            state.exclusive_check.set(next);
            // SAFETY: the sink is this thread's own window; the timer posts WM_TIMER to it.
            unsafe { SetTimer(Some(state.sink), EXCLUSIVE_TIMER, EXCLUSIVE_CHECKS[next], None) };
        }
        // SAFETY: as above; killing a timer that is not set does nothing.
        None => unsafe {
            let _ = KillTimer(Some(state.sink), EXCLUSIVE_TIMER);
        },
    }
}

/// Following the game window's moves and resizes (Command::Follow).
#[derive(Default)]
struct Follow {
    on: bool,
    /// The last top-level window in front that was not Scorpio's.
    target: Option<HWND>,
    /// The location hook, scoped to the target's process and thread.
    hook: Option<HWINEVENTHOOK>,
    /// What was last reported, to send only changes.
    last: Option<Geometry>,
}

impl Follow {
    /// Sets, moves or removes the location hook to match `on` and `target`.
    /// Out of context and scoped to the one window's process and thread
    /// (`idProcess`, `idThread`): the system posts those events to this
    /// thread's queue and loads nothing into the game. Other processes'
    /// location changes never wake Scorpio.
    fn rehook(&mut self) {
        if let Some(hook) = self.hook.take() {
            // SAFETY: the hook was set by this thread and is not used again.
            let _ = unsafe { UnhookWinEvent(hook) };
        }
        self.last = None;
        let Some(target) = self.target.filter(|_| self.on) else { return };
        let mut pid = 0u32;
        // SAFETY: reads the window's owner from the window manager; `pid` is a valid out-pointer.
        let thread = unsafe { GetWindowThreadProcessId(target, Some(&mut pid)) };
        if thread == 0 || pid == 0 {
            return;
        }
        // SAFETY: the callback is a plain function that lives as long as the program.
        let hook = unsafe {
            SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE, None, Some(location_changed), pid, thread, WINEVENT_OUTOFCONTEXT)
        };
        if hook.is_invalid() {
            log::warn!("the location hook could not be set; the game window will not be followed");
        } else {
            log::debug!("following window of pid {pid}");
            self.hook = Some(hook);
        }
    }
}

thread_local! {
    static STATE: OnceCell<State> = const { OnceCell::new() };
}

pub fn start(events: Sender<Event>) -> Result<Platform, Unsupported> {
    let (commands, receiver) = channel::<Command>();
    let (ready, started) = channel::<Result<isize, String>>();
    std::thread::Builder::new()
        .name("scorpio-platform".into())
        .spawn(move || run(events, receiver, ready))
        .map_err(|error| Unsupported(format!("the platform thread could not start: {error}")))?;
    let sink = started
        .recv()
        .unwrap_or_else(|_| Err("the platform thread stopped while starting".into()))
        .map_err(Unsupported)?;
    Ok(Platform {
        commands,
        wake: Box::new(move || {
            // SAFETY: posting to a window handle is allowed from any thread; if
            // the platform thread has ended the handle is dead and this fails.
            let _ = unsafe { PostMessageW(Some(HWND(sink as *mut _)), WM_WAKE, WPARAM(0), LPARAM(0)) };
        }),
        list_windows: Box::new(window::list),
    })
}

fn run(events: Sender<Event>, commands: Receiver<Command>, ready: Sender<Result<isize, String>>) {
    // SAFETY: affects this thread only. Per-monitor v2 makes every coordinate
    // here a physical pixel, whatever the rest of the process chose.
    unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };

    let created = create_window(w!("ScorpioEvents"), sink_procedure, WINDOW_EX_STYLE(0), WINDOW_STYLE(0), Some(HWND_MESSAGE))
        .and_then(|sink| overlay::create(events.clone()).map(|()| sink));
    let sink = match created {
        Ok(sink) => sink,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let state = State { events, commands, sink, listener: RefCell::default(), follow: RefCell::default(), exclusive_check: Cell::new(0) };
    STATE.with(|cell| cell.set(state).ok());

    // Out of context: the system posts the event to this thread's queue. No
    // module is loaded into the game or any other process, unlike an
    // in-context hook or SetWindowsHookEx. Own-process windows are not
    // skipped: the app needs to know when its settings window is in front.
    // SAFETY: the callback is a plain function that lives as long as the program.
    let hook = unsafe {
        SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, None, Some(foreground_changed), 0, 0, WINEVENT_OUTOFCONTEXT)
    };
    if hook.is_invalid() {
        log::warn!("the foreground event hook could not be set; focus changes will not be seen");
    }
    let _ = ready.send(Ok(sink.0 as isize));

    // SAFETY: no arguments; reads which window is in front right now.
    report_foreground(unsafe { GetForegroundWindow() });

    let mut message = MSG::default();
    // SAFETY: `message` is a valid out-structure; the loop ends on WM_QUIT (0) or an error (-1).
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.0 > 0 {
        // SAFETY: `message` was just filled in by GetMessageW.
        unsafe { DispatchMessageW(&message) };
    }

    STATE.with(|state| {
        state.get().map(|state| {
            state.listener.borrow_mut().listen(sink, Vec::new(), Vec::new());
            let mut follow = state.follow.borrow_mut();
            follow.on = false;
            follow.rehook();
        })
    });
    overlay::destroy();
    // SAFETY: the hook and the window belong to this thread and are not used again.
    unsafe {
        if !hook.is_invalid() {
            let _ = UnhookWinEvent(hook);
        }
        let _ = DestroyWindow(sink);
    }
}

/// Registers a window class (once per process) and creates one window of it on this thread.
fn create_window(
    class: PCWSTR,
    procedure: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    ex_style: WINDOW_EX_STYLE,
    style: WINDOW_STYLE,
    parent: Option<HWND>,
) -> Result<HWND, String> {
    let procedure: WNDPROC = Some(procedure);
    // SAFETY: `class` is a static NUL-terminated string and the class structure
    // is valid for the call; registering twice fails harmlessly.
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|error| error.to_string())?.into();
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        RegisterClassW(&WNDCLASSW { lpfnWndProc: procedure, hInstance: instance, hCursor: cursor, lpszClassName: class, ..Default::default() });
        CreateWindowExW(ex_style, class, w!("Scorpio overlay"), style, 0, 0, 0, 0, parent, None, Some(instance), None)
            .map_err(|error| format!("a window could not be created: {error}"))
    }
}

fn report_foreground(hwnd: HWND) {
    if let Some(event) = window::foreground_event(hwnd) {
        STATE.with(|state| {
            if let Some(state) = state.get() {
                // A window of another process in front becomes the one followed.
                if let Event::Foreground(Some(info)) = &event {
                    check_exclusive(state, (info.pid != std::process::id() && info.fullscreen).then_some(0));
                    let mut follow = state.follow.borrow_mut();
                    if info.pid != std::process::id() && follow.target != Some(hwnd) {
                        follow.target = Some(hwnd);
                        if follow.on {
                            follow.rehook();
                            follow.last = Some((info.client, info.monitor, info.fullscreen));
                        }
                    }
                }
                let _ = state.events.send(event);
            }
        });
    }
}

unsafe extern "system" fn location_changed(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    super::counters::record_window_location();
    // The window itself (OBJID_WINDOW, CHILDID_SELF), not the caret, the
    // cursor or a control inside it.
    if object != 0 || child != 0 {
        return;
    }
    STATE.with(|state| {
        let Some(state) = state.get() else { return };
        let Ok(mut follow) = state.follow.try_borrow_mut() else { return };
        if follow.target != Some(hwnd) {
            return;
        }
        let now = window::geometry(hwnd);
        // A drag sends many events for the same place: report changes only.
        if follow.last != Some(now) {
            follow.last = Some(now);
            let pid = window::pid_of(hwnd);
            let (client, monitor, fullscreen) = now;
            let _ = state.events.send(Event::Geometry { pid, client, monitor, fullscreen });
        }
    });
}

unsafe extern "system" fn foreground_changed(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    super::counters::record_foreground();
    // Only the window itself (OBJID_WINDOW, CHILDID_SELF), not a part of it.
    if object == 0 && child == 0 {
        report_foreground(hwnd);
    }
}

unsafe extern "system" fn sink_procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    STATE.with(|state| {
        let Some(state) = state.get() else { return };
        match message {
            WM_WAKE => drain_commands(state),
            WM_TIMER if wparam.0 == EXCLUSIVE_TIMER => {
                super::counters::record_timer();
                // The shell's own answer to "may I show a notification now":
                // nothing here looks at or touches the game.
                // SAFETY: no arguments; reads the shell's presentation state.
                let exclusive = unsafe { SHQueryUserNotificationState() }.is_ok_and(|state| state == QUNS_RUNNING_D3D_FULL_SCREEN);
                let _ = state.events.send(Event::Exclusive(exclusive));
                check_exclusive(state, (!exclusive).then(|| state.exclusive_check.get() + 1));
            }
            WM_TIMER => super::counters::record_timer(),
            WM_INPUT => {
                super::counters::record_raw_input();
                state.listener.borrow_mut().read(lparam, |event| {
                    let _ = state.events.send(event);
                });
            }
            _ => {}
        }
    });
    // WM_INPUT must reach the default procedure too, which frees the packet.
    // SAFETY: forwards the untouched arguments of this window procedure.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn drain_commands(state: &State) {
    loop {
        match state.commands.try_recv() {
            Ok(Command::Show { bitmap, x, y, unlocked, button }) => overlay::show(&bitmap, x, y, unlocked, button),
            Ok(Command::Hide) => overlay::hide(),
            Ok(Command::Raise) => overlay::raise(),
            Ok(Command::Move { x, y }) => overlay::move_to(x, y),
            Ok(Command::Follow(on)) => {
                let mut follow = state.follow.borrow_mut();
                if follow.on != on {
                    follow.on = on;
                    follow.rehook();
                    // Where the window is now: it may have moved while nobody followed it.
                    if let Some(target) = follow.target.filter(|_| on) {
                        let now = window::geometry(target);
                        follow.last = Some(now);
                        let (client, monitor, fullscreen) = now;
                        let _ = state.events.send(Event::Geometry { pid: window::pid_of(target), client, monitor, fullscreen });
                    }
                }
            }
            Ok(Command::ExcludeFromCapture(exclude)) => overlay::exclude_from_capture(exclude),
            Ok(Command::ListenTo { mouse, keys }) => state.listener.borrow_mut().listen(state.sink, mouse, keys),
            // SAFETY: no pointers; ends this thread's message loop.
            Ok(Command::Quit) | Err(TryRecvError::Disconnected) => return unsafe { PostQuitMessage(0) },
            Err(TryRecvError::Empty) => return,
        }
    }
}
