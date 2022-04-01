use std::io::Cursor;

use worker::*;

use super::auth::{authorize, AuthResponse};
use super::upload;
use super::util;
use super::{ImageInfo, IMAGE_NS};
use crate::error::{Error, Result};

pub async fn get(
    ctx: &RouteContext<worker::Context>,
    filename: &str,
    width: Option<u32>,
) -> Result<Response> {
    let auth = authorize(ctx).await?;
    let kv = ctx.kv(IMAGE_NS)?;
    let image_info = kv
        .get(filename)
        .json::<ImageInfo>()
        .await?
        .ok_or_else(|| Error::InternalError("File not found.".into()))?;
    console_log!("Image info: {:?}", image_info);
    let format = image::ImageFormat::from_extension(&image_info.format).expect("Invalid format");

    match width {
        Some(w) => {
            if image_info.width <= w {
                download_file(&image_info.name, "orig", &image_info.format, &auth).await
            } else if image_info.variants.contains(&w) {
                download_file(&image_info.name, &w.to_string(), &image_info.format, &auth).await
            } else {
                let mut res =
                    download_file(&image_info.name, "orig", &image_info.format, &auth).await?;
                if res.status_code() >= 400 {
                    Err(Error::InternalError("Original image not found.".into()))
                } else {
                    let buffer = res.bytes().await?;
                    console_log!(
                        "Try to load image: {} bytes, {}",
                        buffer.len(),
                        format.extensions_str().first().unwrap()
                    );
                    let image = image::load_from_memory_with_format(&buffer, format)?;
                    let resized = util::resize(&image, w);
                    let mut writer = Cursor::new(Vec::new());
                    resized.write_to(&mut writer, format)?;
                    let mime = mime_guess::from_ext(&image_info.format).first_or_octet_stream();

                    let new_buffer = writer.into_inner();
                    upload_on_background(
                        ctx,
                        kv,
                        new_buffer.clone(),
                        filename.to_owned(),
                        auth,
                        w,
                        image_info,
                    );

                    let mut headers = Headers::new();
                    headers.set("Content-Type", &mime.to_string())?;
                    Response::from_bytes(new_buffer)
                        .map(|res| res.with_headers(headers))
                        .map_err(|e| e.into())
                }
            }
        }
        None => download_file(&image_info.name, "orig", &image_info.format, &auth).await,
    }
}

async fn download_file(
    name: &str,
    variant: &str,
    ext: &str,
    auth: &AuthResponse,
) -> Result<Response> {
    console_log!("Download variant {}", variant);
    let mut init = RequestInit::new();
    let mut headers = Headers::new();
    headers.set("Authorization", &auth.authorization_token)?;
    init.with_headers(headers);

    let url = format!(
        "{}/file/{}/{}-{}.{}",
        auth.download_url, auth.allowed.bucket_name, name, variant, ext
    );
    let req = Request::new_with_init(&url, &init)?;

    Fetch::Request(req).send().await.map_err(|e| e.into())
}

fn upload_on_background(
    ctx: &RouteContext<worker::Context>,
    kv: worker::kv::KvStore,
    buffer: Vec<u8>,
    filename: String,
    auth: AuthResponse,
    width: u32,
    mut image_info: ImageInfo,
) {
    ctx.data.wait_until(async move {
        let mime = mime_guess::from_ext(&image_info.format)
            .first_or_octet_stream()
            .to_string();
        let res = upload::upload_file(
            &buffer,
            &auth,
            &mime,
            &image_info.name,
            &width.to_string(),
            &image_info.format,
        )
        .await;
        if res.is_err() {
            console_error!("Upload failed: {:?}", res.unwrap_err());
            return;
        }
        image_info.variants.push(width);
        let kv_data = serde_json::to_string(&image_info).expect("Failed to serialize");

        // Update KV variant
        let res = kv.put(&filename, kv_data).expect("").execute().await;
        if res.is_err() {
            console_error!("Failed to update KV: {}, {}", filename, res.unwrap_err());
        }
    });
}
