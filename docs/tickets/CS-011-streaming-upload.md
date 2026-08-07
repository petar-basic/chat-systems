# CS-011 — Streaming upload with an enforced size cap

**Wave:** 2 — Abuse and resource limits
**Area:** backend/api
**Blocked by:** —
**Blocks:** —
**Audit finding:** S9 (MEDIUM)

## Problem

[`upload_file`](../../backend/api/src/files/routes.rs#L66-L88) reads the whole multipart
field into memory and *then* checks the limit:

```rust
let data = field.bytes().await?;
if data.len() > MAX_FILE_SIZE { ... }
```

`MAX_FILE_SIZE` is 100 MiB and the global `DefaultBodyLimit` is also 100 MiB
([`main.rs:236`](../../backend/api/src/main.rs#L236)), so the check never rejects
anything the router already accepted — it is dead code. Meanwhile the production API
container is capped at **512 MB**
([`docker-compose.prod.yml`](../../docker-compose.prod.yml)).

Five or six concurrent 100 MiB uploads OOM-kill the API. Any authenticated member can do
it from a laptop, and the container restarts, dropping every in-flight request.

`data.to_vec()` doubles it again on the way to storage, and the S3 path buffers a third
copy into `ByteStream`.

## Approach

Stream the field to storage with a running byte count, and abort the moment the cap is
crossed.

1. **Extend the `FileStorage` trait with a streaming upload.** Keep the existing
   `upload` for small internal callers (avatars) and add:
   ```rust
   async fn upload_stream(
       &self,
       key: &str,
       body: impl Stream<Item = Result<Bytes, AppError>> + Send + 'static,
       content_type: &str,
   ) -> AppResult<u64>;
   ```
   returning the number of bytes written.
   - `LocalStorage`: `tokio::fs::File` + `tokio::io::copy`, writing through the existing
     `key_path` guard so the traversal checks still apply.
   - `S3Storage`: `ByteStream::from_body_1_x` over the same stream. Above ~8 MiB switch to
     a multipart upload so a single part never has to be buffered.
2. **Count and cut in the route.** Wrap `field` in a stream adapter that sums lengths and
   yields `AppError::BadRequest` past `MAX_FILE_SIZE`. The upload aborts on the first
   chunk over the line; peak memory is one chunk, not one file.
3. **Clean up the partial object.** If the stream errors, delete the key that was being
   written before returning, so a failed upload does not leave an orphan in MinIO. Log at
   `warn` if the cleanup itself fails — never fail the request twice.
4. **Insert the DB row after the upload succeeds**, using the byte count returned by
   `upload_stream` rather than `data.len()`. This also removes the current window where a
   row exists for an object that failed to write.
5. **Lower the router body limit for everything else.** 100 MiB is only needed on the
   upload route. Apply `DefaultBodyLimit::max(1 * 1024 * 1024)` globally in `build_app`
   and override it per-route on `/files/upload/:ws_id`, so a 100 MiB JSON body can no
   longer be posted to `/api/auth/login`.
6. **Make the cap configurable** as `MAX_UPLOAD_BYTES` in `AppConfig` (default 100 MiB),
   and keep `client_max_body_size` in
   [`nginx.conf`](../../frontend/docker/nginx.conf) in agreement — document that the two
   have to move together.

## Acceptance

- [ ] Uploading a 100 MiB file holds no more than a few MiB of API memory at a time.
- [ ] A file one byte over the cap is rejected with 400 and no object is left behind.
- [ ] Non-upload routes reject bodies over 1 MiB.
- [ ] `MAX_UPLOAD_BYTES` is configurable and documented in `.env.example`.
- [ ] Both storage backends pass the same tests.

## Tests

`http_tests/files.rs`: upload just under and just over the cap; assert 200/400 and that
the storage backend holds no object for the rejected one. Add a local-storage test that
the traversal guard still rejects `..` keys through the streaming path. Verify memory
behaviour manually once with a 100 MiB upload against a 512 MB container.
