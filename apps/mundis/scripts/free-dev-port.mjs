import { execFileSync } from 'node:child_process';

const port = '1420';
const appPath = normalize(process.cwd());

if (process.platform === 'win32') {
  freeWindowsPort();
}

function freeWindowsPort() {
  const output = run('powershell', [
    '-NoProfile',
    '-Command',
    `Get-NetTCPConnection -LocalPort ${port} -State Listen -ErrorAction SilentlyContinue | Select-Object -ExpandProperty OwningProcess -Unique`
  ]);

  for (const rawPid of output.split(/\r?\n/)) {
    const pid = rawPid.trim();
    if (!pid) continue;
    const commandLine = processCommandLine(pid);
    if (normalize(commandLine).includes(appPath) && commandLine.includes('vite')) {
      execFileSync('taskkill', ['/PID', pid, '/T', '/F'], { stdio: 'ignore' });
    }
  }
}

function processCommandLine(pid) {
  return run('powershell', [
    '-NoProfile',
    '-Command',
    `(Get-CimInstance Win32_Process -Filter "ProcessId = ${pid}").CommandLine`
  ]);
}

function run(command, args) {
  try {
    return execFileSync(command, args, { encoding: 'utf8' });
  } catch {
    return '';
  }
}

function normalize(value) {
  return value.replaceAll('\\', '/').toLowerCase();
}
