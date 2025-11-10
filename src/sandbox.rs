/// macOS sandbox support using sandbox_init
///
/// This module provides a safe abstraction over macOS's sandbox_init API
/// to restrict the application's access to only what it needs.
///
/// The sandbox is ONLY enabled for the main transcription app, not for:
/// - `download-model` command (needs network access)
/// - `test-record` command (needs file system access for test recordings)
/// - `test-replay` command (needs file system access)

#[cfg(target_os = "macos")]
pub mod macos {
    use std::ffi::CString;
    use std::ptr;
    use anyhow::{Result, Context};

    // FFI declarations for macOS sandbox_init
    extern "C" {
        fn sandbox_init(
            profile: *const libc::c_char,
            flags: u64,
            errorbuf: *mut *mut libc::c_char,
        ) -> libc::c_int;

        fn sandbox_free_error(errorbuf: *mut libc::c_char);
    }

    /// Initialize the macOS sandbox with a restrictive profile
    ///
    /// This should be called early in main(), before any privileged operations.
    /// Once sandboxed, the process cannot escape the restrictions.
    pub fn init() -> Result<()> {
        // Get the home directory for path substitution
        let home = dirs::home_dir()
            .context("Failed to get home directory")?;
        let home_str = home.to_string_lossy();
        let config_dir = format!("{}/.live-transcribe", home_str);
        let cache_dir = format!("{}/Library/Caches/live-transcribe", home_str);

        // Sandbox profile in Scheme-based SBPL (Sandbox Profile Language)
        // This is the format macOS uses for sandbox profiles
        let profile = format!(
            r#"
(version 1)

;; Deny everything by default (allowlist approach)
;; The "with message" directive helps debug sandbox denials via:
;; log stream --predicate 'sender=="Sandbox" and eventMessage contains "live-transcribe-sandbox-deny"'
(deny default (with message "live-transcribe-sandbox-deny"))

;; Allow basic system operations
(allow process-exec)
(allow process-fork)
(allow signal)
(allow sysctl-read)
(allow sysctl-write)
(allow mach-lookup)

;; Allow audio input (microphone)
(allow device-microphone)
(allow iokit-open (iokit-user-client-class "IOAudioControlUserClient"))
(allow iokit-open (iokit-user-client-class "IOAudioEngineUserClient"))

;; Allow reading everything (no network means no data exfiltration risk)
(allow file-read-metadata file-read-data file-map-executable
    (subpath "/")
)

;; Allow writing to our config directory
(allow file-write* file-write-create
    (subpath "{config_dir}")
)

;; Allow writing to app cache directory (for CoreML/Neural Engine)
(allow file-write* file-write-create
    (subpath "{cache_dir}")
)

;; Allow file extensions for CoreML to share files with ANECompilerService
(allow file-issue-extension
    (require-all
        (extension-class "com.apple.app-sandbox.read")
        (subpath "{config_dir}")
    )
)

;; Allow writing to system cache and temp directories (Metal Performance Shaders, etc.)
(allow file-write* file-write-create file-write-unlink
    (subpath "/private/var/folders")
)

;; Explicitly deny network access (we're fully offline)
(deny network*)

;; Allow CoreML and Metal for GPU acceleration
(allow iokit-open (iokit-user-client-class "AGPMClient"))
(allow iokit-open (iokit-user-client-class "AppleIntelMEUserClient"))
(allow iokit-open (iokit-user-client-class "IOAcceleratorUserClient"))
(allow iokit-open (iokit-user-client-class "IOSurfaceRootUserClient"))
(allow iokit-open (iokit-user-client-class "IOAccelContext"))
(allow iokit-open (iokit-user-client-class "IOAccelDevice"))
(allow iokit-open (iokit-user-client-class "IOAccelSharedUserClient"))
(allow iokit-open (iokit-user-client-class "IOAccelSubmitter2"))
(allow iokit-open (iokit-user-client-class "AGXDeviceUserClient"))
(allow iokit-open (iokit-user-client-class "H11ANEInDirectPathClient"))

;; Allow keyboard input for text injection
(allow iokit-open (iokit-user-client-class "IOHIDParamUserClient"))

;; Allow mach services needed for CoreML
(allow mach-lookup
    (global-name "com.apple.coreml.mlhostd")
    (global-name "com.apple.audio.AudioComponentRegistrar")
    (global-name "com.apple.audio.coreaudiod")
    (global-name "com.apple.coreservices.launchservicesd")
    (global-name "com.apple.SystemConfiguration.configd")
)

;; Allow sysctl operations needed by CoreML
(allow sysctl-read)
(allow sysctl-write)

;; Allow user preference reading (needed by CoreML)
(allow user-preference-read)

;; Allow accessibility for keyboard injection
;; Note: This still requires system-level accessibility permissions
(allow mach-lookup (global-name "com.apple.accessibility.api"))

;; Allow basic IPC for event loop
(allow ipc-posix-shm-read-data)
(allow ipc-posix-shm-write-data)
"#,
            config_dir = config_dir,
            cache_dir = cache_dir
        );

        let profile_cstr = CString::new(profile)
            .context("Failed to create CString from profile")?;

        let mut error_buf: *mut libc::c_char = ptr::null_mut();

        // Call sandbox_init
        // flags = 0 means use default behavior
        let result = unsafe {
            sandbox_init(
                profile_cstr.as_ptr(),
                0, // flags
                &mut error_buf as *mut *mut libc::c_char,
            )
        };

        if result != 0 {
            let error_msg = if !error_buf.is_null() {
                let error_str = unsafe {
                    let msg = std::ffi::CStr::from_ptr(error_buf)
                        .to_string_lossy()
                        .to_string();
                    sandbox_free_error(error_buf);
                    msg
                };
                error_str
            } else {
                "Unknown sandbox error".to_string()
            };

            anyhow::bail!("Failed to initialize sandbox: {}", error_msg);
        }

        println!("✅ Sandbox initialized successfully");
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
pub mod macos {
    use anyhow::Result;

    pub fn init() -> Result<()> {
        // Sandbox only supported on macOS
        println!("⚠️  Sandbox not available on this platform");
        Ok(())
    }
}
