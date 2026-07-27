// Renders the NSIS installer header and sidebar bitmaps from the Forge mark.
//
// The mark itself is `crates/desktop-app/icons/app-icon.svg`. NSIS/MUI2 cannot
// read SVG, and the two bitmaps it wants are fixed, awkward sizes (150x57 and
// 164x314) that no automatic scale of a square icon fills well. So the mark
// geometry is transcribed here once and re-laid-out for each banner: the dark
// field becomes the whole plate, and the anvil is placed inside it.
//
// Keeping this as a generator rather than two opaque binaries means the
// installer art is reviewable in a diff and reproducible byte-for-byte. Run:
//
//   node tools/generate-installer-bitmaps.mjs
//
// The output is 24-bit uncompressed BI_RGB, bottom-up, which is the only BMP
// flavour MUI2 is reliably happy with. No alpha channel: MUI header bitmaps
// with an alpha channel render with black fringes on some Windows themes.

import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outputDirectory = join(root, "crates", "desktop-app", "installer");

// ---------------------------------------------------------------------------
// Geometry, transcribed from crates/desktop-app/icons/app-icon.svg.
// SVG user units; the source viewBox is 0 0 1024 1024.
// ---------------------------------------------------------------------------

// `M166 275h692v57c0 63-70 119-177 145h-88l-55 82v94h126v119H360V653h126v-94
//  l-62-82H320c-90-28-154-82-154-145v-57Z`
const anvilPath = [
  ["M", 166, 275],
  ["L", 858, 275],
  ["L", 858, 332],
  ["C", 858, 395, 788, 451, 681, 477],
  ["L", 593, 477],
  ["L", 538, 559],
  ["L", 538, 653],
  ["L", 664, 653],
  ["L", 664, 772],
  ["L", 360, 772],
  ["L", 360, 653],
  ["L", 486, 653],
  ["L", 486, 559],
  ["L", 424, 477],
  ["L", 320, 477],
  ["C", 230, 449, 166, 395, 166, 332],
  ["L", 166, 275],
];

// `m335 332 55 50-55 50m354-100-55 50 55 50M541 318l-49 128` — the `</>`
// aperture struck through the anvil face.
const emberStrokes = [
  [[335, 332], [390, 382], [335, 432]],
  [[689, 332], [634, 382], [689, 432]],
  [[541, 318], [492, 446]],
];
const emberHalfWidth = 15.5;

// `M218 309h588` at stroke-width 10, white at 28% — the highlight across the
// anvil's top face.
const highlight = [[218, 309], [806, 309]];
const highlightHalfWidth = 5;
const highlightOpacity = 0.28;

// The drawn extent of the mark, used to place it inside each banner.
const markBox = { x: 166, y: 275, width: 692, height: 497 };

const fieldGradient = { from: [0x17, 0x23, 0x30], to: [0x08, 0x0e, 0x15] };
const steelGradient = {
  a: [276, 262],
  b: [705, 780],
  from: [0xff, 0xf9, 0xf0],
  to: [0xcf, 0xc8, 0xbe],
};
const emberGradient = {
  a: [338, 328],
  b: [691, 443],
  from: [0xff, 0xb1, 0x7e],
  to: [0xe7, 0x79, 0x4f],
};
// The field gradient's SVG axis (128,96)->(896,928) expressed as a fraction of
// the 1024 box, so it can be re-projected onto a banner of any aspect ratio.
const fieldAxis = {
  origin: [128 / 1024, 96 / 1024],
  direction: [768 / 1024, 832 / 1024],
};

// ---------------------------------------------------------------------------
// Rasteriser
// ---------------------------------------------------------------------------

function flattenCubic(from, control1, control2, to, segments = 24) {
  const points = [];
  for (let step = 1; step <= segments; step += 1) {
    const t = step / segments;
    const u = 1 - t;
    points.push([
      u * u * u * from[0]
        + 3 * u * u * t * control1[0]
        + 3 * u * t * t * control2[0]
        + t * t * t * to[0],
      u * u * u * from[1]
        + 3 * u * u * t * control1[1]
        + 3 * u * t * t * control2[1]
        + t * t * t * to[1],
    ]);
  }
  return points;
}

function flattenPath(commands) {
  const points = [];
  let current = [0, 0];
  for (const command of commands) {
    if (command[0] === "M" || command[0] === "L") {
      current = [command[1], command[2]];
      points.push(current);
    } else {
      const control1 = [command[1], command[2]];
      const control2 = [command[3], command[4]];
      const to = [command[5], command[6]];
      points.push(...flattenCubic(current, control1, control2, to));
      current = to;
    }
  }
  return points;
}

const anvilPolygon = flattenPath(anvilPath);

function isInsidePolygon(polygon, x, y) {
  let inside = false;
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i, i += 1) {
    const [xi, yi] = polygon[i];
    const [xj, yj] = polygon[j];
    if ((yi > y) !== (yj > y) && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) {
      inside = !inside;
    }
  }
  return inside;
}

function distanceToSegment(x, y, [x1, y1], [x2, y2]) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const lengthSquared = dx * dx + dy * dy;
  const t = lengthSquared === 0
    ? 0
    : Math.min(1, Math.max(0, ((x - x1) * dx + (y - y1) * dy) / lengthSquared));
  const px = x1 + t * dx - x;
  const py = y1 + t * dy - y;
  return Math.sqrt(px * px + py * py);
}

function distanceToPolyline(x, y, polyline) {
  let best = Infinity;
  for (let i = 0; i + 1 < polyline.length; i += 1) {
    best = Math.min(best, distanceToSegment(x, y, polyline[i], polyline[i + 1]));
  }
  return best;
}

