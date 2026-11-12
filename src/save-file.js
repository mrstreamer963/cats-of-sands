import init, { Session, tiled_reference } from '../core/pkg/sand_simulation.js';
import { loadTiled, relativeResource } from './tiled.js';
import { loadLayer, loadImage } from './resources.js';

const format = 'cats-of-sands-save';
const version = 1;

export function downloadSave({ session, selectedIds, camera }) {
  const file = { format, version, session, selectedIds, camera };
  const blob = new Blob([JSON.stringify(file, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = `cats-of-sands-${new Date().toISOString().replaceAll(':', '-')}.json`;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
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

export async function loadSave(file) {
  const snapshot = JSON.parse(await file.text());
  if (!snapshot || snapshot.format !== format || snapshot.version !== version) {
    throw new Error('Неизвестный формат или версия файла сохранения');
  }
  await init();
  const nextSession = Session.from_snapshot(snapshot.session);
  try {
    const nextMap = await savedMap(snapshot);
    validateSavedPresentation(snapshot, nextSession);
    return { session: nextSession, map: nextMap, snapshot };
  } catch (error) {
    nextSession.free();
    throw error;
  }
}
