use std::time::Duration;

use reqwest::header::{ACCEPT, CONTENT_TYPE, LOCATION};

use super::html;
use crate::hooks::ssrf;
use crate::messaging::models::Unfurl;

const TIMEOUT: Duration = Duration::from_secs(3);
const MAX_BODY: usize = 512 * 1024;
const MAX_REDIRECTS: usize = 3;
const USER_AGENT: &str = concat!(
    "Chat-Systems/",
    env!("CARGO_PKG_VERSION"),
    " (link preview)"
);

pub async fn fetch(url: &str, allow_private: bool) -> Option<Unfurl> {
    let mut current = url.to_string();
    for _ in 0..=MAX_REDIRECTS {
        let (page, addrs) = ssrf::resolve_outbound(&current, allow_private).await.ok()?;
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(TIMEOUT)
            .user_agent(USER_AGENT);
        if let Some(domain) = page.domain() {
            builder = builder.resolve_to_addrs(domain, &addrs);
        }
        let client = builder.build().ok()?;
        let mut response = client
            .get(page.clone())
            .header(ACCEPT, "text/html,application/xhtml+xml")
            .send()
            .await
            .ok()?;

        if response.status().is_redirection() {
            let location = response.headers().get(LOCATION)?.to_str().ok()?;
            current = page.join(location).ok()?.to_string();
            continue;
        }
        if !response.status().is_success() || !is_html(&response) {
            return None;
        }

        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            let room = MAX_BODY - body.len();
            body.extend_from_slice(&chunk[..chunk.len().min(room)]);
            if body.len() == MAX_BODY {
                break;
            }
        }
        return html::parse(&String::from_utf8_lossy(&body), &page, url);
    }
    None
}

fn is_html(response: &reqwest::Response) -> bool {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_ascii_lowercase())
        .is_some_and(|value| {
            value.starts_with("text/html") || value.starts_with("application/xhtml+xml")
        })
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::fetch;

    async fn serve(responses: Vec<String>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            for response in responses {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let mut request = [0u8; 4096];
                let _ = socket.read(&mut request).await;
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
        });
        format!("http://{addr}")
    }

    fn html_response(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[tokio::test]
    async fn reads_a_page() {
        let base = serve(vec![html_response(
            r#"<head><meta property="og:title" content="Hello"></head>"#,
        )])
        .await;
        let unfurl = fetch(&format!("{base}/a"), true).await.expect("unfurl");
        assert_eq!(unfurl.title, "Hello");
        assert_eq!(unfurl.url, format!("{base}/a"));
    }

    #[tokio::test]
    async fn follows_a_redirect() {
        let base = serve(vec![
            "HTTP/1.1 301 Moved\r\nlocation: /b\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                .to_string(),
            html_response("<head><title>Moved here</title></head>"),
        ])
        .await;
        let unfurl = fetch(&format!("{base}/a"), true).await.expect("unfurl");
        assert_eq!(unfurl.title, "Moved here");
        assert_eq!(unfurl.url, format!("{base}/a"));
    }

    #[tokio::test]
    async fn refuses_a_private_address() {
        let base = serve(vec![html_response("<head><title>Secret</title></head>")]).await;
        assert!(fetch(&format!("{base}/a"), false).await.is_none());
    }

    #[tokio::test]
    async fn skips_what_is_not_html() {
        let base = serve(vec![
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}"
                .to_string(),
        ])
        .await;
        assert!(fetch(&format!("{base}/a"), true).await.is_none());
    }
}
