# Webhook Guide

Webhooks in InkForge let external services react to post lifecycle events. When a post is saved or published, InkForge sends an HTTP POST with a JSON payload to every URL you configure. Built-in retry logic, concurrency control, and delivery logging give you visibility into every call.

## Supported Events

| Event | Trigger | Hook Type |
|---|---|---|
| `post.after_save` | Post is created or updated (any status change) | Action |
| `post.after_publish` | Post transitions from non-published to published | Action |

Both events fire as Action hooks — they are spawned as fire-and-forget tasks after the database transaction commits. A slow or failing webhook never blocks the HTTP response to the editor.

## Creating a Webhook

Webhooks are managed from the admin panel under **Settings → Webhooks**, or programmatically via the REST API at `/api/v1/webhooks`.

### Via the Admin Panel

1. Navigate to **Settings → Webhooks**.
2. Click **"Add Webhook"**.
3. Fill in the form:
   - **Name** — a human-readable label (e.g. "Build hook").
   - **URL** — the endpoint that will receive the POST.
   - **Events** — comma-separated list; defaults to `post.after_publish`.
   - **Secret** — optional HMAC signing key (see [Signature Verification](#signature-verification)).
   - **Max Retries** — how many times to retry on failure (0–5).
   - **Enabled** — toggle to activate or pause the webhook.
4. Click **"Save"**.

### Via the API

```bash
curl -X POST http://localhost:2000/api/v1/webhooks \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d '{
    "name": "Build hook",
    "url": "https://example.com/webhook",
    "events": "post.after_publish",
    "secret": "my-shared-secret",
    "max_retries": 3,
    "enabled": true
  }'
```

## Payload Format

Every webhook delivery sends a JSON body with the following structure:

```json
{
  "event": "post.after_publish",
  "timestamp": "2026-06-06T12:34:56.789Z",
  "data": {
    "post_id": "a1b2c3d4-...",
    "title": "My First Post",
    "slug": "my-first-post",
    "old_status": "draft",
    "new_status": "published"
  }
}
```

For `post.after_save`, the `data` object contains additional fields:

```json
{
  "data": {
    "post_id": "a1b2c3d4-...",
    "title": "My First Post",
    "slug": "my-first-post",
    "is_new": true,
    "status": "draft",
    "old_status": null
  }
}
```

The `timestamp` field uses RFC 3339 format in UTC. The `event` field matches the trigger event name exactly, so your receiver can dispatch based on it.

## Concurrency Model

All webhooks for a given event are dispatched in parallel, governed by a `tokio::sync::Semaphore` with a configurable permit count. The defaults are:

| Parameter | Default | Config Key |
|---|---|---|
| Max concurrent webhooks | 5 | `webhook.max_concurrency` |
| Total dispatch timeout | 60 seconds | `webhook.timeout_seconds` |

If there are 12 enabled webhooks listening to `post.after_publish`, InkForge sends at most 5 HTTP requests at a time. The remaining 7 wait for a semaphore permit. If the entire batch does not complete within 60 seconds, any outstanding requests are cancelled.

Each individual HTTP request has a 10-second connection timeout (set on the `reqwest::Client`).

## Retry Strategy

When a webhook delivery fails with a 5xx server error or a network error (connection refused, DNS failure, timeout), InkForge retries automatically. The retry behavior:

- **Exponential backoff**: delay = `5 × 2^(attempt-1)` seconds, capped at 60 seconds. The sequence is 5s → 10s → 20s → 40s → 60s.
- **Maximum 5 retries** (6 total attempts including the initial call).
- **4xx client errors are not retried** — a 400-level status is treated as a permanent failure and logged immediately.
- **Build errors (TLS, DNS resolution) are retried** — these are treated as transient.

Retry cap per webhook is configurable via the `max_retries` field (0–5). Setting it to 0 means no retries.

## Delivery Logs

Every delivery attempt is recorded in the `webhook_deliveries` table. Each record stores:

- `webhook_id` — which webhook configuration was triggered
- `event` — the event name
- `request_url` and `request_body` — what was sent
- `response_status` — the HTTP status code (or `null` if the request never reached the server)
- `response_body` — the response payload (truncated if very large)
- `duration_ms` — wall-clock time for this single attempt
- `success` — boolean: `1` if a 2xx response was received, `0` otherwise

View delivery logs in the admin panel by clicking a webhook row, or query directly:

```sql
SELECT * FROM webhook_deliveries WHERE webhook_id = '<your-webhook-id>' ORDER BY created_at DESC LIMIT 20;
```

A separate `__event_failed__` sentinel webhook captures delivery records when the webhook dispatch itself fails (e.g. the database query for enabled webhooks errors out). This prevents silent data loss.

## Signature Verification

If you provide a `secret` when creating a webhook, InkForge attaches an HMAC-SHA256 signature to every request:

```
X-Webhook-Signature: sha256=<hex-encoded-hmac>
```

To verify on your receiver:

```python
import hmac
import hashlib

def verify_signature(payload: bytes, signature: str, secret: str) -> bool:
    expected = hmac.new(
        secret.encode("utf-8"),
        payload,
        hashlib.sha256
    ).hexdigest()
    return hmac.compare_digest(f"sha256={expected}", signature)
```

The payload used for signing is the raw JSON request body, exactly as sent (including whitespace).

## REST API Reference

All webhook CRUD operations are available under `/api/v1/webhooks`. Authentication requires a JWT session cookie or an API key with the `webhooks` scope.

### List Webhooks

```bash
curl http://localhost:2000/api/v1/webhooks \
  -H "Authorization: Bearer YOUR_API_KEY"
```

Returns an array of all webhook configurations.

### Get a Single Webhook

```bash
curl http://localhost:2000/api/v1/webhooks/{webhook-id} \
  -H "Authorization: Bearer YOUR_API_KEY"
```

### Query Deliveries

```bash
curl "http://localhost:2000/api/v1/webhooks/{webhook-id}/deliveries?page=1&page_size=20" \
  -H "Authorization: Bearer YOUR_API_KEY"
```

Paginated. Response includes `data` (array of delivery records) and `total` (total count).

### Update a Webhook

```bash
curl -X PATCH http://localhost:2000/api/v1/webhooks/{webhook-id} \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d '{"enabled": false, "max_retries": 2}'
```

All fields are optional — only include the fields you want to change. Setting `secret` to an empty string clears the existing secret.

### Delete a Webhook

```bash
curl -X DELETE http://localhost:2000/api/v1/webhooks/{webhook-id} \
  -H "Authorization: Bearer YOUR_API_KEY"
```

Returns `{"deleted": true}`. Delivery logs are preserved for audit purposes.

## Testing with webhook.site

For local development, [webhook.site](https://webhook.site) provides a free, instant webhook receiver:

1. Open webhook.site — you get a unique URL like `https://webhook.site/abc123-...`.
2. Create a webhook in InkForge pointing to that URL.
3. Publish a post and watch the payload appear in real time.

This is the fastest way to inspect payload format, headers, and timing before wiring up your actual integration.

## Troubleshooting

- **"No deliveries recorded"** — confirm the webhook is enabled and the event field matches (`post.after_publish`, not `post.after_publish` with extra whitespace).
- **"Webhook returns 4xx"** — check that your receiver accepts `Content-Type: application/json` and the payload shape. 4xx errors are not retried.
- **"Delivery timed out"** — your receiver may be taking longer than 10 seconds to respond. Consider acknowledging immediately and processing asynchronously.
- **"Too many concurrent deliveries"** — increase `webhook.max_concurrency` in `config/inkforge.toml` (restart required).
- **Check logs** — search for `module = "webhook"` in the application logs. Every delivery attempt, retry, and error is logged at the appropriate level.
