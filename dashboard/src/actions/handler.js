import { showToast } from "../dom.js";
import { handleCoreActions } from "./core.js";
import { handleExtensionActions } from "./extensions.js";
import { handleKnowledgeActions } from "./knowledge.js";
import { handleMaintenanceActions } from "./maintenance.js";
import { handlePersonaCronActions } from "./personas-cron.js";
import { handleProjectActions } from "./projects.js";
import { handleSessionActions } from "./sessions.js";

const actionHandlers = [
  handleCoreActions,
  handleKnowledgeActions,
  handleSessionActions,
  handleProjectActions,
  handleExtensionActions,
  handlePersonaCronActions,
  handleMaintenanceActions,
];

export async function handleAction(event, render) {
  const target = event.target.closest("[data-action]");
  if (!target) return;

  const context = { action: target.dataset.action, target };
  try {
    for (const handler of actionHandlers) {
      await handler(context);
    }
    render();
  } catch (error) {
    showToast(error.message, "error");
  }
}