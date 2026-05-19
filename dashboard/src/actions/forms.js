import { $ } from "../dom.js";
export function splitCsv(value) {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

export function splitLines(value) {
  return value
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter(Boolean);
}

export function optionalText(selector) {
  const value = $(selector)?.value.trim() || "";
  return value || null;
}

export function projectActor() {
  return $("#project-actor")?.value.trim() || "user";
}

export function projectIdFrom(target) {
  return target.dataset.project || $("#project-id")?.value.trim() || $("#project-session-project-id")?.value.trim() || "";
}
