use std::error::Error as _;
use std::io::ErrorKind;
use std::time::Duration;

use lettre::message::header::{ContentType, HeaderName, HeaderValue};
use lettre::message::MultiPart;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::Error as SmtpError;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use tracing::{info, warn};
use uuid::Uuid;

use shared_common::errors::{AppError, AppResult};

use super::AuthService;
use crate::config::{AppConfig, SmtpTlsMode};
use crate::email::message::{Button, Email};
use crate::email::outbox::{self, Envelope, NewEmail};
use crate::notifications::text::{self, Words};
use crate::profile::models::Language;

pub(super) fn build_mailer(config: &AppConfig) -> Option<AsyncSmtpTransport<Tokio1Executor>> {
    let creds = Credentials::new(config.smtp_user.clone(), config.smtp_password.clone());
    let builder = match config.smtp_tls_mode {
        SmtpTlsMode::Implicit => {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host).ok()
        }
        SmtpTlsMode::Starttls => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host).ok()
        }
        SmtpTlsMode::None => Some(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
            &config.smtp_host,
        )),
    };
    builder.map(|b| b.port(config.smtp_port).credentials(creds).build())
}

impl AuthService {
    pub async fn language_for(
        &self,
        user_id: Uuid,
        accept_language: Option<&str>,
    ) -> AppResult<Language> {
        let preference = self.repo.language(user_id).await?;
        Ok(text::reader_language(
            text::language(preference.as_deref()),
            accept_language,
        ))
    }

    pub(super) async fn send_reset_email(
        &self,
        to_email: &str,
        reset_url: &str,
        language: Language,
    ) -> AppResult<()> {
        let words = text::words(language);
        let instance = self.config.instance_name.as_str();
        let email = Email {
            subject: words.get("email.reset.subject", &[("instance", instance)]),
            heading: words.get("email.reset.heading", &[]),
            paragraphs: vec![words.get(
                "email.reset.body",
                &[("email", to_email), ("instance", instance)],
            )],
            button: Some(Button {
                label: words.get("email.reset.button", &[]),
                url: reset_url.to_string(),
            }),
            notes: vec![
                words.get("email.reset.expiry", &[]),
                words.get("email.reset.notYou", &[]),
            ],
            ..Email::default()
        };
        self.queue(to_email, email, &words).await
    }

    pub(crate) async fn send_sso_reminder(
        &self,
        to_email: &str,
        language: Language,
    ) -> AppResult<()> {
        let words = text::words(language);
        let instance = self.config.instance_name.as_str();
        let email = Email {
            subject: words.get("email.sso.subject", &[("instance", instance)]),
            heading: words.get("email.sso.heading", &[]),
            paragraphs: vec![
                words.get("email.sso.body", &[("instance", instance)]),
                words.get("email.sso.how", &[("instance", instance)]),
            ],
            button: Some(Button {
                label: words.get("email.sso.button", &[("instance", instance)]),
                url: self.config.public_url.clone(),
            }),
            notes: vec![words.get("email.sso.notYou", &[])],
            ..Email::default()
        };
        self.queue(to_email, email, &words).await
    }

    pub(crate) async fn send_invite_expired(
        &self,
        to_email: &str,
        workspace_name: Option<&str>,
        language: Language,
    ) -> AppResult<()> {
        let words = text::words(language);
        let instance = self.config.instance_name.as_str();
        let (subject, what) = match workspace_name {
            Some(workspace) => (
                words.get("email.inviteExpired.subject", &[("workspace", workspace)]),
                words.get("email.inviteExpired.workspace", &[("workspace", workspace)]),
            ),
            None => (
                words.get(
                    "email.inviteExpired.subjectNoWorkspace",
                    &[("instance", instance)],
                ),
                words.get("email.inviteExpired.noWorkspace", &[]),
            ),
        };
        let email = Email {
            subject,
            heading: words.get("email.inviteExpired.heading", &[]),
            paragraphs: vec![
                words.get("email.inviteExpired.body", &[("instance", instance)]),
                what,
            ],
            ..Email::default()
        };
        self.queue(to_email, email, &words).await
    }

