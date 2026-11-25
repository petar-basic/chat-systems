use keyring_core::{Entry, Error};

const SERVICE: &str = "chat-systems";

pub fn use_device_store() -> keyring_core::Result<()> {
    #[cfg(target_os = "ios")]
    keyring_core::set_default_store(apple_native_keyring_store::protected::Store::new()?);
    #[cfg(target_os = "android")]
    keyring_core::set_default_store(android_native_keyring_store::Store::new()?);
    Ok(())
}

#[tauri::command(async)]
pub fn load_session(server: String) -> Result<Option<String>, String> {
    match entry(&server)?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command(async)]
pub fn save_session(server: String, token: String) -> Result<(), String> {
    entry(&server)?
        .set_password(&token)
        .map_err(|error| error.to_string())
}

#[tauri::command(async)]
pub fn forget_session(server: String) -> Result<(), String> {
    match entry(&server)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn entry(server: &str) -> Result<Entry, String> {
    if server.is_empty() {
        return Err("no server".into());
    }
    Entry::new(SERVICE, server).map_err(|error| error.to_string())
}
