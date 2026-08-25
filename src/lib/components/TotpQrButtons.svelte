<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";

  interface Props {
    /** Called with the decoded payload (otpauth URI or Base32 seed). */
    onpick: (value: string) => void;
  }

  let { onpick }: Props = $props();

  let busy = $state(false);
  let error = $state("");
  let shotUrl = $state<string | null>(null);
  let selecting = $state(false);
  let results = $state<string[]>([]);

  let dragStart: { x: number; y: number } | null = null;
  let rect = $state<{ x: number; y: number; w: number; h: number } | null>(null);

  $effect(() => {
    return () => {
      if (shotUrl) URL.revokeObjectURL(shotUrl);
    };
  });

  async function run(task: () => Promise<void>): Promise<void> {
    if (busy) return;
    busy = true;
    error = "";
    try {
      await task();
    } catch (e) {
      error = typeof e === "string" ? e : String(e);
    } finally {
      busy = false;
    }
  }

  async function captureScreen(): Promise<Uint8Array> {
    const png = await invoke<number[]>("capture_screen_png");
    return new Uint8Array(png);
  }

  function decode(png: Uint8Array): Promise<string[]> {
    return invoke<string[]>("decode_barcode_png", { png: Array.from(png) });
  }

  function handleResults(found: string[]): void {
    if (found.length === 0) {
      error = "未识别到二维码";
      closeOverlay();
      return;
    }
    if (found.length === 1) {
      pick(found[0]);
      return;
    }
    results = found;
  }

  function pick(value: string): void {
    closeOverlay();
    onpick(value);
  }

  function closeOverlay(): void {
    selecting = false;
    results = [];
    dragStart = null;
    rect = null;
    if (shotUrl) {
      URL.revokeObjectURL(shotUrl);
      shotUrl = null;
    }
  }

  const startRegion = () =>
    run(async () => {
      const png = await captureScreen();
      shotUrl = URL.createObjectURL(new Blob([png as BlobPart], { type: "image/png" }));
      selecting = true;
    });

  const startScreen = () =>
    run(async () => {
      handleResults(await decode(await captureScreen()));
    });

  function imgRect(el: HTMLImageElement): {
    left: number;
    top: number;
    width: number;
    height: number;
  } {
    const r = el.getBoundingClientRect();
    const scale = Math.min(r.width / el.naturalWidth, r.height / el.naturalHeight);
    const w = el.naturalWidth * scale;
    const h = el.naturalHeight * scale;
    return {
      left: r.left + (r.width - w) / 2,
      top: r.top + (r.height - h) / 2,
      width: w,
      height: h,
    };
  }

  function onDown(ev: MouseEvent): void {
    dragStart = { x: ev.clientX, y: ev.clientY };
    rect = { x: ev.clientX, y: ev.clientY, w: 0, h: 0 };
  }

  function onMove(ev: MouseEvent): void {
    if (!dragStart) return;
    rect = {
      x: Math.min(dragStart.x, ev.clientX),
      y: Math.min(dragStart.y, ev.clientY),
      w: Math.abs(ev.clientX - dragStart.x),
      h: Math.abs(ev.clientY - dragStart.y),
    };
  }

  function onUp(ev: MouseEvent): void {
    const start = dragStart;
    const box = rect;
    dragStart = null;
    if (!start || !box || box.w < 4 || box.h < 4) {
      rect = null;
      return;
    }
    const img = document.querySelector<HTMLImageElement>(".qr-shot-img");
    if (!img) return;
    const r = imgRect(img);
    const sx = (box.x - r.left) / (r.width / img.naturalWidth);
    const sy = (box.y - r.top) / (r.height / img.naturalHeight);
    const sw = (box.w / r.width) * img.naturalWidth;
    const sh = (box.h / r.height) * img.naturalHeight;
    const cx = Math.max(0, Math.min(img.naturalWidth - 1, sx));
    const cy = Math.max(0, Math.min(img.naturalHeight - 1, sy));
    const cw = Math.max(1, Math.min(img.naturalWidth - cx, sw));
    const ch = Math.max(1, Math.min(img.naturalHeight - cy, sh));

    const canvas = document.createElement("canvas");
    canvas.width = Math.round(cw);
    canvas.height = Math.round(ch);
    canvas.getContext("2d")?.drawImage(img, cx, cy, cw, ch, 0, 0, canvas.width, canvas.height);
    rect = null;
    void run(async () => {
      const blob = await new Promise<Blob | null>((res) => canvas.toBlob(res, "image/png"));
      if (!blob) throw new Error("区域裁剪失败");
      handleResults(await decode(new Uint8Array(await blob.arrayBuffer())));
    });
  }
