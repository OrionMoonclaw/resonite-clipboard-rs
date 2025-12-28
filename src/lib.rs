use anyhow::Result;
use anyhow::anyhow;

use clipboard_rs::{Clipboard, ClipboardContext};
use std::ffi::{CStr, c_char, c_uchar};
use std::io::Read;
use std::ptr::null;
use std::slice;
use std::sync::{Mutex, OnceLock};
use wl_clipboard_rs::copy::{self, Options};
use wl_clipboard_rs::paste::{self, ClipboardType, Seat, get_contents, get_mime_types};

static CLIPBOARD_CTX: OnceLock<Mutex<Result<ClipboardContext>>> = OnceLock::new();

fn get_clipboard_ctx()
-> Result<std::sync::MutexGuard<'static, Result<ClipboardContext, anyhow::Error>>> {
    CLIPBOARD_CTX
        .get_or_init(|| Mutex::new(ClipboardContext::new().map_err(|e| anyhow::anyhow!(e))))
        .lock()
        .map_err(|e| anyhow!("failed to lock clipboard context: {e}"))
}

fn copy_auto_impl(data: &[u8]) -> Result<()> {
    match Options::new().copy(
        wl_clipboard_rs::copy::Source::Bytes(data.into()),
        copy::MimeType::Autodetect,
    ) {
        Ok(_) => return Ok(()),
        Err(e) => eprintln!("falling back to clipboard_rs, wl-clipboard-rs failed with error {e}"),
    }

    let mime_type = infer::get(data)
        .map(|k| k.mime_type())
        .unwrap_or("application/octet-stream");

    let mut ctx_guard = get_clipboard_ctx()?;
    let ctx = ctx_guard
        .as_mut()
        .map_err(|e| anyhow!("clipboard context error: {e}"))?;

    ctx.set_buffer(mime_type, data.to_vec())
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(())
}

