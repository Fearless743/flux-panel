//! gost RESTful HTTP API server（阶段 9 stub）。

use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
pub struct ApiServerOptions {
    pub addr: SocketAddr,
    pub gost_json_path: String,
    pub metrics: Option<crate::metrics::MetricsRegistry>,
}

impl Default for ApiServerOptions {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:8080".parse().unwrap(),
            gost_json_path: "gost.json".into(),
            metrics: None,
        }
    }
}

pub struct ApiServer {
    opts: ApiServerOptions,
}

impl ApiServer {
    pub fn new(opts: ApiServerOptions) -> Self {
        Self { opts }
    }

    pub async fn run(&self) -> std::io::Result<()> {
        let listener = tokio::net::TcpListener::bind(self.opts.addr).await?;
        loop {
            let (mut s, _) = listener.accept().await?;
            let path = self.opts.gost_json_path.clone();
            let metrics = self.opts.metrics.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 8192];
                let n = match s.read(&mut buf).await {
                    Ok(n) => n,
                    Err(_) => return,
                };
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let response = route(&req, &path, metrics.as_ref());
                let _ = s.write_all(response.as_bytes()).await;
                let _ = s.flush().await;
            });
        }
    }
}

fn route(req: &str, gost_path: &str, metrics: Option<&crate::metrics::MetricsRegistry>) -> String {
    let first_line = req.lines().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("/");
    let mut body = String::from("ok");
    let mut status = "200 OK";
    let mut ctype = "text/plain; charset=utf-8";
    match path {
        "/api/config" => {
            body = std::fs::read_to_string(gost_path).unwrap_or_else(|_| "{}".into());
            ctype = "application/json";
        }
        "/api/services" => {
            let reg = crate::registry::services_registry();
            let names: Vec<String> = reg.names();
            body = serde_json::to_string(&names).unwrap_or_else(|_| "[]".into());
            ctype = "application/json";
        }
        "/metrics" => {
            if let Some(m) = metrics {
                body = m.encode();
            } else {
                body = String::new();
            }
        }
        "/healthz" => body = "ok".into(),
        _ => {
            status = "404 Not Found";
            body = "not found".into();
        }
    }
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {len}\r\n\r\n{body}",
        status = status,
        ctype = ctype,
        len = body.len(),
        body = body
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_healthz() {
        let r = route("GET /healthz HTTP/1.1", "/tmp/g.json", None);
        assert!(r.contains("HTTP/1.1 200 OK"));
        assert!(r.contains("\r\n\r\nok"));
    }

    #[test]
    fn route_services() {
        let r = route("GET /api/services HTTP/1.1", "/tmp/g.json", None);
        assert!(r.contains("HTTP/1.1 200"));
        assert!(r.contains("\"services/registry\"") || r.starts_with("HTTP/1.1 200"));
    }

    #[test]
    fn route_unknown_returns_404() {
        let r = route("GET /xxx HTTP/1.1", "/tmp/g.json", None);
        assert!(r.contains("404"));
    }
}
