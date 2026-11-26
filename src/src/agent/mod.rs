//! Documents that let AI agents discover and use the API without a browser.
//!
//! Served on the main domain only: on a session subdomain every path belongs to the
//! session's own files, so `abcd1234.example.com/robots.txt` stays whatever its owner set.
//!
//! - `/llms.txt`: index for language models (<https://llmstxt.org>)
//! - `/docs/api.md`, `/docs/abuse.md`: the agent guide and the abuse reporting guide
//! - `/skill.md`: an Agent Skill (<https://agentskills.io>) for agents that load skills
//! - `/openapi.json`: machine-readable API description
//! - `/.well-known/api-catalog`: RFC 9727 API catalog pointing at the two above
//! - `/robots.txt` with Content Signals, and `/sitemap.xml`
//! - `/` and `/abuse` answer `Accept: text/markdown` with the matching guide, and every
//!   HTML page carries `Link` headers to the documents above.

use axum::{
    body::Body,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    response::Response,
};
use std::sync::OnceLock;

use crate::utils::config::CONFIG;

const LLMS_TXT: &str = include_str!("docs/llms.txt");
const API_MD: &str = include_str!("docs/api.md");
const ABUSE_MD: &str = include_str!("docs/abuse.md");
const SKILL_MD: &str = include_str!("docs/skill.md");
const OPENAPI_JSON: &str = include_str!("docs/openapi.json");

const MARKDOWN: &str = "text/markdown; charset=utf-8";
const PLAIN: &str = "text/plain; charset=utf-8";
const JSON: &str = "application/json";
const LINKSET: &str =
    "application/linkset+json; profile=\"https://www.rfc-editor.org/info/rfc9727\"";

/// Usage preferences for the documents (<https://contentsignals.org>): they exist to be
/// found and read by search engines and AI agents, including for training.
const CONTENT_SIGNAL: &str = "search=yes, ai-input=yes, ai-train=yes";

/// Advertised on every HTML page of the main domain.
const LINK_HEADER: &str = "</llms.txt>; rel=\"describedby\"; type=\"text/plain\", \
     </docs/api.md>; rel=\"alternate\"; type=\"text/markdown\", \
     </docs/api.md>; rel=\"service-doc\"; type=\"text/markdown\", \
     </openapi.json>; rel=\"service-desc\"; type=\"application/json\", \
     </.well-known/api-catalog>; rel=\"api-catalog\"";

struct Documents {
    llms_txt: String,
    api_md: String,
    abuse_md: String,
    skill_md: String,
    openapi_json: String,
    api_catalog: String,
    robots_txt: String,
    sitemap_xml: String,
}

/// The documents, with this instance's domain filled in. Rendered once.
fn documents() -> &'static Documents {
    static DOCUMENTS: OnceLock<Documents> = OnceLock::new();
    DOCUMENTS.get_or_init(|| {
        let origin = origin();
        let render = |template: &str| {
            template
                .replace("{{ORIGIN}}", &origin)
                .replace("{{WS_ORIGIN}}", &origin.replacen("http", "ws", 1))
                .replace("{{SCHEME}}", scheme())
                .replace("{{DOMAIN}}", &CONFIG.server_domain)
        };

        let mut abuse_md = render(ABUSE_MD);
        if let Some(contact) = &CONFIG.abuse_contact {
            abuse_md.push_str(&format!(
                "\nTo reach the operator of this instance directly: {contact}\n"
            ));
        }

        let api_catalog = serde_json::json!({
            "linkset": [{
                "anchor": format!("{origin}/api/v2"),
                "service-desc": [{ "href": format!("{origin}/openapi.json"), "type": JSON }],
                "service-doc": [
                    { "href": format!("{origin}/docs/api.md"), "type": "text/markdown" },
                    { "href": format!("{origin}/llms.txt"), "type": "text/plain" }
                ]
            }]
        })
        .to_string();

        let robots_txt = format!(
            "# requestrepo on {domain}\n\
             # Documentation for AI agents: {origin}/llms.txt\n\
             \n\
             User-agent: *\n\
             Content-Signal: {CONTENT_SIGNAL}\n\
             Allow: /\n\
             Disallow: /admin/\n\
             \n\
             Sitemap: {origin}/sitemap.xml\n",
            domain = CONFIG.server_domain,
        );

        let pages = [
            "/",
            "/abuse",
            "/llms.txt",
            "/docs/api.md",
            "/docs/abuse.md",
            "/skill.md",
            "/openapi.json",
        ];
        let sitemap_xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n{}</urlset>\n",
            pages
                .iter()
                .map(|page| format!("  <url><loc>{origin}{page}</loc></url>\n"))
                .collect::<String>()
        );

        Documents {
            llms_txt: render(LLMS_TXT),
            api_md: render(API_MD),
            abuse_md,
            skill_md: render(SKILL_MD),
            openapi_json: render(OPENAPI_JSON),
            api_catalog,
            robots_txt,
            sitemap_xml,
        }
    })
}

