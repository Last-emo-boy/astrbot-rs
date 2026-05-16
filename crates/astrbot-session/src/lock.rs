use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Clone, Default)]
pub struct SessionLockManager {
    locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

impl SessionLockManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn acquire(&self, session_id: impl Into<String>) -> SessionLockGuard {
        let session_id = session_id.into();
        let lock = {
            let mut locks = self.locks.lock().await;
            locks
                .entry(session_id.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let guard = lock.clone().lock_owned().await;

        SessionLockGuard {
            session_id,
            manager: self.clone(),
            lock,
            _guard: guard,
        }
    }

    pub async fn tracked_session_count(&self) -> usize {
        self.locks.lock().await.len()
    }

    async fn release(&self, session_id: &str, lock: &Arc<Mutex<()>>) {
        let mut locks = self.locks.lock().await;
        if Arc::strong_count(lock) <= 2 {
            locks.remove(session_id);
        }
    }
}

pub struct SessionLockGuard {
    session_id: String,
    manager: SessionLockManager,
    lock: Arc<Mutex<()>>,
    _guard: OwnedMutexGuard<()>,
}

impl SessionLockGuard {
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub async fn release(self) {
        let session_id = self.session_id.clone();
        let manager = self.manager.clone();
        let lock = self.lock.clone();
        drop(self);
        manager.release(&session_id, &lock).await;
    }
}
