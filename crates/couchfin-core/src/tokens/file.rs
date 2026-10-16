//! The fallback token store: a file only the user can read.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use couchfin_api::{AccessToken, ServerUrl};

use super::{TokenStore, TokenStoreError};
use crate::config_dir::ConfigDir;
use crate::fs_util::{read_if_exists, remove_if_exists, write_atomically};

/// Owner read and write, nothing for anyone else.
const PRIVATE: u32 = 0o600;

/// Tokens in `tokens.toml` in a [`ConfigDir`], a table from server address
/// to token, for when the keyring is unavailable.
///
/// The file is created with mode 0600 (set when it is created, never
/// widened first and narrowed after), and a missing config directory with
/// 0700. An existing file that others can read is tightened to 0600 the next
/// time it is read. Writes replace the file atomically.
///
/// # Examples
///
/// ```
/// use std::os::unix::fs::PermissionsExt;
/// use couchfin_core::{AccessToken, ConfigDir, FileTokens, TokenStore};
///
/// let dir = tempfile::tempdir()?;
/// let tokens = FileTokens::new(&ConfigDir::new(dir.path()));
/// let server = "jellyfin.lan:8096".parse()?;
/// tokens.save(&server, &AccessToken::new("0123456789abcdef")?)?;
///
/// let mode = std::fs::metadata(tokens.path())?.permissions().mode();
/// assert_eq!(mode & 0o777, 0o600);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct FileTokens {
    path: PathBuf,
}

impl FileTokens {
    /// Tokens in `dir/tokens.toml`.
    #[must_use]
    pub fn new(dir: &ConfigDir) -> Self {
        Self {
            path: dir.file("tokens.toml"),
        }
    }

    /// The token file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read(&self) -> Result<BTreeMap<String, String>, TokenStoreError> {
        let Some(text) = read_if_exists(&self.path).map_err(|e| io(&e))? else {
            return Ok(BTreeMap::new());
        };
        self.tighten()?;
        // toml's errors quote the offending line, which could be a token, so
        // only say that parsing failed.
        toml::from_str(&text).map_err(|_| TokenStoreError::Invalid {
            detail: format!("{} isn't a table of tokens", self.path.display()),
        })
    }

    fn write(&self, tokens: &BTreeMap<String, String>) -> Result<(), TokenStoreError> {
        if tokens.is_empty() {
            return remove_if_exists(&self.path).map_err(|e| io(&e));
        }
        let text = toml::to_string(tokens).map_err(|_| TokenStoreError::Invalid {
            detail: "couldn't encode the token table".to_owned(),
        })?;
        write_atomically(&self.path, &text, PRIVATE).map_err(|e| io(&e))
    }

    /// Makes an existing file private if group or others can access it.
    fn tighten(&self) -> Result<(), TokenStoreError> {
        let mode = fs::metadata(&self.path)
            .map_err(|e| io(&e))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            tracing::warn!(path = %self.path.display(), "token file was readable by others; making it private");
            fs::set_permissions(&self.path, fs::Permissions::from_mode(PRIVATE))
                .map_err(|e| io(&e))?;
        }
        Ok(())
    }
}

fn io(error: &std::io::Error) -> TokenStoreError {
    TokenStoreError::Io {
        detail: error.to_string(),
    }
}

impl TokenStore for FileTokens {
    fn load(&self, server: &ServerUrl) -> Result<Option<AccessToken>, TokenStoreError> {
        self.read()?
            .remove(server.as_str())
            .map(AccessToken::new)
            .transpose()
            .map_err(|e| TokenStoreError::Invalid {
                detail: e.to_string(),
            })
    }

    fn save(&self, server: &ServerUrl, token: &AccessToken) -> Result<(), TokenStoreError> {
        // A file that can't be parsed is replaced rather than blocking every
        // later login.
        let mut tokens = match self.read() {
            Err(TokenStoreError::Invalid { .. }) => BTreeMap::new(),
            other => other?,
        };
        tokens.insert(server.as_str().to_owned(), token.expose_secret().to_owned());
        self.write(&tokens)
    }

    fn delete(&self, server: &ServerUrl) -> Result<(), TokenStoreError> {
        let mut tokens = self.read()?;
        if tokens.remove(server.as_str()).is_some() {
            self.write(&tokens)?;
        }
        Ok(())
    }
}
