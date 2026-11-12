import { Camera } from './camera.js';
import { drawTiled } from './tiled.js';

export function setupMapPreview(mapChoice, loadMap) {
  const canvas = document.querySelector('#map-preview');
  const context = canvas.getContext('2d');
  const status = document.querySelector('#map-preview-status');
  const name = document.querySelector('#map-preview-name');
  let request = 0;

  return async function updatePreview() {
    const current = ++request;
    name.textContent = mapChoice.selectedOptions[0]?.textContent ?? '';
    canvas.hidden = true;
    status.hidden = false;
    status.textContent = 'Загрузка карты…';
    try {
      const map = await loadMap(mapChoice.value);
      if (current !== request) return;
      if (!map) {
        status.textContent = 'У этой карты нет тайлового превью';
        return;
      }

      const camera = new Camera(map);
      camera.resize(canvas.width, canvas.height);
      const walls = [];
      map.grid.walls.forEach((tile, cell) => {
        if (tile) walls.push([cell, tile.gid]);
      });
      context.clearRect(0, 0, canvas.width, canvas.height);
      context.imageSmoothingEnabled = false;
      drawTiled(context, map, walls, camera);
      canvas.hidden = false;
      status.hidden = true;
    } catch {
      if (current === request) status.textContent = 'Не удалось показать карту';
    }
  };
}
