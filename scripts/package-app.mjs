import { execFileSync, spawnSync } from "node:child_process"
import path from "node:path"
import { selectSigningIdentity } from "./signing-identity.mjs"

let identity = process.env.APPLE_SIGNING_IDENTITY
if (!identity) {
  let identities = execFileSync("/usr/bin/security", ["find-identity", "-v", "-p", "codesigning"], { encoding: "utf8" })
  identity = selectSigningIdentity(identities)
}
if (identity === "-") {
  throw new Error("Ad-hoc signing cannot preserve local privacy permissions. Choose a code-signing certificate.")
}

let metadata = JSON.parse(execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps"], {
  cwd: path.resolve("src-tauri"), encoding: "utf8",
}))
let build = spawnSync("python3", [
  "scripts/build-local.py", "run", "--prepare-node", "--cargo-output-dir", metadata.target_directory,
  "--", "pnpm", "tauri", "build", "--bundles", "app", "--config",
  JSON.stringify({ bundle: { createUpdaterArtifacts: false } }), "--", "--jobs", "4",
], { stdio: "inherit", env: { ...process.env, APPLE_SIGNING_IDENTITY: identity } })
if (build.error) throw build.error
if (build.status !== 0) process.exit(build.status ?? 1)
process.env.APPLE_SIGNING_IDENTITY = identity
await import("./install-app.mjs")
