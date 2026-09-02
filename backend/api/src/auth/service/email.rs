use askama::Template;
use lettre::message::header::ContentType;
use lettre::message::MultiPart;
use lettre::transport::smtp::authentication::Credentials;
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
        self.mailer.is_some()
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

        let email = match html {
            Some(html) => builder.multipart(MultiPart::alternative_plain_html(
                text.to_string(),
                html.to_string(),
            )),
            None => builder
                .header(ContentType::TEXT_PLAIN)
                .body(text.to_string()),
        }
        .map_err(|e| AppError::Internal(format!("Failed to build email: {e}")))?;

        mailer
            .send(email)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to send email: {e}")))?;

        info!("Email sent to {}: {}", to, subject);
        Ok(())
    }
}
