//! Installed apps, as Start's "All apps" list shows them, plus their icons.
//!
//! Store (packaged) apps are identified by their AppUserModelID, desktop apps
//! by the executable their Start menu shortcut points at.

use crate::Result;
use serde::Serialize;
use windows::core::{Interface, PCWSTR, PWSTR};
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::Storage::EnhancedStorage::{
    PKEY_AppUserModel_ID, PKEY_Link_Arguments, PKEY_Link_TargetParsingPath,
};
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, IShellItem2,
    IShellItemImageFactory, SHCreateItemFromParsingName, SHGetKnownFolderItem, KF_FLAG_DEFAULT,
    SIGDN_NORMALDISPLAY, SIIGBF_ICONONLY,
};

/// One launchable entry from Start's app list.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub name: String,
    /// AppUserModelID of a Store app; empty for desktop apps.
    pub app_id: String,
    /// Executable of a desktop app; empty for Store apps.
    pub path: String,
    pub arguments: String,
    /// 32 px PNG as a `data:` URL, when Windows has an icon.
    pub icon: Option<String>,
}

/// COM for the calling thread, released on drop when this call initialized it.
pub(crate) struct ComGuard(bool);

impl ComGuard {
    pub(crate) fn new() -> Self {
        ComGuard(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok())
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

fn take_string(value: PWSTR) -> String {
    let text = unsafe { value.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(value.0 as *const _)) };
    text
}

fn item_string(
    item: &IShellItem2,
    key: &windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY,
) -> String {
    unsafe { item.GetString(key) }
        .map(take_string)
        .unwrap_or_default()
}

/// The package family of a Store app's AppUserModelID (`Family!App`).
pub fn package_family(app_id: &str) -> Option<&str> {
    let (family, app) = app_id.split_once('!')?;
    (!family.is_empty() && !app.is_empty() && !family.contains('\\')).then_some(family)
}

/// List the apps in Start's "All apps", sorted by name. Icons take most of
/// the time, so a caller can list without them first.
pub fn list_installed_apps(self_exe: &str, with_icons: bool) -> Result<Vec<InstalledApp>> {
    let _com = ComGuard::new();
    let folder: IShellItem =
        unsafe { SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, None)? };
    let items: IEnumShellItems = unsafe { folder.BindToHandler(None, &BHID_EnumItems)? };
    let mut apps = Vec::new();
    loop {
        let mut batch = [None];
        let mut fetched = 0;
        if unsafe { items.Next(&mut batch, Some(&mut fetched)) }.is_err() || fetched == 0 {
            break;
        }
        let Some(item) = batch[0].take() else { break };
        if let Some(app) = describe(&item, self_exe, with_icons) {
            apps.push(app);
        }
    }
    apps.sort_by_key(|app| app.name.to_lowercase());
    apps.dedup_by(|a, b| a.name == b.name && a.app_id == b.app_id && a.path == b.path);
    Ok(apps)
}

fn describe(item: &IShellItem, self_exe: &str, with_icon: bool) -> Option<InstalledApp> {
    let item2: IShellItem2 = item.cast().ok()?;
    let name = unsafe { item.GetDisplayName(SIGDN_NORMALDISPLAY) }
        .map(take_string)
        .ok()?;
    let app_id = item_string(&item2, &PKEY_AppUserModel_ID);
    let target = item_string(&item2, &PKEY_Link_TargetParsingPath);
    let (app_id, path, arguments) = if target.to_lowercase().ends_with(".exe") {
        // A desktop shortcut. Skip this app and the launchers it creates.
        if target.eq_ignore_ascii_case(self_exe) || !std::path::Path::new(&target).is_file() {
            return None;
        }
        (
            String::new(),
            target,
            item_string(&item2, &PKEY_Link_Arguments),
        )
    } else if package_family(&app_id)
        .is_some_and(|family| !crate::package_launch::packages_for_family(family).is_empty())
    {
        (app_id, String::new(), String::new())
    } else {
        return None; // Web links, Control Panel items, documents.
    };
    Some(InstalledApp {
        name,
        app_id,
        path,
        arguments,
        icon: with_icon
            .then(|| icon_png(item, 32).map(png_data_url))
            .flatten(),
    })
}

