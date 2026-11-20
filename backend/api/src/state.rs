use crate::auth::service::AuthService;
use crate::config::AppConfig;
use crate::conversations::repo::ConversationRepo;
use crate::files::repo::FileRepo;
use crate::files::storage::FileStorage;
use crate::hooks::repo::HookRepo;
use crate::huddle::repo::HuddleRepo;
use crate::integrations::repo::IntegrationRepo;
use crate::later::repo::LaterRepo;
use crate::messaging::publisher::EventPublisher;
use crate::messaging::repo::MessageRepo;
use crate::notifications::repo::NotificationRepo;
use crate::scheduled::repo::ScheduledRepo;
use crate::sidebar::repo::SidebarRepo;
use crate::slack_import::repo::SlackImportRepo;
use crate::workspace::service::WorkspaceService;

pub struct AppState {
    pub config: AppConfig,
    pub pool: sqlx::PgPool,
    pub redis: redis::aio::ConnectionManager,
    pub auth_service: AuthService,
    pub workspace_service: WorkspaceService,
    pub message_repo: MessageRepo,
    pub publisher: EventPublisher,
    pub file_repo: FileRepo,
    pub file_storage: FileStorage,
    pub hook_repo: HookRepo,
    pub command_repo: crate::commands::repo::CommandRepo,
    pub bot_repo: crate::bots::repo::BotRepo,
    pub app_event_repo: crate::app_events::repo::AppEventRepo,
    pub interaction_repo: crate::interactions::repo::InteractionRepo,
    pub app_shortcut_repo: crate::app_shortcuts::repo::AppShortcutRepo,
    pub app_workflow_step_repo: crate::app_workflow_steps::repo::AppWorkflowStepRepo,
    pub app_unfurl_repo: crate::app_unfurls::repo::AppUnfurlRepo,
    pub app_home_repo: crate::app_home::repo::AppHomeRepo,
    pub app_directory_repo: crate::app_directory::repo::AppDirectoryRepo,
    pub notification_repo: NotificationRepo,
    pub conversation_repo: ConversationRepo,
    pub later_repo: LaterRepo,
    pub draft_repo: crate::drafts::repo::DraftRepo,
    pub search_repo: crate::searches::repo::SearchRepo,
    pub guest_repo: crate::guests::repo::GuestRepo,
    pub invite_request_repo: crate::invite_requests::repo::InviteRequestRepo,
    pub profile_field_repo: crate::profile_fields::repo::ProfileFieldRepo,
    pub custom_response_repo: crate::custom_responses::repo::CustomResponseRepo,
    pub workspace_bot_repo: crate::workspace_bots::repo::WorkspaceBotRepo,
    pub block_repo: crate::blocks::repo::BlockRepo,
    pub poll_repo: crate::polls::repo::PollRepo,
    pub list_repo: crate::lists::repo::ListRepo,
    pub thread_repo: crate::threads::repo::ThreadRepo,
    pub sidebar_repo: SidebarRepo,
    pub integration_repo: IntegrationRepo,
    pub calendar_repo: crate::calendars::repo::CalendarRepo,
    pub slack_import_repo: SlackImportRepo,
    pub scheduled_repo: ScheduledRepo,
    pub retention_repo: crate::retention::repo::RetentionRepo,
    pub export_repo: crate::export::repo::ExportRepo,
    pub totp_repo: crate::auth::totp_repo::TotpRepo,
    pub passkey_repo: crate::auth::passkey_repo::PasskeyRepo,
    pub email_code_repo: crate::auth::email_code_repo::EmailCodeRepo,
    pub instance_settings_repo: crate::admin::settings_repo::InstanceSettingsRepo,
    pub analytics_repo: crate::analytics::repo::AnalyticsRepo,
    pub scim_repo: crate::scim::repo::ScimRepo,
    pub setup_repo: crate::setup::repo::SetupRepo,
    pub push_repo: crate::push::repo::PushRepo,
    pub push_sender: crate::push::sender::PushSender,
    pub emoji_repo: crate::emoji::repo::EmojiRepo,
    pub profile_repo: crate::profile::repo::ProfileRepo,
    pub group_repo: crate::groups::repo::GroupRepo,
    pub huddle_repo: HuddleRepo,
    pub workflow_repo: crate::workflows::repo::WorkflowRepo,
}
