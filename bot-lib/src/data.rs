use crate::config::Config;
use color_eyre::eyre::{Error, Result, bail};
use parking_lot::Mutex;
use rusqlite::Connection;
use std::sync::LazyLock;
use std::time::Duration;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::RwLock;

/// Schema migrations, applied in order; `PRAGMA user_version` records how many
/// have run. Never edit a migration that has been deployed, append a new one.
/// tools/migrate-surrealdb-to-sqlite applies the same list.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_initial.sql")];

#[cfg(not(test))]
const DEFAULT_DB_PATH: &str = "db/kingfisher.sqlite";

static DB: LazyLock<Mutex<Connection>> =
    LazyLock::new(|| Mutex::new(open_db().expect("Failed to open the SQLite database")));

/// Opens and migrates the database now, so a bad path or schema fails at startup.
pub fn setup_db() {
    LazyLock::force(&DB);
}

fn open_db() -> Result<Connection> {
    #[cfg(not(test))]
    let mut conn = {
        let path = std::env::var_os("KINGFISHER_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DB_PATH));

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        tracing::info!(path = %path.display(), "opening SQLite database");
        Connection::open(path)?
    };

    #[cfg(test)]
    let mut conn = Connection::open_in_memory()?;

    conn.pragma_update(None, "foreign_keys", true)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    migrate(&mut conn)?;

    Ok(conn)
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    let version: usize = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;

    let Some(pending) = MIGRATIONS.get(version..) else {
        bail!(
            "database schema version {version} is newer than this build supports ({})",
            MIGRATIONS.len()
        );
    };

    for migration in pending {
        tx.execute_batch(migration)?;
    }
    tx.pragma_update(None, "user_version", MIGRATIONS.len())?;
    tx.commit()?;

    Ok(())
}

/// Runs `f` with exclusive access to the database on the blocking thread pool.
pub(crate) async fn with_db<T: Send + 'static>(
    f: impl FnOnce(&mut Connection) -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(move || f(&mut DB.lock())).await?
}

/// The global state of the bot
/// Arc because I can't be arsed.
pub type State = Arc<RawAppState>;

pub struct RawAppState {
    pub config: Arc<RwLock<Config>>,
    /// Config file watcher that refreshes the config if it changes
    ///
    /// Attached to the AppState to keep the watcher alive
    _watcher: notify::RecommendedWatcher,
    /// The path to the config file.
    /// This is to allow for saving / reloading the config.
    pub config_path: Box<Path>,
}

impl RawAppState {
    pub fn new(config: Config, config_path: PathBuf) -> Result<RawAppState> {
        let config = Arc::new(RwLock::new(config));

        use notify::{
            Event, EventKind, RecursiveMode, Watcher,
            event::{AccessKind, AccessMode, ModifyKind},
        };

        let config_clone = Arc::clone(&config);
        let reload_config_path = config_path.clone();
        let config_path: Box<Path> = config_path.into_boxed_path();

        // Close(Write) is what Linux inotify reports for a local write, but
        // Docker Desktop bind mounts (and macOS FSEvents) only surface data
        // modifications, so reload on those too. A partial write that fails to
        // parse is ignored by `reload`, and the next event picks up the rest.
        let mut watcher = notify::recommended_watcher(move |res| match res {
            Ok(Event {
                kind:
                    EventKind::Access(AccessKind::Close(AccessMode::Write))
                    | EventKind::Modify(ModifyKind::Data(_)),
                ..
            }) => {
                tracing::info!("config changed, reloading...");

                config_clone.blocking_write().reload(&*reload_config_path);
            }
            Err(e) => tracing::error!("watch error: {:?}", e),
            _ => {}
        })
        .expect("Failed to create file watcher");

        watcher
            .watch(&config_path, RecursiveMode::NonRecursive)
            .expect("Failed to watch config file");

        Ok(RawAppState {
            config,
            _watcher: watcher,
            config_path,
        })
    }
}

// User data, which is stored and accessible in all command invocations
pub type PoiseContext<'a> = poise::Context<'a, State, Error>;
