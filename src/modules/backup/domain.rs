use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BackupProvider {
    Local,
    S3,
}

impl BackupProvider {
    pub fn as_str(&self) -> &str {
        match self {
            BackupProvider::Local => "local",
            BackupProvider::S3 => "s3",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "local" => Some(BackupProvider::Local),
            "s3" => Some(BackupProvider::S3),
            _ => None,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BackupStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl BackupStatus {
    pub fn as_str(&self) -> &str {
        match self {
            BackupStatus::Pending => "pending",
            BackupStatus::Running => "running",
            BackupStatus::Completed => "completed",
            BackupStatus::Failed => "failed",
        }
    }

    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(BackupStatus::Pending),
            "running" => Some(BackupStatus::Running),
            "completed" => Some(BackupStatus::Completed),
            "failed" => Some(BackupStatus::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BackupScheduleFrequency {
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

impl BackupScheduleFrequency {
    pub fn as_str(&self) -> &str {
        match self {
            BackupScheduleFrequency::Hourly => "hourly",
            BackupScheduleFrequency::Daily => "daily",
            BackupScheduleFrequency::Weekly => "weekly",
            BackupScheduleFrequency::Monthly => "monthly",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "hourly" => Some(BackupScheduleFrequency::Hourly),
            "daily" => Some(BackupScheduleFrequency::Daily),
            "weekly" => Some(BackupScheduleFrequency::Weekly),
            "monthly" => Some(BackupScheduleFrequency::Monthly),
            _ => None,
        }
    }

    pub fn cron_expression(&self, hour: u32, minute: u32) -> String {
        match self {
            BackupScheduleFrequency::Hourly => format!("0 {} * * * * *", minute),
            BackupScheduleFrequency::Daily => format!("0 {} {} * * * *", minute, hour),
            BackupScheduleFrequency::Weekly => format!("0 {} {} * * 0 *", minute, hour),
            BackupScheduleFrequency::Monthly => format!("0 {} {} 1 * * *", minute, hour),
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Backup {
    pub id: String,
    pub created_at: String,
    pub size: i64,
    pub provider: String,
    pub status: String,
    pub manifest_hash: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BackupSchedule {
    pub id: String,
    pub enabled: bool,
    pub frequency: String,
    pub hour: i32,
    pub minute: i32,
    pub provider: String,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy)]
pub enum RestoreStep {
    Validate,
    Extract,
    Merge,
    Replace,
    ClearCache,
}

impl RestoreStep {
    pub fn as_str(&self) -> &str {
        match self {
            RestoreStep::Validate => "validate",
            RestoreStep::Extract => "extract",
            RestoreStep::Merge => "merge",
            RestoreStep::Replace => "replace",
            RestoreStep::ClearCache => "clear_cache",
        }
    }
}
