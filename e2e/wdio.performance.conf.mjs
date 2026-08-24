import path from "node:path";
import { config as browserConfig } from "./wdio.browser.conf.mjs";
import { projectRoot } from "./wdio.shared.mjs";

export const config = {
  ...browserConfig,
  specs: [path.join(projectRoot, "e2e/specs/performance/**/*.spec.mjs")],
};