fn copy_text_impl(text: String) -> Result<()> {
    match Options::new().copy(
        wl_clipboard_rs::copy::Source::Bytes(text.clone().into_bytes().into()),
        copy::MimeType::Autodetect,
    ) {
        Ok(_) => return Ok(()),
        Err(e) => eprintln!("falling back to clipboard_rs, wl-clipboard-rs failed with error {e}"),
    }

    let mut ctx_guard = get_clipboard_ctx()?;
    let ctx = ctx_guard
        .as_mut()
        .map_err(|e| anyhow!("clipboard context error: {e}"))?;

    ctx.set_text(text).map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

fn copy_with_type_impl(data: &[u8], mime_type: &str) -> Result<()> {
    match Options::new().copy(
        wl_clipboard_rs::copy::Source::Bytes(data.into()),
        copy::MimeType::Specific(mime_type.to_string()),
    ) {
        Ok(_) => return Ok(()),
        Err(e) => eprintln!("falling back to clipboard_rs, wl-clipboard-rs failed with error {e}"),
    }

    let mut ctx_guard = get_clipboard_ctx()?;
    let ctx = ctx_guard
        .as_mut()
        .map_err(|e| anyhow!("clipboard context error: {e}"))?;

    ctx.set_buffer(mime_type, data.to_vec())
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

fn available_mime_types_impl() -> Result<Vec<u8>> {
    match get_mime_types(ClipboardType::Regular, Seat::Unspecified) {
        Ok(t) => {
            let concatenated = t.into_iter().fold(String::new(), |mut acc, t| {
                acc.push_str(&t);
                acc.push('\n');
                acc
            });
            return Ok(concatenated.into_bytes());
        }
        Err(e) => eprintln!("falling back to clipboard_rs, wl-clipboard-rs failed with error {e}"),
    }

    let mut ctx_guard = get_clipboard_ctx()?;
    let ctx = ctx_guard
        .as_mut()
        .map_err(|e| anyhow!("clipboard context error: {e}"))?;

    let mut formats = ctx.available_formats().map_err(|e| anyhow::anyhow!(e))?;

    // Resonite checks for this specific mime type and without it won't try to get text
    if formats.iter().any(|f| f == "UTF8_STRING") {
        formats.push("text/plain;charset=utf-8".to_string());
    }
    let concatenated = formats.join("\n") + "\n";
    Ok(concatenated.into_bytes())
}

fn paste_with_type_impl(mime_type: &str) -> Result<Vec<u8>> {
    match get_contents(
        ClipboardType::Regular,
        Seat::Unspecified,
        paste::MimeType::Specific(mime_type),
    ) {
        Ok((mut p, _)) => {
            let mut contents = Vec::new();
            match p.read_to_end(&mut contents) {
                Ok(_) => {
                    return Ok(contents);
                }
                Err(e) => {
                    eprintln!("falling back to clipboard_rs, wl-clipboard-rs failed with error {e}")
                }
            }
        }
        Err(e) => eprintln!("falling back to clipboard_rs, wl-clipboard-rs failed with error {e}"),
    }

    let mut ctx_guard = get_clipboard_ctx()?;
    let ctx = ctx_guard
        .as_mut()
        .map_err(|e| anyhow!("clipboard context error: {e}"))?;

    let buf = ctx.get_buffer(mime_type).map_err(|e| anyhow::anyhow!(e))?;
    Ok(buf)
}

fn paste_auto_impl() -> Result<Vec<u8>> {
    match get_contents(
        ClipboardType::Regular,
        Seat::Unspecified,
        paste::MimeType::Any,
    ) {
        Ok((mut p, _)) => {
            let mut contents = Vec::new();
            match p.read_to_end(&mut contents) {
                Ok(_) => {
                    return Ok(contents);
                }
                Err(e) => {
                    eprintln!("falling back to clipboard_rs, wl-clipboard-rs failed with error {e}")
                }
            }
        }
        Err(e) => eprintln!("falling back to clipboard_rs, wl-clipboard-rs failed with error {e}"),
    }

    let mut ctx_guard = get_clipboard_ctx()?;
    let ctx = ctx_guard
        .as_mut()
        .map_err(|e| anyhow!("clipboard context error: {e}"))?;

    let formats = ctx.available_formats().map_err(|e| anyhow::anyhow!(e))?;
    let first = formats
        .first()
        .ok_or_else(|| anyhow::anyhow!("no formats available"))?;
    let buf = ctx.get_buffer(first).map_err(|e| anyhow::anyhow!(e))?;
    Ok(buf)
}

fn paste_text_impl() -> Result<Vec<u8>> {
    match get_contents(
        ClipboardType::Regular,
        Seat::Unspecified,
        paste::MimeType::Text,
    ) {
        Ok((mut p, _)) => {
            let mut contents = Vec::new();
            match p.read_to_end(&mut contents) {
                Ok(_) => {
                    return Ok(contents);
                }
                Err(e) => {
                    eprintln!("falling back to clipboard_rs, wl-clipboard-rs failed with error {e}")
                }
            }
        }
        Err(e) => eprintln!("falling back to clipboard_rs, wl-clipboard-rs failed with error {e}"),
    }

    let mut ctx_guard = get_clipboard_ctx()?;
    let ctx = ctx_guard
        .as_mut()
        .map_err(|e| anyhow!("clipboard context error: {e}"))?;

    let text = ctx.get_text().map_err(|e| anyhow::anyhow!(e))?;
    Ok(text.into_bytes())
}

fn alloc_and_copy(bytes: &[u8]) -> (*const c_uchar, usize) {
    let allocated = unsafe { libc::malloc(bytes.len()) };
    if allocated.is_null() {
        return (null(), 0);
    }

    unsafe {
        slice::from_raw_parts_mut(allocated as *mut u8, bytes.len()).copy_from_slice(bytes);
    }

    (allocated as *const c_uchar, bytes.len())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn copy_auto(data: *const c_uchar, data_length: u32) {
    let data_array = unsafe { slice::from_raw_parts(data, data_length as usize) };
    if let Err(e) = copy_auto_impl(data_array) {
        eprintln!("copy_auto error: {e}");
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn copy_text(data: *const c_char) {
    let data_cstr = unsafe { CStr::from_ptr(data) };
    let text = data_cstr.to_string_lossy().to_string();
    if let Err(e) = copy_text_impl(text) {
        eprintln!("copy_text error: {e}");
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn copy_with_type(
    data: *const c_uchar,
    data_length: u32,
    mime_type_raw: *const c_char,
) {
    let data_slice = unsafe { slice::from_raw_parts(data, data_length as usize) };
    let mime_type = unsafe { CStr::from_ptr(mime_type_raw).to_string_lossy() };
    if let Err(e) = copy_with_type_impl(data_slice, &mime_type) {
        eprintln!("copy_with_type error: {e}");
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn available_mime_types(size: *mut u32) -> *const c_uchar {
    match available_mime_types_impl() {
        Ok(bytes) => {
            unsafe { size.write(bytes.len() as u32) };
            let (ptr, _) = alloc_and_copy(&bytes);
            ptr
        }
        Err(e) => {
            eprintln!("available_mime_types error: {e}");
            unsafe { size.write(0) };
            null()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paste_with_type(
    mime_type_raw: *const c_char,
    size: *mut u32,
) -> *const c_uchar {
    let mime_type = unsafe { CStr::from_ptr(mime_type_raw).to_string_lossy() };
    match paste_with_type_impl(&mime_type) {
        Ok(buf) => {
            let (ptr, len) = alloc_and_copy(&buf);
            unsafe { size.write(len as u32) };
            ptr
        }
        Err(e) => {
            eprintln!("paste_with_type error: {e}");
            unsafe { size.write(0) };
            null()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paste_auto(size: *mut u32) -> *const c_uchar {
    match paste_auto_impl() {
        Ok(buf) => {
            let (ptr, len) = alloc_and_copy(&buf);
            unsafe { size.write(len as u32) };
            ptr
        }
        Err(e) => {
            eprintln!("paste_auto error: {e}");
            unsafe { size.write(0) };
            null()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn paste_text(size: *mut u32) -> *const c_uchar {
    match paste_text_impl() {
        Ok(buf) => {
            let (ptr, len) = alloc_and_copy(&buf);
            unsafe { size.write(len as u32) };
            ptr
        }
        Err(e) => {
            eprintln!("paste_text error: {e}");
            unsafe { size.write(0) };
            null()
        }
    }
}
