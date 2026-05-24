import { Application, Container, Graphics, Text } from 'pixi.js';
import type { AtlasState } from './types';

let app: Application | undefined;
let selectionRing: Graphics | undefined;
let regionPositions = new Map<number, RegionPosition>();

type RegionPosition = {
  id: number;
  x: number;
  y: number;
  angle: number;
};

type AtlasRenderOptions = {
  onRegionSelect?: (regionId: number) => void;
};

export async function renderAtlas(target: HTMLElement, atlas: AtlasState, options: AtlasRenderOptions = {}): Promise<void> {
  target.replaceChildren();
  app?.destroy(true);

  app = new Application();
  await app.init({
    width: target.clientWidth || 960,
    height: target.clientHeight || 620,
    backgroundColor: 0x111714,
    antialias: true,
    resolution: window.devicePixelRatio || 1,
    autoDensity: true
  });
  target.appendChild(app.canvas);

  const stage = new Container();
  app.stage.addChild(stage);

  const width = app.screen.width;
  const height = app.screen.height;
  const centerX = width / 2;
  const centerY = height / 2;
  const radius = Math.min(width, height) * 0.34;
  const positions = atlas.regions.map((region, index) => {
    const angle = (Math.PI * 2 * index) / Math.max(1, atlas.regions.length) - Math.PI / 2;
    const wobble = index % 2 === 0 ? 0.92 : 1.08;
    return {
      id: region.id,
      x: centerX + Math.cos(angle) * radius * wobble,
      y: centerY + Math.sin(angle) * radius * wobble,
      angle
    };
  });
  const byId = new Map(positions.map((position) => [position.id, position]));
  regionPositions = byId;

  stage.addChild(drawBackdrop(width, height));
  stage.addChild(drawRoads(atlas, byId));
  selectionRing = new Graphics();

  const animatedNodes: Graphics[] = [];
  for (const region of atlas.regions) {
    const position = byId.get(region.id);
    if (!position) continue;

    const territory = drawTerritory(position, biomeTint(region.biome), region.id);
    territory.on('pointertap', () => {
      options.onRegionSelect?.(region.id);
    });
    stage.addChild(territory);
    animatedNodes.push(territory);

    const label = new Text({
      text: region.name,
      style: {
        fill: 0xf7efd8,
        fontFamily: 'Georgia, serif',
        fontSize: 14,
        align: 'center',
        dropShadow: { color: 0x09100d, blur: 3, distance: 2, alpha: 0.85 }
      }
    });
    label.anchor.set(0.5, 0);
    label.x = position.x;
    label.y = position.y + 48;
    stage.addChild(label);
  }

  for (const settlement of atlas.settlements) {
    const position = byId.get(settlement.region);
    if (!position) continue;
    const marker = new Graphics()
      .rect(-9, -9, 18, 18)
      .fill({ color: settlement.polity === null ? 0xf2c96d : 0xe37555 })
      .rect(-9, -9, 18, 18)
      .stroke({ width: 2, color: 0x101411 })
      .moveTo(-5, 0)
      .lineTo(5, 0)
      .moveTo(0, -5)
      .lineTo(0, 5)
      .stroke({ width: 2, color: 0x3b2718, alpha: 0.75 });
    marker.x = position.x;
    marker.y = position.y;
    marker.rotation = Math.PI / 4;
    stage.addChild(marker);
    animatedNodes.push(marker);
  }

  stage.addChild(selectionRing);

  app.ticker.add((ticker) => {
    const time = ticker.lastTime / 1000;
    for (const [index, node] of animatedNodes.entries()) {
      node.alpha = 0.86 + Math.sin(time * 1.8 + index) * 0.08;
      node.scale.set(1 + Math.sin(time * 1.25 + index * 0.7) * 0.018);
    }
  });
}

export function updateAtlasSelection(selectedRegionId: number | null): void {
  selectionRing?.clear();
  if (selectedRegionId === null || !selectionRing) return;

  const position = regionPositions.get(selectedRegionId);
  if (!position) return;

  selectionRing
    .circle(position.x, position.y, 58)
    .stroke({ width: 3, color: 0xf2c96d, alpha: 0.86 })
    .circle(position.x, position.y, 47)
    .stroke({ width: 2, color: 0xffedb8, alpha: 0.42 });
}

function drawBackdrop(width: number, height: number): Graphics {
  const graphics = new Graphics()
    .rect(0, 0, width, height)
    .fill({ color: 0x111714 })
    .circle(width * 0.5, height * 0.48, Math.min(width, height) * 0.43)
    .fill({ color: 0x1f2e28, alpha: 0.82 })
    .circle(width * 0.5, height * 0.48, Math.min(width, height) * 0.35)
    .stroke({ width: 2, color: 0xd8b36a, alpha: 0.14 });

  for (let index = 0; index < 20; index += 1) {
    const angle = (Math.PI * 2 * index) / 20;
    graphics
      .moveTo(width / 2, height / 2)
      .lineTo(width / 2 + Math.cos(angle) * width, height / 2 + Math.sin(angle) * width)
      .stroke({ width: 1, color: 0xd8b36a, alpha: 0.035 });
  }
  return graphics;
}

function drawRoads(atlas: AtlasState, byId: Map<number, RegionPosition>): Graphics {
  const roads = new Graphics();
  for (const region of atlas.regions) {
    const from = byId.get(region.id);
    if (!from) continue;
    for (const neighborId of region.neighbors) {
      const to = byId.get(neighborId);
      if (!to || neighborId < region.id) continue;
      roads.moveTo(from.x, from.y);
      roads.bezierCurveTo(
        (from.x + to.x) / 2,
        (from.y + to.y) / 2 - 34,
        (from.x + to.x) / 2,
        (from.y + to.y) / 2 + 34,
        to.x,
        to.y
      );
      roads.stroke({ width: 5, color: 0x0b0f0d, alpha: 0.55 });
      roads.stroke({ width: 2, color: 0xd0a767, alpha: 0.78 });
    }
  }
  return roads;
}

function drawTerritory(position: RegionPosition, tint: number, index: number): Graphics {
  const sides = 7;
  const graphics = new Graphics();
  graphics.moveTo(Math.cos(position.angle) * 43, Math.sin(position.angle) * 43);
  for (let side = 0; side <= sides; side += 1) {
    const angle = position.angle + (Math.PI * 2 * side) / sides;
    const radius = 40 + ((side + index) % 3) * 6;
    graphics.lineTo(Math.cos(angle) * radius, Math.sin(angle) * radius);
  }
  graphics
    .fill({ color: tint, alpha: 0.94 })
    .stroke({ width: 4, color: 0x0b0f0d, alpha: 0.88 })
    .circle(0, 0, 18)
    .fill({ color: 0xf4dc9a, alpha: 0.2 });
  graphics.x = position.x;
  graphics.y = position.y;
  graphics.eventMode = 'static';
  graphics.cursor = 'pointer';
  graphics.on('pointerover', () => {
    graphics.alpha = 1;
    graphics.scale.set(1.08);
  });
  graphics.on('pointerout', () => {
    graphics.scale.set(1);
  });
  return graphics;
}

function biomeTint(biome: string): number {
  if (biome === 'Desert') return 0xc89b55;
  if (biome === 'Forest') return 0x4f7d59;
  if (biome === 'Tundra') return 0x8ca9ad;
  return 0x78965f;
}
