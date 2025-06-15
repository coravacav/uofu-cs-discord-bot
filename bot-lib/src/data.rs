use crate::{config::Config, llm};
use bot_db::KingFisherDb;
use color_eyre::eyre::{Error, Result};
use std::{path::Path, sync::Arc};
use tokio::sync::RwLock;

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
    pub llms: llm::LLMS,
    pub db: KingFisherDb,
}

impl RawAppState {
    pub fn new(config: Config, config_path: String) -> Result<RawAppState> {
        let config = Arc::new(RwLock::new(config));

        let llm_tx = llm::setup_llms()?;
        let db = KingFisherDb::new()?;

        use notify::{
            Event, EventKind, RecursiveMode, Watcher,
            event::{AccessKind, AccessMode},
        };

        let config_clone = Arc::clone(&config);
        let reload_config_path = config_path.clone();
        let config_path: Box<Path> = Path::new(&config_path).into();

        let mut watcher = notify::recommended_watcher(move |res| match res {
            Ok(Event {
                kind: EventKind::Access(AccessKind::Close(AccessMode::Write)),
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
            llms: llm_tx,
            db,
        })
    }
}

// User data, which is stored and accessible in all command invocations
pub type PoiseContext<'a> = poise::Context<'a, State, Error>;
