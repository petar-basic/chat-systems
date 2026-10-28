use std::error::Error as _;
use std::io::ErrorKind;
use std::time::Duration;

use askama::Template;
use lettre::message::header::ContentType;
use lettre::message::MultiPart;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::Error as SmtpError;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use tracing::{info, warn};

use shared_common::errors::{AppError, AppResult};

use super::AuthService;
use crate::config::{AppConfig, SmtpTlsMode};
use crate::email::outbox::{self, NewEmail};
use crate::email::templates::InviteEmail;

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
    pub(super) async fn send_reset_email(&self, to_email: &str, reset_url: &str) -> AppResult<()> {
        let body = format!(
            "You requested a password reset.\n\nClick the link below to reset your password:\n{reset_url}\n\nThis link expires in 1 hour."
        );
        self.queue_email(to_email, "Reset your password", &body, None)
            .await
    }

    pub(super) async fn send_sso_reminder(&self, to_email: &str) -> AppResult<()> {
        let body = format!(
            "Somebody asked to reset the password for this address on {}.\n\nThis account signs in with single sign-on, so it has no password to reset. Open {} and choose Sign in with SSO.\n",
            self.config.instance_name, self.config.public_url
        );
        self.queue_email(to_email, "You sign in with SSO", &body, None)
            .await
    }

    pub async fn send_invite_email(
        &self,
        to_email: &str,
        workspace_name: &str,
        invite_url: &str,
    ) -> AppResult<()> {
        let text = format!(
            "You've been invited to join {} on {}.\n\nOpen this link to get started:\n{}\n",
            workspace_name, self.config.instance_name, invite_url
        );
        let html = InviteEmail {
            instance_name: &self.config.instance_name,
            workspace_name,
            invite_url,
            icon_url: self.config.instance_icon_url.as_deref(),
        }
        .render()
        .map_err(|e| AppError::Internal(format!("Failed to render invite email: {e}")))?;

        self.queue_email(
            to_email,
            &format!("Join {} on {}", workspace_name, self.config.instance_name),
            &text,
            Some(&html),
        )
        .await
    }

    async fn queue_email(
        &self,
        to: &str,
        subject: &str,
        text: &str,
        html: Option<&str>,
    ) -> AppResult<()> {
        if !self.can_send_email() {
            warn!(
                "Email not sent, SMTP is not configured: {} to {}",
                subject, to
            );
            return Ok(());
        }
        outbox::enqueue(
            self.repo.pool(),
            NewEmail {
                to,
                subject,
                text,
                html,
            },
        )
        .await?;
        Ok(())
    }

    /// Whether this instance can send at all. The mention digest is off rather
    /// than erroring when SMTP is unconfigured, the same way Web Push is off
    /// without VAPID keys.
    pub fn can_send_email(&self) -> bool {
        !self.config.smtp_host.trim().is_empty() && self.mailer.is_some()
    }

    pub async fn deliver(
        &self,
        to: &str,
        subject: &str,
        text: &str,
        html: Option<&str>,
    ) -> AppResult<()> {
        let mailer = self
            .mailer
            .as_ref()
            .ok_or_else(|| AppError::Internal("Email service not configured".into()))?;

        mailer
            .send(self.message(to, subject, text, html)?)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to send email: {e}")))?;

        info!("Email sent to {}: {}", to, subject);
        Ok(())
    }

    pub async fn send_test_email(&self, to: &str) -> AppResult<Option<String>> {
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

        let name = &self.config.instance_name;
        let message = self.message(
            to,
            &format!("Test email from {name}"),
            &format!("This is a test email from {name}. It reached you, so email works.\n"),
            None,
        )?;

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

    fn message(
        &self,
        to: &str,
        subject: &str,
        text: &str,
        html: Option<&str>,
    ) -> AppResult<Message> {
        let from = format!(
            "{} <{}>",
            self.config.smtp_from_name, self.config.smtp_from_address
        );

        let builder = Message::builder()
            .from(
                from.parse()
                    .map_err(|e| AppError::Internal(format!("Invalid from address: {e}")))?,
            )
            .to(to
                .parse()
                .map_err(|e| AppError::Internal(format!("Invalid to address: {e}")))?)
            .subject(subject);

        match html {
            Some(html) => builder.multipart(MultiPart::alternative_plain_html(
                text.to_string(),
                html.to_string(),
            )),
            None => builder
                .header(ContentType::TEXT_PLAIN)
                .body(text.to_string()),
        }
        .map_err(|e| AppError::Internal(format!("Failed to build email: {e}")))
    }
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
