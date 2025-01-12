use crate::util;
use atrium_api::agent::store::{MemorySessionStore, SessionStore};
use atrium_api::agent::Session;
use std::path::PathBuf;

pub const SESSION_FILE: &str = "session.json";

pub struct LocalFileSessionStore {
    path: PathBuf,
}

impl LocalFileSessionStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl Default for LocalFileSessionStore {
    fn default() -> Self {
        LocalFileSessionStore::new(SESSION_FILE)
    }
}

impl SessionStore for LocalFileSessionStore {
    async fn get_session(&self) -> Option<Session> {
        let Ok(session) = util::load_from_file(&self.path).await else {
            return None;
        };
        session
    }

    async fn set_session(&self, session: Session) {
        let _ = util::dump_to_private_file(&self.path, &session).await;
    }

    async fn clear_session(&self) {
        let _ = util::remove_file(&self.path).await;
    }
}

pub enum ChainableSessionStore {
    LocalFileSessionStore(LocalFileSessionStore),
    MemorySessionStore(MemorySessionStore),
}

impl ChainableSessionStore {
    pub fn memory() -> Self {
        Self::MemorySessionStore(MemorySessionStore::default())
    }
    #[allow(unused)]
    pub fn local_file(path: impl Into<PathBuf>) -> Self {
        Self::LocalFileSessionStore(LocalFileSessionStore::new(path))
    }

    pub fn local_file_default() -> Self {
        Self::LocalFileSessionStore(LocalFileSessionStore::default())
    }

    #[allow(unused)]
    pub fn as_memory(&self) -> Option<&MemorySessionStore> {
        match self {
            &ChainableSessionStore::LocalFileSessionStore(_) => None,
            &ChainableSessionStore::MemorySessionStore(ref s) => Some(s),
        }
    }

    #[allow(unused)]
    pub fn as_local_file(&self) -> Option<&LocalFileSessionStore> {
        match self {
            &ChainableSessionStore::LocalFileSessionStore(ref s) => Some(s),
            &ChainableSessionStore::MemorySessionStore(_) => None,
        }
    }
}

pub struct ChainedSessionStore {
    stores: Vec<ChainableSessionStore>,
}

impl ChainedSessionStore {
    pub fn new(stores: Vec<ChainableSessionStore>) -> Self {
        Self { stores }
    }
}

impl SessionStore for ChainedSessionStore {
    async fn get_session(&self) -> Option<Session> {
        for store in &self.stores {
            let session = match store {
                ChainableSessionStore::LocalFileSessionStore(s) => s.get_session().await,
                ChainableSessionStore::MemorySessionStore(s) => s.get_session().await,
            };
            if session.is_some() {
                return session;
            }
        }
        None
    }

    async fn set_session(&self, session: Session) {
        for store in &self.stores {
            match store {
                ChainableSessionStore::LocalFileSessionStore(s) => {
                    s.set_session(session.clone()).await
                }
                ChainableSessionStore::MemorySessionStore(s) => {
                    s.set_session(session.clone()).await
                }
            };
        }
    }

