use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result, anyhow};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderName, Request, Response, StatusCode, Uri},
    response::IntoResponse,
    routing::any,
};

use crate::{config::ProxyConfig, logging};

#[derive(Clone)]
struct ProxyRoute {
    domain: String,
    upstream_host_port: String,
    base_url: String,
}

#[derive(Clone)]
struct ProxyState {
    routes: Arc<HashMap<String, ProxyRoute>>,
    client: reqwest::Client,
}

pub async fn run(proxies: Vec<ProxyConfig>) -> Result<()> {
    let listen = proxies
        .first()
        .map(|p| p.listen)
        .ok_or_else(|| anyhow!("at least one proxy config required"))?;

    let mut routes = HashMap::<String, ProxyRoute>::new();
    for p in &proxies {
        routes.insert(
            p.domain.clone(),
            ProxyRoute {
                domain: p.domain.clone(),
                upstream_host_port: p.upstream_host_port.clone(),
                base_url: p.base_url(),
            },
        );
    }

    let state = ProxyState {
        routes: Arc::new(routes),
        client: reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .context("failed to build proxy http client")?,
    };

    let app = Router::new()
        .route("/", any(proxy_handler))
        .route("/{*path}", any(proxy_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .with_context(|| format!("failed to bind proxy socket {}", listen))?;

    logging::info("PROXY", &format!("proxy listening on {}", listen));
    axum::serve(listener, app)
        .await
        .context("proxy server failed")
}

async fn proxy_handler(State(state): State<ProxyState>, req: Request<Body>) -> impl IntoResponse {
    let incoming_host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let normalized_host = normalize_host(incoming_host);

    let Some(route) = state.routes.get(&normalized_host) else {
        logging::error(
            "PROXY",
            &format!("no upstream configured for host={}", normalized_host),
        );
        return (StatusCode::BAD_GATEWAY, "no upstream configured for host").into_response();
    };

    let path = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or("/");
    logging::info(
        "PROXY",
        &format!(
            "route host={} domain={} upstream={}",
            incoming_host, route.domain, route.upstream_host_port
        ),
    );
    logging::debug(
        "PROXY",
        &format!(
            "request method={} host={} path={}",
            req.method(),
            normalized_host,
            path
        ),
    );

    match forward(&state.client, req, &route.base_url).await {
        Ok(resp) => {
            logging::debug(
                "PROXY",
                &format!("response status={} host={}", resp.status(), normalized_host),
            );
            resp.into_response()
        }
        Err(err) => {
            logging::error("PROXY", &format!("upstream request failed: {}", err));
            (StatusCode::BAD_GATEWAY, "proxy request failed").into_response()
        }
    }
}

async fn forward(
    client: &reqwest::Client,
    req: Request<Body>,
    base_url: &str,
) -> Result<Response<Body>> {
    let (parts, body) = req.into_parts();
    let target = build_target_url(base_url, &parts.uri);

    let body_bytes = to_bytes(body, usize::MAX)
        .await
        .context("failed to read request body")?;

    let mut upstream_req = client
        .request(parts.method.clone(), target)
        .body(body_bytes.to_vec());

    for (name, value) in &parts.headers {
        if *name != HeaderName::from_static("host") && !is_hop_by_hop(name) {
            upstream_req = upstream_req.header(name, value);
        }
    }

    let upstream_resp = upstream_req
        .send()
        .await
        .context("failed to send upstream request")?;
    let status = upstream_resp.status();
    let headers = upstream_resp.headers().clone();
    let body = upstream_resp
        .bytes()
        .await
        .context("failed to read upstream response body")?;

    let mut resp = Response::builder().status(status);
    for (name, value) in &headers {
        if !is_hop_by_hop(name) {
            resp = resp.header(name, value);
        }
    }

    resp.body(Body::from(body))
        .map_err(|e| anyhow!("failed to build response: {}", e))
}

fn build_target_url(base_url: &str, uri: &Uri) -> String {
    let path_and_query = uri.path_and_query().map(|v| v.as_str()).unwrap_or("/");
    format!("{}{}", base_url.trim_end_matches('/'), path_and_query)
}

fn normalize_host(raw: &str) -> String {
    let host = raw.trim().trim_end_matches('.');
    if host.is_empty() {
        return String::new();
    }

    if host.starts_with('[') {
        if let Some(end) = host.find(']') {
            return host[1..end].to_ascii_lowercase();
        }
    }

    if let Some((name, _port)) = host.rsplit_once(':') {
        if !name.is_empty() && !name.contains(':') {
            return name.to_ascii_lowercase();
        }
    }

    host.to_ascii_lowercase()
}

fn is_hop_by_hop(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection"
            | "proxy-connection"
            | "keep-alive"
            | "te"
            | "trailer"
            | "upgrade"
            | "transfer-encoding"
    )
}

#[cfg(test)]
mod tests {
    use axum::http::Uri;

    use super::{build_target_url, normalize_host};

    #[test]
    fn normalize_host_removes_port() {
        assert_eq!(normalize_host("example.com:443"), "example.com");
        assert_eq!(normalize_host("example.com"), "example.com");
        assert_eq!(normalize_host("example.com."), "example.com");
    }

    #[test]
    fn build_target_keeps_path_and_query() {
        let uri: Uri = "/a?b=1".parse().expect("uri should parse");
        assert_eq!(
            build_target_url("http://localhost:3000", &uri),
            "http://localhost:3000/a?b=1"
        );
    }
}
