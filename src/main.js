import init, { Session } from '../core/pkg/sand_simulation.js';
import { addons } from './addons.js';
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

const selected = restoreSelection();
const checkboxes = addons.map(addon => {
  const label = document.createElement('label');
  const checkbox = document.createElement('input');
  checkbox.type = 'checkbox';
  checkbox.checked = selected.has(addon.id);
  label.append(checkbox, addon.name);
  choices.append(label);
  return checkbox;
});

function selectedAddons() {
  return addons.filter((_, index) => checkboxes[index].checked);
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
    const response = await fetch(import.meta.env.BASE_URL + name);
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

function draw(time) {
  if (previousTime !== undefined) {
    session.advance((time - previousTime) / 1000);
  }
  previousTime = time;

  context.clearRect(0, 0, canvas.width, canvas.height);
  for (const sprite of session.snapshot()) {
    context.fillStyle = sprite.color;
    context.beginPath();
    context.arc(sprite.x, sprite.y, sprite.radius, 0, Math.PI * 2);
    context.fill();
  }
  frame = requestAnimationFrame(draw);
}

form.addEventListener('submit', async event => {
  event.preventDefault();
  if (launch.disabled) return;

  const files = ['config/base.yaml', ...selectedAddons().map(addon => addon.file)];
  fields.disabled = true;
  launch.disabled = true;
  errorMessage.hidden = true;
  try {
    await init();
    const layers = await Promise.all(files.map(loadLayer));
    const next = new Session(layers);
    cancelAnimationFrame(frame);
    session?.free();
    session = next;
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
