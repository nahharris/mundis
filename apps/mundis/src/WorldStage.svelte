<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { renderAtlas, updateAtlasSelection } from './atlas';
  import type { AtlasState } from './types';

  export let atlas: AtlasState | null = null;
  export let selectedRegionId: number | null = null;
  export let onRegionSelect: (regionId: number) => void = () => {};

  let host: HTMLDivElement;
  let resizeObserver: ResizeObserver | null = null;

  async function draw() {
    if (host && atlas) {
      await renderAtlas(host, atlas, { onRegionSelect });
      updateAtlasSelection(selectedRegionId);
    }
  }

  onMount(() => {
    resizeObserver = new ResizeObserver(() => {
      void draw();
    });
    resizeObserver.observe(host);
    void draw();
  });

  $: if (atlas) {
    void draw();
  }

  $: updateAtlasSelection(selectedRegionId);

  onDestroy(() => {
    resizeObserver?.disconnect();
  });
</script>

<div class="world-stage" bind:this={host}></div>