    pub(crate) async fn send_sign_in_code(
        &self,
        to_email: &str,
        code: &str,
        lifetime_mins: i32,
        language: Language,
    ) -> AppResult<()> {
        let words = text::words(language);
        let instance = self.config.instance_name.as_str();
        let email = Email {
            subject: words.get(
                "email.code.subject",
                &[("code", code), ("instance", instance)],
            ),
            heading: words.get("email.code.heading", &[]),
            paragraphs: vec![words.get("email.code.body", &[("instance", instance)])],
            code: Some(code.to_string()),
            notes: vec![
                words.count("email.code.expiry", i64::from(lifetime_mins), &[]),
                words.get("email.code.notYou", &[]),
            ],
            ..Email::default()
        };
        self.queue(to_email, email, &words).await
    }

    pub async fn send_invite_email(
        &self,
        to_email: &str,
        invite: InviteEmail<'_>,
        language: Language,
    ) -> AppResult<()> {
        let words = text::words(language);
        let instance = self.config.instance_name.as_str();
        let workspace = invite.workspace_name;
        let body = match invite.inviter {
            Some(inviter) => words.get(
                "email.invite.body",
                &[
                    ("inviter", inviter),
                    ("workspace", workspace),
                    ("instance", instance),
                ],
            ),
            None => words.get(
                "email.invite.bodyNoInviter",
                &[("workspace", workspace), ("instance", instance)],
            ),
        };
        let days = ((invite.expires_at - chrono::Utc::now()).num_hours() + 23)
            .div_euclid(24)
            .max(1);
        let email = Email {
            subject: words.get(
                "email.invite.subject",
                &[("workspace", workspace), ("instance", instance)],
            ),
            heading: words.get("email.invite.heading", &[("workspace", workspace)]),
            paragraphs: vec![body],
            button: Some(Button {
                label: words.get("email.invite.button", &[("workspace", workspace)]),
                url: invite.url.to_string(),
            }),
            notes: vec![
                words.count("email.invite.expiry", days, &[]),
                words.get("email.invite.notExpected", &[]),
            ],
            ..Email::default()
        };
        self.queue(to_email, email, &words).await
    }

    async fn queue(&self, to: &str, email: Email, words: &Words<'_>) -> AppResult<()> {
        if !self.can_send_email() {
            warn!(
                "Email not sent, SMTP is not configured: {} to {}",
                email.subject, to
            );
            return Ok(());
        }
        let rendered = email.render(to, &self.config, words)?;
        outbox::enqueue(self.repo.pool(), NewEmail::rendered(to, &rendered)).await?;
        Ok(())
    }

    /// Whether this instance can send at all. The mention digest is off rather
    /// than erroring when SMTP is unconfigured, the same way Web Push is off
    /// without VAPID keys.
    pub fn can_send_email(&self) -> bool {
        !self.config.smtp_host.trim().is_empty() && self.mailer.is_some()
    }

    pub async fn deliver(&self, envelope: &Envelope<'_>) -> AppResult<()> {
        let mailer = self
            .mailer
            .as_ref()
            .ok_or_else(|| AppError::Internal("Email service not configured".into()))?;

        mailer
            .send(self.message(envelope)?)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to send email: {e}")))?;

        info!("Email sent to {}: {}", envelope.to, envelope.subject);
        Ok(())
    }

    pub async fn send_test_email(&self, to: &str, language: Language) -> AppResult<Option<String>> {
        if self.config.smtp_host.trim().is_empty() {
            return Ok(Some(
                "SMTP_HOST is not set, so there is no server to send through".into(),
            ));
        }
        let Some(mailer) = self.mailer.as_ref() else {
            return Ok(Some(format!(
                "No mail transport could be built for SMTP_HOST {}. Check the host name and SMTP_TLS_MODE",
                self.config.smtp_host
            )));
        };

        let words = text::words(language);
        let instance = self.config.instance_name.as_str();
        let rendered = Email {
            subject: words.get("email.test.subject", &[("instance", instance)]),
            heading: words.get("email.test.heading", &[]),
            paragraphs: vec![words.get("email.test.body", &[("instance", instance)])],
            ..Email::default()
        }
        .render(to, &self.config, &words)?;
        let message = self.message(&Envelope {
            id: Uuid::new_v4(),
            to,
            subject: &rendered.subject,
            text: &rendered.text,
            html: Some(&rendered.html),
            list_unsubscribe: None,
        })?;

        let server = format!("{}:{}", self.config.smtp_host, self.config.smtp_port);
        match tokio::time::timeout(TEST_EMAIL_TIMEOUT, mailer.send(message)).await {
            Ok(Ok(_)) => {
                info!("Test email sent to {}", to);
                Ok(None)
            }
            Ok(Err(e)) => {
                warn!("Test email to {} failed: {}", to, e);
                Ok(Some(explain_smtp_error(&server, &e)))
            }
            Err(_) => Ok(Some(format!(
                "{server} did not answer within {} seconds. A firewall may be dropping the connection, or SMTP_TLS_MODE does not match the port",
                TEST_EMAIL_TIMEOUT.as_secs()
            ))),
        }
    }

