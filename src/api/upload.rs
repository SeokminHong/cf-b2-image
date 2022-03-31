use std::io::Cursor;
use std::str;

use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;
use worker::*;

use super::auth::{authorize, AuthResponse};
use super::util;
use super::StoredImage;
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
struct UploadFileResponse {
    pub file_id: String,
    pub file_name: String,
}

pub async fn upload<D>(
    ctx: &RouteContext<D>,
    file: Vec<u8>,
    scope: &str,
    filename: &str,
) -> Result<String> {
    let auth = authorize(ctx).await?;

    let format = image::guess_format(&file)?;

    console_log!("Image format: {}", format.extensions_str().first().unwrap());
    let image = image::load_from_memory_with_format(&file, format)?;
    console_log!("Image loaded: {} bytes", image.as_bytes().len());

    let (name, ext) = util::get_filename_and_ext(scope, filename, &format)?;
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

    for w in variants.iter() {
        let resized = util::resize(&image, *w);
        let mut writer = Cursor::new(Vec::new());
        resized.write_to(&mut writer, format)?;
        console_log!("Resize to {}", *w);
        let ret = upload_file(
            writer.into_inner(),
            &auth,
            &mime,
            &name,
            &w.to_string(),
            &ext,
        )
        .await;

        if ret.is_err() {
            console_log!("Failed to upload image variant: {}", w);
        } else {
            console_log!("Uploaded image variant: {}", w);
        }
    }
    let res = upload_file(image.into_bytes(), &auth, &mime, &name, "orig", &ext).await?;
    ctx.kv(IMAGE_NS)?
        .put(
            &name,
            StoredImage {
                id: res.file_id.clone(),
                name: filename.to_string(),
                format: format
                    .extensions_str()
                    .first()
                    .expect("Unsupported format")
                    .to_string(),
                width,
                variants,
            },
        )?
        .execute()
        .await?;

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

async fn upload_file(
    file: Vec<u8>,
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
    let hash = util::get_hash(&file)?;
    headers.set(
        "X-Bz-File-Name",
        format!("{}-{}.{}", name, suffix, ext).as_str(),
    )?;
    headers.set("X-Bz-Content-Sha1", hash.as_str())?;

    let mut init = RequestInit::new();
    init.with_body(Some(util::bytes_to_js_value(&file)))
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
