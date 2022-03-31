use std::io::Cursor;

use worker::*;

use super::auth::{authorize, AuthResponse};
use super::upload;
use super::util;
use super::{ImageInfo, IMAGE_NS};
use crate::error::{Error, Result};

pub async fn get<D>(
    ctx: &RouteContext<D>,
    scope: &str,
    filename: &str,
    width: Option<u32>,
) -> Result<Response> {
    let auth = authorize(ctx).await?;

    let mut image_info: ImageInfo = ctx
        .kv(IMAGE_NS)?
        .get(filename)
        .await?
        .ok_or_else(|| Error::InternalError("File not found.".into()))?
        .as_json()?;
    let format = image::ImageFormat::from_extension(&image_info.format).expect("Invalid format");
    let (name, ext) = util::get_filename_and_ext(scope, filename, &format)?;

    match width {
        Some(w) => {
            if image_info.width <= w {
                download_file(&name, "orig", &ext, &auth).await
            } else if image_info.variants.contains(&w) {
                download_file(&name, &w.to_string(), &ext, &auth).await
            } else {
                let mut res = download_file(&name, "orig", &ext, &auth).await?;
                if res.status_code() >= 400 {
                    Err(Error::InternalError("Original image not found.".into()))
                } else {
                    let buffer = res.bytes().await?;
                    let image = image::load_from_memory_with_format(&buffer, format)?;
                    let resized = util::resize(&image, w);
                    let mut writer = Cursor::new(Vec::new());
                    resized.write_to(&mut writer, format)?;
                    let buffer = writer.into_inner();

                    upload::upload(ctx, buffer.clone(), scope, filename).await?;
                    image_info.variants.push(w);

                    // Update KV variant
                    ctx.kv(IMAGE_NS)?
                        .put(filename, image_info.clone())?
                        .execute()
                        .await?;

                    Response::from_bytes(buffer).map_err(|e| e.into())
                }
            }
        }
        None => download_file(&name, "orig", &ext, &auth).await,
    }
}

async fn download_file(
    name: &str,
    variant: &str,
    ext: &str,
    auth: &AuthResponse,
) -> Result<Response> {
    let mut init = RequestInit::new();
    let mut headers = Headers::new();
    headers.set("Authorization", &auth.authorization_token)?;
    init.with_headers(headers);

    let req = Request::new_with_init(&format!("{}-{}.{}", name, variant, ext), &init)?;

    Fetch::Request(req).send().await.map_err(|e| e.into())
}
