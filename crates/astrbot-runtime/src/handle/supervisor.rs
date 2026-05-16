use astrbot_core::{AstrbotError, EventBus, Result};
use astrbot_platform::PlatformManager;
use tokio::task::JoinHandle;

type RuntimeTask = JoinHandle<Result<()>>;

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
        for task in self.platform_tasks {
            stop_task(task, "platform").await?;
        }
        stop_task(self.event_bus_task, "event bus").await
    }
}

async fn stop_task(task: RuntimeTask, name: &str) -> Result<()> {
    if !task.is_finished() {
        task.abort();
    }

    match task.await {
        Ok(result) => result,
        Err(err) if err.is_cancelled() => Ok(()),
        Err(err) => Err(AstrbotError::Pipeline(format!(
            "{name} task join failed: {err}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use astrbot_core::{AstrbotError, Result};

    use super::{RuntimeTask, stop_task};

    #[tokio::test]
    async fn stop_task_accepts_cancelled_task() {
        let task: RuntimeTask = tokio::spawn(async { std::future::pending::<Result<()>>().await });

        stop_task(task, "pending")
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

        let err = stop_task(task, "failed")
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

        let err = stop_task(task, "panic")
            .await
            .expect_err("join failure should be mapped");
        assert!(err.to_string().contains("panic task join failed"));
    }
}
