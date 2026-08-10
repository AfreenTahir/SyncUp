import { spawnSync } from "node:child_process";

const cargoArgs = process.argv.slice(2);
const command = "cargo";
const args = cargoArgs;

const result = spawnSync(command, args, { stdio: "inherit" });
if (result.error) {
  console.error(`Could not start ${command}: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
