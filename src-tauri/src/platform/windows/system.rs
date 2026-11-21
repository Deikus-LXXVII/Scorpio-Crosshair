//! Small system questions for the problem report and its GitHub link. None
//! of them concerns another process.

use windows::core::{w, HSTRING};
use windows::Wdk::System::SystemServices::RtlGetVersion;
use windows::Win32::System::Console::{AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE};
use windows::Win32::System::SystemInformation::OSVERSIONINFOW;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// "Windows 11 (10.0 build 26100)": RtlGetVersion reports the real version,
/// unlike GetVersionEx, which a manifest can change.
pub fn version() -> String {
    let mut info = OSVERSIONINFOW { dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32, ..Default::default() };
    // SAFETY: `info` is a valid structure with its size set, as the call requires.
    if unsafe { RtlGetVersion(&mut info) }.is_err() {
        return format!("Windows, {}", std::env::consts::ARCH);
    }
    let name = if info.dwMajorVersion == 10 && info.dwBuildNumber >= 22000 { "Windows 11" } else { "Windows 10" };
    format!("{name} ({}.{} build {}), {}", info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber, std::env::consts::ARCH)
}

/// Opens an address in the default browser.
pub fn open_url(url: &str) -> Result<(), String> {
    // SAFETY: both strings live through the call; no window is owned.
    let result = unsafe { ShellExecuteW(None, w!("open"), &HSTRING::from(url), None, None, SW_SHOWNORMAL) };
    // ShellExecute returns a value above 32 on success.
    if result.0 as isize > 32 { Ok(()) } else { Err(format!("the browser could not be opened (code {})", result.0 as isize)) }
}

/// Lets `scorpio --report` print to the console it was started from: a
/// release build has no console of its own. Redirected output needs nothing.
pub fn attach_console() {
    // SAFETY: no pointers. Output already redirected (to a file or a pipe)
    // is left alone; only a missing one is pointed at the parent's console.
    unsafe {
        let redirected = GetStdHandle(STD_OUTPUT_HANDLE).is_ok_and(|handle| !handle.is_invalid() && !handle.0.is_null());
        if !redirected {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}