fn scheme() -> &'static str {
    if CONFIG.tls_enabled {
        "https"
    } else {
        "http"
    }
}

/// The public origin of the main domain, such as `https://requestrepo.com`.
fn origin() -> String {
    crate::http::routes_v2::public_origin(&CONFIG.server_domain)
}

/// Answers a request on the main domain for one of the agent documents, or `None` to let
/// the dashboard handle it.
pub fn serve(method: &Method, path: &str, headers: &HeaderMap) -> Option<Response> {
    if method != Method::GET && method != Method::HEAD {
        return None;
    }
    let docs = documents();
    let wants_markdown = prefers_markdown(headers);

    let response = match path {
        "/llms.txt" => document(&docs.llms_txt, PLAIN),
        "/docs/api.md" | "/index.md" | "/index.html.md" => document(&docs.api_md, MARKDOWN),
        "/docs/abuse.md" | "/abuse.md" => document(&docs.abuse_md, MARKDOWN),
        "/skill.md" | "/SKILL.md" => document(&docs.skill_md, MARKDOWN),
        "/openapi.json" => document(&docs.openapi_json, JSON),
        "/robots.txt" => document(&docs.robots_txt, PLAIN),
        "/sitemap.xml" => document(&docs.sitemap_xml, "application/xml"),
        "/.well-known/api-catalog" => {
            let mut response = document(&docs.api_catalog, LINKSET);
            response.headers_mut().insert(
                header::LINK,
                HeaderValue::from_static("</.well-known/api-catalog>; rel=\"api-catalog\""),
            );
            response
        }
        // Content negotiation: the same URL a person opens, in Markdown for an agent.
        "/" if wants_markdown => negotiated(&docs.api_md),
        "/abuse" if wants_markdown => negotiated(&docs.abuse_md),
        _ => return None,
    };

    Some(if method == Method::HEAD {
        without_body(response)
    } else {
        response
    })
}

/// Adds the discovery `Link` header to the main domain's HTML pages.
pub fn with_discovery_headers(mut response: Response, path: &str) -> Response {
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/html"));
    if is_html {
        let headers = response.headers_mut();
        headers.insert(header::LINK, HeaderValue::from_static(LINK_HEADER));
        // `/` and `/abuse` have a Markdown variant, so caches must keep them apart.
        if path == "/" || path == "/abuse" {
            headers.insert(header::VARY, HeaderValue::from_static("Accept"));
        }
    }
    response
}

/// Whether `Accept` ranks Markdown above HTML. Browsers never ask for Markdown, so any
/// mention of it that HTML does not outrank is taken as an agent asking for it.
fn prefers_markdown(headers: &HeaderMap) -> bool {
    let Some(accept) = headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };

    let quality = |wanted: &str| -> Option<f32> {
        accept.split(',').find_map(|item| {
            let mut parts = item.split(';');
            let media = parts.next()?.trim();
            if !media.eq_ignore_ascii_case(wanted) {
                return None;
            }
            let q = parts
                .find_map(|param| param.trim().strip_prefix("q="))
                .and_then(|q| q.trim().parse::<f32>().ok())
                .unwrap_or(1.0);
            Some(q)
        })
    };

    match (quality("text/markdown"), quality("text/html")) {
        (Some(markdown), Some(html)) => markdown > 0.0 && markdown >= html,
        (Some(markdown), None) => markdown > 0.0,
        _ => false,
    }
}

