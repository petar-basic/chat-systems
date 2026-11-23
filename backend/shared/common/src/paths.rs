pub const API_VERSION: u32 = 1;
pub const API_V1: &str = "/api/v1";
pub const WS_V1: &str = "/ws/v1";
pub const DOWNLOADS: &str = "/files/download/";

pub fn api_url(public_url: &str, path: &str) -> String {
    format!("{public_url}{API_V1}{path}")
}

pub fn download_prefix() -> String {
    format!("{API_V1}{DOWNLOADS}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_url_the_server_hands_out_carries_the_version() {
        assert_eq!(
            api_url("https://chat.example.com", "/hooks/incoming/t"),
            "https://chat.example.com/api/v1/hooks/incoming/t"
        );
        assert_eq!(download_prefix(), "/api/v1/files/download/");
    }
}
