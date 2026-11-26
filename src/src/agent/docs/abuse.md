# Reporting abuse on {{DOMAIN}}

If a subdomain of `{{DOMAIN}}` is being used for abuse, such as a malware beacon, command and control, or a phishing page, report it. Please do not request or resolve the subdomain yourself: its owner would see your request.

- In the browser: {{ORIGIN}}/abuse
- Over the API: `POST {{ORIGIN}}/api/v2/abuse/reports`

```sh
curl -s -X POST {{ORIGIN}}/api/v2/abuse/reports \
  -H 'Content-Type: application/json' \
  -d '{"subdomain": "abcd1234.{{DOMAIN}}", "category": "malware", "reason": "Botnet beacon"}'
```

| Field | Required | Notes |
|-------|----------|-------|
| `subdomain` | yes | The label, any hostname under it, or a full URL |
| `category` | yes | `malware`, `phishing`, `csam`, `spam` or `other` |
| `reason` | no | Up to 300 characters, seen only by the operator |

**A report suspends the subdomain immediately.** A suspended subdomain serves nothing (`410 Gone`), does not resolve (`NXDOMAIN`), refuses mail, and its owner's access stops working. Its incoming traffic is still recorded for the operator, so that the sources of a beacon, which are usually compromised hosts, can be passed on to CERTs.

Each IP address can file 3 reports per hour (an IPv6 /48 counts as one address); beyond that the answer is `429` with `retry_after`. `GET {{ORIGIN}}/api/v2/abuse` returns these rules as JSON.

If your own subdomain was suspended by mistake, create a new session.
