import { createReadStream, statSync } from "node:fs";
import { createServer } from "node:http";
import path from "node:path";

const root = path.resolve(process.argv[2] || "dist-e2e");
const port = Number(process.argv[3] || 1421);
const types = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".wasm": "application/wasm",
};

createServer((request, response) => {
  const url = new URL(request.url || "/", "http://127.0.0.1");
  const relative = decodeURIComponent(url.pathname === "/" ? "/index.html" : url.pathname);
  const file = path.resolve(root, `.${relative}`);
  if (!file.startsWith(`${root}${path.sep}`)) {
    response.writeHead(403).end("forbidden");
    return;
  }
  try {
    if (!statSync(file).isFile()) throw new Error("not a file");
    response.writeHead(200, {
      "Content-Type": types[path.extname(file)] || "application/octet-stream",
      "Cache-Control": "no-store",
    });
    createReadStream(file).pipe(response);
  } catch {
    response.writeHead(404).end("not found");
  }
}).listen(port, "127.0.0.1");
