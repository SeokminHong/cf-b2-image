# Cloudflare & B2 Image Router

- [x] Upload a posted image on B2
- [x] Upload resized variants of the image and upload them
- [ ] Implement GET request
- [ ] Cache images by using Cloudflare Cache API
- [ ] Make jobs asynchronously
- [ ] Implement detailed responses

## Deploy

```bash
wrangler kv:namespace create IMAGE
wrangler kv:namespace create CREDENTIALS
wrangler secret put BUCKET_ID
wrangler secret put BUCKET_KEY
wrangler publish
```
