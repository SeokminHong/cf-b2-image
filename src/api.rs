mod auth;
mod upload;
mod util;

const API_URL: &str = "https://api.backblazeb2.com/b2api/v2";
const CREDENTIAL_NS: &str = "CREDENTIALS";
const CREDENTIAL_KEY: &str = "b2";
const IMAGE_NS: &str = "IMAGE";
const WIDTHS: &[u32] = &[320, 640, 1280, 1920];

pub use auth::authorize;
use serde::{Deserialize, Serialize};
pub use upload::upload;

#[derive(Serialize, Deserialize)]
pub struct StoredImage {
    pub id: String,
    pub name: String,
    pub format: String,
    pub width: u32,
    pub variants: Vec<u32>,
}