/// Look up one app's shell item: `shell:AppsFolder\<id>` or an executable.
pub(crate) fn shell_item(app_id: &str, path: &str) -> Result<IShellItem> {
    let parsing = if app_id.is_empty() {
        path.to_string()
    } else {
        format!("shell:AppsFolder\\{app_id}")
    };
    let wide: Vec<u16> = parsing.encode_utf16().chain(Some(0)).collect();
    Ok(unsafe { SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)? })
}

/// Render an item's icon at `size` px as PNG bytes.
pub(crate) fn icon_png(item: &IShellItem, size: i32) -> Option<Vec<u8>> {
    let factory: IShellItemImageFactory = item.cast().ok()?;
    let bitmap = unsafe { factory.GetImage(SIZE { cx: size, cy: size }, SIIGBF_ICONONLY) }.ok()?;
    let pixels = bitmap_rgba(bitmap);
    unsafe {
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
    }
    let (width, height, rgba) = pixels?;
    encode_png(width, height, &rgba)
}

/// Read a 32-bit bitmap as straight (not premultiplied) RGBA, top row first.
fn bitmap_rgba(bitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    unsafe {
        let mut info = BITMAP::default();
        if GetObjectW(
            HGDIOBJ(bitmap.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut info as *mut _ as *mut _),
        ) == 0
        {
            return None;
        }
        let (width, height) = (info.bmWidth, info.bmHeight.abs());
        if width <= 0 || height <= 0 {
            return None;
        }
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; (width * height * 4) as usize];
        let dc = GetDC(None);
        let lines = GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(bgra.as_mut_ptr() as *mut _),
            &mut header,
            DIB_RGB_COLORS,
        );
        ReleaseDC(None, dc);
        if lines == 0 {
            return None;
        }
        Some((width as u32, height as u32, bgra_to_rgba(bgra)))
    }
}

/// Convert premultiplied BGRA to straight RGBA. An all-zero alpha channel
/// means the bitmap has no alpha, so it is treated as opaque.
fn bgra_to_rgba(mut pixels: Vec<u8>) -> Vec<u8> {
    let has_alpha = pixels.as_chunks::<4>().0.iter().any(|p| p[3] != 0);
    for p in pixels.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
        if !has_alpha {
            p[3] = 255;
        } else if p[3] != 0 && p[3] != 255 {
            let alpha = p[3] as u32;
            for channel in &mut p[0..3] {
                *channel = ((*channel as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    pixels
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().ok()?.write_image_data(rgba).ok()?;
    Some(out)
}

fn png_data_url(png: Vec<u8>) -> String {
    format!("data:image/png;base64,{}", base64(&png))
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// An `.ico` file holding PNG images, which Windows Vista and later read.
pub(crate) fn ico_from_pngs(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0, 0, 1, 0];
    out.extend_from_slice(&(images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (size, png) in images {
        let dim = if *size >= 256 { 0 } else { *size as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0, 1, 0, 32, 0]);
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in images {
        out.extend_from_slice(png);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_store_app_ids() {
        assert_eq!(
            package_family("5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App"),
            Some("5319275A.WhatsAppDesktop_cv1g1gvanyjgm")
        );
        assert_eq!(package_family("Microsoft.Windows.Explorer"), None);
        assert_eq!(
            package_family("{6D809377-6AF0-444B-8957-A3773F02200E}\\app.exe"),
            None
        );
        assert_eq!(package_family("Family!"), None);
    }

    #[test]
    fn encodes_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn unpremultiplies_icon_pixels() {
        // Half-transparent pure red, premultiplied BGRA.
        assert_eq!(bgra_to_rgba(vec![0, 0, 128, 128]), vec![255, 0, 0, 128]);
        // No alpha channel at all: opaque.
        assert_eq!(bgra_to_rgba(vec![1, 2, 3, 0]), vec![3, 2, 1, 255]);
    }

    #[test]
    fn writes_an_ico_directory() {
        let ico = ico_from_pngs(&[(32, vec![1, 2]), (256, vec![3])]);
        assert_eq!(&ico[0..6], &[0, 0, 1, 0, 2, 0]);
        assert_eq!(ico[6], 32);
        assert_eq!(u32::from_le_bytes(ico[18..22].try_into().unwrap()), 38);
        assert_eq!(ico[22], 0); // 256 px is stored as 0
        assert_eq!(u32::from_le_bytes(ico[34..38].try_into().unwrap()), 40);
        assert_eq!(&ico[38..], &[1, 2, 3]);
    }
}
