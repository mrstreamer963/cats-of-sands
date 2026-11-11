import init, { Session, tiled_reference } from '../core/pkg/sand_simulation.js';
import {
  addonCatalogPath, addonRoot, checkAddonId, orderAddons, resourceManifest,
  validateAddonCatalog, validateAddonManifest, validateBaseManifest,
} from './addons.js';
import { Camera } from './camera.js';
import { drawTiled, loadTiled, relativeResource } from './tiled.js';
import './style.css';

const dataRoot = 'data/';
const storageKey = 'cats-of-sands.addons';
const form = document.querySelector('#launch');
const mapChoice = form.querySelector('#map');
const choices = document.querySelector('#addons');
const fields = form.querySelector('fieldset');
const launch = form.querySelector('button');
const errorMessage = document.querySelector('#error');
const canvas = document.querySelector('canvas');
const context = canvas.getContext('2d');

function restoreSelection() {
  try {
    const ids = JSON.parse(localStorage.getItem(storageKey));
    return new Set(Array.isArray(ids) ? ids : []);
  } catch {
    return new Set();
  }
}

let addonChoices = [];
let base;

async function loadJson(name, validate) {
  const { text } = await loadLayer(name);
  try {
    return validate(JSON.parse(text));
  } catch (error) {
    throw new Error(`${name}: ${error.message}`);
  }
}

async function loadAddons() {
  const folders = await loadJson(addonCatalogPath, validateAddonCatalog);
  const addons = await Promise.all(folders.map(async folder => {
    const root = `${addonRoot}/${folder}`;
    const manifestPath = root + '/' + resourceManifest;
    const manifest = await loadJson(manifestPath, validateAddonManifest);
    return { ...manifest, layers: manifest.layers.map(path => root + '/' + path), manifestPath };
  }));
  const ids = new Set();
  for (const addon of addons) {
    try {
      checkAddonId(ids, addon.id);
    } catch (error) {
      throw new Error(`${addon.manifestPath}: ${error.message}`);
    }
  }
  return addons;
}

async function showLaunchOptions() {
  try {
    const [manifest, addons] = await Promise.all([
      loadJson(dataRoot + resourceManifest, validateBaseManifest),
      loadAddons(),
    ]);
    base = manifest;
    for (const path of base.maps) {
      const option = document.createElement('option');
      option.value = path;
      option.textContent = path.slice('maps/'.length).replace(/\.ya?ml$/i, '');
      mapChoice.append(option);
    }
    const selected = restoreSelection();
    addonChoices = addons.map(addon => {
      const label = document.createElement('label');
      const checkbox = document.createElement('input');
      checkbox.type = 'checkbox';
      checkbox.checked = selected.has(addon.id);
      label.append(checkbox, addon.name);
      choices.append(label);
      return { addon, checkbox };
    });
    mapChoice.disabled = false;
    launch.disabled = false;
  } catch (error) {
    errorMessage.textContent = error.message;
    errorMessage.hidden = false;
  }
}

function selectedAddons() {
  return addonChoices.filter(({ checkbox }) => checkbox.checked).map(({ addon }) => addon);
}

choices.addEventListener('change', () => {
  try {
    localStorage.setItem(storageKey, JSON.stringify(selectedAddons().map(addon => addon.id)));
  } catch {
    // The checkboxes retain the selection while the page is open.
  }
});

function resourceUrl(name) {
  return import.meta.env.BASE_URL + name.split('/').map(encodeURIComponent).join('/');
}

async function loadImage(name) {
  const image = new Image();
  image.src = resourceUrl(name);
  try {
    await image.decode();
    return image;
  } catch {
    throw new Error(`Не удалось загрузить атлас ${name}`);
  }
}

async function loadLayer(name) {
  try {
    const response = await fetch(resourceUrl(name));
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    return { name, text: await response.text() };
  } catch (error) {
    throw new Error(`${name}: ${error.message}`);
  }
}

let session;
let frame;
let previousTime;
let renderTick;
let renderEntities = [];
let drawnEntities = [];
const selectedIds = new Set();
const sceneWidth = canvas.width;
const sceneHeight = canvas.height;
let tiledMap;
let camera;
let renderedView;
let walls = [];
let drag;
let suppressClick = false;

function canvasPoint(event) {
  const bounds = canvas.getBoundingClientRect();
  return {
    x: (event.clientX - bounds.left) * canvas.width / bounds.width,
    y: (event.clientY - bounds.top) * canvas.height / bounds.height,
  };
}

canvas.addEventListener('wheel', event => {
  if (!camera?.fit) return;
  event.preventDefault();
  const point = canvasPoint(event);
  const delta = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 120 : 1);
  camera.zoom(point.x, point.y, Math.exp(-delta * 0.0015));
}, { passive: false });

canvas.addEventListener('pointerdown', event => {
  if (!camera?.fit || event.button !== 0 || !event.isPrimary) return;
  suppressClick = false;
  drag = { id: event.pointerId, x: event.clientX, y: event.clientY };
  canvas.setPointerCapture(event.pointerId);
});

