import { invoke } from '@tauri-apps/api/core';
import type {
  AppConfig,
  AtlasState,
  CausalChain,
  CreateSimulationInput,
  HistoryEvent,
  MundisPaths
} from './types';

export async function createSimulation(input: CreateSimulationInput): Promise<AtlasState> {
  return invoke<AtlasState>('create_simulation', { input });
}

export async function loadAtlasState(savePath: string, month: number): Promise<AtlasState> {
  return invoke<AtlasState>('get_atlas_state', { savePath, month });
}

export async function loadCausalChain(
  savePath: string,
  eventId: number,
  depth = 2
): Promise<CausalChain> {
  return invoke<CausalChain>('get_causal_chain', { savePath, eventId, depth });
}

export async function loadEvents(savePath: string, fromMonth = 0, toMonth: number | null = null): Promise<HistoryEvent[]> {
  return invoke<HistoryEvent[]>('query_events', {
    savePath,
    fromMonth,
    toMonth,
    tag: null,
    subject: null,
    eventType: null,
    severity: null
  });
}

export async function getMundisPaths(): Promise<MundisPaths> {
  return invoke<MundisPaths>('get_mundis_paths');
}

export async function loadAppConfig(): Promise<AppConfig> {
  return invoke<AppConfig>('load_app_config');
}

export async function saveAppConfig(config: AppConfig): Promise<AppConfig> {
  return invoke<AppConfig>('save_app_config', { config });
}

export async function openMundisHome(): Promise<void> {
  return invoke<void>('open_mundis_home');
}
