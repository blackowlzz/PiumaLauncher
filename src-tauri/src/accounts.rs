use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::paths;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Offline,
    Microsoft,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub uuid: String,
    pub kind: AccountKind,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AccountStore {
    pub accounts: Vec<Account>,
    pub active: Option<String>,
}

pub fn load() -> AccountStore {
    std::fs::read_to_string(paths::accounts_file())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(store: &AccountStore) -> Result<()> {
    let path = paths::accounts_file();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(store)?)?;
    Ok(())
}

pub fn offline_uuid(name: &str) -> String {
    let digest = Md5::digest(format!("OfflinePlayer:{name}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest);
    uuid::Builder::from_md5_bytes(bytes).into_uuid().simple().to_string()
}

pub fn add_offline(name: &str) -> Result<Account> {
    let name = name.trim();
    let valid = (3..=16).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        return Err(Error::msg("The name must be 3-16 characters: letters, numbers and _"));
    }
    let mut store = load();
    if store.accounts.iter().any(|a| a.kind == AccountKind::Offline && a.name.eq_ignore_ascii_case(name)) {
        return Err(Error::msg("An offline account with this name already exists"));
    }
    let account = Account {
        id: uuid::Uuid::new_v4().simple().to_string(),
        name: name.to_string(),
        uuid: offline_uuid(name),
        kind: AccountKind::Offline,
    };
    if store.active.is_none() {
        store.active = Some(account.id.clone());
    }
    store.accounts.push(account.clone());
    save(&store)?;
    Ok(account)
}

pub fn upsert_microsoft(name: &str, uuid: &str) -> Result<Account> {
    let mut store = load();
    let account = match store.accounts.iter_mut().find(|a| a.kind == AccountKind::Microsoft && a.uuid == uuid) {
        Some(existing) => {
            existing.name = name.to_string();
            existing.clone()
        }
        None => {
            let account = Account {
                id: uuid::Uuid::new_v4().simple().to_string(),
                name: name.to_string(),
                uuid: uuid.to_string(),
                kind: AccountKind::Microsoft,
            };
            store.accounts.push(account.clone());
            account
        }
    };
    store.active = Some(account.id.clone());
    save(&store)?;
    Ok(account)
}

pub fn refresh_microsoft(id: &str, name: &str, uuid: &str) -> Result<()> {
    let mut store = load();
    if let Some(a) = store.accounts.iter_mut().find(|a| a.id == id) {
        if a.name != name || a.uuid != uuid {
            a.name = name.to_string();
            a.uuid = uuid.to_string();
            save(&store)?;
        }
    }
    Ok(())
}

pub fn remove(id: &str) -> Result<()> {
    crate::microsoft::forget(id);
    let mut store = load();
    store.accounts.retain(|a| a.id != id);
    if store.active.as_deref() == Some(id) {
        store.active = store.accounts.first().map(|a| a.id.clone());
    }
    save(&store)
}

pub fn set_active(id: &str) -> Result<()> {
    let mut store = load();
    if !store.accounts.iter().any(|a| a.id == id) {
        return Err(Error::msg("Account not found"));
    }
    store.active = Some(id.to_string());
    save(&store)
}

pub fn get(id: &str) -> Option<Account> {
    load().accounts.into_iter().find(|a| a.id == id)
}

pub fn active() -> Option<Account> {
    let store = load();
    let id = store.active?;
    store.accounts.into_iter().find(|a| a.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_vanilla() {
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
    }
}
