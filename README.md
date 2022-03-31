# Cloudflare & B2 Image Router

- [x] Upload a posted image on B2
- [x] Upload resized variants of the image and upload them
- [x] Implement GET request
- [ ] Cache images by using Cloudflare Cache API
- [ ] Make jobs asynchronously
- [ ] Implement detailed responses
- [ ] Authorize POST request

## Prerequisites

- [B2](https://www.backblaze.com/) Bucket
- A B2 application key
- [Wrangler](https://developers.cloudflare.com/workers/cli-wrangler/)

## Deploy

1. Remove `kv_namespaces` section from `wrangler.toml`.
2. Run `wrangler publish`.
3. Run following commands and add new `kv_namespace` to `wrangler.toml` with generated id.
   ```bash
   wrangler kv:namespace create IMAGE
   wrangler kv:namespace create CREDENTIALS
   ```
4. Run following commands and add your bucket's ID and key.
   ```bash
   wrangler secret BUCKET_ID
   wrangler secret put BUCKET_KEY
   ```
5. Publish.
   ```bash
   wrangler publish
   ```
6. If you have domain, add route on [Cloudflare Dashboard](https://dash.cloudflare.com/) to enable cache.
   > However, any Cache API operations in the Cloudflare Workers dashboard editor, Playground previews, and any `*.workers.dev` deployments will have no impact. For Workers fronted by Cloudflare Access, the Cache API is not currently available. Only Workers deployed to custom domains have access to functional cache operations.
   >
   > https://developers.cloudflare.com/workers/runtime-apis/cache/

```bash
wrangler kv:namespace create IMAGE
wrangler kv:namespace create CREDENTIALS
wrangler secret put BUCKET_ID
wrangler secret put BUCKET_KEY
wrangler publish
```
