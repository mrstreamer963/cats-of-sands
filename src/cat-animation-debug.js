import { catFrame, catFrameDuration, catWalkFrameCount, createCatImages, drawCat } from './cat-animation.js';
import './cat-animation-debug.css';

const direction = document.querySelector('#direction');
const mode = document.querySelector('#mode');
const speed = document.querySelector('#speed');
const anchor = document.querySelector('#anchor');
const previous = document.querySelector('#previous');
const play = document.querySelector('#play');
const next = document.querySelector('#next');
const status = document.querySelector('#status');
const frames = document.querySelector('#frames');
const canvas = document.querySelector('#preview');
const context = canvas.getContext('2d');
const images = createCatImages();
const directions = {
  down: { row: 0, flip: false },
  up: { row: 1, flip: false },
  right: { row: 2, flip: false },
  left: { row: 2, flip: true },
};

let playing = !matchMedia('(prefers-reduced-motion: reduce)').matches;
let elapsed = 0;
let previousTime;
let frameCount;
let thumbs = [];
let displayedIndex;

function draw(time) {
  const moving = mode.value === 'walk';
  const duration = frameCount * catFrameDuration;
  if (playing && moving && previousTime !== undefined) {
    elapsed = (elapsed + (time - previousTime) * Number(speed.value)) % duration;
  }
  previousTime = time;
  const index = Math.floor(elapsed / catFrameDuration);
  if (index !== displayedIndex) {
    const frame = catFrame(images, { ...directions[direction.value], moving, elapsed }, 106);
    const x = canvas.width / 2;
    const y = canvas.height - 34;
    context.clearRect(0, 0, canvas.width, canvas.height);
    drawCat(context, frame, x, y);
    if (anchor.checked) {
      context.strokeStyle = '#a5efe4';
      context.beginPath();
      context.moveTo(x - 18, y);
      context.lineTo(x + 18, y);
      context.moveTo(x, y - 9);
      context.lineTo(x, y + 9);
      context.stroke();
    }
    if (displayedIndex !== undefined) thumbs[displayedIndex].removeAttribute('aria-current');
    thumbs[index].setAttribute('aria-current', 'true');
    const source = frame.sideStep ?? 'столбец атласа';
    status.value = `Кадр ${index + 1} из ${frameCount} · ${source} ${frame.column + 1}`;
    displayedIndex = index;
  }
  requestAnimationFrame(draw);
}

function syncControls() {
  const moving = mode.value === 'walk';
  speed.disabled = previous.disabled = play.disabled = next.disabled = !moving;
  play.textContent = playing ? 'Пауза' : 'Воспроизвести';
}

function selectFrame(index) {
  elapsed = index * catFrameDuration;
  playing = false;
  previousTime = undefined;
  syncControls();
}

function rebuildFrames() {
  const moving = mode.value === 'walk';
  const facing = directions[direction.value];
  frameCount = moving ? catWalkFrameCount : 1;
  elapsed = 0;
  previousTime = displayedIndex = undefined;
  thumbs = Array.from({ length: frameCount }, (_, index) => {
    const button = document.createElement('button');
    button.type = 'button';
    const thumbnail = document.createElement('canvas');
    thumbnail.width = 104;
    thumbnail.height = 112;
    const frame = catFrame(images, { ...facing, moving, elapsed: index * catFrameDuration }, 42);
    drawCat(thumbnail.getContext('2d'), frame, 52, 104);
    button.append(thumbnail, moving ? `Кадр ${index + 1}` : 'Стойка');
    button.addEventListener('click', () => selectFrame(index));
    return button;
  });
  frames.replaceChildren(...thumbs);
  syncControls();
}

try {
  await Promise.all(Object.values(images).map(image => image.decode()));
  direction.addEventListener('change', rebuildFrames);
  mode.addEventListener('change', rebuildFrames);
  anchor.addEventListener('change', () => {
    thumbs[displayedIndex]?.removeAttribute('aria-current');
    displayedIndex = undefined;
  });
  speed.addEventListener('change', () => { previousTime = undefined; });
  play.addEventListener('click', () => {
    playing = !playing;
    previousTime = undefined;
    syncControls();
  });
  previous.addEventListener('click', () => selectFrame((Math.floor(elapsed / catFrameDuration) - 1 + frameCount) % frameCount));
  next.addEventListener('click', () => selectFrame((Math.floor(elapsed / catFrameDuration) + 1) % frameCount));
  rebuildFrames();
  requestAnimationFrame(draw);
} catch {
  const error = document.querySelector('#error');
  error.textContent = 'Не удалось загрузить атлас кота.';
  error.hidden = false;
}
