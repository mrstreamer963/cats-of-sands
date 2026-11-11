const gidMask = 0x0fffffff;

export function relativeResource(parent, reference) {
  if (typeof reference !== 'string' || !reference.trim() ||
      /^(?:\/|[a-z][a-z0-9+.-]*:)|[\\?#]/i.test(reference)) {
    throw new Error(`Недопустимый путь к ресурсу: ${reference}`);
  }
  const segments = parent.split('/').slice(0, -1);
  for (const segment of reference.split('/')) {
    if (segment === '..') {
      if (!segments.length) throw new Error(`Путь выходит за каталог ресурсов: ${reference}`);
      segments.pop();
    } else if (segment && segment !== '.') {
      segments.push(segment);
    }
  }
  return segments.join('/');
}

function integer(value, min = 1, max = 0x7fffffff) {
  return Number.isInteger(value) && value >= min && value <= max;
}

async function loadAtlas(tileset, path, loadImage) {
  const {
    firstgid, tilewidth, tileheight, tilecount, columns,
    margin = 0, spacing = 0, tiles = [],
  } = tileset;
  if (!integer(firstgid, 1, gidMask) || !integer(tilecount, 1, gidMask - firstgid + 1) ||
      !integer(columns, 1, tilecount) || !integer(tilewidth) || !integer(tileheight) ||
      !integer(margin, 0) || !integer(spacing, 0) || !Array.isArray(tiles) ||
      tileset.source || tileset.tileoffset?.x || tileset.tileoffset?.y) {
    throw new Error('Ожидается встроенный атласный тайлсет без смещения');
  }
  const image = await loadImage(relativeResource(path, tileset.image));
  const rows = Math.ceil(tilecount / columns);
  if (2 * margin + columns * tilewidth + (columns - 1) * spacing > image.naturalWidth ||
      2 * margin + rows * tileheight + (rows - 1) * spacing > image.naturalHeight) {
    throw new Error(`Тайлы выходят за границы атласа ${tileset.image}`);
  }
  const types = new Map(tiles.map(tile => [
    tile.id, tile.properties?.find(property => property.name === 'terrain')?.value,
  ]));
  return { firstgid, tilecount, columns, tilewidth, tileheight, margin, spacing, image, types };
}

export async function loadTiled(path, loadLayer, loadImage) {
  try {
    const { text } = await loadLayer(path);
    const map = JSON.parse(text);
    const { width, height, tilewidth, tileheight } = map;
    if (map.orientation !== 'orthogonal' || map.infinite ||
        ![width, height, tilewidth, tileheight, width * tilewidth, height * tileheight,
          width * height].every(value => integer(value)) ||
        !Array.isArray(map.layers) || !Array.isArray(map.tilesets) || !map.tilesets.length) {
      throw new Error('Ожидается ортогональная карта фиксированного размера');
    }
    const atlases = await Promise.all(map.tilesets.map(tileset => loadAtlas(tileset, path, loadImage)));
    atlases.sort((a, b) => b.firstgid - a.firstgid);
    if (atlases.some((atlas, index) => index > 0 &&
        atlas.firstgid + atlas.tilecount > atlases[index - 1].firstgid)) {
      throw new Error('Диапазоны GID тайлсетов пересекаются');
    }
    const tiles = new Map();
    function layer(name, required) {
      const matches = map.layers.filter(layer => layer.name === name);
      const [layer] = matches;
      if (matches.length !== 1 || layer.type !== 'tilelayer' ||
          layer.width !== width || layer.height !== height ||
          layer.x || layer.y || layer.offsetx || layer.offsety ||
          !Array.isArray(layer.data) || layer.data.length !== width * height) {
        throw new Error(`Некорректный тайловый слой ${name}`);
      }
      return layer.data.map((gid, cell) => {
        if (!integer(gid, required ? 1 : 0, 0xffffffff)) {
          throw new Error(`${name}[${cell}]: недопустимый GID ${gid}`);
        }
        if (gid === 0) return null;
        if (!tiles.has(gid)) {
          const id = gid & gidMask;
          const atlas = atlases.find(atlas => id >= atlas.firstgid);
          const local = atlas ? id - atlas.firstgid : -1;
          const terrain = atlas?.types.get(local);
          if (local < 0 || local >= atlas.tilecount || typeof terrain !== 'string' || !terrain.trim()) {
            throw new Error(`${name}[${cell}]: у тайла ${id} отсутствует тип terrain`);
          }
          tiles.set(gid, {
            ...atlas, terrain,
            x: atlas.margin + (local % atlas.columns) * (atlas.tilewidth + atlas.spacing),
            y: atlas.margin + Math.floor(local / atlas.columns) * (atlas.tileheight + atlas.spacing),
          });
        }
        return { terrain: tiles.get(gid).terrain, gid };
      });
    }
    const grid = {
      width, height, tilewidth, tileheight,
      ground: layer('ground', true), walls: layer('walls', false),
    };
    return { grid, tiles, width: width * tilewidth, height: height * tileheight };
  } catch (error) {
    throw new Error(`${path}: ${error.message}`);
  }
}

export function drawTiled(context, map, walls, camera) {
  const { width, height, tilewidth, tileheight, ground } = map.grid;
  const start = camera.world(0, 0);
  const end = camera.world(camera.width, camera.height);
  const left = Math.max(0, Math.floor(start.x / tilewidth));
  const top = Math.max(0, Math.floor(start.y / tileheight));
  const right = Math.min(width, Math.ceil(end.x / tilewidth));
  const bottom = Math.min(height, Math.ceil(end.y / tileheight));

  function drawTile(gid, column, row) {
    const tile = map.tiles.get(gid);
    // Round shared edges in device pixels so fractional zoom does not expose seams.
    const x = Math.round(camera.x + column * tilewidth * camera.scale);
    const y = Math.round(camera.y + row * tileheight * camera.scale);
    const w = Math.round(camera.x + (column + 1) * tilewidth * camera.scale) - x;
    const h = Math.round(camera.y + (row + 1) * tileheight * camera.scale) - y;
    if (w === 0 || h === 0) return;
    const horizontal = gid & 0x80000000 ? -1 : 1;
    const vertical = gid & 0x40000000 ? -1 : 1;
    const diagonal = gid & 0x20000000;
    if (horizontal === 1 && vertical === 1 && !diagonal) {
      context.drawImage(tile.image, tile.x, tile.y, tile.tilewidth, tile.tileheight,
        x, y, w, h);
      return;
    }
    context.save();
    context.translate(x + w / 2, y + h / 2);
    context.scale(w * horizontal, h * vertical);
    if (diagonal) context.transform(0, 1, 1, 0, 0, 0);
    context.drawImage(tile.image, tile.x, tile.y, tile.tilewidth, tile.tileheight,
      -0.5, -0.5, 1, 1);
    context.restore();
  }

  for (let row = top; row < bottom; row++) {
    for (let column = left; column < right; column++) {
      drawTile(ground[row * width + column].gid, column, row);
    }
  }
  for (const [cell, gid] of walls) {
    const column = cell % width;
    const row = Math.floor(cell / width);
    if (column >= left && column < right && row >= top && row < bottom) {
      drawTile(gid, column, row);
    }
  }
}
