import { api } from "../api.js";
import { $, showToast } from "../dom.js";
import { loadBackup } from "../loaders.js";
import { state } from "../state.js";

export async function handleMaintenanceActions({ action, target }) {
    if (action === "project-plan" || action === "dashboard-plan") {
      const path = action === "project-plan" ? "/api/management/update/project-plan" : "/api/management/update/dashboard-plan";
      state.operation = await api(path, {
        method: "POST",
        body: JSON.stringify({
          version: $("#update-version").value.trim(),
          latest: false,
          proxy: $("#update-proxy").value.trim() || null,
          reboot: true,
        }),
      });
      showToast("更新计划已生成");
    }
    if (action === "operation-run") {
      state.operation = await api("/api/management/update/operations/run", {
        method: "POST",
        body: JSON.stringify({ operation_id: $("#operation-id").value.trim() }),
      });
      showToast("Operation 已执行");
    }
    if (action === "operation-get") {
      state.operation = await api(`/api/management/update/operations/${encodeURIComponent($("#operation-id").value.trim())}`);
      showToast("Operation 已刷新");
    }
    if (action === "operation-list") {
      state.operation = await api("/api/management/update/operations");
      showToast("Operation 列表已刷新");
    }
    if (action === "migration-plan") {
      state.operation = await api("/api/management/update/migration-plan", {
        method: "POST",
        body: JSON.stringify({ confirmed: true, platform_id_map: {} }),
      });
      showToast("迁移计划已生成");
    }
    if (action === "package-plan") {
      state.operation = await api("/api/management/update/package-plan", {
        method: "POST",
        body: JSON.stringify({
          package: $("#package-name").value.trim(),
          requirements_path: null,
          mirror: $("#package-mirror").value.trim() || null,
        }),
      });
      showToast("包安装计划已生成");
    }
    if (action === "package-run") {
      state.operation = await api("/api/management/update/package-run", {
        method: "POST",
        body: JSON.stringify({
          package: $("#package-name").value.trim(),
          requirements_path: null,
          mirror: $("#package-mirror").value.trim() || null,
        }),
      });
      showToast("包安装 operation 已完成");
    }
    if (action === "restart-plan" || action === "restart-run") {
      state.operation = await api(
        action === "restart-plan" ? "/api/management/update/restart-plan" : "/api/management/update/restart-run",
        {
          method: "POST",
          body: JSON.stringify({
            reason: $("#restart-reason").value.trim() || "dashboard maintenance",
            delay_secs: 0,
          }),
        },
      );
      showToast(action === "restart-plan" ? "Restart 计划已生成" : "Restart executor 已调用");
    }
    if (action === "backup-precheck") {
      state.backupTask = await api("/api/management/backup/precheck", {
        method: "POST",
        body: JSON.stringify({ manifest: JSON.parse($("#backup-manifest").value) }),
      });
      showToast("备份预检完成");
    }
    if (action === "backup-export") {
      state.backupTask = await api("/api/management/backup/export", {
        method: "POST",
        body: JSON.stringify({
          task_id: $("#backup-task-id").value.trim() || `export-${Date.now()}`,
          astrbot_version: "0.1.0",
          exported_at: new Date().toISOString(),
        }),
      });
      state.backupTask = await api(`/api/management/backup/progress/${encodeURIComponent(state.backupTask.task.task_id)}`);
      showToast("导出任务完成");
    }
    if (action === "backup-import") {
      state.backupTask = await api("/api/management/backup/import", {
        method: "POST",
        body: JSON.stringify({
          task_id: $("#backup-task-id").value.trim() || `import-${Date.now()}`,
          source_id: "dashboard-upload",
          mode: "Merge",
          confirmed: true,
        }),
      });
      state.backupTask = await api(`/api/management/backup/progress/${encodeURIComponent(state.backupTask.task.task_id)}`);
      showToast("导入任务完成");
    }
    if (action === "backup-progress") {
      state.backupTask = await api(`/api/management/backup/progress/${encodeURIComponent($("#backup-task-id").value.trim())}`);
      showToast("备份任务已刷新");
    }
    if (action === "backup-progress-list") {
      state.backupTask = await api("/api/management/backup/progress");
      showToast("备份任务列表已刷新");
    }
    if (action === "backup-files-refresh") {
      await loadBackup();
      showToast("备份文件已刷新");
    }
    if (action === "backup-file-download") {
      state.backupTask = await api("/api/management/backup/files/download", {
        method: "POST",
        body: JSON.stringify({ filename: target.dataset.filename }),
      });
      showToast("备份下载 token 已创建");
    }
    if (action === "backup-file-rename") {
      state.backupTask = await api("/api/management/backup/files/rename", {
        method: "POST",
        body: JSON.stringify({
          filename: target.dataset.filename,
          new_filename: $("#backup-new-filename").value.trim(),
        }),
      });
      await loadBackup();
      showToast("备份文件已重命名");
    }
    if (action === "backup-file-delete") {
      state.backupTask = await api("/api/management/backup/files/delete", {
        method: "POST",
        body: JSON.stringify({ filename: target.dataset.filename }),
      });
      await loadBackup();
      showToast(state.backupTask.deleted ? "备份文件已删除" : "备份文件不存在");
    }
    if (action === "backup-file-restore") {
      state.backupTask = await api("/api/management/backup/files/restore", {
        method: "POST",
        body: JSON.stringify({
          filename: target.dataset.filename,
          task_id: $("#backup-restore-task-id").value.trim() || `restore-${Date.now()}`,
          mode: $("#backup-restore-mode").value,
          confirmed: true,
        }),
      });
      showToast("备份恢复任务已执行");
    }
    if (action === "backup-upload-start") {
      state.backupTask = await api("/api/management/backup/upload/start", {
        method: "POST",
        body: JSON.stringify({
          upload_id: $("#upload-id").value.trim(),
          filename: $("#upload-filename").value.trim(),
          total_size: Number($("#upload-size").value),
          now_unix: Math.floor(Date.now() / 1000),
        }),
      });
      showToast("上传 session 已创建");
    }
    if (action === "backup-upload-chunk") {
      state.backupTask = await api("/api/management/backup/upload/chunk", {
        method: "POST",
        body: JSON.stringify({
          upload_id: $("#upload-id").value.trim(),
          chunk_index: Number($("#upload-chunk-index").value),
          bytes_len: Number($("#upload-chunk-bytes").value),
          now_unix: Math.floor(Date.now() / 1000),
        }),
      });
      showToast("上传分片已记录");
    }
    if (action === "backup-upload-complete") {
      state.backupTask = await api("/api/management/backup/upload/complete", {
        method: "POST",
        body: JSON.stringify({ upload_id: $("#upload-id").value.trim() }),
      });
      showToast("上传完成计划已生成");
    }
    if (action === "backup-upload-abort") {
      state.backupTask = await api("/api/management/backup/upload/abort", {
        method: "POST",
        body: JSON.stringify({ upload_id: $("#upload-id").value.trim() }),
      });
      showToast(state.backupTask.aborted ? "上传会话已取消" : "上传会话不存在");
    }
}
