//! Accounts: lifetime stats, coins and skins, saved to a JSON file.
//!
//! An account is identified by a random token the client keeps in local
//! storage. There are no passwords; losing the token loses the account.

use std::{collections::HashMap, path::PathBuf};

use bevy_ecs::resource::Resource;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

/// Number of skin colors.
pub const SKIN_COUNT: u8 = 10;
/// Skins every account has.
pub const FREE_SKINS: [u8; 3] = [0, 1, 2];
/// Coin price of the other skins.
pub const SKIN_PRICE: u32 = 50;

/// One account.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AccountData {
    /// Token.
    pub token:       String,
    /// Best score in one life.
    pub best_score:  u32,
    /// Kills across all games.
    pub total_kills: u32,
    /// Lives played.
    pub games:       u32,
    /// Coins to spend on skins.
    pub coins:       u32,
    /// Skins bought.
    pub skins:       Vec<u8>,
    /// Selected skin.
    pub skin:        u8,
}

impl AccountData {
    /// Skins usable by this account (free ones included).
    pub fn owned_skins(&self) -> Vec<u8> {
        let mut skins: Vec<u8> = FREE_SKINS.to_vec();
        skins.extend(self.skins.iter().filter(|s| !FREE_SKINS.contains(s)));
        skins
    }
}

/// All accounts.
#[derive(Resource)]
pub struct Accounts {
    /// Where they're saved.
    path:     PathBuf,
    /// Accounts by token.
    accounts: HashMap<String, AccountData>,
}

impl Accounts {
    /// Loads accounts from `path` (starting empty if it doesn't exist).
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let accounts = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<AccountData>>(&s).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|a| (a.token.clone(), a))
            .collect();
        Self { path, accounts }
    }

    /// Accounts kept only in memory (tests).
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self {
            path:     PathBuf::new(),
            accounts: HashMap::new(),
        }
    }

    /// Returns `token` if it's a known account, else creates an account and
    /// returns its new token.
    pub fn login(&mut self, token: &str) -> String {
        if self.accounts.contains_key(token) {
            return token.to_string();
        }
        let mut bytes = [0u8; 16];
        OsRng.fill_bytes(&mut bytes);
        let token = bytes.iter().fold(String::with_capacity(32), |mut s, b| {
            use std::fmt::Write;
            let _ = write!(s, "{b:02x}");
            s
        });
        self.accounts.insert(
            token.clone(),
            AccountData {
                token: token.clone(),
                ..AccountData::default()
            },
        );
        self.save();
        token
    }

    /// Looks up an account.
    pub fn get(&self, token: &str) -> Option<&AccountData> {
        self.accounts.get(token)
    }

    /// Records a finished life. Returns the updated account.
    pub fn record_game(&mut self, token: &str, score: u32, kills: u32, coins: u32) -> Option<AccountData> {
        let account = self.accounts.get_mut(token)?;
        account.games += 1;
        account.total_kills += kills;
        account.best_score = account.best_score.max(score);
        account.coins += coins;
        let account = account.clone();
        self.save();
        Some(account)
    }

    /// Buys a skin. Returns false if it can't be bought.
    pub fn buy_skin(&mut self, token: &str, skin: u8) -> bool {
        let Some(account) = self.accounts.get_mut(token) else {
            return false;
        };
        if skin >= SKIN_COUNT || account.owned_skins().contains(&skin) || account.coins < SKIN_PRICE {
            return false;
        }
        account.coins -= SKIN_PRICE;
        account.skins.push(skin);
        account.skin = skin;
        self.save();
        true
    }

    /// Selects an owned skin. Returns false if it isn't owned.
    pub fn select_skin(&mut self, token: &str, skin: u8) -> bool {
        let Some(account) = self.accounts.get_mut(token) else {
            return false;
        };
        if !account.owned_skins().contains(&skin) {
            return false;
        }
        account.skin = skin;
        self.save();
        true
    }

    /// Writes every account to disk (atomically, through a temp file).
    fn save(&self) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        let all: Vec<&AccountData> = self.accounts.values().collect();
        let Ok(json) = serde_json::to_string_pretty(&all) else {
            return;
        };
        let tmp = self.path.with_extension("json.tmp");
        if std::fs::write(&tmp, json)
            .and_then(|()| std::fs::rename(&tmp, &self.path))
            .is_err()
        {
            tracing::error!("failed to save accounts to {}", self.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_creates_accounts_and_reuses_tokens() {
        let mut accounts = Accounts::in_memory();
        let token = accounts.login("");
        assert_eq!(token.len(), 32);
        assert_eq!(accounts.login(&token), token);
        assert_ne!(accounts.login("made-up"), token);
    }

    #[test]
    fn games_earn_coins_for_skins() {
        let mut accounts = Accounts::in_memory();
        let token = accounts.login("");
        assert!(!accounts.buy_skin(&token, 5));
        let acc = accounts.record_game(&token, 900, 3, 60).unwrap();
        assert_eq!((acc.best_score, acc.total_kills, acc.games, acc.coins), (900, 3, 1, 60));
        assert!(accounts.buy_skin(&token, 5));
        assert!(!accounts.buy_skin(&token, 5), "already owned");
        assert!(accounts.select_skin(&token, 1));
        assert!(!accounts.select_skin(&token, 7));
    }
}
