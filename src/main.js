import init, { Session } from '../core/pkg/sand_simulation.js';
import { orderAddons } from './addons.js';
import './style.css';

const storageKey = 'cats-of-sands.addons';
const form = document.querySelector('#launch');
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

async function showAddons() {
  try {
    const response = await fetch(import.meta.env.BASE_URL + 'config/addons.json');
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    const catalog = await response.json();
    const selected = restoreSelection();
    addonChoices = catalog.addons.map(addon => {
      const label = document.createElement('label');
      const checkbox = document.createElement('input');
      checkbox.type = 'checkbox';
      checkbox.checked = selected.has(addon.id);
      label.append(checkbox, addon.name);
      choices.append(label);
      return { addon, checkbox };
    });
    launch.disabled = false;
  } catch (error) {
    errorMessage.textContent = `Каталог аддонов: ${error.message}`;
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

async function loadLayer(name) {
  try {
    const path = name.split('/').map(encodeURIComponent).join('/');
    const response = await fetch(import.meta.env.BASE_URL + path);
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
let drawnEntities = [];

canvas.addEventListener('click', event => {
  const bounds = canvas.getBoundingClientRect();
  const x = (event.clientX - bounds.left) * canvas.width / bounds.width;
  const y = (event.clientY - bounds.top) * canvas.height / bounds.height;
  const target = drawnEntities.findLast(({ position, sprite }) => Math.hypot(x - position.x, y - position.y) <= sprite.radius);
  if (target) {
    session.click(target.id);
  }
});

function draw(time) {
  if (previousTime !== undefined) {
    session.advance((time - previousTime) / 1000);
    session.drain_events();
  }
  previousTime = time;

  context.clearRect(0, 0, canvas.width, canvas.height);
  drawnEntities = session.snapshot().filter(entity => entity.position !== undefined && entity.sprite !== undefined);
  for (const { position, sprite } of drawnEntities) {
    context.fillStyle = sprite.color;
    context.beginPath();
    context.arc(position.x, position.y, sprite.radius, 0, Math.PI * 2);
    context.fill();
  }
  frame = requestAnimationFrame(draw);
}

form.addEventListener('submit', async event => {
  event.preventDefault();
  if (launch.disabled) return;

  fields.disabled = true;
  launch.disabled = true;
  errorMessage.hidden = true;
  try {
    const addons = orderAddons(selectedAddons());
    const files = ['config/units.yaml', 'config/base.yaml', ...addons.map(addon => addon.path)];
    await init();
    const layers = await Promise.all(files.map(loadLayer));
    const next = new Session(layers);
    cancelAnimationFrame(frame);
    session?.free();
    session = next;
    drawnEntities = [];
    previousTime = undefined;
    canvas.hidden = false;
    frame = requestAnimationFrame(draw);
  } catch (error) {
    errorMessage.textContent = error.message;
    errorMessage.hidden = false;
  } finally {
    fields.disabled = false;
    launch.disabled = false;
  }
});

showAddons();
