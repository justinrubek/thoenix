use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

pub mod error;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TerraformState {
    lock: Option<TerraformLock>,
    pub data: String,
}

impl TerraformState {
    pub fn is_locked(&self) -> bool {
        self.lock.is_some()
    }

    pub fn check_lock(&self, id: &str) -> Result<()> {
        if self.lock.as_ref().is_some_and(|l| l.id != id) {
            return Err(Error::StateLocked);
        }

        Ok(())
    }

    pub fn lock(&mut self, lock: TerraformLock) -> Result<()> {
        if self.is_locked() {
            return Err(Error::StateLocked);
        }

        self.lock = Some(lock);

        Ok(())
    }

    pub fn unlock(&mut self, id: &TerraformLock) -> Result<()> {
        if self.lock.as_ref().is_some_and(|l| l.id != id.id) {
            return Err(Error::StateLocked);
        }

        self.lock = None;

        Ok(())
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TerraformLockQuery {
    #[serde(rename = "ID")]
    pub id: String,
}

/// The data given by the client when locking a state
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct TerraformLock {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Operation")]
    pub operation: String,
    #[serde(rename = "Info")]
    pub info: String,
    #[serde(rename = "Who")]
    pub who: String,
    #[serde(rename = "Version")]
    pub version: String,
}

#[allow(async_fn_in_trait)]
pub trait TerraformStateProvider {
    async fn get_state(&self, id: &str) -> Result<Option<TerraformState>>;
    async fn update_state(&mut self, id: &str, lock_id: &str, data: String) -> Result<()>;
    async fn lock_state(&mut self, id: &str, lock: TerraformLock) -> Result<()>;
    async fn unlock_state(&mut self, id: &str, lock: &TerraformLock) -> Result<()>;
}

/// A state provider that stores state in memory
#[derive(Debug, Default)]
pub struct InMemoryState {
    pub tf_state: std::collections::HashMap<String, TerraformState>,
}

impl InMemoryState {
    pub fn new() -> Self {
        Default::default()
    }

    /// Insert a new state into the provider
    pub async fn create_state(&mut self, id: String) -> Result<()> {
        self.tf_state.insert(id, TerraformState::default());

        Ok(())
    }

    pub async fn expect_not_locked(&self, id: &str) -> Result<()> {
        let state = self.tf_state.get(id).ok_or(error::Error::NotFound)?;

        if state.is_locked() {
            return Err(error::Error::StateLocked);
        }

        Ok(())
    }
}

impl TerraformStateProvider for InMemoryState {
    async fn get_state(&self, id: &str) -> Result<Option<TerraformState>> {
        let state = self.tf_state.get(id).cloned();

        Ok(state)
    }

    async fn update_state(&mut self, id: &str, lock_id: &str, data: String) -> Result<()> {
        // Ensure that the state is not locked
        let state = self.tf_state.get_mut(id).ok_or(error::Error::NotFound)?;
        state.check_lock(lock_id)?;

        state.data = data;

        Ok(())
    }

    async fn lock_state(&mut self, id: &str, lock: TerraformLock) -> Result<()> {
        let state = self.tf_state.get_mut(id).ok_or(error::Error::NotFound)?;

        state.lock(lock)?;

        Ok(())
    }

    async fn unlock_state(&mut self, id: &str, lock: &TerraformLock) -> Result<()> {
        let state = self.tf_state.get_mut(id).ok_or(error::Error::NotFound)?;

        state.unlock(lock)?;

        Ok(())
    }
}