    async fn clear_session(&self) {
        for store in &self.stores {
            match store {
                ChainableSessionStore::LocalFileSessionStore(s) => s.clear_session().await,
                ChainableSessionStore::MemorySessionStore(s) => s.clear_session().await,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_session(handle: &str) -> Session {
        let session = r#"{
            "accessJwt": "test-saved-access-jwt",
            "did": "did:plc:test_did",
            "handle": "HANDLE_NAME",
            "refreshJwt": "test-saved-refresh-jwt"
        }"#
        .replace("HANDLE_NAME", handle);
        serde_json::from_str::<Session>(session.as_str()).unwrap()
    }

    fn create_test_session() -> Session {
        mock_session("test.handle")
    }

    #[tokio::test]
    async fn test_local_file_session_store() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let fs = LocalFileSessionStore::new(temp.path());
        let session = create_test_session();
        fs.set_session(session.clone()).await;

        let session2 = fs.get_session().await.unwrap();
        assert_eq!(session, session2);

        fs.clear_session().await;
        let session3 = fs.get_session().await;
        assert!(session3.is_none());
    }

    struct TestSessionStoreWrapper {
        chained_session_store: ChainedSessionStore,
        #[allow(unused)]
        temp_session_file: tempfile::NamedTempFile,
    }

    fn get_test_chained_session_store() -> TestSessionStoreWrapper {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let fs = ChainableSessionStore::local_file(tmp.path());
        let ms = ChainableSessionStore::memory();
        let chained = ChainedSessionStore {
            stores: vec![fs, ms],
        };
        TestSessionStoreWrapper {
            chained_session_store: chained,
            temp_session_file: tmp,
        }
    }

    fn split_test_chained_session_store(
        store: &TestSessionStoreWrapper,
    ) -> (
        &ChainedSessionStore,
        &LocalFileSessionStore,
        &MemorySessionStore,
    ) {
        let fs = store
            .chained_session_store
            .stores
            .get(0)
            .unwrap()
            .as_local_file()
            .unwrap();
        let ms = store
            .chained_session_store
            .stores
            .get(1)
            .unwrap()
            .as_memory()
            .unwrap();
        (&store.chained_session_store, fs, ms)
    }

    #[tokio::test]
    async fn test_read_from_first_chained_store() {
        let chained_store = get_test_chained_session_store();
        let (chained_store, fs, _) = split_test_chained_session_store(&chained_store);
        let session = create_test_session();
        fs.set_session(session.clone()).await;

        let session2 = chained_store.get_session().await.unwrap();
        assert_eq!(session, session2);
    }
    #[tokio::test]
    async fn test_read_from_second_chained_store() {
        let chained_store = get_test_chained_session_store();
        let (chained_store, _, ms) = split_test_chained_session_store(&chained_store);
        let session = create_test_session();
        ms.set_session(session.clone()).await;

        let session2 = chained_store.get_session().await.unwrap();
        assert_eq!(session, session2);
    }

    #[tokio::test]
    async fn test_read_from_chained_empty_stores() {
        let chained_store = get_test_chained_session_store();
        let session = chained_store.chained_session_store.get_session().await;
        assert_eq!(session, None);
        assert!(session.is_none());
    }

    #[tokio::test]
    async fn test_read_chained_store_with_priority() {
        let chained_store = get_test_chained_session_store();
        let (chained_store, fs, ms) = split_test_chained_session_store(&chained_store);
        let session1 = mock_session("test.handle-1");
        let session2 = mock_session("test.handle-2");
        fs.set_session(session1.clone()).await;
        ms.set_session(session2.clone()).await;

        let read_back = chained_store.get_session().await.unwrap();
        assert_eq!(read_back, session1);
    }

    #[tokio::test]
    async fn test_save_into_both() {
        let chained_store = get_test_chained_session_store();
        let (chained_store, fs, ms) = split_test_chained_session_store(&chained_store);
        let session = create_test_session();
        chained_store.set_session(session.clone()).await;

        let session2 = fs.get_session().await.unwrap();
        let session3 = ms.get_session().await.unwrap();
        assert_eq!(session, session2);
        assert_eq!(session, session3);
    }

    #[tokio::test]
    async fn test_clear_both() {
        let chained_store = get_test_chained_session_store();
        let (chained_store, fs, ms) = split_test_chained_session_store(&chained_store);
        let session = create_test_session();
        fs.set_session(session.clone()).await;
        ms.set_session(session.clone()).await;
        assert!(chained_store.get_session().await.is_some());

        chained_store.clear_session().await;
        let session2 = fs.get_session().await;
        let session3 = ms.get_session().await;
        assert!(session2.is_none());
        assert!(session3.is_none());
    }
}
