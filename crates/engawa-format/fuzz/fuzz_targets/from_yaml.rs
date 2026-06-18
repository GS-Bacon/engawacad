#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Convert bytes to string — replace invalid UTF-8 as needed
    let s = String::from_utf8_lossy(data);

    // Document::from_yaml should never panic on any input
    let _ = engawa_format::Document::from_yaml(&s);
});
