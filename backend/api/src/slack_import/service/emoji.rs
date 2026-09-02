//! Custom emoji, which the export does not carry.

use std::collections::HashMap;

use shared_common::errors::{AppError, AppResult};
use uuid::Uuid;

use super::super::source::{read_json, ExportSource};
use super::Import;
use super::{content_type_for, DRY_RUN_KEY};
use shared_common::errors::is_unique_violation;

impl Import<'_> {
    /// Custom emoji are not in the export — Slack keeps them behind `emoji.list`.
    /// An export carrying `emoji.json` (some tools write one) is used when it is
    /// there, so an import can bring them across without a token.
    pub(crate) async fn import_custom_emoji(
        &mut self,
        source: &mut dyn ExportSource,
    ) -> AppResult<()> {
        let listed = if source.has("emoji.json") {
            read_json::<HashMap<String, String>>(source, "emoji.json")?
        } else {
            match self.slack.custom_emoji().await {
                Ok(listed) => listed,
                Err(why) => {
                    self.report.skip("custom emoji", why);
                    return Ok(());
                }
            }
        };

        // Direct emoji first: an alias is only meaningful once the image it
        // points at has somewhere to point.
        let mut aliases: Vec<(String, String)> = Vec::new();
        let mut stored: HashMap<String, String> = HashMap::new();

        for (name, url) in &listed {
            if let Some(target) = url.strip_prefix("alias:") {
                aliases.push((name.clone(), target.to_string()));
                continue;
            }
            if let Some(key) = self.import_one_emoji(name, url).await? {
                stored.insert(name.clone(), key);
            }
        }

        for (name, target) in aliases {
            let key = match stored.get(&target) {
                Some(key) => key.clone(),
                None => match self.existing_emoji_key(&target).await? {
                    Some(key) => key,
                    None => {
                        self.report.skip(
                            format!("emoji :{name}:"),
                            format!("an alias of :{target}:, which was not imported"),
                        );
                        continue;
                    }
                },
            };
            // The alias reuses the image rather than downloading it twice.
            self.record_emoji(&name, &key).await?;
        }

        Ok(())
    }

    async fn import_one_emoji(&mut self, name: &str, url: &str) -> AppResult<Option<String>> {
        let name = match crate::emoji::routes::validate_name(name) {
            Ok(name) => name,
            Err(e) => {
                self.report.skip(format!("emoji :{name}:"), e.to_string());
                return Ok(None);
            }
        };

        if let Some(key) = self.existing_emoji_key(&name).await? {
            self.report.emoji_already_present += 1;
            return Ok(Some(key));
        }

        if self.dry_run {
            self.report.emoji_imported += 1;
            // Nothing is stored, but the alias pass still has to be able to see
            // that this name went past, or a dry run reports every alias as
            // unimportable and the report stops being worth reading.
            return Ok(Some(DRY_RUN_KEY.to_string()));
        }

        let bytes = match self
            .slack
            .fetch(url, crate::emoji::routes::MAX_EMOJI_BYTES as usize)
            .await
        {
            Ok(bytes) => bytes,
            Err(why) => {
                self.report.skip(format!("emoji :{name}:"), why);
                return Ok(None);
            }
        };
        let content_type = content_type_for(url);
        let storage_key = format!("emoji/{}/{}", self.workspace_id, Uuid::new_v4());
        if let Err(e) = self
            .state
            .file_storage
            .upload(&storage_key, bytes, content_type)
            .await
        {
            self.report.skip(format!("emoji :{name}:"), e.to_string());
            return Ok(None);
        }

        self.record_emoji(&name, &storage_key).await?;
        Ok(Some(storage_key))
    }

    async fn record_emoji(&mut self, name: &str, storage_key: &str) -> AppResult<()> {
        if storage_key == DRY_RUN_KEY {
            self.report.emoji_imported += 1;
            return Ok(());
        }
        match self
            .state
            .emoji_repo
            .create(self.workspace_id, name, storage_key, self.owner_id)
            .await
        {
            Ok(_) => self.report.emoji_imported += 1,
            Err(ref e) if is_unique_violation(e) => self.report.emoji_already_present += 1,
            Err(e) => return Err(AppError::Database(e.to_string())),
        }
        Ok(())
    }

    async fn existing_emoji_key(&self, name: &str) -> AppResult<Option<String>> {
        Ok(self
            .state
            .emoji_repo
            .find_by_name(self.workspace_id, name)
            .await?
            .map(|emoji| emoji.storage_key))
    }
}
