//! Microsoft Store (MSIX) specifics. A packaged app runs with Windows' package identity:
//! autostart goes through a StartupTask declared in the manifest, and files it writes
//! under %APPDATA% are redirected to a private per-package folder.

use std::path::PathBuf;

/// Package family name (e.g. `tmuxhints.dev_8wekyb3d8bbwe`) when running from an MSIX
/// package; `None` for the plain exe and on other platforms.
#[cfg(windows)]
pub fn family_name() -> Option<String> {
    use windows_sys::Win32::Storage::Packaging::Appx::GetCurrentPackageFamilyName;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

    let mut len: u32 = 0;
    // SAFETY: a null buffer with length 0 only asks for the required length. Unpackaged
    // processes get APPMODEL_ERROR_NO_PACKAGE instead.
    let rc = unsafe { GetCurrentPackageFamilyName(&mut len, std::ptr::null_mut()) };
    if rc != ERROR_INSUFFICIENT_BUFFER {
        return None;
    }
    let mut buf = vec![0u16; len as usize];
    // SAFETY: `buf` holds `len` u16s, as the first call asked for.
    let rc = unsafe { GetCurrentPackageFamilyName(&mut len, buf.as_mut_ptr()) };
    (rc == 0).then(|| String::from_utf16_lossy(&buf[..(len as usize).saturating_sub(1)]))
}

#[cfg(not(windows))]
pub fn family_name() -> Option<String> {
    None
}

pub fn is_packaged() -> bool {
    family_name().is_some()
}

/// Where a folder under %APPDATA% really lives on disk. Inside the package the app sees
/// %APPDATA% as usual, but Windows redirects folders that didn't exist before into the
/// package's LocalCache, which is where Explorer and editors outside the package have to
/// look. A folder that already existed (e.g. from the installer build) is used in place.
pub fn real_roaming_path(path: PathBuf) -> PathBuf {
    let (Some(family), Some(roaming), Some(local)) =
        (family_name(), dirs::config_dir(), dirs::data_local_dir())
    else {
        return path;
    };
    let Ok(rest) = path.strip_prefix(&roaming) else {
        return path;
    };
    let redirected = local
        .join("Packages")
        .join(family)
        .join("LocalCache")
        .join("Roaming")
        .join(rest);
    // LocalCache isn't virtualized, so this check sees the real disk.
    if redirected.exists() {
        redirected
    } else {
        path
    }
}

/// Start-at-logon through the manifest's `windows.startupTask` (the Run key isn't
/// honoured for packaged apps). Shown in Settings > Apps > Startup.
#[cfg(windows)]
pub mod startup_task {
    use windows::ApplicationModel::{StartupTask, StartupTaskState};
    use windows::core::HSTRING;

    /// Must match `TaskId` in packaging/msix/AppxManifest.xml.
    const TASK_ID: &str = "tmuxhintsStartup";

    /// Runs WinRT calls on a fresh (MTA) thread so blocking on them can't deadlock the
    /// UI thread that the tray menu runs on.
    fn off_ui_thread<T: Send>(
        f: impl FnOnce() -> windows::core::Result<T> + Send,
    ) -> Result<T, String> {
        std::thread::scope(|s| s.spawn(f).join())
            .map_err(|_| "startup task call panicked".to_string())?
            .map_err(|e| e.message().to_string())
    }

    fn task() -> windows::core::Result<StartupTask> {
        StartupTask::GetAsync(&HSTRING::from(TASK_ID))?.join()
    }

    pub fn is_enabled() -> Result<bool, String> {
        let state = off_ui_thread(|| task()?.State())?;
        Ok(state == StartupTaskState::Enabled || state == StartupTaskState::EnabledByPolicy)
    }

    pub fn set(enabled: bool) -> Result<(), String> {
        if !enabled {
            return off_ui_thread(|| task()?.Disable());
        }
        let state = off_ui_thread(|| task()?.RequestEnableAsync()?.join())?;
        match state {
            StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy => Ok(()),
            StartupTaskState::DisabledByUser => Err(
                "turned off in Settings > Apps > Startup; switch tmuxhints on there".to_string(),
            ),
            StartupTaskState::DisabledByPolicy => {
                Err("start at logon is blocked by your organization's policy".to_string())
            }
            other => Err(format!(
                "could not enable start at logon (state {})",
                other.0
            )),
        }
    }
}
