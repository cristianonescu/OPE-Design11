//! Embeds the app icon (`assets/icons/openstitch.ico`) in `openstitch.exe` on Windows, so the
//! taskbar, Alt+Tab, Explorer and Start-menu shortcuts show it. The MSVC linker takes a compiled
//! resource (`.res`) file as input; this writes one from the `.ico` directly (one `RT_ICON` per
//! image and an `RT_GROUP_ICON` with id 1), so no resource compiler is needed.

use std::path::PathBuf;

fn main() {
    let ico = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons/openstitch.ico");
    println!("cargo:rerun-if-changed={}", ico.display());
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !(windows && msvc) {
        return;
    }
    let data = std::fs::read(&ico).expect("assets/icons/openstitch.ico");
    let res = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("openstitch-icon.res");
    std::fs::write(&res, icon_res(&data).expect("a valid .ico")).expect("write .res");
    println!("cargo:rustc-link-arg-bin=openstitch={}", res.display());
}

const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;

/// The `.res` file for the icon file `ico`.
fn icon_res(ico: &[u8]) -> Option<Vec<u8>> {
    let u16_at = |i: usize| Some(u16::from_le_bytes(ico.get(i..i + 2)?.try_into().ok()?));
    let u32_at = |i: usize| Some(u32::from_le_bytes(ico.get(i..i + 4)?.try_into().ok()?));
    if u16_at(2)? != 1 {
        return None;
    }
    let count = u16_at(4)?;
    // The first entry of every .res file is an empty one.
    let mut out = Vec::new();
    resource(&mut out, 0, 0, 0, &[]);
    let mut group = Vec::new();
    group.extend_from_slice(&[0, 0, 1, 0]);
    group.extend_from_slice(&count.to_le_bytes());
    for k in 0..count {
        let e = 6 + 16 * k as usize;
        let (size, offset) = (u32_at(e + 8)? as usize, u32_at(e + 12)? as usize);
        let id = k + 1;
        resource(
            &mut out,
            RT_ICON,
            id,
            0x1010,
            ico.get(offset..offset + size)?,
        );
        // GRPICONDIRENTRY: the ICONDIRENTRY's first 12 bytes, then the RT_ICON id.
        group.extend_from_slice(ico.get(e..e + 12)?);
        group.extend_from_slice(&id.to_le_bytes());
    }
    resource(&mut out, RT_GROUP_ICON, 1, 0x1030, &group);
    Some(out)
}

/// Appends one resource with an ordinal type and name (US English).
fn resource(out: &mut Vec<u8>, ty: u16, name: u16, flags: u16, data: &[u8]) {
    let header_size: u32 = 32;
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&header_size.to_le_bytes());
    for v in [0xffff, ty, 0xffff, name] {
        out.extend_from_slice(&u16::to_le_bytes(v));
    }
    out.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    out.extend_from_slice(&flags.to_le_bytes());
    let lang: u16 = if ty == 0 { 0 } else { 0x0409 };
    out.extend_from_slice(&lang.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // Version
    out.extend_from_slice(&0u32.to_le_bytes()); // Characteristics
    out.extend_from_slice(data);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}
