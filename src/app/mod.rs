use std::sync::{Arc, Mutex};

use crate::api::{ApiError, JellyfinClient, secret::Secret};

#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub url: String,
    pub version: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("no server selected")]
    NoServer,

    #[error(transparent)]
    Api(#[from] ApiError),
}

#[derive(Default)]
pub struct App {
    client: Mutex<Option<Arc<JellyfinClient>>>,
}

impl App {
    pub async fn connect(&self, url: &str) -> Result<ServerInfo, AppError> {
        let client = Arc::new(JellyfinClient::new(url)?);
        let info = client.get_public_info().await?;

        *self.client.lock().unwrap() = Some(client);
        Ok(ServerInfo {
            name: info.server_name,
            url: url.to_string(),
            version: info.version,
        })
    }

    pub async fn login(&self, username: &str, password: Secret) -> Result<String, AppError> {
        let client = self.client()?;
        let auth = client.login_by_name(username, password).await?;
        // TODO: keep auth.access_token for authenticated requests.
        Ok(auth.user.name.unwrap_or_else(|| username.to_string()))
    }

    pub fn disconnect(&self) {
        *self.client.lock().unwrap() = None;
    }

    fn client(&self) -> Result<Arc<JellyfinClient>, AppError> {
        self.client
            .lock()
            .unwrap()
            .clone()
            .ok_or(AppError::NoServer)
    }
}
