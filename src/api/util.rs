use std::fmt::Write;

use image::imageops;
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use sha1::{Digest, Sha1};
use worker::js_sys::Uint8Array;
use worker::wasm_bindgen::JsValue;

use crate::error::{Error, Result};

pub fn resize(image: &DynamicImage, new_width: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let width = image.width();
    let height = image.height();
    imageops::resize(
        image,
        new_width,
        (height as f64 * (new_width as f64 / width as f64)) as u32,
        imageops::CatmullRom,
    )
}

pub fn bytes_to_js_value(bytes: &[u8]) -> JsValue {
    let typed_array = Uint8Array::new_with_length(bytes.len() as u32);
    typed_array.copy_from(bytes);
    typed_array.into()
}

pub fn get_hash(file: &[u8]) -> Result<String> {
    let mut hasher = Sha1::new();
    hasher.update(&file);
    let hash_buf = hasher.finalize_reset();
    let hash = encode_hex(&hash_buf);
    Ok(hash)
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        write!(&mut s, "{:02x}", b).unwrap();
    }
    s
}

pub fn get_filename_and_ext(
    scope: &str,
    filename: &str,
    format: &ImageFormat,
) -> Result<(String, String)> {
    let mut path = scope.to_string();
    path.push('/');
    path.push_str(filename);

    let extensions = format.extensions_str();
    let ext = extensions.iter().find(|&ext| filename.ends_with(*ext));
    let (name, ext) = if let Some(&ext) = ext {
        (path[..(ext.len() + 1)].to_string(), ext)
    } else {
        (
            path,
            *extensions
                .first()
                .ok_or_else(|| Error::InternalError("Cannot get extension".into()))?,
        )
    };

    Ok((name, ext.to_string()))
}
