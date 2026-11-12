import catIdleUrl from './assets/cat-walk-v2.png';
import catWalkUrl from './assets/cat-walk-8.png';
import catDownTransitionUrl from './assets/cat-walk-down-transition.png';
import catSideStepUrl from './assets/cat-side-step-v2.png';
import catSideNearStepUrl from './assets/cat-side-near-step.png';

export const catFrameDuration = 55;
export const catWalkFrameCount = 8;

// Measure from the top of the ears to the foot anchor, excluding atlas padding.
const catHeightInRadii = 2.35;
const idleReferenceHeight = [295, 297, 295];
const walkReferenceHeight = [226, 222];
const sideNearReferenceHeight = 566;
const sideFarReferenceHeight = 561;
const idleFootX = [196, 184, 206];
const idleFootY = [312, 646, 977];
const sideStepFootX = [279, 817, 1355, 1900, 279, 817, 1355, 1900];
const sideStepFootY = [628, 628, 628, 628, 625, 625, 625, 625];
const walkFootX = [
  [177, 449, 720, 991, 1263, 1534, 1806, 2080],
  [170, 441, 712, 983, 1254, 1525, 1797, 2068],
];
const walkFootY = [
  [236, 236, 236, 233, 236, 236, 235, 238],
  [471, 471, 472, 470, 469, 471, 471, 469],
];

export function catFrame(images, animation, radius) {
  const { row, flip, elapsed, moving } = animation;
  const targetHeight = radius * catHeightInRadii;
  const index = Math.floor(elapsed / catFrameDuration) % catWalkFrameCount;
  if (moving && row === 2) {
    const nearStep = index < 4;
    const column = index % 4;
    const image = nearStep ? images.sideNearStep : images.sideStep;
    const x = Math.floor(image.naturalWidth * column / 4);
    const width = Math.floor(image.naturalWidth * (column + 1) / 4) - x;
    const height = image.naturalHeight;
    const referenceHeight = nearStep ? sideNearReferenceHeight : sideFarReferenceHeight;
    const scale = targetHeight / referenceHeight;
    return {
      image, x, y: 0, width, height, flip, scale, column,
      sideStep: nearStep ? 'ближний шаг' : 'дальний шаг',
      offsetX: (x - sideStepFootX[index]) * scale,
      offsetY: -sideStepFootY[index] * scale,
    };
  }
  let image = moving ? images.walk : images.idle;
  // Use the retouched front-facing frame at the loop boundary.
  if (moving && row === 0 && index === 7) image = images.downTransition;
  const column = moving ? index : 0;
  const columns = moving ? catWalkFrameCount : 5;
  const x = Math.floor(image.naturalWidth * column / columns);
  const y = Math.floor(image.naturalHeight * row / 3);
  const width = Math.floor(image.naturalWidth * (column + 1) / columns) - x;
  const height = Math.floor(image.naturalHeight * (row + 1) / 3) - y;
  const referenceHeight = moving ? walkReferenceHeight[row] : idleReferenceHeight[row];
  const scale = targetHeight / referenceHeight;
  const footX = moving ? walkFootX[row][column] : idleFootX[row];
  const footY = moving ? walkFootY[row][column] : idleFootY[row];
  return {
    image, x, y, width, height, flip, scale, column,
    offsetX: (x - footX) * scale,
    offsetY: (y - footY) * scale,
  };
}

export function createCatImages() {
  const idle = new Image();
  idle.src = catIdleUrl;
  const walk = new Image();
  walk.src = catWalkUrl;
  const downTransition = new Image();
  downTransition.src = catDownTransitionUrl;
  const sideStep = new Image();
  sideStep.src = catSideStepUrl;
  const sideNearStep = new Image();
  sideNearStep.src = catSideNearStepUrl;
  return { idle, walk, downTransition, sideStep, sideNearStep };
}

export function drawCat(context, cat, x, y) {
  context.save();
  context.translate(x, y);
  if (cat.flip) context.scale(-1, 1);
  context.drawImage(
    cat.image, cat.x, cat.y, cat.width, cat.height,
    cat.offsetX, cat.offsetY, cat.width * cat.scale, cat.height * cat.scale,
  );
  context.restore();
}
