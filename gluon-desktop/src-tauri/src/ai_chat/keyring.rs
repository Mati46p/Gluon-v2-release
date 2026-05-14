// ============================================================================
// API KEY STORAGE - System Keyring Integration
// ============================================================================

use keyring::Entry;

const SERVICE_NAME: &str = "com.mati0x.gluon-v2.ai-chat";

/// Store API key in system keyring (Windows Credential Manager)
pub fn store_api_key(provider_type: &str, api_key: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, provider_type)
        .map_err(|e| format!("Keyring error: {}", e))?;

    entry
        .set_password(api_key)
        .map_err(|e| format!("Failed to store API key: {}", e))?;

    println!(
        "[AiChat] API key stored in keyring for provider: {}",
        provider_type
    );
    Ok(())
}

/// Retrieve API key from system keyring
pub fn get_api_key(provider_type: &str) -> Result<String, String> {
    let entry = Entry::new(SERVICE_NAME, provider_type)
        .map_err(|e| format!("Keyring error: {}", e))?;

    entry
        .get_password()
        .map_err(|e| format!("API key not found for {}: {}", provider_type, e))
}

/// Check if API key exists in keyring
pub fn has_api_key(provider_type: &str) -> bool {
    Entry::new(SERVICE_NAME, provider_type)
        .and_then(|e| e.get_password())
        .is_ok()
}

/// Delete API key from keyring
pub fn delete_api_key(provider_type: &str) -> Result<(), String> {
    let entry = Entry::new(SERVICE_NAME, provider_type)
        .map_err(|e| format!("Keyring error: {}", e))?;

    entry
        .delete_credential()
        .map_err(|e| format!("Failed to delete API key: {}", e))?;

    Ok(())
}
