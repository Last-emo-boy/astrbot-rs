use std::error::Error;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use astrbot_runtime::{AstrbotRuntime, RuntimeConfig, RuntimeConfigReloadAction, RuntimeHandle};
use astrbot_web::{
    MaintenanceRestartExecutor, MaintenanceRestartRequest, ManagementConfigApplyExecution,
    ManagementConfigApplyExecutionRequest, ManagementConfigApplyExecutor,
    ManagementConfigApplyFuture, ManagementConfigApplyState,
};
use tokio::sync::mpsc;

use crate::webchat_server::{
    PendingWebChatServer, WebChatServerHandle, prepare_webchat_server_with_config_apply,
};

pub(super) async fn run(config_path: PathBuf) -> Result<(), Box<dyn Error>> {
    let (config_apply_tx, mut config_apply_rx) = mpsc::unbounded_channel();
    let (restart_tx, mut restart_rx) = mpsc::unbounded_channel();
    let config_apply_executor = Arc::new(DashboardRuntimeConfigApplyExecutor {
        sender: config_apply_tx,
    });
    let restart_executor = Arc::new(DashboardRuntimeRestartExecutor { sender: restart_tx });
    let config = RuntimeConfig::from_json_file(&config_path)?;
    let webchat_server_config = config.webchat_server.clone();
    let runtime = AstrbotRuntime::initialize(config)?;
    let pending_webchat_server = prepare_webchat_server_with_config_apply(
        &runtime,
        &webchat_server_config,
        &config_path,
        Some(ManagementConfigApplyState::new(
            config_apply_executor.clone(),
        )),
        Some(restart_executor.clone()),
    )
    .await?;
    let mut handle = Some(runtime.start());
    let mut webchat_server = pending_webchat_server.map(PendingWebChatServer::start);

    println!("AstrBot runtime started. Press Ctrl+C to stop.");
    if let Some(server) = &webchat_server {
        println!(
            "WebChat HTTP server listening on http://{}",
            server.address()
        );
    }

    loop {
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result?;
                break;
            }
            Some(command) = config_apply_rx.recv() => {
                if command.action == RuntimeConfigReloadAction::Noop {
                    continue;
                }
                println!(
                    "Dashboard config apply requested {:?} for fields: {}",
                    command.action,
                    command.changed_fields.join(", ")
                );
                tokio::time::sleep(Duration::from_millis(250)).await;
                let Some(current_handle) = handle.take() else {
                    eprintln!("Dashboard config apply ignored because runtime handle is unavailable");
                    continue;
                };
                match rebuild_runtime_after_dashboard_request(
                    current_handle,
                    webchat_server.take(),
                    &config_path,
                    config_apply_executor.clone(),
                    restart_executor.clone(),
                )
                .await
                {
                    Ok((next_handle, next_server)) => {
                        handle = Some(next_handle);
                        webchat_server = next_server;
                        println!("Dashboard config apply completed with runtime rebuild.");
                    }
                    Err(error) => {
                        eprintln!("Dashboard config apply failed: {error}");
                        break;
                    }
                }
            }
            Some(command) = restart_rx.recv() => {
                println!(
                    "Dashboard runtime restart requested: {}",
                    command.reason.as_deref().unwrap_or("manual restart")
                );
                if command.delay_secs > 0 {
                    tokio::time::sleep(Duration::from_secs(command.delay_secs)).await;
                } else {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }
                let Some(current_handle) = handle.take() else {
                    eprintln!("Dashboard restart ignored because runtime handle is unavailable");
                    continue;
                };
                match rebuild_runtime_after_dashboard_request(
                    current_handle,
                    webchat_server.take(),
                    &config_path,
                    config_apply_executor.clone(),
                    restart_executor.clone(),
                )
                .await
                {
                    Ok((next_handle, next_server)) => {
                        handle = Some(next_handle);
                        webchat_server = next_server;
                        println!("Dashboard runtime restart completed.");
                    }
                    Err(error) => {
                        eprintln!("Dashboard runtime restart failed: {error}");
                        break;
                    }
                }
            }
        }
    }
    if let Some(server) = webchat_server {
        server.stop().await?;
    }
    if let Some(handle) = handle {
        handle.stop().await?;
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct RuntimeConfigApplyCommand {
    action: RuntimeConfigReloadAction,
    changed_fields: Vec<String>,
}

#[derive(Clone, Debug)]
struct DashboardRuntimeConfigApplyExecutor {
    sender: mpsc::UnboundedSender<RuntimeConfigApplyCommand>,
}

impl ManagementConfigApplyExecutor for DashboardRuntimeConfigApplyExecutor {
    fn apply_config_change<'a>(
        &'a self,
        request: ManagementConfigApplyExecutionRequest,
    ) -> ManagementConfigApplyFuture<'a> {
        Box::pin(async move {
            self.sender
                .send(RuntimeConfigApplyCommand {
                    action: request.plan.reload_action,
                    changed_fields: request.plan.changed_fields.clone(),
                })
                .map_err(|error| format!("queue runtime config apply: {error}"))?;
            Ok(ManagementConfigApplyExecution::accepted(
                request.plan.reload_action,
                format!("runtime config apply queued for {}", request.conf_id),
            ))
        })
    }
}

