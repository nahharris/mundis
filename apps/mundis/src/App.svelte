<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import WorldStage from './WorldStage.svelte';
  import { logFrontendError, logFrontendInfo } from './logger';
  import {
    createSimulation,
    getMundisPaths,
    loadAppConfig,
    loadAtlasState,
    loadEvents,
    openMundisHome,
    saveAppConfig
  } from './mundisApi';
  import { generateWorldName, listWorldSaves, recordWorldOpened, resolveWorldSavePath, saveName } from './saves';
  import type {
    AppConfig,
    AtlasPolity,
    AtlasRegion,
    AtlasSettlement,
    AtlasState,
    HistoryEvent,
    MundisPaths,
    RecentSave,
    SimulationProgressEvent,
    WorldSetup
  } from './types';

  type Screen = 'menu' | 'setup' | 'load' | 'settings' | 'progress' | 'world';
  type WorldTab = 'world' | 'chronicle' | 'regions' | 'polities';

  type WorldNavItem = {
    id: WorldTab;
    label: string;
    detail: string;
  };

  type RegionSummary = {
    region: AtlasRegion;
    settlements: AtlasSettlement[];
    polities: AtlasPolity[];
    population: number;
    settledPopulation: number;
    unsettledPopulation: number;
  };

  type PolitySummary = {
    polity: AtlasPolity;
    capital: AtlasSettlement | null;
    settlements: AtlasSettlement[];
    population: number;
  };

  type ChronicleMonth = {
    month: number;
    events: HistoryEvent[];
  };

  type EventSubjectRef = {
    region?: number;
    settlement?: number;
    polity?: number;
    culture?: number;
    'population-group'?: number;
    populationGroup?: number;
  };

  let screen: Screen = 'menu';
  let recentSaves: RecentSave[] = [];
  let mundisPaths: MundisPaths | null = null;
  let savePath = '';
  let worldName = generateWorldName();
  let currentWorldName = '';
  let appConfig: AppConfig = {
    log_level: 'info',
    default_seed: 1,
    default_months: 120,
    default_regions: 8,
    default_settlements: 3,
    default_population_per_mille: 240,
    default_monthly_growth_per_mille: 8,
    default_civilization_enabled: true,
    default_bias: 'plausible'
  };
  let setup: WorldSetup = {
    seed: 1,
    months: 120,
    regions: 8,
    initialSettlements: 3,
    initialPopulationPerMille: 240,
    monthlyGrowthPerMille: 8,
    civilizationEnabled: true,
    bias: 'plausible'
  };
  let progress: SimulationProgressEvent | null = null;
  let activeJobId = '';
  let atlas: AtlasState | null = null;
  let events: HistoryEvent[] = [];
  let selectedEvent: HistoryEvent | null = null;
  let activeWorldTab: WorldTab = 'world';
  let selectedRegionId: number | null = null;
  let worldNavItems: WorldNavItem[] = [];
  let progressListenerReady: Promise<void> = Promise.resolve();
  let stopProgressListener: (() => void) | null = null;
  let error = '';
  let busy = false;
  let toast: { message: string; tone: 'success' | 'error' | 'info' } | null = null;
  let toastTimer: number | undefined;
  const hints = {
    logLevel: 'Controls how much technical detail Mundis writes into its logs.',
    seed: 'The number that reproduces the same generated world.',
    months: 'How many monthly turns the simulation runs before opening the chronicle.',
    regions: 'How many connected world regions Mundis generates.',
    settlements: 'How many early settlements exist at the beginning.',
    populationDensity: 'How full early regions are compared with their carrying capacity.',
    monthlyGrowth: 'How quickly populations tend to grow each month.',
    historicalBias: 'Adjusts the tone of events toward plausible, dramatic, harsh, or peaceful history.',
    civilizationSystems: 'Enables polities, expansion, trade, tension, wars, treaties, and collapse.'
  };

  $: progressPercent = progress ? Math.min(100, Math.round((progress.currentMonth / Math.max(1, progress.totalMonths)) * 100)) : 0;
  $: progressStageLabel =
    progress?.stage === 'complete'
      ? 'Finishing chronicle'
      : progress?.stage === 'running'
        ? `Simulating month ${progress.currentMonth} of ${progress.totalMonths}`
        : 'Preparing simulation';
  $: chronicleEvents = [...events].sort((left, right) => right.month - left.month || right.id - left.id);
  $: chronicleMonths = groupEventsByMonth(chronicleEvents);
  $: regionSummaries = atlas ? summarizeRegions(atlas) : [];
  $: politySummaries = atlas ? summarizePolities(atlas) : [];
  $: selectedRegion = atlas?.regions.find((region) => region.id === selectedRegionId) ?? null;
  $: selectedRegionSummary = selectedRegion ? regionSummaries.find((summary) => summary.region.id === selectedRegion.id) ?? null : null;
  $: worldNavItems = atlas
    ? [
        { id: 'world', label: 'World', detail: `${atlas.population.toLocaleString()} people` },
        { id: 'chronicle', label: 'Chronicle', detail: 'Timeline' },
        { id: 'regions', label: 'Regions', detail: activeWorldTab === 'regions' && selectedRegion ? selectedRegion.name : `${atlas.regions.length} regions` },
        { id: 'polities', label: 'Polities', detail: `${atlas.polities.length} systems` }
      ]
    : [];

  onMount(() => {
    void loadSettings();
    void refreshWorldSaves();
    progressListenerReady = listen<SimulationProgressEvent>('simulation-progress', (event) => {
      const update = normalizeProgressEvent(event.payload);
      if (update.jobId === activeJobId) {
        progress = update;
      }
    })
      .then((stop) => {
        stopProgressListener = stop;
      })
      .catch(async (caught) => {
        await logFrontendError('simulation.progress.listen.failed', { error: String(caught) });
      });
    return () => {
      stopProgressListener?.();
    };
  });

  async function loadSettings() {
    try {
      const [paths, config] = await Promise.all([getMundisPaths(), loadAppConfig()]);
      mundisPaths = paths;
      appConfig = config;
      setup = setupFromConfig(config);
    } catch (caught) {
      error = String(caught);
      await logFrontendError('settings.load.failed', { error });
    }
  }

  async function saveSettings() {
    error = '';
    busy = true;
    try {
      appConfig = await saveAppConfig(appConfig);
      setup = setupFromConfig(appConfig);
      showToast('Settings saved', 'success');
      await logFrontendInfo('settings.save.done', { configFile: mundisPaths?.configFile });
    } catch (caught) {
      error = String(caught);
      showToast('Could not save settings', 'error');
      await logFrontendError('settings.save.failed', { error });
    } finally {
      busy = false;
    }
  }

  async function openHomeFolder() {
    try {
      await openMundisHome();
      showToast('Opened Mundis folder', 'info');
    } catch (caught) {
      error = String(caught);
      showToast('Could not open folder', 'error');
      await logFrontendError('settings.home.open.failed', { error });
    }
  }

  function showToast(message: string, tone: 'success' | 'error' | 'info' = 'info') {
    toast = { message, tone };
    if (toastTimer) {
      window.clearTimeout(toastTimer);
    }
    toastTimer = window.setTimeout(() => {
      toast = null;
    }, 2600);
  }

  function setupFromConfig(config: AppConfig): WorldSetup {
    return {
      seed: config.default_seed,
      months: config.default_months,
      regions: config.default_regions,
      initialSettlements: config.default_settlements,
      initialPopulationPerMille: config.default_population_per_mille,
      monthlyGrowthPerMille: config.default_monthly_growth_per_mille,
      civilizationEnabled: config.default_civilization_enabled,
      bias: config.default_bias
    };
  }

  async function refreshWorldSaves() {
    try {
      recentSaves = await listWorldSaves();
    } catch (caught) {
      error = String(caught);
      await logFrontendError('world.saves.list.failed', { error });
    }
  }

  async function startNewWorld() {
    error = '';
    let resolved;
    try {
      resolved = await resolveWorldSavePath(worldName);
    } catch (caught) {
      error = String(caught);
      await logFrontendError('world.save.resolve.failed', { error });
      return;
    }

    savePath = resolved.path;
    currentWorldName = resolved.name;
    await progressListenerReady;
    activeJobId = crypto.randomUUID();
    progress = {
      jobId: activeJobId,
      savePath,
      currentMonth: 0,
      totalMonths: setup.months,
      eventsWritten: 0,
      population: 0,
      stage: 'initializing'
    };
    busy = true;
    screen = 'progress';
    await paintFrame();

    await logFrontendInfo('simulation.create.start', { savePath, ...setup });
    try {
      atlas = await createSimulation({ jobId: activeJobId, savePath, ...setup });
      events = await loadEvents(savePath, 0, atlas.month);
      selectedEvent = events.at(-1) ?? null;
      selectedRegionId = null;
      activeWorldTab = 'world';
      await refreshWorldSaves();
      screen = 'world';
      await logFrontendInfo('simulation.create.done', {
        savePath,
        month: atlas.month,
        events: events.length,
        population: atlas.population
      });
    } catch (caught) {
      error = String(caught);
      screen = 'setup';
      await logFrontendError('simulation.create.failed', { error });
    } finally {
      busy = false;
    }
  }

  async function openWorld(path?: string) {
    error = '';
    if (!path) {
      error = 'Choose a world to load.';
      return;
    }
    const selectedPath = path;

    busy = true;
    await logFrontendInfo('world.open.start', { savePath: selectedPath });
    try {
      const loadedEvents = await loadEvents(selectedPath);
      const latestMonth = loadedEvents.reduce((month, event) => Math.max(month, event.month), 0);
      atlas = await loadAtlasState(selectedPath, latestMonth);
      await recordWorldOpened(selectedPath);
      events = loadedEvents;
      selectedEvent = events.at(-1) ?? null;
      selectedRegionId = null;
      activeWorldTab = 'world';
      savePath = selectedPath;
      currentWorldName = recentSaves.find((save) => save.path === selectedPath)?.name ?? saveName(selectedPath);
      await refreshWorldSaves();
      screen = 'world';
      await logFrontendInfo('world.open.done', { savePath, month: atlas.month, events: events.length });
    } catch (caught) {
      error = String(caught);
      await logFrontendError('world.open.failed', { error });
    } finally {
      busy = false;
    }
  }

  function returnToMenu() {
    error = '';
    selectedRegionId = null;
    screen = 'menu';
  }

  async function showLoadScreen() {
    error = '';
    await refreshWorldSaves();
    screen = 'load';
  }

  function selectRegion(regionId: number) {
    selectedRegionId = activeWorldTab === 'regions' && selectedRegionId === regionId ? null : regionId;
    activeWorldTab = 'regions';
  }

  function selectEvent(event: HistoryEvent) {
    selectedEvent = event;
    selectedRegionId = null;
    activeWorldTab = 'chronicle';
  }

  function focusRelatedEvent(eventId: number) {
    const related = events.find((event) => event.id === eventId);
    if (related) {
      selectEvent(related);
    }
  }

  function groupEventsByMonth(sortedEvents: HistoryEvent[]): ChronicleMonth[] {
    const grouped: ChronicleMonth[] = [];
    for (const event of sortedEvents) {
      const existing = grouped.at(-1);
      if (existing && existing.month === event.month) {
        existing.events.push(event);
      } else {
        grouped.push({ month: event.month, events: [event] });
      }
    }
    return grouped;
  }

  function switchWorldTab(tab: WorldTab) {
    activeWorldTab = tab;
    if (tab !== 'regions') {
      selectedRegionId = null;
    }
  }

  function summarizeRegions(state: AtlasState): RegionSummary[] {
    return state.regions
      .map((region) => {
        const settlements = state.settlements.filter((settlement) => settlement.region === region.id);
        const settledPopulation = settlements.reduce((total, settlement) => total + settlement.population, 0);
        const settlementIds = new Set(settlements.map((settlement) => settlement.id));
        const polities = state.polities.filter(
          (polity) =>
            polity.controlled_regions.includes(region.id) ||
            polity.controlled_settlements.some((settlementId) => settlementIds.has(settlementId))
        );

        return {
          region,
          settlements,
          polities,
          population: region.population,
          settledPopulation,
          unsettledPopulation: Math.max(0, region.population - settledPopulation)
        };
      })
      .sort((left, right) => right.population - left.population || left.region.name.localeCompare(right.region.name));
  }

  function summarizePolities(state: AtlasState): PolitySummary[] {
    return state.polities
      .map((polity) => {
        const controlledSettlements = new Set(polity.controlled_settlements);
        const settlements = state.settlements.filter((settlement) => controlledSettlements.has(settlement.id));
        const capital = state.settlements.find((settlement) => settlement.id === polity.capital) ?? null;

        return {
          polity,
          capital,
          settlements,
          population: settlements.reduce((total, settlement) => total + settlement.population, 0)
        };
      })
      .sort((left, right) => right.population - left.population || left.polity.name.localeCompare(right.polity.name));
  }

  function regionName(regionId: number) {
    return atlas?.regions.find((region) => region.id === regionId)?.name ?? `Region ${regionId}`;
  }

  function polityName(polityId: number | null) {
    if (polityId === null) return 'Independent';
    return atlas?.polities.find((polity) => polity.id === polityId)?.name ?? `Polity ${polityId}`;
  }

  function chronicleLine(event: HistoryEvent) {
    const subject = eventSubjectName(event) ?? summarySubject(event.summary);
    return subject ? `${subject} ${event.event_type}` : event.event_type;
  }

  function eventSubjectName(event: HistoryEvent) {
    const refs = event.subjects
      .map((subject) => (typeof subject === 'object' && subject !== null ? (subject as EventSubjectRef) : null))
      .filter((subject): subject is EventSubjectRef => subject !== null);

    for (const ref of refs) {
      if (typeof ref.polity === 'number') {
        const polity = atlas?.polities.find((candidate) => candidate.id === ref.polity);
        if (polity) return polity.name;
      }
    }

    for (const ref of refs) {
      if (typeof ref.settlement === 'number') {
        const settlement = atlas?.settlements.find((candidate) => candidate.id === ref.settlement);
        if (settlement) return settlement.name;
      }
    }

    for (const ref of refs) {
      if (typeof ref.region === 'number') {
        const region = atlas?.regions.find((candidate) => candidate.id === ref.region);
        if (region) return region.name;
      }
    }

    return null;
  }

  function summarySubject(summary: string) {
    const breakpoints = [
      ' changed ',
      ' grew ',
      ' formed ',
      ' strained ',
      ' was ',
      ' began ',
      ' entered ',
      ' named ',
      ' collapsed ',
      ' expanded ',
      ' passed ',
      ' declared ',
      ' signed ',
      ' reshaped ',
      ' stirred '
    ];
    const match = breakpoints
      .map((breakpoint) => summary.indexOf(breakpoint))
      .filter((index) => index > 0)
      .sort((left, right) => left - right)
      .at(0);

    if (!match) return null;
    return summary.slice(0, match).trim();
  }

  function formatLabel(value: string) {
    return value
      .replace(/_/g, ' ')
      .replace(/([a-z])([A-Z])/g, '$1 $2')
      .replace(/\b\w/g, (letter) => letter.toUpperCase());
  }

  function settlementStatusLabel(status: AtlasSettlement['status']) {
    return formatLabel(status);
  }

  function settlementPopulationLabel(settlement: AtlasSettlement) {
    if (settlement.population > 0) return `${settlement.population.toLocaleString()} people`;
    if (settlement.status === 'abandoned') return 'Abandoned';
    return 'No resident population';
  }

  async function paintFrame() {
    await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
  }

  function normalizeProgressEvent(payload: SimulationProgressEvent | Record<string, unknown>): SimulationProgressEvent {
    const raw = payload as SimulationProgressEvent & {
      job_id?: string;
      save_path?: string;
      current_month?: number;
      total_months?: number;
      events_written?: number;
    };

    return {
      jobId: raw.jobId ?? raw.job_id ?? '',
      savePath: raw.savePath ?? raw.save_path ?? '',
      currentMonth: raw.currentMonth ?? raw.current_month ?? 0,
      totalMonths: raw.totalMonths ?? raw.total_months ?? setup.months,
      eventsWritten: raw.eventsWritten ?? raw.events_written ?? 0,
      population: raw.population ?? 0,
      stage: raw.stage ?? 'running'
    };
  }
