export type AtlasState = {
  month: number;
  population: number;
  regions: AtlasRegion[];
  settlements: AtlasSettlement[];
  polities: AtlasPolity[];
};

export type AtlasRegion = {
  id: number;
  name: string;
  climate: string;
  biome: string;
  carrying_capacity: number;
  population: number;
  neighbors: number[];
};

export type AtlasSettlement = {
  id: number;
  name: string;
  region: number;
  population: number;
  polity: number | null;
  status: 'active' | 'declining' | 'abandoned';
  stability: number;
};

export type AtlasPolity = {
  id: number;
  name: string;
  capital: number;
  controlled_regions: number[];
  controlled_settlements: number[];
  cohesion: number;
};

export type HistoryEvent = {
  id: number;
  month: number;
  event_type: string;
  severity: string;
  tags: string[];
  subjects: unknown[];
  causes: string[];
  consequences: string[];
  caused_by?: number[];
  summary: string;
};

export type CausalChain = {
  event: HistoryEvent;
  causes: HistoryEvent[];
  effects: HistoryEvent[];
};

export type CreateSimulationInput = {
  jobId: string;
  savePath: string;
  seed: number;
  months: number;
  regions: number;
  initialSettlements: number;
  initialPopulationPerMille: number;
  monthlyGrowthPerMille: number;
  civilizationEnabled: boolean;
  bias: 'plausible' | 'dramatic' | 'harsh' | 'peaceful';
};

export type AppConfig = {
  log_level: 'debug' | 'info' | 'warn' | 'error';
  default_seed: number;
  default_months: number;
  default_regions: number;
  default_settlements: number;
  default_population_per_mille: number;
  default_monthly_growth_per_mille: number;
  default_civilization_enabled: boolean;
  default_bias: WorldSetup['bias'];
};

export type MundisPaths = {
  home: string;
  savesDir: string;
  logsDir: string;
  frontendLogFile: string;
  backendLogFile: string;
  configFile: string;
};

export type SimulationProgressEvent = {
  jobId: string;
  savePath: string;
  currentMonth: number;
  totalMonths: number;
  eventsWritten: number;
  population: number;
  stage: 'initializing' | 'running' | 'complete';
};

export type WorldSetup = Omit<CreateSimulationInput, 'jobId' | 'savePath'>;

export type RecentSave = {
  path: string;
  name: string;
  openedAt?: number | null;
};

export type SavePathOutput = {
  name: string;
  path: string;
};
