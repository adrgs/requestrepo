# requestrepo API guide for agents

requestrepo gives you a throwaway subdomain of `{{DOMAIN}}` and records every HTTP request, DNS query and SMTP message sent to it. You can also choose what that subdomain answers. Everything below works with `curl` and `jq`; there is nothing to install.

- Base URL: `{{ORIGIN}}/api/v2`
- Bodies are JSON. Errors are JSON too: `{"error": "<what went wrong>", "code": "<stable_code>"}`.
- Machine-readable description: `{{ORIGIN}}/openapi.json`

## 1. Create a session

```sh
curl -s -X POST {{ORIGIN}}/api/v2/sessions
```

```json
{
  "token": "eyJ...",
  "subdomain": "abcd1234",
  "domain": "abcd1234.{{DOMAIN}}",
  "url": "{{SCHEME}}://abcd1234.{{DOMAIN}}/",
  "email": "anything@abcd1234.{{DOMAIN}}"
}
```

- Keep `token`: it is the only key to the session and is valid for a year. Send it as `Authorization: Bearer <token>` (or `?token=<token>`) on every other call.
- `url` is the HTTP base (`domain` plus scheme, and the port if it is not the default). Use `domain` for DNS and mail.
- Anything sent to `domain` or to any name under it (`x.abcd1234.{{DOMAIN}}`) is captured: HTTP and HTTPS on any path, DNS queries of any type, and mail to any address at it.
- Session creation is rate limited per IP. On a private instance that requires a password, add `-H "Authorization: Bearer <ADMIN_TOKEN>"`.

A convenient shell setup:

```sh
S=$(curl -s -X POST {{ORIGIN}}/api/v2/sessions)
export RR_TOKEN=$(jq -r .token <<<"$S") RR_DOMAIN=$(jq -r .domain <<<"$S") RR_URL=$(jq -r .url <<<"$S")
rr() { curl -s -H "Authorization: Bearer $RR_TOKEN" "{{ORIGIN}}/api/v2$@"; }
```

## 2. Read what was captured

Three recipes cover almost everything. Add `view=summary` to get one compact object per request instead of the full capture.

**What has come in so far** (the newest 20, oldest first):

```sh
rr "/requests?last=20&view=summary"
```

```json
{
  "requests": [
    { "_id": "6f0c...", "type": "http", "date": 1795708400, "ip": "203.0.113.7", "country": "DE",
      "summary": "POST /callback?id=1", "body_text": "{\"ok\": true}" },
    { "_id": "71a2...", "type": "dns", "date": 1795708402, "ip": "172.217.45.20",
      "summary": "A probe-1.abcd1234.{{DOMAIN}}" }
  ],
  "cursor": "71a2...",
  "pagination": { "total": 2, "limit": 100, "has_more": false }
}
```

**Wait for the next one**, ignoring anything already there. `after=now` plus `wait` (seconds, at most 60) holds the response until something new arrives:

```sh
R=$(rr "/requests?after=now&wait=60&view=summary")
```

Start this *before* you trigger whatever should call back, or run the trigger in the background, so the callback cannot arrive before you are listening.

**Follow everything**, without gaps or repeats: pass each response's `cursor` as the next `after`.

```sh
CURSOR=now
while true; do
  R=$(rr "/requests?after=$CURSOR&wait=60&view=summary")
  jq -c '.requests[]' <<<"$R"
  CURSOR=$(jq -r .cursor <<<"$R")
done
```

- `cursor` is the newest request in the response. When nothing arrived in time, `requests` is empty and `cursor` is unchanged, so passing it again is always right.
- Without `after`, `wait` only waits when the session has no matching requests at all: existing history is returned at once.
- Filter with `type=http|dns|smtp|tcp`. Plain paging: `limit` (default 100, max 1000) and `offset`, oldest first.
- `view=summary` gives `summary` (one line: method and path, query type and name lowercased, or mail sender, recipient and subject) and `body_text` (the body as text, up to 1000 characters, with `body_truncated`). For headers and raw bytes, get the full capture: `rr "/requests/<id>"`.
- If `after` names a request that was deleted or evicted, the answer is `404 cursor_not_found`; use `after=now` to start over.
- Delete one: `rr "/requests/<id>" -X DELETE`. Delete all: `rr "/requests" -X DELETE`.
- In scripts, feed JSON to `jq` with `<<<"$R"` or a pipe from `curl`, not `echo "$R"`: zsh's `echo` turns `\n` inside the JSON into real newlines.

### Captured request fields

