---
name: requestrepo
description: Capture out-of-band HTTP, DNS and SMTP interactions and serve custom HTTP responses or DNS records from a throwaway subdomain of {{DOMAIN}}, using only curl. Use when testing webhooks, integrations or email delivery, when you need to see exactly what a client or server sends, when checking whether a system makes an outbound HTTP or DNS request, or when a test needs a URL or hostname you control.
---

# requestrepo

requestrepo ({{ORIGIN}}) records every request to a session subdomain and lets you script its responses. The API is JSON over HTTPS; this skill only needs `curl` and `jq`.

## Start a session

```sh
S=$(curl -s -X POST {{ORIGIN}}/api/v2/sessions)
export RR_TOKEN=$(echo "$S" | jq -r .token) RR_DOMAIN=$(echo "$S" | jq -r .domain) RR_URL=$(echo "$S" | jq -r .url)
rr() { curl -s -H "Authorization: Bearer $RR_TOKEN" "{{ORIGIN}}/api/v2$@"; }
echo "$RR_DOMAIN"
```

Use `$RR_URL` for HTTP callbacks and `$RR_DOMAIN` (or any name under it, such as `probe1.$RR_DOMAIN`) for DNS and mail. Unique sub-names per check tell you which one triggered a request. HTTP, HTTPS, DNS and mail to any address at the domain are all captured.

## Wait for a callback

Never sleep-and-poll. Long-poll instead; `wait` is in seconds (max 60):

```sh
R=$(rr "/requests?wait=60")                       # first capture
CURSOR=$(echo "$R" | jq -r .cursor)
R=$(rr "/requests?after=$CURSOR&wait=60")         # only newer ones
echo "$R" | jq '.requests[] | {type, ip, method, path, domain, query_type}'
```

An empty `requests` array means nothing arrived in time. Filter with `type=http|dns|smtp`. Bodies are base64 in `raw` (`jq -r '.requests[].raw | @base64d'`).

## Serve a response

```sh
rr "/files/hello.txt" -X PUT -H 'Content-Type: application/json' -d '{"body": "hello"}'
rr "/files/r" -X PUT -H 'Content-Type: application/json' \
  -d '{"status_code": 302, "headers": [{"header": "Location", "value": "http://127.0.0.1/"}]}'
```

The file is then served at `${RR_URL}hello.txt`. Paths without a file serve `index.html`.

## Set DNS records

```sh
rr "/dns" -X PUT -H 'Content-Type: application/json' \
  -d '{"records": [{"domain": "internal", "type": "A", "value": "127.0.0.1"}]}'
```

`internal.$RR_DOMAIN` now resolves to 127.0.0.1. Types: A, AAAA, CNAME, TXT. `PUT /dns` replaces all records.

## Notes

- Errors are `{"error", "code"}`. `invalid_token` or `subdomain_suspended`: create a new session.
- Full reference: {{ORIGIN}}/docs/api.md; OpenAPI: {{ORIGIN}}/openapi.json.
- Only test systems you are authorized to test. Subdomains used for abuse are suspended.
