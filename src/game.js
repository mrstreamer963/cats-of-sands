import { Camera } from './camera.js';
import { drawTiled } from './tiled.js';

export function createGame(canvas, isPlaying, showPlannedPaths) {
  const context = canvas.getContext('2d');
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
  let pendingCamera;

  function cameraSnapshot() {
    if (!camera?.fit || !camera.width || !camera.height) return null;
    const center = camera.world(camera.width / 2, camera.height / 2);
    return { center, zoom: camera.scale / camera.fit };
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
    frame = requestAnimationFrame(draw);
  }

  function clearSession() {
    cancelAnimationFrame(frame);
    session?.free();
    session = undefined;
    tiledMap = undefined;
    camera = undefined;
    pendingCamera = undefined;
    renderEntities = [];
    drawnEntities = [];
    selectedIds.clear();
  }

  function canvasPoint(event) {
    const bounds = canvas.getBoundingClientRect();
    return {
      x: (event.clientX - bounds.left) * canvas.width / bounds.width,
      y: (event.clientY - bounds.top) * canvas.height / bounds.height,
    };
  }

  canvas.addEventListener('wheel', event => {
    if (!isPlaying() || !camera?.fit) return;
    event.preventDefault();
    const point = canvasPoint(event);
    const delta = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 120 : 1);
    camera.zoom(point.x, point.y, Math.exp(-delta * 0.0015));
  }, { passive: false });

  canvas.addEventListener('pointerdown', event => {
    if (!isPlaying() || !camera?.fit || event.button !== 0 || !event.isPrimary) return;
    suppressClick = false;
    drag = { id: event.pointerId, x: event.clientX, y: event.clientY };
    canvas.setPointerCapture(event.pointerId);
  });

  canvas.addEventListener('pointermove', event => {
    if (!isPlaying() || drag?.id !== event.pointerId) return;
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
    if (!isPlaying() || !session || suppressClick) return;
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
    if (isPlaying() && previousTime !== undefined) {
      session.advance((time - previousTime) / 1000);
      session.drain_events();
    }
    previousTime = isPlaying() ? time : undefined;

    const tick = session.current_tick();
    if (tick !== renderTick) {
      renderEntities = session.render_snapshot();
      if (tiledMap) walls = session.wall_snapshot();
      renderTick = tick;
    }
    const alpha = session.interpolation_alpha();
    drawnEntities = renderEntities.map(({ id, position, previous_position: previous, sprite, planned_path }) => ({
      id,
      sprite,
      plannedPath: planned_path,
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
    if (showPlannedPaths()) {
      context.strokeStyle = '#fff3df';
      context.lineWidth = camera ? 2 * devicePixelRatio / camera.scale : 2;
      for (const { id, position, plannedPath } of drawnEntities) {
        if (!selectedIds.has(id) || !plannedPath?.length) continue;
        context.beginPath();
        context.moveTo(position.x, position.y);
        for (const point of plannedPath) context.lineTo(point.x, point.y);
        context.stroke();
      }
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

  function resetInput() {
    previousTime = undefined;
    drag = undefined;
    suppressClick = false;
  }

  function snapshot() {
    return {
      session: session.export_snapshot(),
      selectedIds: [...selectedIds],
      camera: cameraSnapshot(),
    };
  }

  return { installSession, clearSession, resetInput, snapshot };
}
