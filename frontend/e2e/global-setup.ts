import { execSync } from 'node:child_process';

export default function clearLoginThrottle() {
  if (process.env.E2E_SKIP_THROTTLE_RESET) return;
  try {
    execSync(
      "docker compose exec -T redis sh -c \"redis-cli --scan --pattern 'rate_limit:login*' | xargs -r redis-cli del\"",
      { cwd: new URL('../..', import.meta.url).pathname, stdio: 'ignore' },
    );
  } catch {
    process.stderr.write('[e2e] could not clear login throttle keys; repeated runs may hit HTTP 429\n');
  }
}
