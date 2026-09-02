//! Imports a Slack export into a workspace.
//!
//! A CLI rather than an endpoint: an import is long, restartable and supervised
//! by whoever is doing the migration, not a request somebody is waiting on.
//!
//! ```text
//! chat-import --workspace <uuid|slug> --export ./slack-export.zip [--dry-run] [--slack-token xoxb-…]
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use chat_api::config::AppConfig;
use chat_api::slack_import::files::{HttpSlackClient, OfflineSlack, SlackClient};
use chat_api::slack_import::service::Import;
use chat_api::slack_import::source;
use chat_api::{build_state, connect_pool, init_tracing};
use uuid::Uuid;

struct Args {
    workspace: String,
    export: PathBuf,
    dry_run: bool,
    slack_token: Option<String>,
    fetch_files: bool,
}

fn usage() -> &'static str {
    "usage: chat-import --workspace <uuid|slug> --export <path> [--dry-run] \
     [--slack-token <token>] [--no-files]"
}

fn parse_args() -> Result<Args, String> {
    let mut workspace = None;
    let mut export = None;
    let mut dry_run = false;
    let mut slack_token = std::env::var("SLACK_TOKEN").ok();
    let mut fetch_files = true;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--workspace" => workspace = args.next(),
            "--export" => export = args.next().map(PathBuf::from),
            "--slack-token" => slack_token = args.next(),
            "--dry-run" => dry_run = true,
            "--no-files" => fetch_files = false,
            "-h" | "--help" => return Err(usage().to_string()),
            other => return Err(format!("unknown argument {other}\n{}", usage())),
        }
    }

    Ok(Args {
        workspace: workspace.ok_or_else(|| format!("--workspace is required\n{}", usage()))?,
        export: export.ok_or_else(|| format!("--export is required\n{}", usage()))?,
        dry_run,
        slack_token,
        fetch_files,
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    dotenvy::dotenv().ok();
    init_tracing();

    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("import failed: {message}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> Result<(), String> {
    let config = AppConfig::from_env();
    let pool = connect_pool(&config).await.map_err(|e| e.to_string())?;
    let state = build_state(pool.clone(), config)
        .await
        .map_err(|e| e.to_string())?;

    let workspace_id = resolve_workspace(&pool, &args.workspace).await?;
    let mut source = source::open(&args.export).map_err(|e| e.to_string())?;

    let slack: Box<dyn SlackClient> = if args.fetch_files {
        Box::new(HttpSlackClient::new(args.slack_token.clone()))
    } else {
        Box::new(OfflineSlack)
    };

    if args.dry_run {
        println!("dry run: nothing will be written");
    }

    let report = Import::open(&state, slack.as_ref(), workspace_id, args.dry_run)
        .await
        .map_err(|e| e.to_string())?
        .run(source.as_mut())
        .await
        .map_err(|e| e.to_string())?;

    println!("\n{}", report.summary());
    if !report.skipped.is_empty() {
        println!("\nnot imported:");
        for skipped in &report.skipped {
            println!("  {} — {}", skipped.what, skipped.why);
        }
    }

    Ok(())
}

/// A uuid if it parses as one, otherwise the workspace slug — whichever the
/// operator has to hand.
async fn resolve_workspace(pool: &sqlx::PgPool, workspace: &str) -> Result<Uuid, String> {
    if let Ok(id) = Uuid::parse_str(workspace) {
        return Ok(id);
    }

    sqlx::query_scalar!(
        "SELECT id FROM workspaces WHERE slug = $1 AND is_active = true",
        workspace
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("no workspace with slug {workspace}"))
}