</script>

<span class="totp-qr-btns">
  <button
    type="button"
    class="totp-qr-btn"
    disabled={busy}
    title="截图选取：截图并框选二维码"
    aria-label="截图选取"
    onclick={startRegion}
  >
    <AppIcon name="crop" size={13} />
  </button>
  <button
    type="button"
    class="totp-qr-btn"
    disabled={busy}
    title="屏幕识别：识别当前屏幕中的二维码"
    aria-label="屏幕识别"
    onclick={startScreen}
  >
    <AppIcon name="scan" size={13} />
  </button>
</span>

{#if selecting && shotUrl}
  <div
    class="qr-shot-overlay"
    role="dialog"
    tabindex="-1"
    aria-label="框选二维码"
    onmousedown={onDown}
    onmousemove={onMove}
    onmouseup={onUp}
  >
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <img class="qr-shot-img" src={shotUrl} alt="屏幕截图" draggable="false" />
    {#if rect}
      <div
        class="qr-shot-rect"
        style:left="{rect.x}px"
        style:top="{rect.y}px"
        style:width="{rect.w}px"
        style:height="{rect.h}px"
      ></div>
    {/if}
    <span class="qr-shot-hint">拖动框选二维码区域，Esc 取消</span>
  </div>
{/if}

{#if results.length > 0}
  <ModalShell
    title={`识别到 ${results.length} 个码`}
    description="请选择要使用的一项"
    size="small"
    closeOnEscape
    onclose={() => (results = [])}
  >
    {#snippet children()}
      <ul class="qr-pick-list">
        {#each results as item, i (i)}
          <li>
            <Button onclick={() => pick(item)}>
              <span class="qr-pick-item mono">{item}</span>
            </Button>
          </li>
        {/each}
      </ul>
    {/snippet}
    {#snippet actions()}
      <Button onclick={() => (results = [])}>取消</Button>
    {/snippet}
  </ModalShell>
{/if}

{#if error}
  <span class="totp-qr-error">{error}</span>
{/if}

<svelte:window
  onkeydown={(e) => (selecting || results.length > 0) && e.key === "Escape" && closeOverlay()}
/>

<style>
  .totp-qr-btns {
    display: inline-flex;
    gap: 2px;
  }

  .totp-qr-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: transparent;
    cursor: pointer;
  }

  .totp-qr-btn:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .totp-qr-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .totp-qr-error {
    position: absolute;
    top: calc(100% + 3px);
    left: 0;
    color: var(--danger-color);
    font-size: var(--font-size-tiny, 10px);
  }

  .qr-shot-overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.85);
    cursor: crosshair;
    user-select: none;
  }

  .qr-shot-img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }

  .qr-shot-rect {
    position: fixed;
    border: 2px solid var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 15%, transparent);
    pointer-events: none;
  }

  .qr-shot-hint {
    position: fixed;
    top: 12px;
    left: 50%;
    transform: translateX(-50%);
    padding: 4px 10px;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--surface-bg);
    font-size: var(--font-size-tiny, 10px);
    pointer-events: none;
  }

  .qr-pick-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
    overflow: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .qr-pick-item {
    display: block;
    width: 100%;
    font-family: var(--font-mono);
    text-align: left;
    word-break: break-all;
  }
</style>
