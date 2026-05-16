use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronJob {
    pub job_id: String,
    pub name: String,
    pub kind: CronJobKind,
    pub schedule: CronJobSchedule,
    pub payload: Value,
    pub description: Option<String>,
    pub enabled: bool,
    pub persistent: bool,
    pub status: CronJobStatus,
    pub last_error: Option<String>,
}

impl CronJob {
    pub fn basic(
        job_id: impl Into<String>,
        name: impl Into<String>,
        schedule: CronJobSchedule,
    ) -> Self {
        Self::new(job_id, name, CronJobKind::Basic, schedule)
    }

    pub fn active_agent(
        job_id: impl Into<String>,
        name: impl Into<String>,
        schedule: CronJobSchedule,
        payload: ActiveAgentCronPayload,
    ) -> Self {
        let mut job = Self::new(job_id, name, CronJobKind::ActiveAgent, schedule);
        job.payload = serde_json::to_value(payload).unwrap_or_else(|_| json!({}));
        job.persistent = true;
        job
    }

    pub fn new(
        job_id: impl Into<String>,
        name: impl Into<String>,
        kind: CronJobKind,
        schedule: CronJobSchedule,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            name: name.into(),
            kind,
            schedule,
            payload: json!({}),
            description: None,
            enabled: true,
            persistent: false,
            status: CronJobStatus::Scheduled,
            last_error: None,
        }
    }

    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.trim().is_empty()).then_some(description);
        self
    }

    pub fn persistent(mut self, persistent: bool) -> Self {
        self.persistent = persistent;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn mark_running(&mut self) {
        self.status = CronJobStatus::Running;
        self.last_error = None;
    }

    pub fn mark_completed(&mut self) {
        self.status = CronJobStatus::Completed;
        self.last_error = None;
    }

    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = CronJobStatus::Failed;
        self.last_error = Some(error.into());
    }

    pub fn active_agent_payload(&self) -> Option<ActiveAgentCronPayload> {
        (self.kind == CronJobKind::ActiveAgent)
            .then(|| serde_json::from_value(self.payload.clone()).ok())
            .flatten()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CronJobKind {
    Basic,
    ActiveAgent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronJobSchedule {
    pub spec: CronScheduleSpec,
    pub timezone: Option<String>,
}

impl CronJobSchedule {
    pub fn cron(expression: impl Into<String>) -> Self {
        Self {
            spec: CronScheduleSpec::Cron {
                expression: expression.into(),
            },
            timezone: None,
        }
    }

    pub fn run_once_at(run_at: impl Into<String>) -> Self {
        Self {
            spec: CronScheduleSpec::RunOnce {
                run_at: run_at.into(),
            },
            timezone: None,
        }
    }

    pub fn with_timezone(mut self, timezone: impl Into<String>) -> Self {
        let timezone = timezone.into();
        self.timezone = (!timezone.trim().is_empty()).then_some(timezone);
        self
    }

    pub fn cron_expression(&self) -> Option<&str> {
        match &self.spec {
            CronScheduleSpec::Cron { expression } => Some(expression),
            CronScheduleSpec::RunOnce { .. } => None,
        }
    }

    pub fn run_at(&self) -> Option<&str> {
        match &self.spec {
            CronScheduleSpec::Cron { .. } => None,
            CronScheduleSpec::RunOnce { run_at } => Some(run_at),
        }
    }

    pub fn is_run_once(&self) -> bool {
        matches!(self.spec, CronScheduleSpec::RunOnce { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CronScheduleSpec {
    Cron { expression: String },
    RunOnce { run_at: String },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CronJobStatus {
    #[default]
    Scheduled,
    Running,
    Completed,
    Failed,
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveAgentCronPayload {
    pub session: String,
    pub note: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

impl ActiveAgentCronPayload {
    pub fn new(session: impl Into<String>, note: impl Into<String>) -> Self {
        Self {
            session: session.into(),
            note: note.into(),
            sender_id: None,
            origin: None,
        }
    }

    pub fn with_sender_id(mut self, sender_id: impl Into<String>) -> Self {
        let sender_id = sender_id.into();
        self.sender_id = (!sender_id.trim().is_empty()).then_some(sender_id);
        self
    }

    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        let origin = origin.into();
        self.origin = (!origin.trim().is_empty()).then_some(origin);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{ActiveAgentCronPayload, CronJob, CronJobKind, CronJobSchedule};

    #[test]
    fn active_agent_job_round_trips_typed_payload() {
        let job = CronJob::active_agent(
            "job-1",
            "follow up",
            CronJobSchedule::run_once_at("2026-02-02T08:00:00+08:00"),
            ActiveAgentCronPayload::new("webchat:conv-1", "send summary")
                .with_sender_id("user-1")
                .with_origin("tool"),
        );

        assert_eq!(job.kind, CronJobKind::ActiveAgent);
        assert!(job.persistent);
        assert!(job.schedule.is_run_once());
        assert_eq!(
            job.active_agent_payload()
                .expect("payload should decode")
                .note,
            "send summary"
        );
    }

    #[test]
    fn cron_schedule_keeps_cron_expression_and_timezone_typed() {
        let schedule = CronJobSchedule::cron("0 8 * * mon-fri").with_timezone("Asia/Shanghai");

        assert_eq!(schedule.cron_expression(), Some("0 8 * * mon-fri"));
        assert_eq!(schedule.timezone.as_deref(), Some("Asia/Shanghai"));
        assert!(!schedule.is_run_once());
    }
}
