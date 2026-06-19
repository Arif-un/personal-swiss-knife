use crate::ssh::SshResult;

const SERVICE: &str = "swiss-knife-ssh";

/// Fetch a stored passphrase for a key/identity, if any.
pub fn get_passphrase(key_id: &str) -> Option<String> {
    let entry = keyring::Entry::new(SERVICE, key_id).ok()?;
    entry.get_password().ok()
}

/// Store a passphrase for a key/identity in the OS keychain.
pub fn set_passphrase(key_id: &str, secret: &str) -> SshResult<()> {
    let entry = keyring::Entry::new(SERVICE, key_id)?;
    entry.set_password(secret)?;
    Ok(())
}

/// Remove a stored passphrase.
#[allow(dead_code)]
pub fn delete_passphrase(key_id: &str) -> SshResult<()> {
    if let Ok(entry) = keyring::Entry::new(SERVICE, key_id) {
        let _ = entry.delete_credential();
    }
    Ok(())
}
