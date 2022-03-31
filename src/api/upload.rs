use std::str;
use std::thread;

use futures::executor::block_on;
use serde::{Deserialize, Serialize};
use worker::wasm_bindgen::JsValue;
use worker::*;

use super::auth::{authorize, AuthResponse};
use super::util;
use super::StoredImage;
use super::{IMAGE_NS, WIDTHS};
use crate::error::{Error, Result};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadUrlResponse {
    pub upload_url: String,
    pub authorization_token: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadFileResponse {
    pub file_id: String,
}

pub async fn upload<D>(
    ctx: &RouteContext<D>,
    file: Vec<u8>,
    scope: &str,
    filename: &str,
) -> Result<String> {
    let auth = authorize(ctx).await?;

    let format = image::guess_format(&file)?;
    let image = image::load_from_memory_with_format(&file, format)?;

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
    let mime = mime_guess::from_ext(ext)
        .first_or_octet_stream()
        .to_string();

    let width = image.width();
    let variants = WIDTHS
        .iter()
        .filter(|&w| *w < width)
        .copied()
        .collect::<Vec<_>>();

    // Copy variables for threading
    let t_image = image.clone();
    let t_variants = variants.clone();
    let t_auth = auth.clone();
    let t_name = name.clone();
    let t_mime = mime.clone();
    let t_ext = ext.to_string();
    // Resize and upload image variants on a separate thread
    thread::spawn(move || {
        t_variants.iter().for_each(|&w| {
            console_log!("Resizing {} to {}", t_name, w);
            let resized = util::resize(&t_image, w);
            let ret = block_on(upload_file(
                resized,
                &t_auth,
                &t_mime,
                &t_name,
                &w.to_string(),
                &t_ext,
            ));
            if ret.is_err() {
                console_log!("Failed to upload image variant: {}", w);
            } else {
                console_log!("Uploaded image variant: {}", w);
            }
        })
    });

    let res = upload_file(image.into_bytes(), &auth, &mime, &name, "orig", ext).await?;
    ctx.kv(IMAGE_NS)?.put(
        &name,
        StoredImage {
            id: res.file_id.clone(),
            name: filename.to_string(),
            width,
            variants: variants.iter().copied().collect::<Vec<_>>(),
        },
    )?;
    console_log!("Uploaded {}", filename);

    Ok(res.file_id)
}

async fn get_upload_url(auth: &AuthResponse) -> Result<UploadUrlResponse> {
    let mut headers = Headers::new();
    headers.set("Authorization", auth.authorization_token.as_str())?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from(format!(
            "{{\"bucketId\": \"{}\"}}",
            auth.allowed.bucket_id
        ))));
    let req = Request::new_with_init(auth.api_url.as_str(), &init)?;

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

    Fetch::Request(req)
        .send()
        .await?
        .json::<UploadFileResponse>()
        .await
        .map_err(|e| e.into())
}
