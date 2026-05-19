use astrbot_core::{AstrbotError, EventBus, Result};
use astrbot_platform::PlatformManager;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

type RuntimeTask = JoinHandle<Result<()>>;
const PLATFORM_STOP_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct RuntimeTaskSet {
    event_bus_task: RuntimeTask,
    platform_tasks: Vec<RuntimeTask>,
}

impl RuntimeTaskSet {
    pub(super) fn spawn(event_bus: EventBus, platform_manager: &PlatformManager) -> Self {
        let platform_tasks = platform_manager.spawn_all();
        let event_bus_task = tokio::spawn(async move { event_bus.run().await });

        Self {
            event_bus_task,
            platform_tasks,
        }
    }

    pub(super) async fn stop(self) -> Result<()> {
        let mut first_error = None;
        for task in self.platform_tasks {
            if let Err(err) = stop_platform_task(task, "platform", PLATFORM_STOP_TIMEOUT).await {
                remember_error(&mut first_error, err);
            }
        }
        if let Err(err) = abort_task(self.event_bus_task, "event bus").await {
            remember_error(&mut first_error, err);
        }
        if let Some(err) = first_error {
            Err(err)
        } else {
            Ok(())
        }
    }
}

async fn stop_platform_task(mut task: RuntimeTask, name: &str, timeout: Duration) -> Result<()> {
    if task.is_finished() {
        return join_task(task.await, name);
    }

    tokio::select! {
        joined = &mut task => join_task(joined, name),
        _ = sleep(timeout) => abort_task(task, name).await,
    }
}

async fn abort_task(task: RuntimeTask, name: &str) -> Result<()> {
    if !task.is_finished() {
        task.abort();
    }

    join_task(task.await, name)
}

fn join_task(
    joined: std::result::Result<Result<()>, tokio::task::JoinError>,
    name: &str,
) -> Result<()> {
    match joined {
        Ok(result) => result,
        Err(err) if err.is_cancelled() => Ok(()),
        Err(err) => Err(AstrbotError::Pipeline(format!(
            "{name} task join failed: {err}"
        ))),
    }
}

fn remember_error(first_error: &mut Option<AstrbotError>, err: AstrbotError) {
    if first_error.is_none() {
        *first_error = Some(err);
    }
}

#[cfg(test)]
mod tests {
    use astrbot_core::{AstrbotError, Result};
    use tokio::time::Duration;

    use super::{RuntimeTask, abort_task, stop_platform_task};

    #[tokio::test]
    async fn stop_task_accepts_cancelled_task() {
        let task: RuntimeTask = tokio::spawn(async { std::future::pending::<Result<()>>().await });

        abort_task(task, "pending")
            .await
            .expect("cancelled task should stop cleanly");
    }

    #[tokio::test]
    async fn stop_task_propagates_task_result_error() {
        let task: RuntimeTask =
            tokio::spawn(async { Err(AstrbotError::Pipeline("inner failure".to_string())) });
        while !task.is_finished() {
            tokio::task::yield_now().await;
        }

        let err = stop_platform_task(task, "failed", Duration::from_millis(50))
            .await
            .expect_err("task error should be propagated");
        assert!(err.to_string().contains("inner failure"));
    }

    #[tokio::test]
    async fn stop_task_maps_join_failure() {
        let task: RuntimeTask = tokio::spawn(async {
            #[allow(unreachable_code)]
            {
                panic!("boom");
                Ok(())
            }
        });
        while !task.is_finished() {
            tokio::task::yield_now().await;
        }

        let err = stop_platform_task(task, "panic", Duration::from_millis(50))
            .await
            .expect_err("join failure should be mapped");
        assert!(err.to_string().contains("panic task join failed"));
    }

    #[tokio::test]
    async fn platform_stop_waits_for_graceful_exit_before_abort() {
        let task: RuntimeTask = tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok(())
        });

        stop_platform_task(task, "graceful", Duration::from_millis(100))
            .await
            .expect("graceful task should complete");
    }
}
