use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Utc};
use log::{debug, error, info};
use rusqlite::{params, Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri_specta::Event;

/// Database migrations for transcription history.
/// Each migration is applied in order. The library tracks which migrations
/// have been applied using SQLite's user_version pragma.
///
/// Note: For users upgrading from tauri-plugin-sql, migrate_from_tauri_plugin_sql()
/// converts the old _sqlx_migrations table tracking to the user_version pragma,
/// ensuring migrations don't re-run on existing databases.
static MIGRATIONS: &[M] = &[
    M::up(
        "CREATE TABLE IF NOT EXISTS transcription_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_name TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            saved BOOLEAN NOT NULL DEFAULT 0,
            title TEXT NOT NULL,
            transcription_text TEXT NOT NULL
        );",
    ),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_processed_text TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_prompt TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_requested BOOLEAN NOT NULL DEFAULT 0;"),
];

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct PaginatedHistory {
    pub entries: Vec<HistoryEntry>,
    pub has_more: bool,
}

/// One day's worth of dictation activity, used to render the usage heatmap.
/// `date` is a local-calendar day formatted as `YYYY-MM-DD`.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct DayActivity {
    pub date: String,
    pub words: u32,
    pub recordings: u32,
}

/// Aggregated, privacy-preserving usage statistics computed entirely from the
/// local transcription history database. Nothing here leaves the machine.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct UsageInsights {
    pub total_words: u64,
    pub total_recordings: u64,
    pub words_this_month: u64,
    pub words_prev_month: u64,
    /// Percentage change in words vs the previous calendar month. `None` when
    /// the previous month had no dictation (so a percentage is undefined).
    pub month_change_pct: Option<f64>,
    pub words_this_week: u64,
    pub avg_words_per_recording: u32,
    pub current_streak_days: u32,
    pub longest_streak_days: u32,
    pub active_days: u32,
    pub first_recording_ts: Option<i64>,
    /// Per-day activity for the trailing heatmap window, oldest day first.
    pub daily_activity: Vec<DayActivity>,
    pub dictionary_words: u32,
    pub learned_corrections: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(tag = "action")]
pub enum HistoryUpdatePayload {
    #[serde(rename = "added")]
    Added { entry: HistoryEntry },
    #[serde(rename = "updated")]
    Updated { entry: HistoryEntry },
    #[serde(rename = "deleted")]
    Deleted { id: i64 },
    #[serde(rename = "toggled")]
    Toggled { id: i64 },
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct HistoryEntry {
    pub id: i64,
    pub file_name: String,
    pub timestamp: i64,
    pub saved: bool,
    pub title: String,
    pub transcription_text: String,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
    pub post_process_requested: bool,
}

pub struct HistoryManager {
    app_handle: AppHandle,
    recordings_dir: PathBuf,
    db_path: PathBuf,
}

impl HistoryManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        // Create recordings directory in app data dir
        let app_data_dir = crate::portable::app_data_dir(app_handle)?;
        let recordings_dir = app_data_dir.join("recordings");
        let db_path = app_data_dir.join("history.db");

        // Ensure recordings directory exists
        if !recordings_dir.exists() {
            fs::create_dir_all(&recordings_dir)?;
            debug!("Created recordings directory: {:?}", recordings_dir);
        }

        let manager = Self {
            app_handle: app_handle.clone(),
            recordings_dir,
            db_path,
        };

        // Initialize database and run migrations synchronously
        manager.init_database()?;

