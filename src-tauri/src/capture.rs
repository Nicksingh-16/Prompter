use enigo::{Enigo, Key, Keyboard, Settings, Direction};
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