#[derive(Clone, Debug)]
struct RuntimeRestartCommand {
    reason: Option<String>,
    delay_secs: u64,
}

#[derive(Clone, Debug)]
struct DashboardRuntimeRestartExecutor {
    sender: mpsc::UnboundedSender<RuntimeRestartCommand>,
}

impl MaintenanceRestartExecutor for DashboardRuntimeRestartExecutor {
    fn restart(&self, request: &MaintenanceRestartRequest) -> Result<String, String> {
        self.sender
            .send(RuntimeRestartCommand {
                reason: request.reason.clone(),
                delay_secs: request.delay_secs,
            })
            .map_err(|error| format!("queue runtime restart: {error}"))?;
        Ok(format!(
            "runtime restart queued: {}",
            request.reason.as_deref().unwrap_or("manual restart")
        ))
    }
}

async fn rebuild_runtime_after_dashboard_request(
    handle: RuntimeHandle,
    webchat_server: Option<WebChatServerHandle>,
    config_path: &Path,
    config_apply_executor: Arc<DashboardRuntimeConfigApplyExecutor>,
    restart_executor: Arc<DashboardRuntimeRestartExecutor>,
) -> Result<(RuntimeHandle, Option<WebChatServerHandle>), Box<dyn Error>> {
    if let Some(server) = webchat_server {
        server.stop().await?;
    }
    handle.stop().await?;

    let config = RuntimeConfig::from_json_file(config_path)?;
    let webchat_server_config = config.webchat_server.clone();
    let runtime = AstrbotRuntime::initialize(config)?;
    let pending_webchat_server = prepare_webchat_server_with_config_apply(
        &runtime,
        &webchat_server_config,
        config_path,
        Some(ManagementConfigApplyState::new(config_apply_executor)),
        Some(restart_executor),
    )
    .await?;
    let handle = runtime.start();
    let webchat_server = pending_webchat_server.map(PendingWebChatServer::start);
    Ok((handle, webchat_server))
}

#[cfg(test)]
mod tests {
    use super::DashboardRuntimeRestartExecutor;
    use astrbot_web::{MaintenanceRestartExecutor, MaintenanceRestartRequest};
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn dashboard_restart_executor_queues_runtime_restart_command() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let executor = DashboardRuntimeRestartExecutor { sender };

        let message = executor
            .restart(&MaintenanceRestartRequest {
                reason: Some("dashboard update".to_string()),
                delay_secs: 2,
            })
            .expect("restart should queue");
        let command = receiver.recv().await.expect("command should be queued");

        assert_eq!(message, "runtime restart queued: dashboard update");
        assert_eq!(command.reason.as_deref(), Some("dashboard update"));
        assert_eq!(command.delay_secs, 2);
    }
}
