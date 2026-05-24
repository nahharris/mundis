import { invoke } from '@tauri-apps/api/core';
import type { RecentSave, SavePathOutput } from './types';

export async function resolveWorldSavePath(name: string): Promise<SavePathOutput> {
  return invoke<SavePathOutput>('resolve_world_save_path', { name });
}

export async function listWorldSaves(): Promise<RecentSave[]> {
  return invoke<RecentSave[]>('list_world_saves');
}

export async function recordWorldOpened(savePath: string): Promise<void> {
  return invoke<void>('record_world_opened', { savePath });
}

export function generateWorldName(): string {
  const ages = ['Amber', 'Iron', 'Verdant', 'Cinder', 'Silver', 'Storm', 'Crown', 'Dawn'];
  const forms = ['Reach', 'Vale', 'March', 'Coast', 'Hearth', 'Crownlands', 'Basin', 'Horizon'];
  const suffix = Math.floor(100 + Math.random() * 900);
  return `${pick(ages)} ${pick(forms)} ${suffix}`;
}

export function saveName(path: string): string {
  return path
    .split(/[\\/]/)
    .at(-1)
    ?.replace(/\.mundis$/i, '')
    .split('-')
    .filter(Boolean)
    .map((part) => `${part.charAt(0).toUpperCase()}${part.slice(1)}`)
    .join(' ') || 'World';
}

function pick(values: string[]): string {
  return values[Math.floor(Math.random() * values.length)];
}