fn document(body: &str, content_type: &'static str) -> Response {
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=300")
        // Public documentation: let browser-based agents read it too.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::from(body.to_string()))
        .unwrap();
    if content_type == MARKDOWN || content_type == PLAIN {
        let headers = response.headers_mut();
        headers.insert("content-signal", HeaderValue::from_static(CONTENT_SIGNAL));
        // A rough token estimate, as Cloudflare's Markdown for Agents provides
        headers.insert(
            "x-markdown-tokens",
            HeaderValue::from(body.len().div_ceil(4)),
        );
    }
    response
}

fn negotiated(body: &str) -> Response {
    let mut response = document(body, MARKDOWN);
    response
        .headers_mut()
        .insert(header::VARY, HeaderValue::from_static("Accept"));
    // A cache that ignores Vary must not hand Markdown to a browser.
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

fn without_body(response: Response) -> Response {
    let (parts, _) = response.into_parts();
    Response::from_parts(parts, Body::empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accept(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, HeaderValue::from_str(value).unwrap());
        headers
    }

    #[test]
    fn markdown_is_chosen_only_when_asked_for() {
        assert!(prefers_markdown(&accept("text/markdown")));
        assert!(prefers_markdown(&accept("text/markdown, text/html")));
        assert!(prefers_markdown(&accept("text/html;q=0.5, text/markdown")));
        assert!(!prefers_markdown(&accept("text/markdown;q=0.5, text/html")));
        assert!(!prefers_markdown(&accept("text/markdown;q=0")));
        assert!(!prefers_markdown(&accept(
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
        )));
        assert!(!prefers_markdown(&accept("*/*")));
        assert!(!prefers_markdown(&HeaderMap::new()));
    }

    #[test]
    fn documents_are_rendered_for_this_instance() {
        let docs = documents();
        for (name, body) in [
            ("llms.txt", &docs.llms_txt),
            ("api.md", &docs.api_md),
            ("abuse.md", &docs.abuse_md),
            ("skill.md", &docs.skill_md),
            ("openapi.json", &docs.openapi_json),
        ] {
            assert!(!body.contains("{{"), "{name} has an unrendered placeholder");
            assert!(
                body.contains(&CONFIG.server_domain),
                "{name} names no domain"
            );
        }
        // llms.txt starts with the H1 the format requires
        assert!(docs.llms_txt.starts_with("# "));
        // The skill's frontmatter names it after its directory
        assert!(docs.skill_md.starts_with("---\nname: requestrepo\n"));
    }

    #[test]
    fn machine_readable_documents_parse() {
        let docs = documents();
        let openapi: serde_json::Value = serde_json::from_str(&docs.openapi_json).unwrap();
        assert_eq!(openapi["openapi"], "3.1.0");
        let catalog: serde_json::Value = serde_json::from_str(&docs.api_catalog).unwrap();
        assert!(catalog["linkset"][0]["service-desc"][0]["href"]
            .as_str()
            .unwrap()
            .ends_with("/openapi.json"));
    }

    /// Every `$ref` in the OpenAPI description points at a component that exists.
    #[test]
    fn openapi_references_resolve() {
        let openapi: serde_json::Value = serde_json::from_str(&documents().openapi_json).unwrap();
        fn walk(value: &serde_json::Value, root: &serde_json::Value) {
            match value {
                serde_json::Value::Object(map) => {
                    if let Some(reference) = map.get("$ref").and_then(|r| r.as_str()) {
                        let pointer = reference.trim_start_matches('#');
                        assert!(root.pointer(pointer).is_some(), "dangling {reference}");
                    }
                    map.values().for_each(|v| walk(v, root));
                }
                serde_json::Value::Array(items) => items.iter().for_each(|v| walk(v, root)),
                _ => {}
            }
        }
        walk(&openapi, &openapi);
    }

    #[test]
    fn only_get_and_head_are_answered() {
        let none = HeaderMap::new();
        assert!(serve(&Method::GET, "/llms.txt", &none).is_some());
        assert!(serve(&Method::POST, "/llms.txt", &none).is_none());
        assert!(serve(&Method::GET, "/", &none).is_none(), "the dashboard");
        assert!(serve(&Method::GET, "/", &accept("text/markdown")).is_some());
    }
}
