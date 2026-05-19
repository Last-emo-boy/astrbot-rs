import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";

const root = process.cwd();
const port = Number(process.env.PORT || 4173);

const contentTypes = new Map([
  [".css", "text/css; charset=utf-8"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".png", "image/png"],
  [".svg", "image/svg+xml"],
  [".webp", "image/webp"],
]);

const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url || "/", `http://${request.headers.host || "127.0.0.1"}`);
    const requestPath = decodeURIComponent(url.pathname);
    const filePath = requestPath.startsWith("/api/")
      ? null
      : assetPathFor(requestPath);
    if (!filePath) {
      response.writeHead(404, { "content-type": "text/plain; charset=utf-8" });
      response.end("not found");
      return;
    }
    const body = await readFile(filePath);
    response.writeHead(200, { "content-type": contentTypes.get(extname(filePath)) || "application/octet-stream" });
    response.end(body);
  } catch (error) {
    response.writeHead(404, { "content-type": "text/plain; charset=utf-8" });
    response.end(String(error));
  }
});

server.listen(port, "127.0.0.1");

function assetPathFor(requestPath) {
  if (isSpaRoute(requestPath)) {
    return join(root, "index.html");
  }
  const relative = normalize(requestPath).replace(/^[/\\]+/, "");
  if (!relative || relative.startsWith("..")) {
    return null;
  }
  return join(root, relative);
}

function isSpaRoute(requestPath) {
  return requestPath === "/"
    || requestPath === "/main"
    || requestPath === "/welcome"
    || requestPath === "/dashboard/default"
    || requestPath === "/auth/login"
    || requestPath === "/config"
    || requestPath === "/normal"
    || requestPath === "/system"
    || requestPath === "/settings"
    || requestPath === "/settings/backup"
    || requestPath === "/settings/update"
    || requestPath === "/about"
    || requestPath === "/providers"
    || requestPath === "/platforms"
    || requestPath === "/extension"
    || requestPath === "/extension-marketplace"
    || requestPath === "/extension/tools"
    || requestPath === "/extension/skills"
    || requestPath === "/subagent"
    || requestPath === "/console"
    || requestPath === "/logs"
    || requestPath === "/trace"
    || requestPath === "/tool-use"
    || requestPath === "/chat"
    || requestPath.startsWith("/chat/")
    || requestPath === "/conversation"
    || requestPath === "/session-management"
    || requestPath === "/persona"
    || requestPath === "/cron"
    || requestPath === "/chatbox"
    || requestPath.startsWith("/chatbox/")
    || requestPath === "/knowledge-base"
    || requestPath.startsWith("/knowledge-base/")
    || requestPath === "/alkaid/knowledge-base"
    || requestPath === "/alkaid"
    || requestPath === "/alkaid/long-term-memory"
    || requestPath === "/alkaid/other";
}