function axialFraction(x, y, { a, b }) {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const lengthSquared = dx * dx + dy * dy;
  const t = ((x - a[0]) * dx + (y - a[1]) * dy) / lengthSquared;
  return Math.min(1, Math.max(0, t));
}

function mix(from, to, t) {
  return [
    from[0] + (to[0] - from[0]) * t,
    from[1] + (to[1] - from[1]) * t,
    from[2] + (to[2] - from[2]) * t,
  ];
}

/**
 * Samples one point. `u`/`v` are the banner-relative position in 0..1 and
 * `x`/`y` are the same point mapped back into SVG user units.
 */
function sample(u, v, x, y) {
  const axisLengthSquared =
    fieldAxis.direction[0] ** 2 + fieldAxis.direction[1] ** 2;
  const fieldT = Math.min(1, Math.max(0,
    ((u - fieldAxis.origin[0]) * fieldAxis.direction[0]
      + (v - fieldAxis.origin[1]) * fieldAxis.direction[1]) / axisLengthSquared));
  let colour = mix(fieldGradient.from, fieldGradient.to, fieldT);

  if (isInsidePolygon(anvilPolygon, x, y)) {
    colour = mix(steelGradient.from, steelGradient.to, axialFraction(x, y, steelGradient));
  }

  for (const stroke of emberStrokes) {
    if (distanceToPolyline(x, y, stroke) <= emberHalfWidth) {
      colour = mix(emberGradient.from, emberGradient.to, axialFraction(x, y, emberGradient));
      break;
    }
  }

  if (distanceToPolyline(x, y, highlight) <= highlightHalfWidth) {
    colour = mix(colour, [0xff, 0xff, 0xff], highlightOpacity);
  }

  return colour;
}

/**
 * @param {number} width banner width in pixels
 * @param {number} height banner height in pixels
 * @param {{ x: number, y: number, height: number }} placement where the mark
 *   sits inside the banner, in pixels; the width follows from the mark aspect.
 */
function render(width, height, placement) {
  const supersample = 4;
  const scale = placement.height / markBox.height;
  const pixels = Buffer.alloc(width * height * 3);
  for (let py = 0; py < height; py += 1) {
    for (let px = 0; px < width; px += 1) {
      let r = 0;
      let g = 0;
      let b = 0;
      for (let sy = 0; sy < supersample; sy += 1) {
        for (let sx = 0; sx < supersample; sx += 1) {
          const bx = px + (sx + 0.5) / supersample;
          const by = py + (sy + 0.5) / supersample;
          const colour = sample(
            bx / width,
            by / height,
            markBox.x + (bx - placement.x) / scale,
            markBox.y + (by - placement.y) / scale,
          );
          r += colour[0];
          g += colour[1];
          b += colour[2];
        }
      }
      const samples = supersample * supersample;
      const offset = (py * width + px) * 3;
      pixels[offset] = Math.round(r / samples);
      pixels[offset + 1] = Math.round(g / samples);
      pixels[offset + 2] = Math.round(b / samples);
    }
  }
  return pixels;
}

/** Packs RGB samples as a 24-bit bottom-up BI_RGB bitmap. */
function encodeBitmap(width, height, pixels) {
  const rowStride = (width * 3 + 3) & ~3;
  const imageSize = rowStride * height;
  const header = Buffer.alloc(54);
  header.write("BM", 0, "ascii");
  header.writeUInt32LE(54 + imageSize, 2);
  header.writeUInt32LE(54, 10);
  header.writeUInt32LE(40, 14);
  header.writeInt32LE(width, 18);
  header.writeInt32LE(height, 22);
  header.writeUInt16LE(1, 26);
  header.writeUInt16LE(24, 28);
  header.writeUInt32LE(0, 30);
  header.writeUInt32LE(imageSize, 34);
  header.writeInt32LE(2835, 38);
  header.writeInt32LE(2835, 42);

  const body = Buffer.alloc(imageSize);
  for (let y = 0; y < height; y += 1) {
    const sourceRow = (height - 1 - y) * width * 3;
    const targetRow = y * rowStride;
    for (let x = 0; x < width; x += 1) {
      body[targetRow + x * 3] = pixels[sourceRow + x * 3 + 2];
      body[targetRow + x * 3 + 1] = pixels[sourceRow + x * 3 + 1];
      body[targetRow + x * 3 + 2] = pixels[sourceRow + x * 3];
    }
  }
  return Buffer.concat([header, body]);
}

const markAspect = markBox.width / markBox.height;

/** Centres a mark of the given pixel height horizontally in a banner. */
function centred(bannerWidth, markHeight, y) {
  return { x: (bannerWidth - markHeight * markAspect) / 2, y, height: markHeight };
}

const banners = [
  {
    // MUI2 header, shown on every interior installer and uninstaller page.
    // The mark is centred so it reads correctly whether MUI places the image
    // on the left or the right of the header strip.
    name: "header.bmp",
    width: 150,
    height: 57,
    placement: centred(150, 41, 8),
  },
  {
    // MUI2 welcome/finish sidebar. The mark sits a little above true centre —
    // the usual optical correction — and clear of the page text that Windows
    // draws to the right of this panel.
    name: "sidebar.bmp",
    width: 164,
    height: 314,
    placement: centred(164, 118 / markAspect, 105),
  },
];

mkdirSync(outputDirectory, { recursive: true });
for (const banner of banners) {
  const pixels = render(banner.width, banner.height, banner.placement);
  const bitmap = encodeBitmap(banner.width, banner.height, pixels);
  const target = join(outputDirectory, banner.name);
  writeFileSync(target, bitmap);
  const digest = createHash("sha256").update(bitmap).digest("hex");
  process.stdout.write(
    `${banner.name} ${banner.width}x${banner.height} ${bitmap.length} bytes sha256=${digest}\n`,
  );
}
