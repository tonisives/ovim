import { execFile } from "node:child_process"
import fs from "node:fs/promises"
import path from "node:path"
import { promisify } from "node:util"

let executeFile = promisify(execFile)
let { stdout: cargoMetadata } = await executeFile(
  "cargo",
  ["metadata", "--format-version", "1", "--no-deps"],
  { cwd: path.resolve("src-tauri") },
)
let cargoTargetDirectory = JSON.parse(cargoMetadata).target_directory
let source = await fs.realpath(path.join(cargoTargetDirectory, "release", "bundle", "macos", "ovim.app"))
let outputDirectory = path.resolve(process.env.OVIM_OUTPUT_DIR || "build")
let { stdout: executable } = await executeFile("/usr/libexec/PlistBuddy", [
  "-c", "Print :CFBundleExecutable", path.join(source, "Contents", "Info.plist"),
])
await fs.access(path.join(source, "Contents", "MacOS", executable.trim()), fs.constants.X_OK)
await fs.mkdir(outputDirectory, { recursive: true })
outputDirectory = await fs.realpath(outputDirectory)

let destination = path.join(outputDirectory, "ovim.app")
if (source === destination) {
  console.log(`Built ${destination}`)
  process.exit(0)
}

let temporaryDirectory = await fs.mkdtemp(path.join(outputDirectory, ".ovim-install-"))
let stagedApplication = path.join(temporaryDirectory, "ovim.app")
let previousApplication = path.join(temporaryDirectory, "previous.app")
let backedUp = false
let installed = false
let preserveTemporaryDirectory = false

try {
  await executeFile("/usr/bin/ditto", [source, stagedApplication])
  // Keep existing certificate signatures; sign worker bundles on the host.
  let signature = ""
  try {
    let result = await executeFile("/usr/bin/codesign", ["--display", "--verbose=2", stagedApplication])
    signature = result.stderr
  } catch (error) {
    if (error.code !== 1 || !error.stderr?.includes("code object is not signed at all")) {
      throw error
    }
  }
  if (!signature.includes("Authority=")) {
    let identity = process.env.APPLE_SIGNING_IDENTITY
    if (!identity || identity === "-") {
      throw new Error("Local installation requires APPLE_SIGNING_IDENTITY to select a code-signing certificate.")
    }
    await executeFile("/usr/bin/codesign", ["--force", "--deep", "--options", "runtime", "--timestamp=none", "--sign", identity, stagedApplication])
  }
  await executeFile("/usr/bin/codesign", ["--verify", "--deep", "--strict", stagedApplication])

  try {
    await fs.rename(destination, previousApplication)
    backedUp = true
  } catch (error) {
    if (error.code !== "ENOENT") {
      throw error
    }
  }

  try {
    await fs.rename(stagedApplication, destination)
    installed = true
  } catch (error) {
    try {
      if (installed) {
        await fs.rm(destination, { recursive: true, force: true })
      }
      if (backedUp) {
        await fs.rename(previousApplication, destination)
        backedUp = false
      }
    } catch (restoreError) {
      preserveTemporaryDirectory = true
      throw new AggregateError(
        [error, restoreError],
        `Failed to install ${destination} and restore the previous app`,
      )
    }
    throw error
  }

  console.log(`Installed ${destination}`)
} finally {
  if (preserveTemporaryDirectory) {
    console.error(`Installation files preserved at ${temporaryDirectory}`)
  } else {
    await fs.rm(temporaryDirectory, { recursive: true, force: true })
  }
}
