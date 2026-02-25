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
use hyper::{body::Incoming, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use rustls::ServerConfig;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tower::ServiceExt;

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

pub async fn run(proxies: Vec<ProxyConfig>, tls_config: Arc<ServerConfig>) -> Result<()> {
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

    // Route every path through the same reverse-proxy handler.
    let app = Router::new()
        .route("/", any(proxy_handler))
        .route("/{*path}", any(proxy_handler))
        .with_state(state);

    let listener = TcpListener::bind(listen)
        .await
        .with_context(|| format!("failed to bind proxy socket {}", listen))?;
    let acceptor = TlsAcceptor::from(tls_config);

    logging::info("PROXY", &format!("https proxy listening on {}", listen));

    loop {
        let (stream, peer) = listener
            .accept()
            .await
            .context("failed to accept proxy tcp connection")?;

        let acceptor = acceptor.clone();
        let app = app.clone();

        tokio::spawn(async move {
            // TLS handshake happens before HTTP routing; SNI-based certificate
            // selection is handled inside rustls resolver.
            let tls_stream = match acceptor.accept(stream).await {
                Ok(v) => v,
                Err(err) => {
                    logging::error(
                        "PROXY",
                        &format!("tls handshake failed peer={} err={}", peer, err),
                    );
                    return;
                }
            };

            let io = TokioIo::new(tls_stream);
            let service = service_fn(move |req: Request<Incoming>| {
                let app = app.clone();
                async move { app.oneshot(req.map(Body::new)).await }
            });

            if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                logging::error(
                    "PROXY",
                    &format!("connection handling failed peer={} err={}", peer, err),
                );
            }
        });
    }
}

async fn proxy_handler(State(state): State<ProxyState>, req: Request<Body>) -> impl IntoResponse {
    let incoming_host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let normalized_host = normalize_host(incoming_host);

    // Upstream selection is based on HTTP Host so multiple domains can share
    // a single listener address/port.
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

    // Remove hop-by-hop headers and rewrite Host implicitly for the upstream.
    // Why: these headers are per-connection metadata and must not be forwarded.
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
    // Normalize host values from either "example.com" or "example.com:443"
    // into a route key.
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
    use axum::http::{HeaderName, Uri};

    use super::{build_target_url, is_hop_by_hop, normalize_host};

    #[test]
    fn normalize_host_removes_port() {
        assert_eq!(normalize_host("example.com:443"), "example.com");
        assert_eq!(normalize_host("example.com"), "example.com");
        assert_eq!(normalize_host("example.com."), "example.com");
    }

    #[test]
    fn normalize_host_ipv6() {
        assert_eq!(normalize_host("[::1]:443"), "::1");
        assert_eq!(normalize_host("[::1]"), "::1");
    }

    #[test]
    fn normalize_host_empty() {
        assert_eq!(normalize_host(""), "");
        assert_eq!(normalize_host("   "), "");
    }

    #[test]
    fn build_target_keeps_path_and_query() {
        let uri: Uri = "/a?b=1".parse().expect("uri should parse");
        assert_eq!(
            build_target_url("http://localhost:3000", &uri),
            "http://localhost:3000/a?b=1"
        );
    }

    #[test]
    fn hop_by_hop_headers() {
        assert!(is_hop_by_hop(&HeaderName::from_static("connection")));
        assert!(is_hop_by_hop(&HeaderName::from_static("transfer-encoding")));
        assert!(is_hop_by_hop(&HeaderName::from_static("keep-alive")));
        assert!(!is_hop_by_hop(&HeaderName::from_static("content-type")));
        assert!(!is_hop_by_hop(&HeaderName::from_static("host")));
    }
}