    pub(super) fn message(&self, envelope: &Envelope<'_>) -> AppResult<Message> {
        let from = format!(
            "{} <{}>",
            self.config.smtp_from_name, self.config.smtp_from_address
        );
        let domain = self
            .config
            .smtp_from_address
            .rsplit_once('@')
            .map_or("localhost", |(_, domain)| domain);

        let mut builder = Message::builder()
            .from(
                from.parse()
                    .map_err(|e| AppError::Internal(format!("Invalid from address: {e}")))?,
            )
            .to(envelope
                .to
                .parse()
                .map_err(|e| AppError::Internal(format!("Invalid to address: {e}")))?)
            .subject(envelope.subject)
            .message_id(Some(format!("<{}@{}>", envelope.id, domain)))
            .raw_header(HeaderValue::new(
                HeaderName::new_from_ascii_str("Auto-Submitted"),
                "auto-generated".into(),
            ));
        if let Some(url) = envelope.list_unsubscribe {
            builder = builder
                .raw_header(HeaderValue::new(
                    HeaderName::new_from_ascii_str("List-Unsubscribe"),
                    format!("<{url}>"),
                ))
                .raw_header(HeaderValue::new(
                    HeaderName::new_from_ascii_str("List-Unsubscribe-Post"),
                    "List-Unsubscribe=One-Click".into(),
                ));
        }

        match envelope.html {
            Some(html) => builder.multipart(MultiPart::alternative_plain_html(
                envelope.text.to_string(),
                html.to_string(),
            )),
            None => builder
                .header(ContentType::TEXT_PLAIN)
                .body(envelope.text.to_string()),
        }
        .map_err(|e| AppError::Internal(format!("Failed to build email: {e}")))
    }
}

pub struct InviteEmail<'a> {
    pub workspace_name: &'a str,
    pub inviter: Option<&'a str>,
    pub url: &'a str,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

const TEST_EMAIL_TIMEOUT: Duration = Duration::from_secs(15);

fn io_cause(e: &SmtpError) -> Option<&std::io::Error> {
    let mut source = e.source();
    while let Some(cause) = source {
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            return Some(io);
        }
        source = cause.source();
    }
    None
}

pub fn explain_smtp_error(server: &str, e: &SmtpError) -> String {
    if let Some(code) = e.status() {
        return match code.to_string().as_str() {
            "530" | "534" | "535" | "538" => format!(
                "{server} refused the sign-in. Check SMTP_USER and SMTP_PASSWORD ({e})"
            ),
            _ => format!(
                "{server} refused the message. Check that SMTP_FROM_ADDRESS may send through it ({e})"
            ),
        };
    }
    if e.is_tls() {
        return format!(
            "The TLS handshake with {server} failed. Check SMTP_TLS_MODE: port 465 expects implicit, port 587 expects starttls ({e})"
        );
    }
    if e.is_timeout() {
        return format!(
            "{server} did not answer in time. A firewall may be dropping the connection, or SMTP_TLS_MODE does not match the port"
        );
    }
    if let Some(io) = io_cause(e) {
        return match io.kind() {
            ErrorKind::ConnectionRefused => format!(
                "Nothing accepted the connection at {server}. Check SMTP_HOST and SMTP_PORT, and that the mail server is running"
            ),
            ErrorKind::TimedOut => format!(
                "{server} did not answer in time. A firewall may be dropping the connection"
            ),
            _ => format!("{server} could not be reached: {io}. Check SMTP_HOST and SMTP_PORT"),
        };
    }
    if e.is_client() || e.is_response() {
        return format!(
            "{server} does not speak the TLS mode SMTP_TLS_MODE asks for. Try implicit for port 465 or starttls for port 587 ({e})"
        );
    }
    format!("Sending through {server} failed: {e}")
}
