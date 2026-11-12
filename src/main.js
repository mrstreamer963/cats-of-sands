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
const main = document.querySelector('main');
const launchScreen = document.querySelector('#launch-screen');
const form = document.querySelector('#launch');
const mapChoice = form.querySelector('#map');
const choices = document.querySelector('#addons');
const fields = form.querySelector('fieldset');
const launch = form.querySelector('button');
const errorMessage = document.querySelector('#error');
const game = document.querySelector('#game');
const canvas = game.querySelector('canvas');
const context = canvas.getContext('2d');
const overlay = document.querySelector('#game-overlay');
const pauseMenu = document.querySelector('#pause-menu');
const loadMenu = document.querySelector('#load-menu');
const confirmMenu = document.querySelector('#confirm-menu');
const confirmMessage = document.querySelector('#confirm-message');
const gameError = document.querySelector('#game-error');
const saveFile = document.querySelector('#save-file');

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

function cameraSnapshot() {
  if (!camera?.fit || !camera.width || !camera.height) return null;
  const center = camera.world(camera.width / 2, camera.height / 2);
  return { center, zoom: camera.scale / camera.fit };
}

function saveGame() {
  try {
    const file = {
      format: 'cats-of-sands-save',
      version: 1,
      session: session.export_snapshot(),
      selectedIds: [...selectedIds],
      camera: cameraSnapshot(),
    };
    const blob = new Blob([JSON.stringify(file, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `cats-of-sands-${new Date().toISOString().replaceAll(':', '-')}.json`;
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    gameError.textContent = 'Сохранение скачано.';
    gameError.hidden = false;
  } catch (error) {
    gameError.textContent = `Не удалось сохранить игру: ${error.message}`;
    gameError.hidden = false;
  }
}

async function savedMap(snapshot) {
  const layers = snapshot.session?.layers;
  if (!Array.isArray(layers)) throw new Error('В файле отсутствуют слои игры');
  if (layers.some(({ name, text }) => typeof text !== 'string' ||
      typeof name !== 'string' || !/^(?:data|addons)\//.test(name) ||
      name.split('/').some(segment => !segment || segment === '.' || segment === '..' || /[\\?#\u0000-\u001f\u007f]/u.test(segment)))) {
    throw new Error('В файле указан недопустимый путь к слою игры');
  }
  for (const layer of [...layers].reverse()) {
    let reference;
    try {
      reference = tiled_reference(layer.text);
    } catch {
      continue;
    }
    if (reference) {
      return loadTiled(relativeResource(layer.name, reference), loadLayer, loadImage);
    }
  }
  return undefined;
}

function validateSavedPresentation(snapshot, nextSession) {
  if (!Array.isArray(snapshot.selectedIds) ||
      snapshot.selectedIds.some(id => !Number.isInteger(id) || id < 1)) {
    throw new Error('В файле повреждён список выделенных юнитов');
  }
  const liveIds = new Set(nextSession.snapshot().map(entity => entity.id));
  if (snapshot.selectedIds.some(id => !liveIds.has(id))) {
    throw new Error('В файле выделены отсутствующие юниты');
  }
  if (snapshot.camera !== null && snapshot.camera !== undefined) {
    const { center, zoom } = snapshot.camera;
    if (!center || !Number.isFinite(center.x) || !Number.isFinite(center.y) ||
        !Number.isFinite(zoom) || zoom < 1 || zoom > 8) {
      throw new Error('В файле повреждено состояние камеры');
    }
  }
}

async function readSavedFile(file) {
  if (!file) return;
  pendingImport?.session?.free();
  pendingImport = undefined;
  try {
    const snapshot = JSON.parse(await file.text());
    if (!snapshot || snapshot.format !== 'cats-of-sands-save' || snapshot.version !== 1) {
      throw new Error('Неизвестный формат или версия файла сохранения');
    }
    await init();
    const nextSession = Session.from_snapshot(snapshot.session);
    try {
      const nextMap = await savedMap(snapshot);
      validateSavedPresentation(snapshot, nextSession);
      pendingImport = { session: nextSession, map: nextMap, snapshot };
    } catch (error) {
      nextSession.free();
      throw error;
    }
    askConfirmation({
      title: 'Заменить текущую партию?',
      message: 'Текущая партия будет заменена выбранным файлом.',
      action: loadPendingImport,
      confirmState: 'confirm-load',
    });
  } catch (error) {
    gameError.textContent = `Не удалось загрузить игру: ${error.message}`;
    gameError.hidden = false;
    showGameState('paused');
    document.querySelector('#open-load').focus();
  } finally {
    saveFile.value = '';
  }
}

function installSession(nextSession, nextMap, presentation = {}) {
  cancelAnimationFrame(frame);
  session?.free();
  session = nextSession;
  tiledMap = nextMap;
  camera = nextMap ? new Camera(nextMap) : undefined;
  pendingCamera = presentation.camera ?? null;
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
  for (const id of presentation.selectedIds ?? []) selectedIds.add(id);
  renderTick = undefined;
  previousTime = undefined;
  gameError.hidden = true;
  showGameState('playing');
  frame = requestAnimationFrame(draw);
}

function loadPendingImport() {
  if (!pendingImport) return;
  const loaded = pendingImport;
  pendingImport = undefined;
  installSession(loaded.session, loaded.map, {
    camera: loaded.snapshot.camera,
    selectedIds: loaded.snapshot.selectedIds,
  });
  pauseGame();
}

function leaveToMainMenu() {
  cancelAnimationFrame(frame);
  session?.free();
  session = undefined;
  tiledMap = undefined;
  camera = undefined;
  pendingCamera = undefined;
  renderEntities = [];
  drawnEntities = [];
  selectedIds.clear();
  gameError.hidden = true;
  showGameState('menu');
  mapChoice.focus();
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
let gameState = 'menu';
let confirmationAction;
let confirmationReturnState = 'paused';
let pendingImport;
let pendingCamera;

function showGameState(next) {
  gameState = next;
  const inGame = next !== 'menu';
  launchScreen.hidden = inGame;
  game.hidden = !inGame;
  main.classList.toggle('in-game', inGame);
  canvas.inert = next !== 'playing';
  overlay.hidden = !inGame || next === 'playing';
  pauseMenu.hidden = next !== 'paused';
  loadMenu.hidden = next !== 'loading-file';
  confirmMenu.hidden = !next.startsWith('confirm-');
}

function pauseGame() {
  if (gameState !== 'playing') return;
  previousTime = undefined;
  drag = undefined;
  suppressClick = false;
  showGameState('paused');
  document.querySelector('#resume').focus();
}

function resumeGame() {
  if (gameState !== 'paused') return;
  previousTime = undefined;
  showGameState('playing');
  canvas.focus();
}

function askConfirmation({ title, message, action, returnState = 'paused', confirmState = 'confirm-exit' }) {
  confirmationAction = action;
  confirmationReturnState = returnState;
  document.querySelector('#confirm-title').textContent = title;
  confirmMessage.textContent = message;
  showGameState(confirmState);
  document.querySelector('#confirm-yes').focus();
}

function canvasPoint(event) {
  const bounds = canvas.getBoundingClientRect();
  return {
    x: (event.clientX - bounds.left) * canvas.width / bounds.width,
    y: (event.clientY - bounds.top) * canvas.height / bounds.height,
  };
}

canvas.addEventListener('wheel', event => {
  if (gameState !== 'playing' || !camera?.fit) return;
  event.preventDefault();
  const point = canvasPoint(event);
  const delta = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 120 : 1);
  camera.zoom(point.x, point.y, Math.exp(-delta * 0.0015));
}, { passive: false });

canvas.addEventListener('pointerdown', event => {
  if (gameState !== 'playing' || !camera?.fit || event.button !== 0 || !event.isPrimary) return;
  suppressClick = false;
  drag = { id: event.pointerId, x: event.clientX, y: event.clientY };
  canvas.setPointerCapture(event.pointerId);
});

canvas.addEventListener('pointermove', event => {
  if (gameState !== 'playing' || drag?.id !== event.pointerId) return;
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
  if (gameState !== 'playing' || !session || suppressClick) return;
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
  if (gameState === 'playing' && previousTime !== undefined) {
    session.advance((time - previousTime) / 1000);
    session.drain_events();
  }
  previousTime = gameState === 'playing' ? time : undefined;

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
    if (pendingCamera) {
      camera.scale = camera.fit * pendingCamera.zoom;
      camera.x = camera.width / 2 - pendingCamera.center.x * camera.scale;
      camera.y = camera.height / 2 - pendingCamera.center.y * camera.scale;
      camera.constrain();
      pendingCamera = undefined;
    }
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

function cancelConfirmation() {
  confirmationAction = undefined;
  pendingImport?.session?.free();
  pendingImport = undefined;
  showGameState(confirmationReturnState);
  document.querySelector('#resume').focus();
}

document.addEventListener('keydown', event => {
  if (event.key !== 'Escape' || event.repeat || gameState === 'menu') return;
  event.preventDefault();
  event.stopPropagation();
  if (gameState === 'playing') {
    pauseGame();
  } else if (gameState === 'paused') {
    resumeGame();
  } else if (gameState === 'loading-file') {
    showGameState('paused');
    document.querySelector('#open-load').focus();
  } else if (gameState.startsWith('confirm-')) {
    cancelConfirmation();
  }
});

document.querySelector('#resume').addEventListener('click', resumeGame);
document.querySelector('#save-game').addEventListener('click', saveGame);
document.querySelector('#open-load').addEventListener('click', () => {
  showGameState('loading-file');
  saveFile.focus();
});
document.querySelector('#cancel-load').addEventListener('click', () => {
  showGameState('paused');
  document.querySelector('#open-load').focus();
});
saveFile.addEventListener('change', () => readSavedFile(saveFile.files?.[0]));
document.querySelector('#open-main-menu').addEventListener('click', () => askConfirmation({
  title: 'Выйти в главное меню?',
  message: 'Текущая партия будет закрыта. Сохраните её в файл, если хотите продолжить позже.',
  action: leaveToMainMenu,
}));
document.querySelector('#confirm-yes').addEventListener('click', async () => {
  const action = confirmationAction;
  confirmationAction = undefined;
  try {
    await action?.();
  } catch (error) {
    gameError.textContent = `Не удалось выполнить действие: ${error.message}`;
    gameError.hidden = false;
    showGameState('paused');
    document.querySelector('#resume').focus();
  }
});
document.querySelector('#confirm-no').addEventListener('click', cancelConfirmation);

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
    installSession(next, nextMap);
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
