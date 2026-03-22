use enigo::{Enigo, Key, Keyboard, Settings};
use std::thread::sleep;
use std::time::Duration;
use arboard::Clipboard;

pub fn inject_text(text: String) {
    let mut clipboard = Clipboard::new().unwrap();
    
    // 1. Write to clipboard
    let _ = clipboard.set_text(text);
    
    // 2. Simulate Ctrl+V after enough delay for window to hide
    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    sleep(Duration::from_millis(300)); // Increased for safety
    
    #[cfg(target_os = "windows")]
    {
        let _ = enigo.key(Key::Control, enigo::Direction::Press);
        sleep(Duration::from_millis(50));
        let _ = enigo.key(Key::Unicode('v'), enigo::Direction::Click);
        sleep(Duration::from_millis(50));
        let _ = enigo.key(Key::Control, enigo::Direction::Release);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = enigo.key(Key::Meta, enigo::Direction::Press);
        let _ = enigo.key(Key::Unicode('v'), enigo::Direction::Click);
        let _ = enigo.key(Key::Meta, enigo::Direction::Release);
    }
}
