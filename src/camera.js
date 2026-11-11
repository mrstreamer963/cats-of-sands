export class Camera {
  constructor(map) {
    this.map = map;
  }

  resize(width, height) {
    const fit = Math.min(width / this.map.width, height / this.map.height);
    if (this.width === undefined) {
      this.x = 0;
      this.y = 0;
      this.scale = fit;
    } else if (this.width !== width || this.height !== height) {
      const center = this.world(this.width / 2, this.height / 2);
      this.scale = Math.min(fit * 8, Math.max(fit, this.scale * fit / this.fit));
      this.x = width / 2 - center.x * this.scale;
      this.y = height / 2 - center.y * this.scale;
    }
    this.fit = fit;
    this.width = width;
    this.height = height;
    this.constrain();
  }

  world(x, y) {
    return { x: (x - this.x) / this.scale, y: (y - this.y) / this.scale };
  }

  zoom(x, y, factor) {
    const point = this.world(x, y);
    this.scale = Math.max(this.fit, Math.min(this.fit * 8, this.scale * factor));
    this.x = x - point.x * this.scale;
    this.y = y - point.y * this.scale;
    this.constrain();
  }

  pan(x, y) {
    this.x += x;
    this.y += y;
    this.constrain();
  }

  constrain() {
    const horizontalSpace = this.width - this.map.width * this.scale;
    const verticalSpace = this.height - this.map.height * this.scale;
    this.x = horizontalSpace >= 0 ? horizontalSpace / 2 : Math.max(horizontalSpace, Math.min(0, this.x));
    this.y = verticalSpace >= 0 ? verticalSpace / 2 : Math.max(verticalSpace, Math.min(0, this.y));
  }
}
