use std::{
    collections::HashMap,
    fmt::Debug,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bot_lib::config::Config;
use poise::serenity_prelude::{ChannelId, CreateMessage, EditMessage, Http, MessageId, UserId};
use tokio::{
    sync::{RwLock, mpsc},
    time::{Instant, MissedTickBehavior},
};
use tracing::{
    Event, Level, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{Layer, layer::Context};

const ERROR_QUEUE_CAPACITY: usize = 256;
const ALERT_WINDOW: Duration = Duration::from_secs(60 * 60);
const EDIT_DEBOUNCE: Duration = Duration::from_secs(5);
const SEND_RETRY_DELAY: Duration = Duration::from_secs(60);
const MAX_ALERT_CHARS: usize = 1_800;

pub struct ErrorNotificationLayer {
    sender: mpsc::Sender<ErrorEvent>,
}

pub struct ErrorNotificationWorker {
    receiver: mpsc::Receiver<ErrorEvent>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ErrorCallsite {
    target: String,
    file: Option<String>,
    line: Option<u32>,
}

#[derive(Clone, Debug)]
struct ErrorEvent {
    callsite: ErrorCallsite,
    details: String,
    occurred_at: u64,
}

#[derive(Clone, Copy)]
struct DiscordMessage {
    channel_id: ChannelId,
    message_id: MessageId,
}

struct Incident {
    recipient: UserId,
    window_started: Instant,
    first_at: u64,
    latest_at: u64,
    occurrences: u64,
    latest_details: String,
    discord_message: Option<DiscordMessage>,
    last_delivery_attempt: Instant,
    dirty: bool,
}

#[derive(Default)]
struct ErrorFieldVisitor {
    message: Option<String>,
    fields: Vec<String>,
}

pub fn new() -> (ErrorNotificationLayer, ErrorNotificationWorker) {
    let (sender, receiver) = mpsc::channel(ERROR_QUEUE_CAPACITY);

    (
        ErrorNotificationLayer { sender },
        ErrorNotificationWorker { receiver },
    )
}

impl<S> Layer<S> for ErrorNotificationLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let metadata = event.metadata();
        if *metadata.level() != Level::ERROR || !is_application_target(metadata.target()) {
            return;
        }

        let mut visitor = ErrorFieldVisitor::default();
        event.record(&mut visitor);

        let details = visitor.finish();
        let error_event = ErrorEvent {
            callsite: ErrorCallsite {
                target: metadata.target().to_owned(),
                file: metadata.file().map(str::to_owned),
                line: metadata.line(),
            },
            details,
            occurred_at: unix_timestamp(),
        };

        // Error reporting must never block the application that is reporting the error.
        let _ = self.sender.try_send(error_event);
    }
}

impl ErrorNotificationWorker {
    pub async fn run(mut self, http: Arc<Http>, config: Arc<RwLock<Config>>) {
        let mut incidents = HashMap::<ErrorCallsite, Incident>::new();
        let mut edit_interval = tokio::time::interval(EDIT_DEBOUNCE);
        edit_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                event = self.receiver.recv() => {
                    let Some(event) = event else {
                        break;
                    };

                    let recipient = config.read().await.ids.error_notification_user_id;
                    let Some(recipient) = recipient else {
                        incidents.clear();
                        continue;
                    };

                    record_event(&http, &mut incidents, recipient, event).await;
                }
                _ = edit_interval.tick() => {
                    flush_dirty_incidents(&http, &mut incidents).await;
                }
            }
        }
    }
}

async fn record_event(
    http: &Http,
    incidents: &mut HashMap<ErrorCallsite, Incident>,
    recipient: UserId,
    event: ErrorEvent,
) {
    let now = Instant::now();
    let is_current_window = incidents
        .get(&event.callsite)
        .is_some_and(|incident| incident.is_current_window(recipient, now));

    if is_current_window {
        let incident = incidents
            .get_mut(&event.callsite)
            .expect("incident was checked above");
        incident.record(event);
        return;
    }

    let mut incident = Incident {
        recipient,
        window_started: now,
        first_at: event.occurred_at,
        latest_at: event.occurred_at,
        occurrences: 1,
        latest_details: event.details,
        discord_message: None,
        last_delivery_attempt: now,
        dirty: true,
    };

    match send_alert(http, &event.callsite, &incident).await {
        Ok(discord_message) => {
            incident.discord_message = Some(discord_message);
            incident.dirty = false;
        }
        Err(error) => {
            tracing::warn!(
                ?error,
                recipient = %recipient,
                "Failed to send an error notification DM"
            );
        }
    }

    incidents.insert(event.callsite, incident);
}

async fn flush_dirty_incidents(http: &Http, incidents: &mut HashMap<ErrorCallsite, Incident>) {
    for (callsite, incident) in incidents.iter_mut().filter(|(_, incident)| incident.dirty) {
        let result = if let Some(discord_message) = incident.discord_message {
            edit_alert(http, discord_message, callsite, incident).await
        } else if incident.last_delivery_attempt.elapsed() >= SEND_RETRY_DELAY {
            incident.last_delivery_attempt = Instant::now();
            send_alert(http, callsite, incident).await.map(|message| {
                incident.discord_message = Some(message);
            })
        } else {
            continue;
        };

        match result {
            Ok(()) => incident.dirty = false,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    recipient = %incident.recipient,
                    "Failed to update an error notification DM"
                );
            }
        }
    }
}

