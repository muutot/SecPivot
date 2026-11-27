import type { Action } from "svelte/action";

/** A geometric lens map, never a capture of page contents. The center is
 * neutral; only a narrow rounded rim displaces the backdrop. */
export function createRimMap(
  width: number,
  height: number,
  radius: number,
): Uint8ClampedArray<ArrayBuffer> {
  if (
    !Number.isInteger(width) ||
    !Number.isInteger(height) ||
    width < 1 ||
    height < 1 ||
    width > 1024 ||
    height > 1024 ||
    !Number.isFinite(radius) ||
    radius < 0
  ) {
    throw new RangeError("Invalid lens dimensions");
  }
  const pixels = new Uint8ClampedArray(width * height * 4);
  const r = Math.min(radius, width / 2, height / 2);
  const band = Math.min(8, Math.max(2, r));
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const px = x + 0.5 - width / 2;
      const py = y + 0.5 - height / 2;
      const qx = Math.abs(px) - (width / 2 - r);
      const qy = Math.abs(py) - (height / 2 - r);
      const ox = Math.max(qx, 0);
      const oy = Math.max(qy, 0);
      const length = Math.hypot(ox, oy);
      const depth = -(length + Math.min(Math.max(qx, qy), 0) - r);
      let nx = 0;
      let ny = 0;
      if (depth >= 0 && depth < band) {
        const strength = Math.sin((depth / band) * Math.PI) * 96;
        nx = (length > 0 ? ox / length : qx > qy ? 1 : 0) * Math.sign(px) * strength;
        ny = (length > 0 ? oy / length : qy >= qx ? 1 : 0) * Math.sign(py) * strength;
      }
      const i = (y * width + x) * 4;
      pixels[i] = 128 + nx;
      pixels[i + 1] = 128 + ny;
      pixels[i + 2] = 128;
      pixels[i + 3] = 255;
    }
  }
  return pixels;
}

let lensId = 0;

/** Chromium SVG backdrop optics. Other engines keep the CSS frosted finish.
 * Resources are local, sized only from geometry, and released on mode/unmount. */
export const liquidGlass: Action<HTMLElement> = (node) => {
  const root = document.documentElement;
  const media = [
    matchMedia("(forced-colors: active)"),
    matchMedia("(prefers-reduced-transparency: reduce)"),
    matchMedia("(prefers-contrast: more)"),
    matchMedia("(prefers-contrast: less)"),
  ];
  let svg: SVGSVGElement | null = null;
  let frame = 0;
  let lastGeometry = "";
  const clear = () => {
    node.style.removeProperty("--liquid-refraction-filter");
    svg?.remove();
    svg = null;
    lastGeometry = "";
  };
  const render = () => {
    frame = 0;
    if (
      !root.dataset.theme?.startsWith("liquid-") ||
      media.some((query) => query.matches) ||
      !/(?:Chrome|Chromium|Edg)\//.test(navigator.userAgent) ||
      !CSS.supports("backdrop-filter", 'url("#lens")')
    ) {
      clear();
      return;
    }
    const bounds = node.getBoundingClientRect();
    if (!bounds.width || !bounds.height) {
      clear();
      return;
    }
    const ratio = Math.min(1, 1024 / Math.max(bounds.width, bounds.height));
    const width = Math.max(1, Math.round(bounds.width * ratio));
    const height = Math.max(1, Math.round(bounds.height * ratio));
    const radius = (parseFloat(getComputedStyle(node).borderTopLeftRadius) || 0) * ratio;
    const geometry = `${width}/${height}/${radius}`;
    if (svg && geometry === lastGeometry) return;
    clear();
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext("2d");
    if (!context) return;
    context.putImageData(new ImageData(createRimMap(width, height, radius), width, height), 0, 0);
    const ns = "http://www.w3.org/2000/svg";
    svg = document.createElementNS(ns, "svg");
    svg.setAttribute("aria-hidden", "true");
    svg.style.cssText = "position:fixed;width:0;height:0;pointer-events:none";
    const filter = document.createElementNS(ns, "filter");
    const id = `secpivot-lens-${++lensId}`;
    filter.id = id;
    filter.setAttribute("x", "0");
    filter.setAttribute("y", "0");
    filter.setAttribute("width", "1");
    filter.setAttribute("height", "1");
    filter.setAttribute("color-interpolation-filters", "sRGB");
    const map = document.createElementNS(ns, "feImage");
    map.setAttribute("href", canvas.toDataURL());
    map.setAttribute("preserveAspectRatio", "none");
    map.setAttribute("result", "rim");
    const displacement = document.createElementNS(ns, "feDisplacementMap");
    displacement.setAttribute("in", "SourceGraphic");
    displacement.setAttribute("in2", "rim");
    displacement.setAttribute("scale", "4");
    displacement.setAttribute("xChannelSelector", "R");
    displacement.setAttribute("yChannelSelector", "G");
    filter.append(map, displacement);
    svg.append(filter);
    document.body.append(svg);
    node.style.setProperty("--liquid-refraction-filter", `url("#${id}") blur(8px)`);
    lastGeometry = geometry;
  };
  const schedule = () => {
    if (!frame) frame = requestAnimationFrame(render);
  };
  const resize = new ResizeObserver(schedule);
  resize.observe(node);
  const mode = new MutationObserver(schedule);
  mode.observe(root, { attributes: true, attributeFilter: ["data-theme"] });
  for (const query of media) query.addEventListener("change", schedule);
  schedule();
  return {
    destroy() {
      cancelAnimationFrame(frame);
      resize.disconnect();
      mode.disconnect();
      for (const query of media) query.removeEventListener("change", schedule);
      clear();
    },
  };
};
