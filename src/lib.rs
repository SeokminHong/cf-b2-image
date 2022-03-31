mod api;
mod error;
mod utils;

use std::collections::HashMap;

use worker::*;

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or_else(|| "unknown region".into())
    );
}

async fn get_image(image: &str, queries: HashMap<String, String>) -> Result<Response> {
    Response::ok(format!("{}, {:?}", image, queries))
}

#[event(fetch)]
pub async fn main(req: Request, env: Env) -> Result<Response> {
    log_request(&req);

    // Optionally, get more helpful error messages written to the console in the case of a panic.
    utils::set_panic_hook();

    // Optionally, use the Router to handle matching endpoints, use ":name" placeholders, or "*name"
    // catch-alls to match on specific patterns. Alternatively, use `Router::with_data(D)` to
    // provide arbitrary data that will be accessible in each route via the `ctx.data()` method.
    let router = Router::new();

    // Add as many routes as your Worker needs! Each route will get a `Request` for handling HTTP
    // functionality and a `RouteContext` which you can use to  and get route parameters and
    // Environment bindings like KV Stores, Durable Objects, Secrets, and Variables.
    router
        .get("/", |_, _| Response::ok(""))
        .get_async("/images/:image", |req, ctx| async move {
            let image = ctx.param("image").ok_or_else(|| {
                Error::RustError("Missing required parameter: images/:image".to_string())
            })?;

            let url = req.url()?;
            let queries = url
                .query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect::<HashMap<_, _>>();

            get_image(image, queries).await
        })
        .post_async("/upload", |mut req, ctx| async move {
            let headers = req.headers();
            let filename = headers.get("X-File-Name")?.ok_or_else(|| {
                Error::RustError("Missing required header: X-File-Name".to_string())
            })?;
            let scope = headers
                .get("X-Scope")?
                .ok_or_else(|| Error::RustError("Missing required header: X-Scope".to_string()))?;
            let body = req.bytes().await?;

            console_log!(
                "Filename: {}, Scope: {}, Length: {}",
                filename,
                scope,
                body.len()
            );

            match api::upload(&ctx, body, &scope, &filename).await {
                Ok(id) => Response::ok(id),
                Err(e) => match e {
                    error::Error::AuthError(e) => Response::error(e, 403),
                    error::Error::InternalError(e) => Response::error(e.to_string(), 500),
                },
            }
        })
        .run(req, env)
        .await
}
