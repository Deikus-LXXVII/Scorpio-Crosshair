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
mod window;

use super::{Command, Event, Platform, Unsupported};
use std::cell::{OnceCell, RefCell};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::HiDpi::{SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetForegroundWindow, GetMessageW, LoadCursorW,
    PostMessageW, PostQuitMessage, RegisterClassW, EVENT_SYSTEM_FOREGROUND, HWND_MESSAGE, IDC_ARROW, MSG,
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
    let state = State { events, commands, sink, listener: RefCell::default() };
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

    STATE.with(|state| state.get().map(|state| state.listener.borrow_mut().listen(sink, Vec::new(), Vec::new())));
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
                let _ = state.events.send(event);
            }
        });
    }
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
            WM_INPUT => state.listener.borrow_mut().read(lparam, |event| {
                let _ = state.events.send(event);
            }),
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
            Ok(Command::ExcludeFromCapture(exclude)) => overlay::exclude_from_capture(exclude),
            Ok(Command::ListenTo { mouse, keys }) => state.listener.borrow_mut().listen(state.sink, mouse, keys),
            // SAFETY: no pointers; ends this thread's message loop.
            Ok(Command::Quit) | Err(TryRecvError::Disconnected) => return unsafe { PostQuitMessage(0) },
            Err(TryRecvError::Empty) => return,
        }
    }
}