Every request has `_id`, `type`, `date` (Unix seconds), `ip`, `country` (two-letter code, may be absent) and `raw` (base64 of the raw body or packet; decode with `base64 -d`).

| `type` | Extra fields |
|--------|--------------|
| `http` | `method`, `path`, `query` (with leading `?`), `url`, `protocol`, `headers` (object), `port` |
| `dns`  | `query_type` (`A`, `AAAA`, `TXT`, ...), `domain` (the full name queried), `reply`, `port` |
| `smtp` | `from`, `to`, `cc`, `subject`, `text_body`, `html_body`, `attachments`, `data` (full message) |
| `tcp`  | `port` |

```sh
# Decoded body of every HTTP request
rr "/requests?type=http&view=summary" | jq -r '.requests[].body_text // empty'
# Every distinct name that was looked up
rr "/requests?type=dns&view=summary" | jq -r '.requests[].summary' | sort -u
```

## 3. Choose what the subdomain answers

Each path of the subdomain serves a file you define; `index.html` is served for paths that have no file. A file is a status code, a list of headers and a body.

```sh
# Plain text body; Content-Type is guessed from the extension
rr "/files/status.json" -X PUT -H 'Content-Type: application/json' \
  -d '{"body": "{\"ok\": true}"}'

# Full control: status, headers, binary body as base64 in `raw`
rr "/files/redirect" -X PUT -H 'Content-Type: application/json' -d '{
  "status_code": 302,
  "headers": [{"header": "Location", "value": "https://example.com/"}],
  "body": ""
}'

rr "/files/redirect" -X DELETE     # remove one file
rr "/files"                        # the whole tree: {"<path>": {"raw", "headers", "status_code"}}
rr "/files/index.html"             # one file
```

`PUT /files/<path>` takes `body` (text) or `raw` (base64), never both, plus optional `status_code` (default 200) and `headers` (default: a guessed `Content-Type` and `Access-Control-Allow-Origin: *`). The answer includes the public `url` of the file. `PUT /files` replaces the whole tree at once, with every body base64 in `raw`.

## 4. DNS records

By default every name under the subdomain resolves to this server. You can override that per name:

```sh
rr "/dns" -X PUT -H 'Content-Type: application/json' -d '{"records": [
  {"domain": "local",  "type": "A",     "value": "127.0.0.1"},
  {"domain": "*",      "type": "A",     "value": "10.0.0.1%10.0.0.2"},
  {"domain": "txt",    "type": "TXT",   "value": "hello"},
  {"domain": "alias",  "type": "CNAME", "value": "example.com."}
]}'
rr "/dns"
```

- `domain` is relative to the session: `local` means `local.<subdomain>.{{DOMAIN}}`, and `*` matches any name without its own record. The session's own name (`@`) cannot be changed.
- Types: `A`, `AAAA`, `CNAME`, `TXT`. Separate several addresses with `%`.
- `PUT /dns` replaces all records of the session; send `{"records": []}` to clear them.
- Queries are captured whether or not a record matches them.

## 5. Share a captured request

```sh
rr "/requests/<id>/share" -X POST            # {"share_token": "..."}
curl -s {{ORIGIN}}/api/v2/requests/shared/<share_token>   # no auth needed
```

## 6. Live updates (optional)

Long polling (section 2) is all most agents need. For a stream, open a WebSocket to `{{WS_ORIGIN}}/api/v2/ws`, send `{"cmd": "connect", "token": "<token>"}`, and receive `{"cmd": "request", "subdomain": "...", "data": {...}}` for each capture.

## Errors

| HTTP | `code` | Meaning |
|------|--------|---------|
| 401 | `missing_token`, `invalid_token` | No token, or not a valid one. Create a new session. |
| 403 | `subdomain_suspended` | The subdomain was suspended for abuse. Create a new session; `contact` says whom to ask if it is a mistake. |
| 403 | `admin_required` | This instance requires a password to create sessions. |
| 400 | `invalid_body`, `invalid_type`, ... | The request itself is wrong; `error` says how. |
| 404 | `not_found`, `cursor_not_found` | No such request or file; or see section 2. |
| 415 | `unsupported_media_type` | Send JSON bodies with `Content-Type: application/json`. |
| 413 | `file_tree_too_large` | The response files exceed the size limit. |
| 429 | `rate_limited` | Slow down; see `retry_after` (seconds) when present. |

## Acceptable use

requestrepo is for testing systems you are allowed to test. Using it for malware command and control, phishing or other abuse gets the subdomain suspended: it stops resolving and serving, and its token stops working. To report abuse you have seen, see `{{ORIGIN}}/docs/abuse.md`.
