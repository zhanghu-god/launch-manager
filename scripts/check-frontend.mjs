import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const html = fs.readFileSync(path.join(root, 'src/index.html'), 'utf8');
const js = fs.readFileSync(path.join(root, 'src/main.js'), 'utf8');
const lib = fs.readFileSync(path.join(root, 'src-tauri/src/lib.rs'), 'utf8');

const ids = [...html.matchAll(/id="([^"]+)"/g)].map((match) => match[1]);
const uniqueIds = new Set(ids);
const references = new Set([...js.matchAll(/\$\('([^']+)'\)/g)].map((match) => match[1]));
const commands = new Set([...js.matchAll(/requireInvoke\(\)\('([^']+)'/g)].map((match) => match[1]));

const missingIds = [...references].filter((id) => !uniqueIds.has(id));
const missingCommands = [...commands].filter((command) => !lib.includes(`commands::${command},`));
const duplicateIds = ids.filter((id, index) => ids.indexOf(id) !== index);

if (missingIds.length || missingCommands.length || duplicateIds.length) {
  console.error({ missingIds, missingCommands, duplicateIds });
  process.exitCode = 1;
} else {
  console.log(`前端静态检查通过：${uniqueIds.size} 个元素 ID，${commands.size} 个 Tauri 命令。`);
}
