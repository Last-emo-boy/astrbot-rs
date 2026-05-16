use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use crate::{
    CronJob, CronJobKind, CronJobStatus, ProactiveAgentWakeRequest, ProactiveAgentWakeService,
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

impl InMemoryCronJobRepository {
    pub fn new() -> Self {
        Self::default()
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

#[derive(Clone, Debug, PartialEq, Eq)]
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

fn lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("cron scheduler lock: {err}"))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use astrbot_core::{MessageChain, MessageSession, MessageSink, MessageStream};
    use async_trait::async_trait;

    use crate::{
        ActiveAgentCronPayload, CronJob, CronJobKind, CronJobSchedule, CronScheduleDriver,
        CronScheduler, CronSchedulerState, DueCronScheduleDriver, InMemoryCronJobRepository,
        ProactiveAgentWakeService, RecordingCronEventSink,
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
}