canvas.addEventListener('pointermove', event => {
  if (drag?.id !== event.pointerId) return;
  const dx = event.clientX - drag.x;
  const dy = event.clientY - drag.y;
  if (!suppressClick && Math.hypot(dx, dy) < 4) return;
  suppressClick = true;
  const bounds = canvas.getBoundingClientRect();
  camera.pan(dx * canvas.width / bounds.width, dy * canvas.height / bounds.height);
  drag.x = event.clientX;
  drag.y = event.clientY;
});

canvas.addEventListener('lostpointercapture', event => {
  if (drag?.id === event.pointerId) drag = undefined;
});

canvas.addEventListener('click', event => {
  if (!session || suppressClick) return;
  const point = canvasPoint(event);
  if (camera && !renderedView) return;
  const { x, y } = renderedView ? renderedView.world(point.x, point.y) : point;
  if (tiledMap && (x < 0 || y < 0 || x >= tiledMap.width || y >= tiledMap.height)) return;
  const target = drawnEntities.findLast(({ position, sprite }) => Math.hypot(x - position.x, y - position.y) <= sprite.radius);
  if (target) {
    if (event.shiftKey) {
      if (!selectedIds.delete(target.id)) selectedIds.add(target.id);
    } else {
      selectedIds.clear();
      selectedIds.add(target.id);
      session.click(target.id);
    }
  } else if (selectedIds.size > 0) {
    session.move_group_to(Uint32Array.from(selectedIds), x, y);
  }
});

function draw(time) {
  if (previousTime !== undefined) {
    session.advance((time - previousTime) / 1000);
    session.drain_events();
  }
  previousTime = time;

  const tick = session.current_tick();
  if (tick !== renderTick) {
    renderEntities = session.render_snapshot();
    if (tiledMap) walls = session.wall_snapshot();
    renderTick = tick;
  }
  const alpha = session.interpolation_alpha();
  drawnEntities = renderEntities.map(({ id, position, previous_position: previous, sprite }) => ({
    id,
    sprite,
    position: {
      // A wrap across the scene edge should not sweep through the canvas.
      x: !tiledMap && Math.abs(position.x - previous.x) > sceneWidth / 2
        ? position.x
        : previous.x + (position.x - previous.x) * alpha,
      y: previous.y + (position.y - previous.y) * alpha,
    },
  }));

  if (camera) {
    const bounds = canvas.getBoundingClientRect();
    const width = Math.max(1, Math.round(bounds.width * devicePixelRatio));
    const height = Math.max(1, Math.round(bounds.height * devicePixelRatio));
    if (canvas.width !== width) canvas.width = width;
    if (canvas.height !== height) canvas.height = height;
    camera.resize(width, height);
  }
  context.resetTransform();
  context.clearRect(0, 0, canvas.width, canvas.height);
  if (camera) {
    renderedView = Object.assign(new Camera(tiledMap), camera);
    context.imageSmoothingEnabled = false;
    drawTiled(context, tiledMap, walls, camera);
    context.setTransform(camera.scale, 0, 0, camera.scale, camera.x, camera.y);
  }
  for (const { id, position, sprite } of drawnEntities) {
    context.fillStyle = sprite.color;
    context.beginPath();
    context.arc(position.x, position.y, sprite.radius, 0, Math.PI * 2);
    context.fill();
    if (selectedIds.has(id)) {
      context.strokeStyle = '#ffffff';
      context.lineWidth = camera ? devicePixelRatio / camera.scale : 1;
      context.stroke();
    }
  }
  frame = requestAnimationFrame(draw);
}

form.addEventListener('submit', async event => {
  event.preventDefault();
  if (launch.disabled) return;

  fields.disabled = true;
  mapChoice.disabled = true;
  launch.disabled = true;
  errorMessage.hidden = true;
  try {
    const addons = orderAddons(selectedAddons());
    const files = [
      ...base.layers.map(path => dataRoot + path),
      dataRoot + mapChoice.value,
      ...addons.flatMap(addon => addon.layers),
    ];
    await init();
    const layers = await Promise.all(files.map(loadLayer));
    const selectedLayer = layers[base.layers.length];
    let reference;
    try {
      reference = tiled_reference(selectedLayer.text);
      if (reference) reference = relativeResource(selectedLayer.name, reference);
    } catch (error) {
      throw new Error(`${selectedLayer.name}: ${error.message}`);
    }
    const nextMap = reference ? await loadTiled(reference, loadLayer, loadImage) : undefined;
    let next;
    try {
      next = new Session(layers, nextMap?.grid);
    } catch (error) {
      throw new Error(`${selectedLayer.name}: ${error.message}`);
    }
    cancelAnimationFrame(frame);
    session?.free();
    session = next;
    tiledMap = nextMap;
    camera = nextMap ? new Camera(nextMap) : undefined;
    renderedView = undefined;
    drag = undefined;
    suppressClick = false;
    canvas.classList.toggle('tile-map', Boolean(camera));
    if (!camera) {
      canvas.width = sceneWidth;
      canvas.height = sceneHeight;
    }
    drawnEntities = [];
    selectedIds.clear();
    renderTick = undefined;
    previousTime = undefined;
    canvas.hidden = false;
    frame = requestAnimationFrame(draw);
  } catch (error) {
    errorMessage.textContent = error.message;
    errorMessage.hidden = false;
  } finally {
    fields.disabled = false;
    mapChoice.disabled = false;
    launch.disabled = false;
  }
});

showLaunchOptions();