</script>

<main class:world-open={screen === 'world'}>
  {#if screen === 'menu'}
    <section class="menu-screen">
      <div class="brand-block">
        <p class="kicker">Living history simulator</p>
        <h1>Mundis</h1>
        <p class="promise">Shape the starting conditions, then watch a planet remember itself.</p>
      </div>

      <div class="menu-actions">
        <button class="primary-action" onclick={() => (screen = 'setup')} disabled={busy}>New World</button>
        <button onclick={() => showLoadScreen()} disabled={busy}>Load World</button>
        <button onclick={() => (screen = 'settings')} disabled={busy}>Settings</button>
      </div>

      {#if recentSaves.length}
        <section class="recent-worlds" aria-label="Recent worlds">
          <h2>Continue</h2>
          <div>
            {#each recentSaves.slice(0, 3) as recent}
              <button onclick={() => openWorld(recent.path)} disabled={busy}>
                <span>{recent.name}</span>
                {#if recent.openedAt}
                  <small>Last opened {new Date(recent.openedAt * 1000).toLocaleString()}</small>
                {/if}
              </button>
            {/each}
          </div>
        </section>
      {/if}
    </section>
  {:else if screen === 'load'}
    <section class="load-screen">
      <nav class="top-bar">
        <button class="ghost" onclick={returnToMenu}>Back</button>
        <strong>Mundis</strong>
      </nav>

      <div class="load-layout">
        <section class="setup-intro">
          <p class="kicker">Load world</p>
          <h1>Choose a chronicle</h1>
          <p>Open any saved world from your Mundis saves folder.</p>
          {#if error}
            <p class="error">{error}</p>
          {/if}
        </section>

        <section class="world-list" aria-label="Saved worlds">
          {#if recentSaves.length}
            {#each recentSaves as saved}
              <button onclick={() => openWorld(saved.path)} disabled={busy}>
                <span>{saved.name}</span>
                {#if saved.openedAt}
                  <small>Last opened {new Date(saved.openedAt * 1000).toLocaleString()}</small>
                {/if}
              </button>
            {/each}
          {:else}
            <p class="empty-state">No saved worlds yet.</p>
          {/if}
        </section>
      </div>
    </section>
  {:else if screen === 'settings'}
    <section class="setup-screen">
      <nav class="top-bar">
        <button class="ghost" onclick={returnToMenu}>Back</button>
        <strong>Mundis</strong>
      </nav>

      <div class="settings-layout">
        <section class="setup-intro">
          <p class="kicker">Settings</p>
          <h1>Tune the world forge</h1>
          <p>Mundis keeps your configuration and logs in its home directory so the game state is easy to inspect.</p>
          <button class="folder-action" onclick={() => openHomeFolder()} disabled={busy}>Open Mundis Folder</button>
          {#if error}
            <p class="error">{error}</p>
          {/if}
        </section>

        <form class="setup-panel settings-panel" onsubmit={(event) => { event.preventDefault(); void saveSettings(); }}>
          <label>
            <span class="label-row">Log level <span class="hint" title={hints.logLevel}>i</span></span>
            <div class="segmented">
              {#each ['debug', 'info', 'warn', 'error'] as level}
                <button
                  type="button"
                  class:selected={appConfig.log_level === level}
                  onclick={() => (appConfig.log_level = level as AppConfig['log_level'])}
                >
                  {level}
                </button>
              {/each}
            </div>
          </label>

          <div class="field-grid">
            <label>
              <span class="label-row">Default seed <span class="hint" title={hints.seed}>i</span></span>
              <input type="number" min="1" bind:value={appConfig.default_seed} />
            </label>
            <label>
              <span class="label-row">Default months <span class="hint" title={hints.months}>i</span></span>
              <input type="number" min="1" bind:value={appConfig.default_months} />
            </label>
            <label>
              <span class="label-row">Default regions <span class="hint" title={hints.regions}>i</span></span>
              <input type="number" min="3" bind:value={appConfig.default_regions} />
            </label>
            <label>
              <span class="label-row">Default settlements <span class="hint" title={hints.settlements}>i</span></span>
              <input type="number" min="1" bind:value={appConfig.default_settlements} />
            </label>
          </div>

          <label>
            <span class="label-row">Default population density <span class="hint" title={hints.populationDensity}>i</span></span>
            <input type="range" min="50" max="900" step="10" bind:value={appConfig.default_population_per_mille} />
            <span>{appConfig.default_population_per_mille} per mille</span>
          </label>

          <label>
            <span class="label-row">Default monthly growth <span class="hint" title={hints.monthlyGrowth}>i</span></span>
            <input type="range" min="0" max="40" step="1" bind:value={appConfig.default_monthly_growth_per_mille} />
            <span>{appConfig.default_monthly_growth_per_mille} per mille</span>
          </label>

          <fieldset>
            <legend class="label-row">Default historical bias <span class="hint" title={hints.historicalBias}>i</span></legend>
            <div class="segmented">
              {#each ['plausible', 'dramatic', 'harsh', 'peaceful'] as bias}
                <button
                  type="button"
                  class:selected={appConfig.default_bias === bias}
                  onclick={() => (appConfig.default_bias = bias as AppConfig['default_bias'])}
                >
                  {bias}
                </button>
              {/each}
            </div>
          </fieldset>

          <label class="toggle">
            <input type="checkbox" bind:checked={appConfig.default_civilization_enabled} />
            <span class="label-row">Default civilization systems <span class="hint" title={hints.civilizationSystems}>i</span></span>
          </label>

          <button class="primary-action" type="submit" disabled={busy}>Save Settings</button>
        </form>
      </div>
    </section>
  {:else if screen === 'setup'}
    <section class="setup-screen">
      <nav class="top-bar">
        <button class="ghost" onclick={returnToMenu}>Back</button>
        <strong>Mundis</strong>
      </nav>

      <div class="setup-layout">
        <section class="setup-intro">
          <p class="kicker">New world</p>
          <h1>Found the first age</h1>
          <p>Name the world, then pick the pressure, scale, and temperament of the simulation. Mundis stores saves in its own home directory.</p>
          {#if error}
            <p class="error">{error}</p>
          {/if}
        </section>

        <form class="setup-panel" onsubmit={(event) => { event.preventDefault(); void startNewWorld(); }}>
          <label>
            World name
            <div class="name-row">
              <input bind:value={worldName} maxlength="64" />
              <button type="button" onclick={() => (worldName = generateWorldName())}>Roll</button>
            </div>
          </label>

          <div class="field-grid">
            <label>
              Seed
              <input type="number" min="1" bind:value={setup.seed} />
            </label>
            <label>
              Months
              <input type="number" min="1" bind:value={setup.months} />
            </label>
            <label>
              Regions
              <input type="number" min="3" bind:value={setup.regions} />
            </label>
            <label>
              Settlements
              <input type="number" min="1" bind:value={setup.initialSettlements} />
            </label>
          </div>

          <label>
            Population density
            <input type="range" min="50" max="900" step="10" bind:value={setup.initialPopulationPerMille} />
            <span>{setup.initialPopulationPerMille} per mille</span>
          </label>

          <label>
            Monthly growth
            <input type="range" min="0" max="40" step="1" bind:value={setup.monthlyGrowthPerMille} />
            <span>{setup.monthlyGrowthPerMille} per mille</span>
          </label>

          <fieldset>
            <legend>Historical bias</legend>
            <div class="segmented">
              {#each ['plausible', 'dramatic', 'harsh', 'peaceful'] as bias}
                <button
                  type="button"
                  class:selected={setup.bias === bias}
                  onclick={() => (setup.bias = bias as WorldSetup['bias'])}
                >
                  {bias}
                </button>
              {/each}
            </div>
          </fieldset>

          <label class="toggle">
            <input type="checkbox" bind:checked={setup.civilizationEnabled} />
            Civilization systems
          </label>

          <button class="primary-action" type="submit" disabled={busy}>Begin World</button>
        </form>
      </div>
    </section>
  {:else if screen === 'progress'}
    <section class="progress-screen">
      <div class="progress-orbit" aria-hidden="true">
        <span></span>
        <span></span>
        <span></span>
      </div>
      <div class="progress-copy">
        <p class="kicker">Simulating {currentWorldName}</p>
        <h1>Writing the first chronicle</h1>
        <p>{progressStageLabel}</p>
        <div class:indeterminate={progressPercent === 0 && busy} class="progress-track">
          <div style={`width: ${progressPercent}%`}></div>
        </div>
        <small>{progressPercent}% · {progress?.eventsWritten ?? 0} events recorded · {(progress?.population ?? 0).toLocaleString()} people</small>
      </div>
    </section>
  {:else if screen === 'world' && atlas}
    <section class="game-shell">
      <header class="command-bar">
        <button class="ghost" onclick={returnToMenu}>Menu</button>
        <div>
          <strong>{currentWorldName}</strong>
          <span>Month {atlas.month}</span>
        </div>
        <button onclick={() => showLoadScreen()} disabled={busy}>Load</button>
      </header>

      <section class="world-layout">
        <aside class="left-rail nav-rail">
          <p class="kicker">Navigation</p>
          <div class="world-tabs" role="tablist" aria-label="World panels">
            {#each worldNavItems as item}
              <button
                class:selected={activeWorldTab === item.id}
                role="tab"
                aria-selected={activeWorldTab === item.id}
                onclick={() => switchWorldTab(item.id)}
              >
                <span>{item.label}</span>
                <small>{item.detail}</small>
              </button>
            {/each}
          </div>

          <section class="rail-card">
            <h2>Atlas Focus</h2>
            {#if selectedRegion}
              <strong>{selectedRegion.name}</strong>
              <small>{selectedRegion.biome} · {selectedRegion.climate}</small>
              <button onclick={() => switchWorldTab('regions')}>Inspect Region</button>
            {:else}
              <small>Click a region on the atlas to inspect its current state.</small>
            {/if}
          </section>
        </aside>

        <section class="stage-panel">
          <WorldStage {atlas} selectedRegionId={activeWorldTab === 'regions' ? selectedRegionId : null} onRegionSelect={selectRegion} />
        </section>

        <aside class="right-rail">
          {#if activeWorldTab === 'world'}
            <section class="tab-panel">
              <div class="panel-heading">
                <p class="kicker">World</p>
                <h2>Current State</h2>
              </div>

              <div class="metric-grid">
                <article>
                  <strong>{atlas.population.toLocaleString()}</strong>
                  <span>Population</span>
                </article>
                <article>
                  <strong>{atlas.month}</strong>
                  <span>Month</span>
                </article>
                <article>
                  <strong>{atlas.regions.length}</strong>
                  <span>Regions</span>
                </article>
                <article>
                  <strong>{atlas.settlements.length}</strong>
                  <span>Settlements</span>
                </article>
                <article>
                  <strong>{atlas.polities.length}</strong>
                  <span>Polities</span>
                </article>
                <article>
                  <strong>{events.length.toLocaleString()}</strong>
                  <span>Events</span>
                </article>
              </div>

              <div class="panel-section">
                <h3>Most Populated Regions</h3>
                <div class="compact-list panel-scroll short-list">
                  {#each regionSummaries.slice(0, 6) as summary}
                    <button class:selected={selectedRegionId === summary.region.id} onclick={() => selectRegion(summary.region.id)}>
                      <span>{summary.region.name}</span>
                      <small>{summary.population.toLocaleString()} people · {summary.settlements.length} settlements</small>
                    </button>
                  {/each}
                </div>
              </div>
            </section>
          {:else if activeWorldTab === 'chronicle'}
            <section class="tab-panel chronicle-panel">
              <div class="panel-heading">
                <p class="kicker">Chronicle</p>
                <h2>Timeline</h2>
              </div>

              <div class="timeline panel-scroll">
                {#each chronicleMonths as month}
                  <section class="timeline-month">
                    <div class="timeline-month-marker">
                      <span aria-hidden="true"></span>
                      <strong>Month {month.month}</strong>
                    </div>
                    <div class="timeline-events">
                      {#each month.events as event}
                        <button class:selected={selectedEvent?.id === event.id} onclick={() => selectEvent(event)}>
                          {chronicleLine(event)}
                        </button>
                      {/each}
                    </div>
                  </section>
                {/each}
              </div>

              {#if selectedEvent}
                <article class="event-detail">
                  <p class="kicker">{selectedEvent.severity}</p>
                  <h3>{formatLabel(selectedEvent.event_type)}</h3>
                  <p>{selectedEvent.summary}</p>
                  {#if selectedEvent.causes.length}
                    <div class="event-detail-list">
                      <strong>Causes</strong>
                      {#each selectedEvent.causes as cause}
                        <span>{cause}</span>
                      {/each}
                    </div>
                  {/if}
                  {#if selectedEvent.consequences.length}
                    <div class="event-detail-list">
                      <strong>Consequences</strong>
                      {#each selectedEvent.consequences as consequence}
                        <span>{consequence}</span>
                      {/each}
                    </div>
                  {/if}
                  {#if selectedEvent.caused_by?.length}
                    <div class="event-detail-list">
                      <strong>Related events</strong>
                      {#each selectedEvent.caused_by as relatedId}
                        <button type="button" class="related-event" onclick={() => focusRelatedEvent(relatedId)}>
                          Event #{relatedId}
                        </button>
                      {/each}
                    </div>
                  {/if}
                  {#if selectedEvent.tags.length}
                    <div class="tag-row">
                      {#each selectedEvent.tags as tag}
                        <span>{tag}</span>
                      {/each}
                    </div>
                  {/if}
                </article>
              {/if}
            </section>
          {:else if activeWorldTab === 'regions'}
            <section class="tab-panel regions-panel">
              <div class="panel-heading">
                <p class="kicker">Regions</p>
                <h2>{selectedRegion ? selectedRegion.name : 'Regional Index'}</h2>
              </div>

              {#if selectedRegion && selectedRegionSummary}
                <article class="region-detail">
                  <dl class="detail-grid">
                    <div>
                      <dt>Biome</dt>
                      <dd>{selectedRegion.biome}</dd>
                    </div>
                    <div>
                      <dt>Climate</dt>
                      <dd>{selectedRegion.climate}</dd>
                    </div>
                    <div>
                      <dt>Population</dt>
                      <dd>{selectedRegionSummary.population.toLocaleString()}</dd>
                    </div>
                    <div>
                      <dt>Capacity</dt>
                      <dd>{selectedRegion.carrying_capacity.toLocaleString()}</dd>
                    </div>
                  </dl>

                  <div class="panel-section">
                    <h3>Neighbors</h3>
                    <div class="tag-row">
                      {#each selectedRegion.neighbors as neighborId}
                        <button class="chip-button" onclick={() => selectRegion(neighborId)}>{regionName(neighborId)}</button>
                      {/each}
                    </div>
                  </div>

                  <div class="panel-section">
                    <h3>Settlements</h3>
                    {#if selectedRegionSummary.settlements.length}
                      <div class="settlement-list">
                        {#each selectedRegionSummary.settlements as settlement}
                          <article>
                            <strong>{settlement.name}</strong>
                            <small>
                              {settlementPopulationLabel(settlement)} · {settlementStatusLabel(settlement.status)} · stability {settlement.stability}
                            </small>
                            <small>
                              {polityName(settlement.polity)}
                            </small>
                          </article>
                        {/each}
                      </div>
                    {:else}
                      <p class="empty-state">No settlements recorded in this region.</p>
                    {/if}
                  </div>

                  {#if selectedRegionSummary.unsettledPopulation > 0}
                    <div class="panel-section">
                      <h3>Unsettled Population</h3>
                      <p class="note-copy">
                        {selectedRegionSummary.unsettledPopulation.toLocaleString()} people remain in the region outside an active settlement.
                      </p>
                    </div>
                  {/if}

                  {#if selectedRegionSummary.polities.length}
                    <div class="panel-section">
                      <h3>Political Presence</h3>
                      <div class="tag-row">
                        {#each selectedRegionSummary.polities as polity}
                          <span>{polity.name}</span>
                        {/each}
                      </div>
                    </div>
                  {/if}
                </article>
              {:else}
                <p class="empty-state">Pick a region on the atlas to inspect its biome, settlements, and political presence.</p>
              {/if}

              <div class="panel-section region-index">
                <h3>All Regions</h3>
                <div class="compact-list panel-scroll">
                  {#each regionSummaries as summary}
                    <button class:selected={selectedRegionId === summary.region.id} onclick={() => selectRegion(summary.region.id)}>
                      <span>{summary.region.name}</span>
                      <small>{summary.region.biome} · {summary.population.toLocaleString()} people</small>
                    </button>
                  {/each}
                </div>
              </div>
            </section>
          {:else if activeWorldTab === 'polities'}
            <section class="tab-panel">
              <div class="panel-heading">
                <p class="kicker">Polities</p>
                <h2>Civilization Systems</h2>
              </div>

              {#if politySummaries.length}
                <div class="compact-list panel-scroll polity-list">
                  {#each politySummaries as summary}
                    <article>
                      <strong>{summary.polity.name}</strong>
                      <small>
                        {summary.population.toLocaleString()} people · cohesion {summary.polity.cohesion}
                        {#if summary.capital}
                          · capital {summary.capital.name}
                        {/if}
                      </small>
                      <span>{summary.polity.controlled_regions.length} regions · {summary.settlements.length} settlements</span>
                    </article>
                  {/each}
                </div>
              {:else}
                <p class="empty-state">No active polities exist in this world state.</p>
              {/if}
            </section>
          {/if}
        </aside>
      </section>
    </section>
  {/if}

  {#if toast}
    <aside class:success={toast.tone === 'success'} class:error-toast={toast.tone === 'error'} class="toast">
      {toast.message}
    </aside>
  {/if}
</main>
