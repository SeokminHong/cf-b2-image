use serde::{Deserialize, Serialize};
use worker::*;

use super::{API_URL, CREDENTIAL_KEY, CREDENTIAL_NS};
use crate::error::{Error, Result};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub api_url: String,
    pub authorization_token: String,
    pub allowed: AllowedBucket,
    pub download_url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AllowedBucket {
    pub bucket_id: String,
    pub bucket_name: String,
}

pub async fn authorize<D>(ctx: &RouteContext<D>) -> Result<AuthResponse> {
    authorize_impl(ctx)
        .await
        .map_err(|_| Error::AuthError("Authorization error".to_string()))
}

async fn authorize_impl<D>(ctx: &RouteContext<D>) -> Result<AuthResponse> {
    let kv = ctx.kv(CREDENTIAL_NS)?;
    if let Some(auth) = kv.get(CREDENTIAL_KEY).json::<AuthResponse>().await? {
        console_log!("Auth cache hit!");
        return Ok(auth);
    }

    let mut init = RequestInit::new();

    let mut headers = Headers::new();
    let encoded = base64::encode(format!(
        "{}:{}",
        ctx.secret("BUCKET_ID")?.to_string(),
        ctx.secret("BUCKET_KEY")?.to_string()
    ));
    headers.set("Authorization", &format!("Basic {}", encoded))?;
    init.with_headers(headers).with_method(Method::Get);

    let req = Request::new_with_init(&format!("{}/b2_authorize_account", API_URL), &init)?;
    let res: AuthResponse = Fetch::Request(req).send().await?.json().await?;
    let res_str = serde_json::to_string(&res)?;

    kv.put(CREDENTIAL_KEY, res_str)?
        .expiration_ttl(24 * 60 * 60)
        .execute()
        .await?;

    console_log!("Authorized!");

    Ok(res)
}
