use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::SystemTime;

use astrbot_core::{AstrbotError, Result};
use astrbot_storage::SqliteJsonStore;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{
    CronJob, CronJobKind, CronJobStatus, CronScheduleSpec, ProactiveAgentWakeRequest,
    ProactiveAgentWakeService,
};

type HandlerFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

pub type BasicCronHandler = Arc<dyn Fn(CronJob) -> HandlerFuture + Send + Sync>;

#[async_trait]
pub trait CronJobRepository: Send + Sync {
    async fn upsert_job(&self, job: CronJob) -> Result<()>;

    async fn job(&self, job_id: &str) -> Result<Option<CronJob>>;

    async fn delete_job(&self, job_id: &str) -> Result<bool>;

    async fn list_jobs(&self, kind: Option<CronJobKind>) -> Result<Vec<CronJob>>;
}

#[derive(Default)]
pub struct InMemoryCronJobRepository {
    jobs: RwLock<HashMap<String, CronJob>>,
}

#[derive(Clone, Debug)]
pub struct SqliteCronJobRepository {
    store: SqliteJsonStore,
}

impl InMemoryCronJobRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SqliteCronJobRepository {
    pub fn new(store: SqliteJsonStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl CronJobRepository for InMemoryCronJobRepository {
    async fn upsert_job(&self, job: CronJob) -> Result<()> {
        self.jobs
            .write()
            .map_err(lock_error)?
            .insert(job.job_id.clone(), job);
        Ok(())
    }

    async fn job(&self, job_id: &str) -> Result<Option<CronJob>> {
        Ok(self.jobs.read().map_err(lock_error)?.get(job_id).cloned())
    }

    async fn delete_job(&self, job_id: &str) -> Result<bool> {
        Ok(self
            .jobs
            .write()
            .map_err(lock_error)?
            .remove(job_id)
            .is_some())
    }

    async fn list_jobs(&self, kind: Option<CronJobKind>) -> Result<Vec<CronJob>> {
        let mut jobs = self
            .jobs
            .read()
            .map_err(lock_error)?
            .values()
            .filter(|job| kind.is_none_or(|kind| job.kind == kind))
            .cloned()
            .collect::<Vec<_>>();
        jobs.sort_by(|left, right| left.job_id.cmp(&right.job_id));
        Ok(jobs)
    }
}

#[async_trait]
impl CronJobRepository for SqliteCronJobRepository {
    async fn upsert_job(&self, job: CronJob) -> Result<()> {
        self.store.put_json("cron_jobs", &job.job_id, &job)
    }

    async fn job(&self, job_id: &str) -> Result<Option<CronJob>> {
        self.store.get_json("cron_jobs", job_id)
    }

    async fn delete_job(&self, job_id: &str) -> Result<bool> {
        self.store.delete_json("cron_jobs", job_id)
    }

    async fn list_jobs(&self, kind: Option<CronJobKind>) -> Result<Vec<CronJob>> {
        let mut jobs = self
            .store
            .list_json::<CronJob>("cron_jobs")?
            .into_iter()
            .filter(|job| kind.is_none_or(|kind| job.kind == kind))
            .collect::<Vec<_>>();
        jobs.sort_by(|left, right| left.job_id.cmp(&right.job_id));
        Ok(jobs)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchedulerJobSnapshot {
    pub job_id: String,
    pub schedule_key: String,
    pub enabled: bool,
}

impl SchedulerJobSnapshot {
    pub fn from_job(job: &CronJob) -> Self {
        let schedule_key = job
            .schedule
            .cron_expression()
            .or_else(|| job.schedule.run_at())
            .unwrap_or_default()
            .to_string();
        Self {
            job_id: job.job_id.clone(),
            schedule_key,
            enabled: job.enabled,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronTickReport {
    pub checked_count: usize,
    pub due_count: usize,
    pub ran_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub ran_job_ids: Vec<String>,
    pub failed_job_ids: Vec<String>,
}

impl CronTickReport {
    fn mark_due(&mut self) {
        self.due_count += 1;
    }

    fn mark_ran(&mut self, job_id: impl Into<String>) {
        self.ran_count += 1;
        self.ran_job_ids.push(job_id.into());
    }

    fn mark_failed(&mut self, job_id: impl Into<String>) {
        self.failed_count += 1;
        self.failed_job_ids.push(job_id.into());
    }

    fn mark_skipped(&mut self) {
        self.skipped_count += 1;
    }
}

pub trait CronScheduleDriver: Send + Sync {
    fn start(&self) -> Result<()>;

    fn shutdown(&self) -> Result<()>;

    fn schedule(&self, job: &CronJob) -> Result<()>;

    fn remove(&self, job_id: &str) -> Result<()>;

    fn scheduled_jobs(&self) -> Vec<SchedulerJobSnapshot>;
}

#[derive(Default)]
pub struct DueCronScheduleDriver {
    started: RwLock<bool>,
    scheduled: RwLock<HashMap<String, SchedulerJobSnapshot>>,
}

impl DueCronScheduleDriver {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CronScheduleDriver for DueCronScheduleDriver {
    fn start(&self) -> Result<()> {
        *self.started.write().map_err(lock_error)? = true;
        Ok(())
    }

    fn shutdown(&self) -> Result<()> {
        *self.started.write().map_err(lock_error)? = false;
        self.scheduled.write().map_err(lock_error)?.clear();
        Ok(())
    }

    fn schedule(&self, job: &CronJob) -> Result<()> {
        if !*self.started.read().map_err(lock_error)? {
            return Err(AstrbotError::Pipeline(
                "cron schedule driver is not started".to_string(),
            ));
        }
        self.scheduled
            .write()
            .map_err(lock_error)?
            .insert(job.job_id.clone(), SchedulerJobSnapshot::from_job(job));
        Ok(())
    }

    fn remove(&self, job_id: &str) -> Result<()> {
        self.scheduled.write().map_err(lock_error)?.remove(job_id);
        Ok(())
    }

    fn scheduled_jobs(&self) -> Vec<SchedulerJobSnapshot> {
        let mut jobs = self
            .scheduled
            .read()
            .expect("scheduled jobs lock")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        jobs.sort_by(|left, right| left.job_id.cmp(&right.job_id));
        jobs
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CronSchedulerState {
    #[default]
    Stopped,
    Running,
}

pub struct CronScheduler {
    repository: Arc<dyn CronJobRepository>,
    driver: Arc<dyn CronScheduleDriver>,
    proactive_wake: Arc<ProactiveAgentWakeService>,
    basic_handlers: RwLock<HashMap<String, BasicCronHandler>>,
    state: RwLock<CronSchedulerState>,
}

impl CronScheduler {
    pub fn new(
        repository: Arc<dyn CronJobRepository>,
        driver: Arc<dyn CronScheduleDriver>,
        proactive_wake: Arc<ProactiveAgentWakeService>,
    ) -> Self {
        Self {
            repository,
            driver,
            proactive_wake,
            basic_handlers: RwLock::new(HashMap::new()),
            state: RwLock::new(CronSchedulerState::Stopped),
        }
    }

    pub fn state(&self) -> CronSchedulerState {
        *self.state.read().expect("cron scheduler state lock")
    }

    pub fn register_basic_handler(&self, job_id: impl Into<String>, handler: BasicCronHandler) {
        self.basic_handlers
            .write()
            .expect("basic handlers lock")
            .insert(job_id.into(), handler);
    }

    pub async fn start(&self) -> Result<()> {
        {
            let mut state = self.state.write().map_err(lock_error)?;
            if *state == CronSchedulerState::Running {
                return Ok(());
            }
            self.driver.start()?;
            *state = CronSchedulerState::Running;
        }
        self.sync_from_repository().await
    }

    pub async fn shutdown(&self) -> Result<()> {
        let mut state = self.state.write().map_err(lock_error)?;
        if *state == CronSchedulerState::Stopped {
            return Ok(());
        }
        self.driver.shutdown()?;
        *state = CronSchedulerState::Stopped;
        Ok(())
    }

    pub async fn add_job(&self, job: CronJob) -> Result<()> {
        self.repository.upsert_job(job.clone()).await?;
        if self.state() == CronSchedulerState::Running && job.enabled {
            self.driver.schedule(&job)?;
        }
        Ok(())
    }

    pub async fn delete_job(&self, job_id: &str) -> Result<bool> {
        self.driver.remove(job_id)?;
        self.basic_handlers
            .write()
            .map_err(lock_error)?
            .remove(job_id);
        self.repository.delete_job(job_id).await
    }

    pub async fn list_jobs(&self, kind: Option<CronJobKind>) -> Result<Vec<CronJob>> {
        self.repository.list_jobs(kind).await
    }

    pub async fn job(&self, job_id: &str) -> Result<Option<CronJob>> {
        self.repository.job(job_id).await
    }

    pub fn scheduled_jobs(&self) -> Vec<SchedulerJobSnapshot> {
        self.driver.scheduled_jobs()
    }

    pub async fn sync_from_repository(&self) -> Result<()> {
        let jobs = self.repository.list_jobs(None).await?;
        for job in jobs {
            if !job.enabled || !job.persistent {
                continue;
            }
            if job.kind == CronJobKind::Basic && !self.has_basic_handler(&job.job_id) {
                continue;
            }
            self.driver.schedule(&job)?;
        }
        Ok(())
    }

    pub async fn run_job(&self, job_id: &str) -> Result<()> {
        let Some(mut job) = self.repository.job(job_id).await? else {
            return Err(AstrbotError::Pipeline(format!(
                "cron job {job_id} not found"
            )));
        };
        if !job.enabled {
            job.status = CronJobStatus::Disabled;
            self.repository.upsert_job(job).await?;
            return Ok(());
        }

        job.mark_running();
        self.repository.upsert_job(job.clone()).await?;
        let run_result = match job.kind {
            CronJobKind::Basic => self.run_basic_job(job.clone()).await,
            CronJobKind::ActiveAgent => self.run_active_agent_job(&job).await,
        };

        match run_result {
            Ok(()) => {
                job.mark_completed();
                self.repository.upsert_job(job.clone()).await?;
                if job.schedule.is_run_once() {
                    self.delete_job(job_id).await?;
                }
                Ok(())
            }
            Err(err) => {
                job.mark_failed(err.to_string());
                self.repository.upsert_job(job).await?;
                Err(err)
            }
        }
    }

    pub async fn tick_due(&self, now: SystemTime) -> Result<CronTickReport> {
        if self.state() != CronSchedulerState::Running {
            return Err(AstrbotError::Pipeline(
                "cron scheduler is not running".to_string(),
            ));
        }

        let jobs = self.repository.list_jobs(None).await?;
        let mut report = CronTickReport {
            checked_count: jobs.len(),
            ..CronTickReport::default()
        };
        for job in jobs {
            if !job.enabled {
                report.mark_skipped();
                continue;
            }
            if !run_once_due(&job, now) {
                continue;
            }
            report.mark_due();
            match self.run_job(&job.job_id).await {
                Ok(()) => report.mark_ran(job.job_id),
                Err(_) => report.mark_failed(job.job_id),
            }
        }

        Ok(report)
    }

    async fn run_basic_job(&self, job: CronJob) -> Result<()> {
        let handler = self
            .basic_handlers
            .read()
            .map_err(lock_error)?
            .get(&job.job_id)
            .cloned()
            .ok_or_else(|| {
                AstrbotError::Pipeline(format!(
                    "basic cron job handler not found for {}",
                    job.job_id
                ))
            })?;
        handler(job).await
    }

    async fn run_active_agent_job(&self, job: &CronJob) -> Result<()> {
        let request = ProactiveAgentWakeRequest::from_job(job)?;
        self.proactive_wake.wake(request).await
    }

    fn has_basic_handler(&self, job_id: &str) -> bool {
        self.basic_handlers
            .read()
            .map(|handlers| handlers.contains_key(job_id))
            .unwrap_or(false)
    }
}

fn run_once_due(job: &CronJob, now: SystemTime) -> bool {
    let CronScheduleSpec::RunOnce { run_at } = &job.schedule.spec else {
        return false;
    };
    parse_unix_or_rfc3339(run_at).is_some_and(|run_at| run_at <= now)
}

fn parse_unix_or_rfc3339(value: &str) -> Option<SystemTime> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(seconds));
    }

    parse_datetime_with_optional_offset(value)
}

fn parse_datetime_with_optional_offset(value: &str) -> Option<SystemTime> {
    if let Some(value) = value.strip_suffix('Z') {
        return system_time_from_timestamp(parse_simple_datetime_timestamp(value)?);
    }
    if let Some((datetime, offset_seconds)) = split_timezone_offset(value) {
        return system_time_from_timestamp(
            parse_simple_datetime_timestamp(datetime)? - offset_seconds,
        );
    }
    system_time_from_timestamp(parse_simple_datetime_timestamp(value)?)
}

fn split_timezone_offset(value: &str) -> Option<(&str, i64)> {
    let time_start = value.find('T')? + 1;
    let offset_index = value[time_start..]
        .rfind(|character| character == '+' || character == '-')
        .map(|index| time_start + index)?;
    let offset = &value[offset_index..];
    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let mut parts = offset[1..].split(':');
    let hours = parts.next()?.parse::<i64>().ok()?;
    let minutes = parts.next()?.parse::<i64>().ok()?;
    if parts.next().is_some() || hours > 23 || minutes > 59 {
        return None;
    }
    Some((
        &value[..offset_index],
        sign * (hours * 3_600 + minutes * 60),
    ))
}

fn parse_simple_datetime_timestamp(value: &str) -> Option<i64> {
    let (date, time) = value.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i32>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let time = time.split_once('.').map_or(time, |(time, _)| time);
    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u32>().ok()?;
    let minute = time_parts.next()?.parse::<u32>().ok()?;
    let second = time_parts
        .next()
        .map(|second| second.parse::<u32>().ok())
        .unwrap_or(Some(0))?;
    if time_parts.next().is_some() {
        return None;
    }

    unix_timestamp(year, month, day, hour, minute, second)
}

fn system_time_from_timestamp(timestamp: i64) -> Option<SystemTime> {
    (timestamp >= 0)
        .then(|| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64))
}

fn unix_timestamp(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Option<i64> {
    if !(1..=12).contains(&month) || day == 0 || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let month_days = days_in_month(year, month)?;
    if day > month_days {
        return None;
    }

    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second))
}

fn days_in_month(year: i32, month: u32) -> Option<u32> {
    Some(match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return None,
    })
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let mp = i32::try_from(month).expect("month should fit i32");
    let doy = (153 * (mp + if mp > 2 { -3 } else { 9 }) + 2) / 5
        + i32::try_from(day).expect("day should fit i32")
        - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    i64::from(era) * 146_097 + i64::from(doe) - 719_468
}

fn lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("cron scheduler lock: {err}"))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use astrbot_core::{MessageChain, MessageSession, MessageSink, MessageStream};
    use astrbot_storage::SqliteJsonStore;
    use async_trait::async_trait;

    use crate::{
        ActiveAgentCronPayload, CronJob, CronJobKind, CronJobRepository, CronJobSchedule,
        CronScheduleDriver, CronScheduler, CronSchedulerState, DueCronScheduleDriver,
        InMemoryCronJobRepository, ProactiveAgentWakeService, RecordingCronEventSink,
        SqliteCronJobRepository,
    };

    use super::{BasicCronHandler, Result};

    struct NoopMessageSink;

    #[async_trait]
    impl MessageSink for NoopMessageSink {
        async fn send(&self, _session: &MessageSession, _chain: MessageChain) -> Result<()> {
            Ok(())
        }

        async fn send_streaming(
            &self,
            _session: &MessageSession,
            _stream: MessageStream,
        ) -> Result<()> {
            Ok(())
        }
    }

    fn scheduler() -> (
        CronScheduler,
        Arc<DueCronScheduleDriver>,
        Arc<RecordingCronEventSink>,
    ) {
        let driver = Arc::new(DueCronScheduleDriver::new());
        let event_sink = Arc::new(RecordingCronEventSink::new());
        let wake = Arc::new(ProactiveAgentWakeService::new(
            event_sink.clone(),
            Arc::new(NoopMessageSink),
        ));
        (
            CronScheduler::new(
                Arc::new(InMemoryCronJobRepository::new()),
                driver.clone(),
                wake,
            ),
            driver,
            event_sink,
        )
    }

    #[tokio::test]
    async fn scheduler_starts_and_syncs_persistent_jobs_with_handlers() {
        let (scheduler, driver, _) = scheduler();
        let basic_job = CronJob::basic("basic-1", "cleanup", CronJobSchedule::cron("0 0 * * *"))
            .persistent(true);
        let active_job = CronJob::active_agent(
            "active-1",
            "wake",
            CronJobSchedule::cron("0 8 * * *"),
            ActiveAgentCronPayload::new("webchat:conversation-1", "hello"),
        );
        scheduler
            .add_job(basic_job)
            .await
            .expect("basic job should save");
        scheduler
            .add_job(active_job)
            .await
            .expect("active job should save");

        scheduler.start().await.expect("scheduler should start");

        assert_eq!(scheduler.state(), CronSchedulerState::Running);
        let scheduled = driver.scheduled_jobs();
        assert_eq!(scheduled.len(), 1);
        assert_eq!(scheduled[0].job_id, "active-1");
    }

    #[tokio::test]
    async fn scheduler_runs_basic_handler_outside_runtime_handle() {
        let (scheduler, _, _) = scheduler();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let handler_calls = calls.clone();
        let handler: BasicCronHandler = Arc::new(move |job| {
            let handler_calls = handler_calls.clone();
            Box::pin(async move {
                handler_calls
                    .lock()
                    .expect("handler calls lock")
                    .push(job.job_id);
                Ok(())
            })
        });
        scheduler.register_basic_handler("basic-1", handler);
        scheduler
            .add_job(CronJob::basic(
                "basic-1",
                "cleanup",
                CronJobSchedule::cron("0 0 * * *"),
            ))
            .await
            .expect("job should save");

        scheduler
            .run_job("basic-1")
            .await
            .expect("basic job should run");

        assert_eq!(
            calls.lock().expect("handler calls lock").as_slice(),
            ["basic-1"]
        );
        let jobs = scheduler
            .list_jobs(Some(CronJobKind::Basic))
            .await
            .expect("jobs should list");
        assert_eq!(jobs[0].status, crate::CronJobStatus::Completed);
    }

    #[tokio::test]
    async fn scheduler_runs_active_agent_job_through_event_sink() {
        let (scheduler, _, event_sink) = scheduler();
        scheduler
            .add_job(CronJob::active_agent(
                "active-1",
                "wake",
                CronJobSchedule::cron("0 8 * * *"),
                ActiveAgentCronPayload::new("webchat:conversation-1", "hello"),
            ))
            .await
            .expect("job should save");

        scheduler
            .run_job("active-1")
            .await
            .expect("active job should run");

        let events = event_sink.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "cron:active-1");
        assert!(events[0].is_wake());
    }

    #[tokio::test]
    async fn run_once_job_is_deleted_after_success() {
        let (scheduler, _, event_sink) = scheduler();
        scheduler
            .add_job(CronJob::active_agent(
                "active-1",
                "wake",
                CronJobSchedule::run_once_at("2026-02-02T08:00:00+08:00"),
                ActiveAgentCronPayload::new("webchat:conversation-1", "hello"),
            ))
            .await
            .expect("job should save");

        scheduler
            .run_job("active-1")
            .await
            .expect("active job should run");

        assert_eq!(event_sink.events().len(), 1);
        assert!(
            scheduler
                .list_jobs(None)
                .await
                .expect("jobs should list")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn tick_due_runs_due_run_once_jobs_and_leaves_future_jobs() {
        let (scheduler, _, event_sink) = scheduler();
        scheduler
            .add_job(CronJob::active_agent(
                "due-1",
                "due wake",
                CronJobSchedule::run_once_at("2026-05-17T00:00:00Z"),
                ActiveAgentCronPayload::new("webchat:conversation-1", "due"),
            ))
            .await
            .expect("due job should save");
        scheduler
            .add_job(CronJob::active_agent(
                "future-1",
                "future wake",
                CronJobSchedule::run_once_at("2026-05-18T00:00:00Z"),
                ActiveAgentCronPayload::new("webchat:conversation-1", "future"),
            ))
            .await
            .expect("future job should save");
        scheduler.start().await.expect("scheduler should start");

        let report = scheduler
            .tick_due(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_779_033_600),
            )
            .await
            .expect("tick should run");

        assert_eq!(report.checked_count, 2);
        assert_eq!(report.due_count, 1);
        assert_eq!(report.ran_job_ids, vec!["due-1"]);
        assert_eq!(event_sink.events().len(), 1);
        let jobs = scheduler.list_jobs(None).await.expect("jobs should list");
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, "future-1");
    }

    #[tokio::test]
    async fn tick_due_accepts_dashboard_local_and_offset_run_once_times() {
        let (scheduler, _, event_sink) = scheduler();
        scheduler
            .add_job(CronJob::active_agent(
                "local-1",
                "local wake",
                CronJobSchedule::run_once_at("2026-05-17T00:00"),
                ActiveAgentCronPayload::new("webchat:conversation-1", "local"),
            ))
            .await
            .expect("local job should save");
        scheduler
            .add_job(CronJob::active_agent(
                "offset-1",
                "offset wake",
                CronJobSchedule::run_once_at("2026-05-17T08:00:00+08:00"),
                ActiveAgentCronPayload::new("webchat:conversation-1", "offset"),
            ))
            .await
            .expect("offset job should save");
        scheduler.start().await.expect("scheduler should start");

        let report = scheduler
            .tick_due(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_779_033_600),
            )
            .await
            .expect("tick should run");

        assert_eq!(report.due_count, 2);
        assert_eq!(event_sink.events().len(), 2);
    }

    #[tokio::test]
    async fn tick_due_rejects_stopped_scheduler() {
        let (scheduler, _, _) = scheduler();

        let err = scheduler
            .tick_due(std::time::SystemTime::UNIX_EPOCH)
            .await
            .expect_err("stopped scheduler should reject tick");

        assert!(err.to_string().contains("not running"));
    }

    #[tokio::test]
    async fn sqlite_cron_repository_persists_jobs() {
        let store = SqliteJsonStore::open_in_memory().expect("sqlite store should open");
        let repository = SqliteCronJobRepository::new(store.clone());
        repository
            .upsert_job(CronJob::active_agent(
                "active-1",
                "wake",
                CronJobSchedule::cron("0 8 * * *"),
                ActiveAgentCronPayload::new("webchat:conversation-1", "hello"),
            ))
            .await
            .expect("job should store");

        let reloaded = SqliteCronJobRepository::new(store);
        assert_eq!(
            reloaded
                .job("active-1")
                .await
                .expect("job should load")
                .expect("job should exist")
                .name,
            "wake"
        );
    }
}
