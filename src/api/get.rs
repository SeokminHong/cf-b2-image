use std::io::Cursor;

use worker::*;

use super::auth::{authorize, AuthResponse};
use super::upload;
use super::util;
use super::{ImageInfo, IMAGE_NS};
use crate::error::{Error, Result};

pub async fn get<D>(ctx: &RouteContext<D>, filename: &str, width: Option<u32>) -> Result<Response> {
    let auth = authorize(ctx).await?;

    let mut image_info: ImageInfo = ctx
        .kv(IMAGE_NS)?
        .get(filename)
        .await?
        .ok_or_else(|| Error::InternalError("File not found.".into()))?
        .as_json()?;
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

                    let new_buffer = writer.into_inner();
                    let mime = mime_guess::from_ext(&image_info.format)
                        .first_or_octet_stream()
                        .to_string();
                    upload::upload_file(
                        &new_buffer,
                        &auth,
                        &mime,
                        &image_info.name,
                        &w.to_string(),
                        &image_info.format,
                    )
                    .await?;
                    image_info.variants.push(w);
                    let kv_data = serde_json::to_string(&image_info)?;

                    // Update KV variant
                    ctx.kv(IMAGE_NS)?.put(filename, kv_data)?.execute().await?;

                    Response::from_bytes(new_buffer).map_err(|e| e.into())
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
