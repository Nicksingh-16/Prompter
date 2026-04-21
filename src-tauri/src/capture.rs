use enigo::{Enigo, Key, Keyboard, Settings, Direction};

// ── Active app detection ───────────────────────────────────────────────────

/// Returns the lowercase executable name of the foreground window's process
/// (e.g. `"code.exe"`, `"chrome.exe"`). Used to enrich the prompt with
/// application context so the AI knows whether it's inside an IDE, email client, etc.
#[cfg(target_os = "windows")]
pub fn get_active_app() -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    extern "system" {
        fn GetForegroundWindow() -> isize;
        fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn QueryFullProcessImageNameW(handle: isize, flags: u32, buf: *mut u16, size: *mut u32) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 { return None; }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 { return None; }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 { return None; }
        let mut buf = [0u16; 260];
        let mut size = 260u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(handle);
        if ok == 0 { return None; }
        OsString::from_wide(&buf[..size as usize])
            .to_string_lossy()
            .split(['\\', '/'])
            .last()
            .map(|s| s.to_lowercase())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_active_app() -> Option<String> { None }

/// Map a process name to a human-readable app category for prompt enrichment.
pub fn classify_app(exe: &str) -> &'static str {
    match exe {
        e if e.contains("code") || e.contains("devenv") || e.contains("rider")
            || e.contains("idea") || e.contains("clion") || e.contains("cursor") => "code_editor",
        e if e.contains("chrome") || e.contains("msedge") || e.contains("firefox")
            || e.contains("brave") || e.contains("opera") => "browser",
        e if e.contains("outlook") || e.contains("thunderbird") => "email_client",
        e if e.contains("slack") || e.contains("teams") || e.contains("discord")
            || e.contains("telegram") || e.contains("whatsapp") => "messaging",
        e if e.contains("word") || e.contains("excel") || e.contains("powerpnt")
            || e.contains("winword") => "office",
        e if e.contains("notion") || e.contains("obsidian") || e.contains("typora")
            || e.contains("roam") || e.contains("logseq") => "notes",
        e if e.contains("terminal") || e.contains("cmd") || e.contains("powershell")
            || e.contains("wt") || e.contains("windowsterminal") => "terminal",
        _ => "other",
    }
}
use std::thread::sleep;
use std::time::Duration;
use arboard::Clipboard;

pub fn capture_text() -> Option<String> {
    let mut clipboard = Clipboard::new().ok()?;
    
    // 1. Save original clipboard
    let original_content = clipboard.get_text().ok().unwrap_or_default();
    
    let mut enigo = Enigo::new(&Settings::default()).ok()?;
    
    // 2. MODIFIER RELEASE (Standardizing on Key::Alt and Key::Shift)
    #[cfg(target_os = "windows")]
    {
        let _ = enigo.key(Key::Alt, Direction::Release);
        sleep(Duration::from_millis(5));
        let _ = enigo.key(Key::Shift, Direction::Release);
        sleep(Duration::from_millis(5));
        let _ = enigo.key(Key::Control, Direction::Release);
        sleep(Duration::from_millis(5));
    }
    
    sleep(Duration::from_millis(50));
    
    // 3. Ctrl+C (Tightened)
    #[cfg(target_os = "windows")]
    {
        let _ = enigo.key(Key::Control, Direction::Press);
        sleep(Duration::from_millis(40));
        let _ = enigo.key(Key::C, Direction::Press);
        sleep(Duration::from_millis(40));
        let _ = enigo.key(Key::C, Direction::Release);
        sleep(Duration::from_millis(40));
        let _ = enigo.key(Key::Control, Direction::Release);
    }
    
    #[cfg(target_os = "macos")]
    {
        let _ = enigo.key(Key::Meta, Direction::Press);
        sleep(Duration::from_millis(40));
        let _ = enigo.key(Key::C, Direction::Press);
        sleep(Duration::from_millis(40));
        let _ = enigo.key(Key::C, Direction::Release);
        sleep(Duration::from_millis(40));
        let _ = enigo.key(Key::Meta, Direction::Release);
    }

    // 4. Wait for OS/Clipboard propagation
    sleep(Duration::from_millis(150));
    
    // 5. Read captured text
    let captured = clipboard.get_text().ok();
    
    // 6. Restore original clipboard
    let _ = clipboard.set_text(original_content.clone());
    
    captured
}
