# Reporting abuse on {{DOMAIN}}

If a subdomain of `{{DOMAIN}}` is being used for abuse, such as a malware beacon, command and control, or a phishing page, report it. Please do not request or resolve the subdomain yourself: its owner would see your request.

- In the browser: {{ORIGIN}}/abuse
- Over the API:

```sh
curl -s -X POST {{ORIGIN}}/api/v2/abuse/reports \
  -H 'Content-Type: application/json' \
  -d '{"subdomain": "abcd1234.{{DOMAIN}}"}'
```

`subdomain` can be the label, any hostname under it, or a full URL. Nothing else is needed. Only a subdomain that is in use can be reported (`404 unknown_subdomain` otherwise).

**A report suspends the subdomain immediately.** A suspended subdomain serves nothing (`410 Gone`), does not resolve (`NXDOMAIN`), refuses mail, and its owner's access stops working.

Each IP address can file 3 reports per hour (an IPv6 /48 counts as one address); beyond that the answer is `429` with `retry_after`. `GET {{ORIGIN}}/api/v2/abuse` returns these rules as JSON. On a private instance, reports need the instance's admin token, like everything else there.

If your own subdomain was suspended by mistake, create a new session.
