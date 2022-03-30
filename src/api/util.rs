use std::fmt::Write;

use image::imageops;
use image::DynamicImage;
use sha1::{Digest, Sha1};
use worker::js_sys::Uint8Array;
use worker::wasm_bindgen::JsValue;

use crate::error::Result;

pub fn resize(image: &DynamicImage, new_width: u32) -> Vec<u8> {
    let width = image.width();
    let height = image.height();
    let resized = imageops::resize(
        image,
        new_width,
        height * (new_width / width),
        imageops::CatmullRom,
    );
    resized.into_raw()
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
