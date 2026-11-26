---
name: requestrepo
description: Capture out-of-band HTTP, DNS and SMTP interactions and serve custom HTTP responses or DNS records from a throwaway subdomain of {{DOMAIN}}, using only curl. Use when testing webhooks, integrations or email delivery, when you need to see exactly what a client or server sends, when checking whether a system makes an outbound HTTP or DNS request, or when a test needs a URL or hostname you control.
---

# requestrepo

requestrepo ({{ORIGIN}}) records every request to a session subdomain and lets you script its responses. The API is JSON over HTTPS; this skill only needs `curl` and `jq`.

## Start a session

```sh
S=$(curl -s -X POST {{ORIGIN}}/api/v2/sessions)
export RR_TOKEN=$(jq -r .token <<<"$S") RR_DOMAIN=$(jq -r .domain <<<"$S") RR_URL=$(jq -r .url <<<"$S")
rr() { curl -s -H "Authorization: Bearer $RR_TOKEN" "{{ORIGIN}}/api/v2$@"; }
echo "$RR_DOMAIN"
```

Use `$RR_URL` for HTTP callbacks and `$RR_DOMAIN` (or any name under it, such as `probe1.$RR_DOMAIN`) for DNS and mail. Unique sub-names per check tell you which one triggered a request. HTTP, HTTPS, DNS and mail to any address at the domain are all captured.

## See what arrived, and wait for callbacks

```sh
rr "/requests?last=20&view=summary"                 # what came in so far, newest 20
R=$(rr "/requests?after=now&wait=60&view=summary")  # block until the next one (max 60 s)
CURSOR=$(jq -r .cursor <<<"$R")
R=$(rr "/requests?after=$CURSOR&wait=60&view=summary")  # then the one after, and so on
```

Never sleep-and-poll. Start the `after=now` wait before triggering the callback (or trigger it in the background). An empty `requests` array means nothing arrived in time; pass the same cursor again. `view=summary` gives one line per request (`summary`) and the body as text (`body_text`); drop it, or `GET /requests/<id>`, for headers and raw bytes. Filter with `type=http|dns|smtp`. Use `jq ... <<<"$R"`, not `echo "$R" | jq` (zsh's echo mangles `\n`).

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
