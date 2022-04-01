use std::io::Cursor;
use std::str;

use image::{DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;
use worker::*;

use super::auth::{authorize, AuthResponse};
use super::util;
use super::ImageInfo;
use super::{IMAGE_NS, WIDTHS};
use crate::error::Result;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct UploadUrlResponse {
    pub upload_url: String,
    pub authorization_token: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UploadFileResponse {
    pub file_id: String,
    pub file_name: String,
}

pub async fn upload(
    ctx: &RouteContext<worker::Context>,
    file: Vec<u8>,
    filename: &str,
) -> Result<String> {
    let auth = authorize(ctx).await?;

    let format = image::guess_format(&file)?;

    console_log!("Image format: {}", format.extensions_str().first().unwrap());
    let image = image::load_from_memory_with_format(&file, format)?;
    console_log!("Image loaded: {} bytes", image.as_bytes().len());

    let (name, ext) = util::get_filename_and_ext(filename, &format)?;
    let mime = mime_guess::from_ext(&ext)
        .first_or_octet_stream()
        .to_string();

    let width = image.width();
    let variants = WIDTHS
        .iter()
        .filter(|&w| *w < width)
        .copied()
        .collect::<Vec<_>>();

    console_log!("Width: {}, Variants: {:?}", width, variants);

    upload_variants(
        ctx,
        auth.clone(),
        image.clone(),
        variants.clone(),
        filename.to_string(),
        format,
    );
    let mut writer = Cursor::new(Vec::new());
    image.write_to(&mut writer, format)?;
    let res = upload_file(&writer.into_inner(), &auth, &mime, &name, "orig", &ext).await?;

    let kv_data = serde_json::to_string(&ImageInfo {
        id: res.file_id.clone(),
        name,
        format: format
            .extensions_str()
            .first()
            .expect("Unsupported format")
            .to_string(),
        width,
        variants,
    })?;
    ctx.kv(IMAGE_NS)?.put(filename, kv_data)?.execute().await?;

    console_log!("Uploaded {}", filename);

    Ok(res.file_id)
}

async fn get_upload_url(auth: &AuthResponse) -> Result<UploadUrlResponse> {
    let mut headers = Headers::new();
    headers.set("Authorization", auth.authorization_token.as_str())?;

    console_log!("{{\"bucketId\": \"{}\"}}", auth.allowed.bucket_id);

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from(format!(
            "{{\"bucketId\": \"{}\"}}",
            auth.allowed.bucket_id
        ))));
    let req = Request::new_with_init(
        &format!("{}/b2api/v2/b2_get_upload_url", auth.api_url),
        &init,
    )?;

    Fetch::Request(req)
        .send()
        .await?
        .json::<UploadUrlResponse>()
        .await
        .map_err(|e| e.into())
}

pub async fn upload_file(
    file: &[u8],
    auth: &AuthResponse,
    mime: &str,
    name: &str,
    suffix: &str,
    ext: &str,
) -> Result<UploadFileResponse> {
    let upload_url_res = get_upload_url(auth).await?;
    let mut headers = Headers::new();
    headers.set("Authorization", upload_url_res.authorization_token.as_str())?;
    headers.set("Content-Type", mime)?;
    let hash = util::get_hash(file)?;
    headers.set(
        "X-Bz-File-Name",
        format!("{}-{}.{}", name, suffix, ext).as_str(),
    )?;
    headers.set("X-Bz-Content-Sha1", hash.as_str())?;

    let mut init = RequestInit::new();
    init.with_body(Some(util::bytes_to_js_value(file)))
        .with_method(Method::Post)
        .with_headers(headers);

    let req = Request::new_with_init(upload_url_res.upload_url.as_str(), &init)?;

    let res = Fetch::Request(req)
        .send()
        .await?
        .json::<UploadFileResponse>()
        .await;
    console_log!("Upload result: {:?}", res);

    res.map_err(|e| e.into())
}

fn upload_variants(
    ctx: &RouteContext<worker::Context>,
    auth: AuthResponse,
    image: DynamicImage,
    variants: Vec<u32>,
    filename: String,
    format: ImageFormat,
) {
    ctx.data.wait_until(async move {
        let (name, ext) =
            util::get_filename_and_ext(&filename, &format).expect("Unsupported format");
        let mime = mime_guess::from_ext(&ext)
            .first_or_octet_stream()
            .to_string();
        let mut jobs = vec![];
        for w in variants.iter() {
            jobs.push(async {
                let resized = util::resize(&image, *w);
                let mut writer = Cursor::new(Vec::new());
                let ret = resized.write_to(&mut writer, format);
                if ret.is_err() {
                    return;
                }
                console_log!("Resize to {}", *w);
                let ret = upload_file(
                    &writer.into_inner(),
                    &auth,
                    &mime,
                    &name,
                    &w.to_string(),
                    &ext,
                )
                .await;

                if ret.is_err() {
                    console_log!("Failed to upload image variant: {}", *w);
                } else {
                    console_log!("Uploaded image variant: {}", *w);
                }
            })
        }
        futures::future::join_all(jobs).await;
    });
}
