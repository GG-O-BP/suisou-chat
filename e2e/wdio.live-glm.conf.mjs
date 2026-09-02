import path from "node:path";
import { config as nativeConfig } from "./wdio.native.conf.mjs";
import { projectRoot } from "./wdio.shared.mjs";

if (process.env.SUISOU_E2E_LIVE !== "1") {
  throw new Error(
    "Set SUISOU_E2E_LIVE=1 to run the real Z.ai GLM Coding Plan smoke test",
  );
}

export const config = {
  ...nativeConfig,
  specs: [path.join(projectRoot, "e2e/specs/live/live-glm.spec.mjs")],
  mochaOpts: {
    ui: "bdd",
    timeout: 240_000,
  },
};
