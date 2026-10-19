#![no_main]

use libfuzzer_sys::fuzz_target;

// Any input must produce Ok or Err from the shv reader, never a panic or hang.
fuzz_target!(|data: &[u8]| {
    let _ = openstitch_formats::shv::read(data);
});
