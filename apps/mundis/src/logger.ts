import { invoke } from '@tauri-apps/api/core';

type LogLevel = 'info' | 'error';

type LogPayload = {
  level: LogLevel;
  message: string;
  context?: unknown;
  timestamp: string;
};

export async function logFrontendInfo(message: string, context?: unknown) {
  await recordFrontendLog('info', message, context);
}

export async function logFrontendError(message: string, context?: unknown) {
  await recordFrontendLog('error', message, context);
}

async function recordFrontendLog(level: LogLevel, message: string, context?: unknown) {
  const payload: LogPayload = {
    level,
    message,
    context,
    timestamp: new Date().toISOString()
  };

  if (level === 'error') {
    console.error('[mundis]', message, context ?? '');
  } else {
    console.info('[mundis]', message, context ?? '');
  }

  appendLocalLog(payload);

  try {
    await invoke('record_frontend_log', payload);
  } catch (caught) {
    console.warn('[mundis] failed to persist frontend log', caught);
  }
}

function appendLocalLog(payload: LogPayload) {
  const key = 'mundis.frontend.logs';
  const existing = JSON.parse(window.localStorage.getItem(key) ?? '[]') as LogPayload[];
  existing.push(payload);
  window.localStorage.setItem(key, JSON.stringify(existing.slice(-200)));
}
