use std::path::{Path, PathBuf};

use url::Url;

pub fn same_origin(server: &Url, target: &Url) -> bool {
    target.scheme() == "about" || server.origin() == target.origin()
}

pub fn opens_outside(server: &Url, target: &Url) -> bool {
    matches!(target.scheme(), "https" | "http" | "mailto") && !same_origin(server, target)
}

pub fn download_path(dir: &Path, suggested: &Path, url: &Url) -> PathBuf {
    let name = suggested
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            url.path_segments()
                .and_then(|mut s| s.next_back().map(str::to_owned))
                .filter(|n| !n.is_empty())
        })
        .unwrap_or_else(|| "download".into());

    let candidate = dir.join(&name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(&name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(&name);
    let extension = path.extension().and_then(|e| e.to_str());
    (1..)
        .map(|n| match extension {
            Some(ext) => dir.join(format!("{stem} ({n}).{ext}")),
            None => dir.join(format!("{stem} ({n})")),
        })
        .find(|p| !p.exists())
        .unwrap_or(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn a_window_stays_on_its_own_server() {
        let server = url("https://chat.example.com/");
        assert!(same_origin(
            &server,
            &url("https://chat.example.com/app/1/2")
        ));
        assert!(same_origin(
            &server,
            &url("blob:https://chat.example.com/3f2a")
        ));
        assert!(same_origin(&server, &url("about:blank")));
    }

    #[test]
    fn another_host_scheme_or_port_is_somewhere_else() {
        let server = url("https://chat.example.com/");
        assert!(!same_origin(&server, &url("https://example.com/")));
        assert!(!same_origin(&server, &url("http://chat.example.com/")));
        assert!(!same_origin(
            &server,
            &url("https://chat.example.com:8443/")
        ));
        assert!(!same_origin(
            &server,
            &url("https://chat.example.com.evil.test/")
        ));
    }

    #[test]
    fn a_link_elsewhere_opens_in_the_browser() {
        let server = url("https://chat.example.com/");
        assert!(opens_outside(&server, &url("https://example.com/docs")));
        assert!(opens_outside(&server, &url("http://chat.example.com/")));
        assert!(opens_outside(&server, &url("mailto:ana@example.com")));
    }

    #[test]
    fn a_link_on_the_server_or_to_another_scheme_does_not() {
        let server = url("https://chat.example.com/");
        assert!(!opens_outside(
            &server,
            &url("https://chat.example.com/app/1/2")
        ));
        assert!(!opens_outside(&server, &url("about:blank")));
        assert!(!opens_outside(&server, &url("file:///etc/passwd")));
        assert!(!opens_outside(&server, &url("javascript:alert(1)")));
    }

    #[test]
    fn a_download_never_overwrites_a_file() {
        let dir = std::env::temp_dir().join(format!("chat-systems-dl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = url("https://chat.example.com/api/files/1/download");

        let first = download_path(&dir, Path::new("report.pdf"), &file);
        assert_eq!(first, dir.join("report.pdf"));
        std::fs::write(&first, b"x").unwrap();
        assert_eq!(
            download_path(&dir, Path::new("report.pdf"), &file),
            dir.join("report (1).pdf")
        );
        assert_eq!(
            download_path(&dir, Path::new(""), &file),
            dir.join("download")
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
