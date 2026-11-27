//! The launcher's resources: its icon (drawn here, so the repository holds no
//! binary file), the manifest (common controls 6, so the controls look like
//! Windows 10/11 ones, and DPI awareness) and the version block, compiled with
//! the Windows SDK's `rc.exe`; and the build label (the commit).

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    label();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let Some(rc) = find_rc() else {
        println!(
            "cargo:warning=rc.exe not found (Windows SDK): the launcher gets no icon and old-style controls"
        );
        return;
    };
    std::fs::write(out.join("opti.ico"), icon(&[16, 24, 32, 48, 64, 256])).expect("icon");
    std::fs::write(out.join("opti.manifest"), MANIFEST).expect("manifest");
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".into());
    let nums: Vec<u32> = version
        .split('.')
        .map(|p| p.parse().unwrap_or(0))
        .chain(std::iter::repeat(0))
        .take(4)
        .collect();
    let script = format!(
        r#"1 ICON "{ico}"
1 24 "{manifest}"
1 VERSIONINFO
FILEVERSION {a},{b},{c},{d}
PRODUCTVERSION {a},{b},{c},{d}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "FileDescription", "opti-vicity launcher"
      VALUE "ProductName", "opti-vicity"
      VALUE "FileVersion", "{version}"
      VALUE "ProductVersion", "{version}"
      VALUE "OriginalFilename", "opti-vicity.exe"
      VALUE "LegalCopyright", "GPL-3.0-or-later"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        ico = escaped(&out.join("opti.ico")),
        manifest = escaped(&out.join("opti.manifest")),
        a = nums[0],
        b = nums[1],
        c = nums[2],
        d = nums[3],
    );
    let rc_file = out.join("opti.rc");
    std::fs::write(&rc_file, script).expect("rc script");
    let res = out.join("opti.res");
    let status = std::process::Command::new(&rc)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res)
        .arg(&rc_file)
        .status();
    match status {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", res.display()),
        other => println!("cargo:warning=rc.exe failed ({other:?}): the launcher gets no icon"),
    }
}

/// The short commit, shown in the launcher.
fn label() {
    let root =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default()).join("../..");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    let commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=OPTI_BUILD={commit}");
    if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]).map(PathBuf::from) {
        let branch = git(&["symbolic-ref", "-q", "HEAD"]).map(|r| dir.join(r));
        for path in [
            Some(dir.join("HEAD")),
            branch,
            Some(dir.join("packed-refs")),
        ]
        .into_iter()
        .flatten()
        {
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}

fn escaped(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

/// `rc.exe`: `RC` if set, else the newest x64 one in the Windows 10/11 SDK.
fn find_rc() -> Option<PathBuf> {
    if let Some(rc) = std::env::var_os("RC") {
        return Some(PathBuf::from(rc));
    }
    let kits = PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&kits)
        .ok()?
        .flatten()
        .map(|e| e.path().join(r"x64\rc.exe"))
        .filter(|p| p.is_file())
        .collect();
    found.sort();
    found.pop()
}

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="opti-vicity.launcher" version="1.0.0.0" processorArchitecture="*"/>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0"
        processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/>
    </dependentAssembly>
  </dependency>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
    </windowsSettings>
  </application>
</assembly>
"#;

/// The icon: a rounded square in a blue-to-violet gradient with a white
/// "play" triangle, at each size (4x4 supersampled), as 32-bit BMP entries.
fn icon(sizes: &[u32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| bmp_entry(s, &render(s))).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&s, image) in sizes.iter().zip(&images) {
        let side = if s >= 256 { 0 } else { s as u8 };
        out.extend_from_slice(&[side, side, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(image.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += image.len() as u32;
    }
    for image in images {
        out.extend_from_slice(&image);
    }
    out
}

/// RGBA, top row first.
fn render(size: u32) -> Vec<u8> {
    const SS: u32 = 4;
    let n = size as f32;
    let radius = n * 0.22;
    let inside_square = |x: f32, y: f32| {
        let (cx, cy) = (x.clamp(radius, n - radius), y.clamp(radius, n - radius));
        let (dx, dy) = (x - cx, y - cy);
        dx * dx + dy * dy <= radius * radius
    };
    // The play triangle: a little right of the middle.
    let (ax, ay) = (n * 0.38, n * 0.27);
    let (bx, by) = (n * 0.38, n * 0.73);
    let (tx, ty) = (n * 0.75, n * 0.50);
    let edge = |px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32| {
        (x1 - x0) * (py - y0) - (y1 - y0) * (px - x0)
    };
    let inside_play = |x: f32, y: f32| {
        let (e0, e1, e2) = (
            edge(x, y, ax, ay, bx, by),
            edge(x, y, bx, by, tx, ty),
            edge(x, y, tx, ty, ax, ay),
        );
        (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0) || (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0)
    };
    let mut px = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let (mut cover, mut white) = (0u32, 0u32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let fx = x as f32 + (sx as f32 + 0.5) / SS as f32;
                    let fy = y as f32 + (sy as f32 + 0.5) / SS as f32;
                    if inside_square(fx, fy) {
                        cover += 1;
                        if inside_play(fx, fy) {
                            white += 1;
                        }
                    }
                }
            }
            let total = SS * SS;
            let t = (x + y) as f32 / (2.0 * n);
            let (r0, g0, b0) = (0x2f as f32, 0x6b as f32, 0xff as f32);
            let (r1, g1, b1) = (0x8b as f32, 0x3d as f32, 0xf0 as f32);
            let w = white as f32 / cover.max(1) as f32;
            let mix = |a: f32, b: f32| ((a + (b - a) * t) * (1.0 - w) + 255.0 * w).round() as u8;
            let i = ((y * size + x) * 4) as usize;
            px[i] = mix(r0, r1);
            px[i + 1] = mix(g0, g1);
            px[i + 2] = mix(b0, b1);
            px[i + 3] = (cover * 255 / total) as u8;
        }
    }
    px
}

fn bmp_entry(size: u32, rgba: &[u8]) -> Vec<u8> {
    let mask_row = size.div_ceil(32) * 4;
    let mut out = Vec::new();
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(size as i32).to_le_bytes());
    out.extend_from_slice(&((size * 2) as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(size * size * 4 + mask_row * size).to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    for y in (0..size).rev() {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            out.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    out.extend(std::iter::repeat_n(0u8, (mask_row * size) as usize));
    out
}