        Ok(manager)
    }

    fn init_database(&self) -> Result<()> {
        info!("Initializing database at {:?}", self.db_path);

        let mut conn = Connection::open(&self.db_path)?;

        // Handle migration from tauri-plugin-sql to rusqlite_migration
        // tauri-plugin-sql used _sqlx_migrations table, rusqlite_migration uses user_version pragma
        self.migrate_from_tauri_plugin_sql(&conn)?;

        // Create migrations object and run to latest version
        let migrations = Migrations::new(MIGRATIONS.to_vec());

        // Validate migrations in debug builds
        #[cfg(debug_assertions)]
        migrations.validate().expect("Invalid migrations");

        // Get current version before migration
        let version_before: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        debug!("Database version before migration: {}", version_before);

        // Apply any pending migrations
        migrations.to_latest(&mut conn)?;

        // Get version after migration
        let version_after: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

        if version_after > version_before {
            info!(
                "Database migrated from version {} to {}",
                version_before, version_after
            );
        } else {
            debug!("Database already at latest version {}", version_after);
        }

        Ok(())
    }

    /// Migrate from tauri-plugin-sql's migration tracking to rusqlite_migration's.
    /// tauri-plugin-sql used a _sqlx_migrations table, while rusqlite_migration uses
    /// SQLite's user_version pragma. This function checks if the old system was in use
    /// and sets the user_version accordingly so migrations don't re-run.
    fn migrate_from_tauri_plugin_sql(&self, conn: &Connection) -> Result<()> {
        // Check if the old _sqlx_migrations table exists
        let has_sqlx_migrations: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_sqlx_migrations {
            return Ok(());
        }

        // Check current user_version
        let current_version: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

        if current_version > 0 {
            // Already migrated to rusqlite_migration system
            return Ok(());
        }

        // Get the highest version from the old migrations table
        let old_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if old_version > 0 {
            info!(
                "Migrating from tauri-plugin-sql (version {}) to rusqlite_migration",
                old_version
            );

            // Set user_version to match the old migration state
            conn.pragma_update(None, "user_version", old_version)?;

            // Optionally drop the old migrations table (keeping it doesn't hurt)
            // conn.execute("DROP TABLE IF EXISTS _sqlx_migrations", [])?;

            info!(
                "Migration tracking converted: user_version set to {}",
                old_version
            );
        }

        Ok(())
    }

    fn get_connection(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    fn map_history_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryEntry> {
        Ok(HistoryEntry {
            id: row.get("id")?,
            file_name: row.get("file_name")?,
            timestamp: row.get("timestamp")?,
            saved: row.get("saved")?,
            title: row.get("title")?,
            transcription_text: row.get("transcription_text")?,
            post_processed_text: row.get("post_processed_text")?,
            post_process_prompt: row.get("post_process_prompt")?,
            post_process_requested: row.get("post_process_requested")?,
        })
    }

    pub fn recordings_dir(&self) -> &std::path::Path {
        &self.recordings_dir
    }

    /// Save a new history entry to the database.
    /// The WAV file should already have been written to the recordings directory.
    pub fn save_entry(
        &self,
        file_name: String,
        transcription_text: String,
        post_process_requested: bool,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,
    ) -> Result<HistoryEntry> {
        let timestamp = Utc::now().timestamp();
        let title = self.format_timestamp_title(timestamp);

        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &file_name,
                timestamp,
                false,
                &title,
                &transcription_text,
                &post_processed_text,
                &post_process_prompt,
                post_process_requested,
            ],
        )?;

        let entry = HistoryEntry {
            id: conn.last_insert_rowid(),
            file_name,
            timestamp,
            saved: false,
            title,
            transcription_text,
            post_processed_text,
            post_process_prompt,
            post_process_requested,
        };

        debug!("Saved history entry with id {}", entry.id);

        self.cleanup_old_entries()?;

        // Emit typed event for real-time frontend updates
        if let Err(e) = (HistoryUpdatePayload::Added {
            entry: entry.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(entry)
    }

    /// Update an existing history entry with new transcription results (used by retry).
    pub fn update_transcription(
        &self,
        id: i64,
        transcription_text: String,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,
    ) -> Result<HistoryEntry> {
        let conn = self.get_connection()?;
        let updated = conn.execute(
            "UPDATE transcription_history
             SET transcription_text = ?1,
                 post_processed_text = ?2,
                 post_process_prompt = ?3
             WHERE id = ?4",
            params![
                transcription_text,
                post_processed_text,
                post_process_prompt,
                id
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("History entry {} not found", id));
        }

        let entry = conn
            .query_row(
                "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested
                 FROM transcription_history WHERE id = ?1",
                params![id],
                Self::map_history_entry,
            )?;

        debug!("Updated transcription for history entry {}", id);

        if let Err(e) = (HistoryUpdatePayload::Updated {
            entry: entry.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(entry)
    }

    pub fn cleanup_old_entries(&self) -> Result<()> {
        let retention_period = crate::settings::get_recording_retention_period(&self.app_handle);

        match retention_period {
            crate::settings::RecordingRetentionPeriod::Never => {
                // Don't delete anything
                return Ok(());
            }
            crate::settings::RecordingRetentionPeriod::PreserveLimit => {
                // Use the old count-based logic with history_limit
                let limit = crate::settings::get_history_limit(&self.app_handle);
                return self.cleanup_by_count(limit);
            }
            _ => {
                // Use time-based logic
                return self.cleanup_by_time(retention_period);
            }
        }
    }

    fn delete_entries_and_files(&self, entries: &[(i64, String)]) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let conn = self.get_connection()?;
        let mut deleted_count = 0;

        for (id, file_name) in entries {
            // Delete database entry
            conn.execute(
                "DELETE FROM transcription_history WHERE id = ?1",
                params![id],
            )?;

            // Delete WAV file
            let file_path = self.recordings_dir.join(file_name);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    error!("Failed to delete WAV file {}: {}", file_name, e);
                } else {
                    debug!("Deleted old WAV file: {}", file_name);
                    deleted_count += 1;
                }
            }
        }

        Ok(deleted_count)
    }

    fn cleanup_by_count(&self, limit: usize) -> Result<()> {
        let conn = self.get_connection()?;

        // Get all entries that are not saved, ordered by timestamp desc
        let mut stmt = conn.prepare(
            "SELECT id, file_name FROM transcription_history WHERE saved = 0 ORDER BY timestamp DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>("id")?, row.get::<_, String>("file_name")?))
        })?;

        let mut entries: Vec<(i64, String)> = Vec::new();
        for row in rows {
            entries.push(row?);
        }

        if entries.len() > limit {
            let entries_to_delete = &entries[limit..];
            let deleted_count = self.delete_entries_and_files(entries_to_delete)?;

            if deleted_count > 0 {
                debug!("Cleaned up {} old history entries by count", deleted_count);
            }
        }

        Ok(())
    }

    fn cleanup_by_time(
        &self,
        retention_period: crate::settings::RecordingRetentionPeriod,
    ) -> Result<()> {
        let conn = self.get_connection()?;

        // Calculate cutoff timestamp (current time minus retention period)
        let now = Utc::now().timestamp();
        let cutoff_timestamp = match retention_period {
            crate::settings::RecordingRetentionPeriod::Days3 => now - (3 * 24 * 60 * 60), // 3 days in seconds
            crate::settings::RecordingRetentionPeriod::Weeks2 => now - (2 * 7 * 24 * 60 * 60), // 2 weeks in seconds
            crate::settings::RecordingRetentionPeriod::Months3 => now - (3 * 30 * 24 * 60 * 60), // 3 months in seconds (approximate)
            _ => unreachable!("Should not reach here"),
        };

        // Get all unsaved entries older than the cutoff timestamp
        let mut stmt = conn.prepare(
            "SELECT id, file_name FROM transcription_history WHERE saved = 0 AND timestamp < ?1",
        )?;

        let rows = stmt.query_map(params![cutoff_timestamp], |row| {
            Ok((row.get::<_, i64>("id")?, row.get::<_, String>("file_name")?))
        })?;

        let mut entries_to_delete: Vec<(i64, String)> = Vec::new();
        for row in rows {
            entries_to_delete.push(row?);
        }

        let deleted_count = self.delete_entries_and_files(&entries_to_delete)?;

        if deleted_count > 0 {
            debug!(
                "Cleaned up {} old history entries based on retention period",
                deleted_count
            );
        }

        Ok(())
    }

    pub async fn get_history_entries(
        &self,
        cursor: Option<i64>,
        limit: Option<usize>,
    ) -> Result<PaginatedHistory> {
        let conn = self.get_connection()?;
        let limit = limit.map(|l| l.min(100));

        let mut entries: Vec<HistoryEntry> = match (cursor, limit) {
            (Some(cursor_id), Some(lim)) => {
                let fetch_count = (lim + 1) as i64;
                let mut stmt = conn.prepare(
                    "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested
                     FROM transcription_history
                     WHERE id < ?1
                     ORDER BY id DESC
                     LIMIT ?2",
                )?;
                let result = stmt
                    .query_map(params![cursor_id, fetch_count], Self::map_history_entry)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            }
            (None, Some(lim)) => {
                let fetch_count = (lim + 1) as i64;
                let mut stmt = conn.prepare(
                    "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested
                     FROM transcription_history
                     ORDER BY id DESC
                     LIMIT ?1",
                )?;
                let result = stmt
                    .query_map(params![fetch_count], Self::map_history_entry)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            }
            (_, None) => {
                let mut stmt = conn.prepare(
                    "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested
                     FROM transcription_history
                     ORDER BY id DESC",
                )?;
                let result = stmt
                    .query_map([], Self::map_history_entry)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            }
        };

        let has_more = limit.is_some_and(|lim| entries.len() > lim);
        if has_more {
            entries.pop();
        }

        Ok(PaginatedHistory { entries, has_more })
    }

    #[cfg(test)]
    fn get_latest_entry_with_conn(conn: &Connection) -> Result<Option<HistoryEntry>> {
        let mut stmt = conn.prepare(
            "SELECT
                id,
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested
             FROM transcription_history
             ORDER BY timestamp DESC
             LIMIT 1",
        )?;

        let entry = stmt.query_row([], Self::map_history_entry).optional()?;
        Ok(entry)
    }

    /// Get the latest entry with non-empty transcription text.
    pub fn get_latest_completed_entry(&self) -> Result<Option<HistoryEntry>> {
        let conn = self.get_connection()?;
        Self::get_latest_completed_entry_with_conn(&conn)
    }

    fn get_latest_completed_entry_with_conn(conn: &Connection) -> Result<Option<HistoryEntry>> {
        let mut stmt = conn.prepare(
            "SELECT
                id,
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested
             FROM transcription_history
             WHERE transcription_text != ''
             ORDER BY timestamp DESC
             LIMIT 1",
        )?;

        let entry = stmt.query_row([], Self::map_history_entry).optional()?;
        Ok(entry)
    }

    pub async fn toggle_saved_status(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;

        // Get current saved status
        let current_saved: bool = conn.query_row(
            "SELECT saved FROM transcription_history WHERE id = ?1",
            params![id],
            |row| row.get("saved"),
        )?;

        let new_saved = !current_saved;

        conn.execute(
            "UPDATE transcription_history SET saved = ?1 WHERE id = ?2",
            params![new_saved, id],
        )?;

        debug!("Toggled saved status for entry {}: {}", id, new_saved);

        // Emit history updated event
        if let Err(e) = (HistoryUpdatePayload::Toggled { id }).emit(&self.app_handle) {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(())
    }

    pub fn get_audio_file_path(&self, file_name: &str) -> PathBuf {
        self.recordings_dir.join(file_name)
    }

    pub async fn get_entry_by_id(&self, id: i64) -> Result<Option<HistoryEntry>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                id,
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested
             FROM transcription_history
             WHERE id = ?1",
        )?;

        let entry = stmt.query_row([id], Self::map_history_entry).optional()?;

        Ok(entry)
    }

    pub async fn delete_entry(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;

        // Get the entry to find the file name
        if let Some(entry) = self.get_entry_by_id(id).await? {
            // Delete the audio file first
            let file_path = self.get_audio_file_path(&entry.file_name);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    error!("Failed to delete audio file {}: {}", entry.file_name, e);
                    // Continue with database deletion even if file deletion fails
                }
            }
        }

        // Delete from database
        conn.execute(
            "DELETE FROM transcription_history WHERE id = ?1",
            params![id],
        )?;

        debug!("Deleted history entry with id: {}", id);

        // Emit history updated event
        if let Err(e) = (HistoryUpdatePayload::Deleted { id }).emit(&self.app_handle) {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(())
    }

    /// Aggregate the entire local history into usage insights.
    ///
    /// This method performs only I/O — reading rows and settings — and defers
    /// all arithmetic to [`Self::aggregate_usage_insights`], a pure function
    /// that is unit-tested in isolation. We count words in Rust rather than SQL
    /// because correct word counting needs Unicode-aware whitespace splitting
    /// that SQLite cannot do, and the row count is bounded by retention so a
    /// single pass is cheap.
    pub fn compute_usage_insights(&self) -> Result<UsageInsights> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT timestamp, transcription_text, post_processed_text
             FROM transcription_history",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let settings = crate::settings::get_settings(&self.app_handle);

        Ok(Self::aggregate_usage_insights(
            rows,
            Local::now().date_naive(),
            settings.custom_words.len() as u32,
            settings.learned_corrections.len() as u32,
        ))
    }

    /// Pure aggregation of raw history rows into [`UsageInsights`]. Free of I/O
    /// and wall-clock access (`today` is injected by the caller), so it can be
    /// tested deterministically regardless of the machine's timezone.
    fn aggregate_usage_insights(
        rows: Vec<(i64, String, Option<String>)>,
        today: NaiveDate,
        dictionary_words: u32,
        learned_corrections: u32,
    ) -> UsageInsights {
        use std::collections::{HashMap, HashSet};

        let cur_year = today.year();
        let cur_month = today.month();
        let (prev_year, prev_month) = if cur_month == 1 {
            (cur_year - 1, 12)
        } else {
            (cur_year, cur_month - 1)
        };
        // "This week" is a rolling 7-day window ending today (inclusive).
        let week_start = today - Duration::days(6);

        let mut total_words: u64 = 0;
        let mut total_recordings: u64 = 0;
        let mut words_this_month: u64 = 0;
        let mut words_prev_month: u64 = 0;
        let mut words_this_week: u64 = 0;
        let mut first_ts: Option<i64> = None;
        let mut by_day: HashMap<NaiveDate, (u64, u32)> = HashMap::new();

        for (ts, text, processed) in rows {
            // Prefer the post-processed (cleaned) text when present, since that
            // is what actually got pasted; otherwise fall back to the raw text.
            let chosen = match processed {
                Some(p) if !p.trim().is_empty() => p,
                _ => text,
            };
            let words = chosen.split_whitespace().count() as u64;

            total_recordings += 1;
            total_words += words;
            first_ts = Some(match first_ts {
                Some(f) => f.min(ts),
                None => ts,
            });

            let date = match DateTime::from_timestamp(ts, 0) {
                Some(dt) => dt.with_timezone(&Local).date_naive(),
                None => continue,
            };

            let bucket = by_day.entry(date).or_insert((0, 0));
            bucket.0 += words;
            bucket.1 += 1;

            if date.year() == cur_year && date.month() == cur_month {
                words_this_month += words;
            } else if date.year() == prev_year && date.month() == prev_month {
                words_prev_month += words;
            }
            if date >= week_start && date <= today {
                words_this_week += words;
            }
        }

        let month_change_pct = if words_prev_month > 0 {
            Some(
                (words_this_month as f64 - words_prev_month as f64) / words_prev_month as f64
                    * 100.0,
            )
        } else {
            None
        };

        let avg_words_per_recording = if total_recordings > 0 {
            (total_words / total_recordings) as u32
        } else {
            0
        };

        // Heatmap window: a trailing 26 weeks (182 days), oldest day first so
        // the frontend can chunk it into week-columns left-to-right.
        const HEATMAP_DAYS: i64 = 182;
        let start_date = today - Duration::days(HEATMAP_DAYS - 1);
        let mut daily_activity = Vec::with_capacity(HEATMAP_DAYS as usize);
        let mut cursor = start_date;
        while cursor <= today {
            let (w, c) = by_day.get(&cursor).copied().unwrap_or((0, 0));
            daily_activity.push(DayActivity {
                date: cursor.format("%Y-%m-%d").to_string(),
                words: w as u32,
                recordings: c,
            });
            cursor += Duration::days(1);
        }

        // Longest streak across all of history: scan active days in order and
        // count the longest run of consecutive calendar days.
        let mut active_dates: Vec<NaiveDate> = by_day.keys().copied().collect();
        active_dates.sort_unstable();
        let active_days = active_dates.len() as u32;

        let mut longest_streak_days = 0u32;
        let mut run = 0u32;
        let mut prev: Option<NaiveDate> = None;
        for &day in &active_dates {
            run = match prev {
                Some(p) if day == p + Duration::days(1) => run + 1,
                _ => 1,
            };
            longest_streak_days = longest_streak_days.max(run);
            prev = Some(day);
        }

        // Current streak: consecutive active days ending today, allowing
        // yesterday as a one-day grace so the streak does not "reset" before
        // the user has had a chance to dictate today.
        let active_set: HashSet<NaiveDate> = active_dates.iter().copied().collect();
        let mut anchor = if active_set.contains(&today) {
            Some(today)
        } else if active_set.contains(&(today - Duration::days(1))) {
            Some(today - Duration::days(1))
        } else {
            None
        };
        let mut current_streak_days = 0u32;
        while let Some(day) = anchor {
            if active_set.contains(&day) {
                current_streak_days += 1;
                anchor = Some(day - Duration::days(1));
            } else {
                break;
            }
        }

        UsageInsights {
            total_words,
            total_recordings,
            words_this_month,
            words_prev_month,
            month_change_pct,
            words_this_week,
            avg_words_per_recording,
            current_streak_days,
            longest_streak_days,
            active_days,
            first_recording_ts: first_ts,
            daily_activity,
            dictionary_words,
            learned_corrections,
        }
    }

    fn format_timestamp_title(&self, timestamp: i64) -> String {
        if let Some(utc_datetime) = DateTime::from_timestamp(timestamp, 0) {
            // Convert UTC to local timezone
            let local_datetime = utc_datetime.with_timezone(&Local);
            local_datetime.format("%B %e, %Y - %l:%M%p").to_string()
        } else {
            format!("Recording {}", timestamp)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE transcription_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                saved BOOLEAN NOT NULL DEFAULT 0,
                title TEXT NOT NULL,
                transcription_text TEXT NOT NULL,
                post_processed_text TEXT,
                post_process_prompt TEXT,
                post_process_requested BOOLEAN NOT NULL DEFAULT 0
            );",
        )
        .expect("create transcription_history table");
        conn
    }

    fn insert_entry(conn: &Connection, timestamp: i64, text: &str, post_processed: Option<&str>) {
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                format!("mahflow-{}.wav", timestamp),
                timestamp,
                false,
                format!("Recording {}", timestamp),
                text,
                post_processed,
                Option::<String>::None,
                false,
            ],
        )
        .expect("insert history entry");
    }

    #[test]
    fn get_latest_entry_returns_none_when_empty() {
        let conn = setup_conn();
        let entry = HistoryManager::get_latest_entry_with_conn(&conn).expect("fetch latest entry");
        assert!(entry.is_none());
    }

    #[test]
    fn get_latest_entry_returns_newest_entry() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "first", None);
        insert_entry(&conn, 200, "second", Some("processed"));

        let entry = HistoryManager::get_latest_entry_with_conn(&conn)
            .expect("fetch latest entry")
            .expect("entry exists");

        assert_eq!(entry.timestamp, 200);
        assert_eq!(entry.transcription_text, "second");
        assert_eq!(entry.post_processed_text.as_deref(), Some("processed"));
    }

    #[test]
    fn get_latest_completed_entry_skips_empty_entries() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "completed", None);
        insert_entry(&conn, 200, "", None);

        let entry = HistoryManager::get_latest_completed_entry_with_conn(&conn)
            .expect("fetch latest completed entry")
            .expect("completed entry exists");

        assert_eq!(entry.timestamp, 100);
        assert_eq!(entry.transcription_text, "completed");
    }

    // --- Usage insights aggregation -------------------------------------

    fn day(date: &str) -> NaiveDate {
        NaiveDate::parse_from_str(date, "%Y-%m-%d").expect("valid date")
    }

    /// A UTC timestamp that lands on `date` at noon local time. Noon keeps us
    /// clear of DST transitions, so the row buckets to the expected local day
    /// no matter what timezone the test runs in.
    fn ts_for_local_date(date: NaiveDate) -> i64 {
        use chrono::TimeZone;
        Local
            .from_local_datetime(&date.and_hms_opt(12, 0, 0).unwrap())
            .single()
            .expect("unambiguous local datetime")
            .timestamp()
    }

    fn row(date: NaiveDate, text: &str, processed: Option<&str>) -> (i64, String, Option<String>) {
        (
            ts_for_local_date(date),
            text.to_string(),
            processed.map(|s| s.to_string()),
        )
    }

    #[test]
    fn insights_empty_history() {
        let insights = HistoryManager::aggregate_usage_insights(vec![], day("2026-06-26"), 0, 0);
        assert_eq!(insights.total_words, 0);
        assert_eq!(insights.total_recordings, 0);
        assert_eq!(insights.current_streak_days, 0);
        assert_eq!(insights.longest_streak_days, 0);
        assert_eq!(insights.active_days, 0);
        assert_eq!(insights.month_change_pct, None);
        assert_eq!(insights.daily_activity.len(), 182);
        assert!(insights.first_recording_ts.is_none());
    }

    #[test]
    fn insights_counts_words_and_prefers_processed_text() {
        let today = day("2026-06-26");
        let rows = vec![
            row(today, "hello world", None),                // 2 words (raw)
            row(today, "raw text here", Some("two words")), // 2 words (prefers processed)
            row(today, "  ", Some("   ")),                  // blank processed -> falls back to raw -> 0
        ];
        let insights = HistoryManager::aggregate_usage_insights(rows, today, 3, 5);
        assert_eq!(insights.total_recordings, 3);
        assert_eq!(insights.total_words, 4);
        assert_eq!(insights.avg_words_per_recording, 1); // 4 / 3 == 1 (integer div)
        assert_eq!(insights.dictionary_words, 3);
        assert_eq!(insights.learned_corrections, 5);
    }

    #[test]
    fn insights_month_change_percentage() {
        let today = day("2026-06-26");
        let rows = vec![
            row(day("2026-05-10"), "one two three four", None), // 4 words, previous month
            row(day("2026-06-01"), "a b c d e f", None),        // 6 words, this month
        ];
        let insights = HistoryManager::aggregate_usage_insights(rows, today, 0, 0);
        assert_eq!(insights.words_prev_month, 4);
        assert_eq!(insights.words_this_month, 6);
        assert_eq!(insights.month_change_pct, Some(50.0)); // (6 - 4) / 4 * 100
    }

    #[test]
    fn insights_month_change_none_without_baseline() {
        let today = day("2026-06-26");
        let rows = vec![row(day("2026-06-02"), "only this month words here", None)];
        let insights = HistoryManager::aggregate_usage_insights(rows, today, 0, 0);
        assert_eq!(insights.words_prev_month, 0);
        assert_eq!(insights.month_change_pct, None);
    }

    #[test]
    fn insights_month_rollover_to_previous_year() {
        let today = day("2026-01-15");
        let rows = vec![
            row(day("2025-12-20"), "december words counted right here", None), // Dec 2025 = prev month
            row(day("2026-01-05"), "january one", None),
        ];
        let insights = HistoryManager::aggregate_usage_insights(rows, today, 0, 0);
        assert_eq!(insights.words_prev_month, 5);
        assert_eq!(insights.words_this_month, 2);
    }

    #[test]
    fn insights_streaks_and_week() {
        let today = day("2026-06-26");
        let rows = vec![
            // Current run ending today: 24, 25, 26.
            row(day("2026-06-26"), "w", None),
            row(day("2026-06-25"), "w", None),
            row(day("2026-06-24"), "w", None),
            // Earlier, longer run: 01, 02, 03, 04.
            row(day("2026-06-01"), "w", None),
            row(day("2026-06-02"), "w", None),
            row(day("2026-06-03"), "w", None),
            row(day("2026-06-04"), "w", None),
        ];
        let insights = HistoryManager::aggregate_usage_insights(rows, today, 0, 0);
        assert_eq!(insights.current_streak_days, 3);
        assert_eq!(insights.longest_streak_days, 4);
        assert_eq!(insights.active_days, 7);
        // Rolling 7-day window (Jun 20..26): only 24, 25, 26 are active.
        assert_eq!(insights.words_this_week, 3);
    }

    #[test]
    fn insights_current_streak_grace_for_yesterday() {
        let today = day("2026-06-26");
        let rows = vec![
            row(day("2026-06-25"), "w", None),
            row(day("2026-06-24"), "w", None),
        ];
        let insights = HistoryManager::aggregate_usage_insights(rows, today, 0, 0);
        // No activity today, but yesterday is active -> streak counts 25 and 24.
        assert_eq!(insights.current_streak_days, 2);
    }

    #[test]
    fn insights_current_streak_zero_when_stale() {
        let today = day("2026-06-26");
        let rows = vec![row(day("2026-06-20"), "w", None)];
        let insights = HistoryManager::aggregate_usage_insights(rows, today, 0, 0);
        assert_eq!(insights.current_streak_days, 0);
    }

    #[test]
    fn insights_heatmap_window_bounds() {
        let today = day("2026-06-26");
        let rows = vec![row(today, "one two three", None)];
        let insights = HistoryManager::aggregate_usage_insights(rows, today, 0, 0);

        assert_eq!(insights.daily_activity.len(), 182);

        let last = insights.daily_activity.last().unwrap();
        assert_eq!(last.date, "2026-06-26");
        assert_eq!(last.words, 3);
        assert_eq!(last.recordings, 1);

        // 182-day inclusive window => first day is today minus 181 days.
        assert_eq!(insights.daily_activity[0].date, "2025-12-27");
    }
}
