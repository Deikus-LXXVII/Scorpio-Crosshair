//! A process's executable path from its pid, without ever opening the process.
//!
//! The usual route (`OpenProcess` + `QueryFullProcessImageNameW`) takes a handle
//! to the game, which is exactly what anti-cheats watch for, so it is banned
//! here. `NtQuerySystemInformation(SystemProcessIdInformation)` asks the kernel
//! for the image name of a pid instead: no handle to the process ever exists.

use std::ffi::c_void;
use std::sync::Mutex;
use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{GetLogicalDrives, QueryDosDeviceW};

const SYSTEM_PROCESS_ID_INFORMATION: u32 = 0x58;
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;

#[repr(C)]
struct UnicodeString {
    length: u16,         // bytes in use
    maximum_length: u16, // bytes available; the kernel writes the needed size here
    buffer: *mut u16,
}

#[repr(C)]
struct ProcessIdInformation {
    process_id: *mut c_void, // a HANDLE-sized pid, not a handle
    image_name: UnicodeString,
}

#[link(name = "ntdll")]
extern "system" {
    fn NtQuerySystemInformation(class: u32, info: *mut c_void, length: u32, returned: *mut u32) -> i32;
}

/// `(name, path)` of a process: the DOS path of its executable and that path's
/// file stem. Either may be empty when the system won't say; that is never
/// worked around by opening the process.
pub fn image(pid: u32) -> (String, String) {
    let Some(nt_path) = nt_image_path(pid) else {
        return (String::new(), String::new());
    };
    let name = nt_path.rsplit('\\').next().unwrap_or("");
    let name = name.rsplit_once('.').map_or(name, |(stem, _)| stem).to_string();
    (name, dos_path(&nt_path).unwrap_or_default())
}

/// The kernel's name for the executable, like `\Device\HarddiskVolume3\Games\cs2.exe`.
fn nt_image_path(pid: u32) -> Option<String> {
    let mut buffer = vec![0u16; 512];
    for _ in 0..2 {
        let mut info = ProcessIdInformation {
            process_id: pid as usize as *mut c_void,
            image_name: UnicodeString {
                length: 0,
                maximum_length: (buffer.len() * 2) as u16,
                buffer: buffer.as_mut_ptr(),
            },
        };
        // SAFETY: `info` is the structure this class expects and its string
        // points at `buffer`, which is `maximum_length` bytes and outlives the call.
        let status = unsafe {
            NtQuerySystemInformation(
                SYSTEM_PROCESS_ID_INFORMATION,
                (&mut info as *mut ProcessIdInformation).cast(),
                size_of::<ProcessIdInformation>() as u32,
                std::ptr::null_mut(),
            )
        };
        if status == STATUS_INFO_LENGTH_MISMATCH {
            let needed = usize::from(info.image_name.maximum_length) / 2;
            if needed <= buffer.len() {
                return None;
            }
            buffer = vec![0u16; needed];
            continue;
        }
        let length = usize::from(info.image_name.length) / 2;
        if status < 0 || length == 0 || length > buffer.len() {
            return None;
        }
        return Some(String::from_utf16_lossy(&buffer[..length]));
    }
    None
}

/// Device name -> drive, like `\Device\HarddiskVolume3` -> `C:`. Filled on
/// first use and again when a path names a device not seen yet (a new drive).
static DRIVES: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

fn dos_path(nt_path: &str) -> Option<String> {
    const NETWORK: &str = "\\Device\\Mup\\";
    if let Some(share) = nt_path.strip_prefix(NETWORK) {
        return Some(format!("\\\\{share}"));
    }
    let mut drives = DRIVES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(path) = with_drive(&drives, nt_path) {
        return Some(path);
    }
    *drives = read_drives();
    with_drive(&drives, nt_path)
}

fn with_drive(drives: &[(String, String)], nt_path: &str) -> Option<String> {
    drives.iter().find_map(|(device, drive)| {
        // The `\` after the device keeps HarddiskVolume1 from matching HarddiskVolume10.
        let rest = nt_path.strip_prefix(device.as_str())?;
        rest.starts_with('\\').then(|| format!("{drive}{rest}"))
    })
}

fn read_drives() -> Vec<(String, String)> {
    // SAFETY: no arguments; returns a bit per existing drive letter.
    let mask = unsafe { GetLogicalDrives() };
    let mut drives = Vec::new();
    for letter in (0..26u8).filter(|bit| mask & (1 << bit) != 0).map(|bit| (b'A' + bit) as char) {
        let drive = format!("{letter}:");
        let wide: Vec<u16> = drive.encode_utf16().chain([0]).collect();
        let mut device = [0u16; 512];
        // SAFETY: `wide` is NUL-terminated and `device` is a writable buffer of the given length.
        let written = unsafe { QueryDosDeviceW(PCWSTR(wide.as_ptr()), Some(&mut device)) } as usize;
        if written == 0 {
            continue;
        }
        // The answer is a list of NUL-separated names; the first is the current mapping.
        let end = device.iter().position(|&unit| unit == 0).unwrap_or(written);
        drives.push((String::from_utf16_lossy(&device[..end]), drive));
    }
    drives
}
