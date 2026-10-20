use askama::Template;

#[derive(Template)]
#[template(path = "invite_email.html")]
pub struct InviteEmail<'a> {
    pub instance_name: &'a str,
    pub workspace_name: &'a str,
    pub invite_url: &'a str,
    pub icon_url: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_invite_escapes_what_it_interpolates() {
        let html = InviteEmail {
            instance_name: "Acme <b>",
            workspace_name: "Ops & <script>alert(1)</script>",
            invite_url: "https://chat.example/invite/abc?x=1&y=2",
            icon_url: Some("https://cdn.example/icon.png"),
        }
        .render()
        .unwrap();

        assert!(
            html.contains("Ops &#38; &#60;script&#62;")
                || html.contains("Ops &amp; &lt;script&gt;")
        );
        assert!(!html.contains("<script>"));
        assert!(html.contains(r#"src="https://cdn.example/icon.png""#));
        assert!(html.contains("x=1&#38;y=2") || html.contains("x=1&amp;y=2"));
    }

    #[test]
    fn a_non_https_icon_is_left_out() {
        let html = InviteEmail {
            instance_name: "Acme",
            workspace_name: "Ops",
            invite_url: "https://chat.example/invite/abc",
            icon_url: Some("http://cdn.example/icon.png"),
        }
        .render()
        .unwrap();
        assert!(!html.contains("<img"));
    }
}
