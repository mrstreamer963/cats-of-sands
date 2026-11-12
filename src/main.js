import { createGame } from './game.js';
import { setupLaunch } from './launch.js';
import { downloadSave, loadSave } from './save-file.js';
import './style.css';

const main = document.querySelector('main');
const launchScreen = document.querySelector('#launch-screen');
const game = document.querySelector('#game');
const canvas = game.querySelector('canvas');
const overlay = document.querySelector('#game-overlay');
const pauseMenu = document.querySelector('#pause-menu');
const settingsMenu = document.querySelector('#settings-menu');
const openSettings = document.querySelector('#open-settings');
const showPlannedPaths = document.querySelector('#show-planned-paths');
const loadMenu = document.querySelector('#load-menu');
const confirmMenu = document.querySelector('#confirm-menu');
const confirmMessage = document.querySelector('#confirm-message');
const gameError = document.querySelector('#game-error');
const saveFile = document.querySelector('#save-file');
const mapChoice = document.querySelector('#map');

let gameState = 'menu';
let confirmationAction;
let confirmationReturnState = 'paused';
let pendingImport;
const plannedPathsKey = 'cats-of-sands.show-planned-paths';
try {
  showPlannedPaths.checked = localStorage.getItem(plannedPathsKey) === 'true';
} catch {
  // The checkbox still works for the current page if storage is unavailable.
}
const runtime = createGame(canvas, () => gameState === 'playing', () => showPlannedPaths.checked);

function showGameState(next) {
  gameState = next;
  const inGame = next !== 'menu';
  launchScreen.hidden = inGame;
  game.hidden = !inGame;
  main.classList.toggle('in-game', inGame);
  canvas.inert = next !== 'playing';
  overlay.hidden = !inGame || next === 'playing';
  pauseMenu.hidden = next !== 'paused';
  settingsMenu.hidden = next !== 'settings';
  loadMenu.hidden = next !== 'loading-file';
  confirmMenu.hidden = !next.startsWith('confirm-');
}

function pauseGame() {
  if (gameState !== 'playing') return;
  runtime.resetInput();
  showGameState('paused');
  document.querySelector('#resume').focus();
}

function resumeGame() {
  if (gameState !== 'paused') return;
  runtime.resetInput();
  showGameState('playing');
  canvas.focus();
}

function closeSettings() {
  showGameState('paused');
  openSettings.focus();
}

function askConfirmation({ title, message, action, returnState = 'paused', confirmState = 'confirm-exit' }) {
  confirmationAction = action;
  confirmationReturnState = returnState;
  document.querySelector('#confirm-title').textContent = title;
  confirmMessage.textContent = message;
  showGameState(confirmState);
  document.querySelector('#confirm-yes').focus();
}

function saveGame() {
  try {
    downloadSave(runtime.snapshot());
    gameError.textContent = 'Сохранение скачано.';
    gameError.hidden = false;
  } catch (error) {
    gameError.textContent = `Не удалось сохранить игру: ${error.message}`;
    gameError.hidden = false;
  }
}

async function readSavedFile(file) {
  if (!file) return;
  pendingImport?.session?.free();
  pendingImport = undefined;
  try {
    pendingImport = await loadSave(file);
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

function installSession(session, map, presentation) {
  runtime.installSession(session, map, presentation);
  gameError.hidden = true;
  showGameState('playing');
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
  runtime.clearSession();
  gameError.hidden = true;
  showGameState('menu');
  mapChoice.focus();
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
  } else if (gameState === 'settings') {
    closeSettings();
  } else if (gameState === 'loading-file') {
    showGameState('paused');
    document.querySelector('#open-load').focus();
  } else if (gameState.startsWith('confirm-')) {
    cancelConfirmation();
  }
});

document.querySelector('#resume').addEventListener('click', resumeGame);
openSettings.addEventListener('click', () => {
  showGameState('settings');
  showPlannedPaths.focus();
});
document.querySelector('#close-settings').addEventListener('click', closeSettings);
showPlannedPaths.addEventListener('change', () => {
  try {
    localStorage.setItem(plannedPathsKey, String(showPlannedPaths.checked));
  } catch {
    // Keep the current page's choice if storage is unavailable.
  }
});
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

setupLaunch(installSession);
