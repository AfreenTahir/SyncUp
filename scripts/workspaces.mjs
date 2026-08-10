import { readFileSync } from "node:fs";
import { delimiter, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const action = process.argv[2];
if (!action) process.exit(1);

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const rootPackage = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const binaryPath = join(root, "node_modules", ".bin");

for (const workspace of rootPackage.workspaces) {
  const cwd = join(root, workspace);
  const packageJson = JSON.parse(readFileSync(join(cwd, "package.json"), "utf8"));
  const script = packageJson.scripts?.[action];
  if (!script) continue;
  console.log(`\n> ${packageJson.name} ${action}`);
  const pathKey = Object.keys(process.env).find((key) => key.toLowerCase() === "path") ?? "PATH";
  const result = spawnSync(script, {
    cwd,
    shell: true,
    stdio: "inherit",
    env: { ...process.env, [pathKey]: `${binaryPath}${delimiter}${process.env[pathKey] ?? ""}` },
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
