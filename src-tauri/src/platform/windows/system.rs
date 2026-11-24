//! Small system questions for the problem report and its GitHub link. None
//! of them concerns another process.
//!
//! Hardware and display queries run only inside the short-lived
//! `scorpio --system-facts` child process. `dxgi.dll` is loaded dynamically
//! with `LoadLibraryW` + `GetProcAddress` so it never enters the executable's
//! import table and is never loaded into the main Scorpio process.

use crate::report::{self, DisplayFact, GpuFact, PresentationFacts, ProcessCost, SystemFacts};
use std::ffi::c_void;
use windows::core::{s, w, GUID, HRESULT, HSTRING, Interface, PCWSTR};
use windows::Wdk::System::SystemServices::RtlGetVersion;
use windows::Win32::Foundation::{FILETIME, FreeLibrary};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709, DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020,
    DXGI_COLOR_SPACE_RGB_STUDIO_G2084_NONE_P2020,
};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIFactory1, IDXGIOutput2, IDXGIOutput6, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND,
};
use windows::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};
use windows::Win32::System::Console::{AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::Registry::{RegGetValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX, OSVERSIONINFOW};
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
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

/// Scorpio's own process CPU time and private memory usage. Only queries the
/// pseudo-handle returned by `GetCurrentProcess`; never opens another process.
pub fn process_cost() -> ProcessCost {
    // SAFETY: GetCurrentProcess returns a pseudo-handle to this process only.
    let me = unsafe { GetCurrentProcess() };
    let (mut created, mut exited, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: `me` is our own process pseudo-handle and all out-pointers are valid.
    let cpu_ms = unsafe { GetProcessTimes(me, &mut created, &mut exited, &mut kernel, &mut user) }.ok().map(|()| {
        let to_u64 = |ft: FILETIME| (u64::from(ft.dwHighDateTime) << 32) | u64::from(ft.dwLowDateTime);
        (to_u64(kernel) + to_u64(user)) / 10_000
    });
    let mut mem = PROCESS_MEMORY_COUNTERS_EX { cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32, ..Default::default() };
    // SAFETY: PROCESS_MEMORY_COUNTERS_EX begins with PROCESS_MEMORY_COUNTERS and `cb` is its exact size.
    let private_memory_bytes = unsafe {
        K32GetProcessMemoryInfo(me, (&mut mem as *mut PROCESS_MEMORY_COUNTERS_EX).cast::<PROCESS_MEMORY_COUNTERS>(), mem.cb)
            .as_bool()
            .then_some(mem.PrivateUsage as u64)
    };
    ProcessCost { cpu_ms, private_memory_bytes }
}

/// Hardware, display and presentation facts, gathered inside `--system-facts`.
pub fn system_facts() -> SystemFacts {
    let processor = reg_string(
        HKEY_LOCAL_MACHINE,
        w!("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0"),
        w!("ProcessorNameString"),
    )
    .filter(|s| !s.is_empty());

    let memory_mb = total_memory_mb();
    let presentation = presentation_settings();
    let mut gpus = Vec::new();
    let mut displays = Vec::new();
    let mut notes = Vec::new();
    collect_dxgi(&mut gpus, &mut displays, &mut notes);

    SystemFacts { processor, memory_mb, gpus, displays, presentation, notes }
}

fn total_memory_mb() -> Option<u64> {
    let mut status = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    // SAFETY: `status` is valid and `dwLength` is set as required.
    unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
    Some((status.ullTotalPhys + 524_288) / 1_048_576)
}

fn presentation_settings() -> PresentationFacts {
    let hags = match reg_u32(
        HKEY_LOCAL_MACHINE,
        w!("SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers"),
        w!("HwSchMode"),
    ) {
        Some(2) => Some("on (HwSchMode=2)".into()),
        Some(1) => Some("off (HwSchMode=1)".into()),
        Some(other) => Some(format!("unknown (HwSchMode={other})")),
        None => Some("default (not set)".into()),
    };

    let windowed_optimisations = match reg_string(
        HKEY_CURRENT_USER,
        w!("Software\\Microsoft\\DirectX\\UserGpuPreferences"),
        w!("DirectXUserGlobalSettings"),
    ) {
        Some(raw) => Some(report::parse_directx_user_global_settings(&raw).unwrap_or_else(|| "default (not set)".into())),
        None => Some("default (not set)".into()),
    };

    let game_mode = match reg_u32(
        HKEY_CURRENT_USER,
        w!("Software\\Microsoft\\GameBar"),
        w!("AutoGameModeEnabled"),
    ) {
        Some(1) => Some("on".into()),
        Some(0) => Some("off".into()),
        Some(other) => Some(format!("unknown ({other})")),
        None => Some("default (not set)".into()),
    };

    let compat_layer = Some(
        std::env::var("__COMPAT_LAYER")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "not set".into()),
    );

    PresentationFacts { hags, windowed_optimisations, game_mode, compat_layer, ..Default::default() }
}

fn reg_u32(root: HKEY, subkey: PCWSTR, value: PCWSTR) -> Option<u32> {
    let mut data = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: static NUL-terminated subkey/value names and valid out-buffer of `size` bytes.
    let status = unsafe { RegGetValueW(root, subkey, value, RRF_RT_REG_DWORD, None, Some((&mut data as *mut u32).cast()), Some(&mut size)) };
    status.is_ok().then_some(data)
}

fn reg_string(root: HKEY, subkey: PCWSTR, value: PCWSTR) -> Option<String> {
    let mut size = 0u32;
    // SAFETY: querying required byte length with null data pointer.
    let status = unsafe { RegGetValueW(root, subkey, value, RRF_RT_REG_SZ, None, None, Some(&mut size)) };
    if status.is_err() || size == 0 {
        return None;
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    // SAFETY: `buf` is at least `size` bytes long and outlives the call.
    let status = unsafe { RegGetValueW(root, subkey, value, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut size)) };
    if status.is_err() {
        return None;
    }
    let units = ((size as usize) / 2).min(buf.len());
    let end = buf[..units].iter().position(|&c| c == 0).unwrap_or(units);
    Some(String::from_utf16_lossy(&buf[..end]).trim().to_string())
}

/// Enumerates graphics cards and displays through dynamically loaded `dxgi.dll`.
/// Never calls any statically imported DXGI function and never creates a D3D device.
fn collect_dxgi(gpus: &mut Vec<GpuFact>, displays: &mut Vec<DisplayFact>, notes: &mut Vec<String>) {
    type CreateDxgiFactory1 = unsafe extern "system" fn(*const GUID, *mut *mut c_void) -> HRESULT;

    // SAFETY: `dxgi.dll` is a standard Windows system DLL; loaded only in this child process.
    let module = match unsafe { LoadLibraryW(w!("dxgi.dll")) } {
        Ok(module) => module,
        Err(error) => {
            notes.push(format!("dxgi.dll could not be loaded: {error}"));
            return;
        }
    };

    {
        // SAFETY: `module` is valid and `CreateDXGIFactory1` is a NUL-terminated ASCII name.
        let proc = unsafe { GetProcAddress(module, s!("CreateDXGIFactory1")) };
        let Some(proc) = proc else {
            notes.push("CreateDXGIFactory1 was not found in dxgi.dll".into());
            // SAFETY: no DXGI COM interfaces were created; unloading the library we loaded.
            unsafe { let _ = FreeLibrary(module); }
            return;
        };
        // SAFETY: `CreateDXGIFactory1` has the standard signature `(REFIID, void**) -> HRESULT`.
        let create_factory: CreateDxgiFactory1 = unsafe { std::mem::transmute(proc) };
        let mut raw_factory: *mut c_void = std::ptr::null_mut();
        // SAFETY: `IDXGIFactory1::IID` and `raw_factory` are valid pointers.
        let hr = unsafe { create_factory(&IDXGIFactory1::IID, &mut raw_factory) };
        if hr.is_err() || raw_factory.is_null() {
            notes.push(format!("CreateDXGIFactory1 failed: 0x{:08x}", hr.0 as u32));
        } else {
            // SAFETY: `CreateDXGIFactory1` succeeded and transferred one reference to `raw_factory`.
            let factory = unsafe { IDXGIFactory1::from_raw(raw_factory) };
            enumerate_dxgi(&factory, gpus, displays);
        }
        // All COM interfaces (`factory`, adapters, outputs) are dropped before leaving this scope.
    }

    // SAFETY: all COM objects from `dxgi.dll` have already been released above.
    unsafe { let _ = FreeLibrary(module); }
}

fn enumerate_dxgi(factory: &IDXGIFactory1, gpus: &mut Vec<GpuFact>, displays: &mut Vec<DisplayFact>) {
    for adapter_index in 0u32.. {
        // SAFETY: COM vtable call on a valid `IDXGIFactory1`.
        let adapter = match unsafe { factory.EnumAdapters1(adapter_index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(_) => break,
        };
        // SAFETY: COM vtable call on a valid `IDXGIAdapter1`.
        if let Ok(desc) = unsafe { adapter.GetDesc1() } {
            let name = utf16_z(&desc.Description);
            let dedicated_memory_mb = Some((desc.DedicatedVideoMemory as u64 + 524_288) / 1_048_576);
            let software = (desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32)) != 0;
            // SAFETY: `CheckInterfaceSupport(&IDXGIDevice::IID)` reads the UMD driver version
            // without creating any D3D device.
            let driver_version = unsafe { adapter.CheckInterfaceSupport(&IDXGIDevice::IID) }.ok().map(|raw| {
                let v = raw as u64;
                format!("{}.{}.{}.{}", (v >> 48) & 0xFFFF, (v >> 32) & 0xFFFF, (v >> 16) & 0xFFFF, v & 0xFFFF)
            });
            gpus.push(GpuFact { name, dedicated_memory_mb, driver: None, driver_version, software });
        }

        for output_index in 0u32.. {
            // SAFETY: COM vtable call on a valid `IDXGIAdapter1`.
            let output = match unsafe { adapter.EnumOutputs(output_index) } {
                Ok(output) => output,
                Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(_) => break,
            };
            // SAFETY: COM vtable call on a valid `IDXGIOutput`.
            let Ok(desc) = (unsafe { output.GetDesc() }) else { continue };
            if !desc.AttachedToDesktop.as_bool() {
                continue;
            }
            let name = utf16_z(&desc.DeviceName);
            let mut width = (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left).max(0) as u32;
            let mut height = (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top).max(0) as u32;
            let primary = desc.DesktopCoordinates.left == 0 && desc.DesktopCoordinates.top == 0;
            let mut refresh_hz = None;

            let mut mode = DEVMODEW { dmSize: size_of::<DEVMODEW>() as u16, ..Default::default() };
            // SAFETY: `desc.DeviceName` is NUL-terminated UTF-16 and `mode.dmSize` is set.
            if unsafe { EnumDisplaySettingsW(PCWSTR(desc.DeviceName.as_ptr()), ENUM_CURRENT_SETTINGS, &mut mode) }.as_bool() {
                if mode.dmPelsWidth > 0 && mode.dmPelsHeight > 0 {
                    width = mode.dmPelsWidth;
                    height = mode.dmPelsHeight;
                }
                if mode.dmDisplayFrequency > 0 {
                    refresh_hz = Some(mode.dmDisplayFrequency);
                }
            }

            let hdr = output.cast::<IDXGIOutput6>().ok().and_then(|out6| {
                // SAFETY: COM vtable call on a valid `IDXGIOutput6`.
                unsafe { out6.GetDesc1() }.ok().map(|d1| {
                    matches!(
                        d1.ColorSpace,
                        DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020
                            | DXGI_COLOR_SPACE_RGB_STUDIO_G2084_NONE_P2020
                            | DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709
                    )
                })
            });

            // SAFETY: COM vtable call on a valid `IDXGIOutput2`.
            let overlay_planes = output.cast::<IDXGIOutput2>().ok().map(|out2| unsafe { out2.SupportsOverlays().as_bool() });

            displays.push(DisplayFact { name, width, height, refresh_hz, hdr, overlay_planes, primary });
        }
    }
}

fn utf16_z(units: &[u16]) -> String {
    let end = units.iter().position(|&c| c == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end]).trim().to_string()
}