async fn send_alert(
    http: &Http,
    callsite: &ErrorCallsite,
    incident: &Incident,
) -> poise::serenity_prelude::Result<DiscordMessage> {
    let user = incident.recipient.to_user(http).await?;
    let dm_channel = user.create_dm_channel(http).await?;
    let message = dm_channel
        .send_message(
            http,
            CreateMessage::new().content(format_alert(callsite, incident)),
        )
        .await?;

    Ok(DiscordMessage {
        channel_id: message.channel_id,
        message_id: message.id,
    })
}

async fn edit_alert(
    http: &Http,
    discord_message: DiscordMessage,
    callsite: &ErrorCallsite,
    incident: &Incident,
) -> poise::serenity_prelude::Result<()> {
    discord_message
        .channel_id
        .edit_message(
            http,
            discord_message.message_id,
            EditMessage::new().content(format_alert(callsite, incident)),
        )
        .await?;

    Ok(())
}

fn format_alert(callsite: &ErrorCallsite, incident: &Incident) -> String {
    let source = match (&callsite.file, callsite.line) {
        (Some(file), Some(line)) => format!("{file}:{line}"),
        (Some(file), None) => file.clone(),
        (None, _) => "unknown source".to_owned(),
    };
    let details = incident.latest_details.replace("```", "'''");
    let prefix = format!(
        "🚨 **KingFisher error**\n\
         **Target:** `{}`\n\
         **Source:** `{source}`\n\
         **First occurrence:** <t:{}:F>\n\
         **Latest occurrence:** <t:{}:R>\n\
         **Occurrences this hour:** {}\n\
         ```text\n",
        callsite.target, incident.first_at, incident.latest_at, incident.occurrences
    );
    let suffix = "\n```";
    let available = MAX_ALERT_CHARS.saturating_sub(prefix.chars().count() + suffix.chars().count());

    format!("{prefix}{}{suffix}", truncate_chars(&details, available))
}

fn truncate_chars(value: &str, max_chars: usize) -> &str {
    value
        .char_indices()
        .nth(max_chars)
        .map_or(value, |(index, _)| &value[..index])
}

fn is_application_target(target: &str) -> bool {
    target == "bot"
        || target.starts_with("bot::")
        || target == "bot_lib"
        || target.starts_with("bot_lib::")
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Incident {
    fn is_current_window(&self, recipient: UserId, now: Instant) -> bool {
        self.recipient == recipient && now.duration_since(self.window_started) < ALERT_WINDOW
    }

    fn record(&mut self, event: ErrorEvent) {
        self.latest_at = event.occurred_at;
        self.occurrences = self.occurrences.saturating_add(1);
        self.latest_details = event.details;
        self.dirty = true;
    }
}

impl ErrorFieldVisitor {
    fn record_value(&mut self, field: &Field, value: String) {
        if field.name() == "message" {
            self.message = Some(value);
        } else {
            self.fields.push(format!("{}={value}", field.name()));
        }
    }

    fn finish(self) -> String {
        let mut details = self
            .message
            .unwrap_or_else(|| "Application error".to_owned());
        if !self.fields.is_empty() {
            details.push('\n');
            details.push_str(&self.fields.join("\n"));
        }
        details
    }
}

impl Visit for ErrorFieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.record_value(field, format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_value(field, value.to_owned());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_value(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_value(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_value(field, value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_target_filter_excludes_dependencies() {
        assert!(is_application_target("bot"));
        assert!(is_application_target("bot::module"));
        assert!(is_application_target("bot_lib::starboard"));
        assert!(!is_application_target("serenity::gateway"));
    }

    #[test]
    fn alert_format_is_bounded_and_escapes_code_fences() {
        let callsite = ErrorCallsite {
            target: "bot_lib::starboard".to_owned(),
            file: Some("bot-lib/src/starboard.rs".to_owned()),
            line: Some(42),
        };
        let incident = Incident {
            recipient: UserId::new(1),
            window_started: Instant::now(),
            first_at: 1,
            latest_at: 2,
            occurrences: 3,
            latest_details: format!("```{}", "x".repeat(3_000)),
            discord_message: None,
            last_delivery_attempt: Instant::now(),
            dirty: true,
        };

        let alert = format_alert(&callsite, &incident);
        assert!(alert.chars().count() <= MAX_ALERT_CHARS);
        assert_eq!(alert.matches("```").count(), 2);
        assert!(alert.contains("Occurrences this hour:** 3"));
    }

    #[test]
    fn incident_groups_same_recipient_and_callsite_for_one_hour() {
        let callsite = ErrorCallsite {
            target: "bot_lib::starboard".to_owned(),
            file: Some("bot-lib/src/starboard.rs".to_owned()),
            line: Some(42),
        };
        let started = Instant::now();
        let mut incident = Incident {
            recipient: UserId::new(1),
            window_started: started,
            first_at: 1,
            latest_at: 1,
            occurrences: 1,
            latest_details: "first".to_owned(),
            discord_message: None,
            last_delivery_attempt: started,
            dirty: false,
        };

        assert!(incident.is_current_window(UserId::new(1), started + Duration::from_secs(3_599)));
        assert!(!incident.is_current_window(UserId::new(1), started + ALERT_WINDOW));
        assert!(!incident.is_current_window(UserId::new(2), started));

        incident.record(ErrorEvent {
            callsite,
            details: "latest".to_owned(),
            occurred_at: 2,
        });

        assert_eq!(incident.occurrences, 2);
        assert_eq!(incident.latest_at, 2);
        assert_eq!(incident.latest_details, "latest");
        assert!(incident.dirty);
    }
}
