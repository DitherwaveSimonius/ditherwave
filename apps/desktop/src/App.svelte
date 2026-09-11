<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { open, save } from '@tauri-apps/plugin-dialog'

  type ParamValue = number | boolean | string

  interface ParamDescriptor {
    key: string
    displayName: string
    kind: 'bool' | 'int' | 'float' | 'choice'
    min: number
    max: number
    step: number
    options: string[]
    default: ParamValue
  }
  interface EffectDescriptor {
    key: string
    displayName: string
    category: string
    params: ParamDescriptor[]
  }
  interface EffectNode {
    id: string
    effectKey: string
    params: Record<string, ParamValue>
    enabled: boolean
  }
  interface ImagePayload {
    width: number
    height: number
    pngBase64: string
  }

  let effects = $state<EffectDescriptor[]>([])
  let addEffectKey = $state('')
  let stack = $state<EffectNode[]>([])
  let previewSrc = $state<string | null>(null)
  let hasImage = $state(false)
  let busy = $state(false)
  let error = $state<string | null>(null)

  function effectByKey(key: string): EffectDescriptor | undefined {
    return effects.find((e) => e.key === key)
  }

  function defaultParams(effect: EffectDescriptor): Record<string, ParamValue> {
    return Object.fromEntries(effect.params.map((p) => [p.key, p.default]))
  }

  onMount(async () => {
    try {
      effects = await invoke<EffectDescriptor[]>('list_effects')
      if (effects.length > 0) addEffectKey = effects[0].key
    } catch (e) {
      error = String(e)
    }
  })

  async function withBusy(fn: () => Promise<void>) {
    busy = true
    error = null
    try {
      await fn()
    } catch (e) {
      error = String(e)
    } finally {
      busy = false
    }
  }

  function syncStack() {
    if (!hasImage) return
    withBusy(async () => {
      const payload = await invoke<ImagePayload>('set_stack', { nodes: stack })
      previewSrc = `data:image/png;base64,${payload.pngBase64}`
    })
  }

  function openImage() {
    withBusy(async () => {
      const path = await open({
        multiple: false,
        filters: [
          {
            name: 'All supported',
            extensions: [
              'png',
              'jpg',
              'jpeg',
              'svg',
              'cr2',
              'cr3',
              'nef',
              'arw',
              'raf',
              'rw2',
              'orf',
              'dng',
              'pef',
              'srw',
            ],
          },
          { name: 'Images', extensions: ['png', 'jpg', 'jpeg'] },
          { name: 'Vector (SVG)', extensions: ['svg'] },
          {
            name: 'RAW Photos',
            extensions: ['cr2', 'cr3', 'nef', 'arw', 'raf', 'rw2', 'orf', 'dng', 'pef', 'srw'],
          },
        ],
      })
      if (!path || Array.isArray(path)) return
      const payload = await invoke<ImagePayload>('open_image', { path })
      previewSrc = `data:image/png;base64,${payload.pngBase64}`
      hasImage = true
      stack = []
    })
  }

  function exportImage() {
    withBusy(async () => {
      const path = await save({
        defaultPath: 'dithered.png',
        filters: [{ name: 'PNG', extensions: ['png'] }],
      })
      if (!path) return
      await invoke('export_image', { path })
    })
  }

  function exportDotsSvg() {
    withBusy(async () => {
      const path = await save({
        defaultPath: 'dithered-dots.svg',
        filters: [{ name: 'SVG', extensions: ['svg'] }],
      })
      if (!path) return
      await invoke('export_dots_svg', { path })
    })
  }

  function exportRecipe() {
    withBusy(async () => {
      const path = await save({
        defaultPath: 'recipe.toml',
        filters: [{ name: 'Recipe', extensions: ['toml'] }],
      })
      if (!path) return
      await invoke('export_recipe', { path })
    })
  }

  function importRecipe() {
    withBusy(async () => {
      const path = await open({
        multiple: false,
        filters: [{ name: 'Recipe', extensions: ['toml'] }],
      })
      if (!path || Array.isArray(path)) return
      const result = await invoke<{ nodes: EffectNode[]; image: ImagePayload }>('import_recipe', { path })
      stack = result.nodes
      previewSrc = `data:image/png;base64,${result.image.pngBase64}`
    })
  }

  function addNode() {
    const effect = effectByKey(addEffectKey)
    if (!effect) return
    stack = [
      ...stack,
      {
        id: crypto.randomUUID(),
        effectKey: effect.key,
        params: defaultParams(effect),
        enabled: true,
      },
    ]
    syncStack()
  }

  function removeNode(id: string) {
    stack = stack.filter((n) => n.id !== id)
    syncStack()
  }

  function toggleNode(id: string, enabled: boolean) {
    stack = stack.map((n) => (n.id === id ? { ...n, enabled } : n))
    syncStack()
  }

  function moveNode(index: number, delta: number) {
    const target = index + delta
    if (target < 0 || target >= stack.length) return
    const next = [...stack]
    ;[next[index], next[target]] = [next[target], next[index]]
    stack = next
    syncStack()
  }

  function updateParam(nodeId: string, key: string, value: ParamValue) {
    stack = stack.map((n) => (n.id === nodeId ? { ...n, params: { ...n.params, [key]: value } } : n))
    syncStack()
  }
</script>

<div class="toolbar">
  <button onclick={openImage} disabled={busy}>Open…</button>
  <select bind:value={addEffectKey}>
    {#each effects as effect (effect.key)}
      <option value={effect.key}>{effect.displayName}</option>
    {/each}
  </select>
  <button onclick={addNode} disabled={busy || !hasImage || !addEffectKey}>Add effect</button>
  <span class="spacer"></span>
  <button onclick={importRecipe} disabled={busy || !hasImage}>Load Recipe…</button>
  <button onclick={exportRecipe} disabled={busy || stack.length === 0}>Save Recipe…</button>
  <button onclick={exportImage} disabled={busy || !hasImage}>Export…</button>
  <button onclick={exportDotsSvg} disabled={busy || !hasImage} title="Export as vector dots (print/embroidery)"
    >Export Dots SVG…</button
  >
  {#if error}<span class="error">{error}</span>{/if}
</div>

<div class="body">
  <div class="preview">
    {#if previewSrc}
      <img src={previewSrc} alt="Preview" />
    {:else}
      <p class="hint">Open an image to get started.</p>
    {/if}
  </div>

  <div class="stack">
    <h2>Effects</h2>
    {#if stack.length === 0}
      <p class="hint">No effects yet — pick one above and click "Add effect".</p>
    {/if}
    {#each stack as n, i (n.id)}
      {@const effect = effectByKey(n.effectKey)}
      <div class="node" class:disabled={!n.enabled}>
        <div class="node-header">
          <input
            type="checkbox"
            checked={n.enabled}
            onchange={(e) => toggleNode(n.id, e.currentTarget.checked)}
          />
          <span class="node-name">{effect?.displayName ?? n.effectKey}</span>
          <button class="icon-btn" onclick={() => moveNode(i, -1)} disabled={i === 0} title="Move up">↑</button>
          <button
            class="icon-btn"
            onclick={() => moveNode(i, 1)}
            disabled={i === stack.length - 1}
            title="Move down">↓</button
          >
          <button class="icon-btn" onclick={() => removeNode(n.id)} title="Remove">×</button>
        </div>
        {#if effect}
          {#each effect.params as param (param.key)}
            <label class="param">
              <span>{param.displayName}</span>
              {#if param.kind === 'bool'}
                <input
                  type="checkbox"
                  checked={n.params[param.key] as boolean}
                  onchange={(e) => updateParam(n.id, param.key, e.currentTarget.checked)}
                />
              {:else if param.kind === 'choice'}
                <select
                  value={n.params[param.key] as string}
                  onchange={(e) => updateParam(n.id, param.key, e.currentTarget.value)}
                >
                  {#each param.options as option (option)}
                    <option value={option}>{option}</option>
                  {/each}
                </select>
              {:else}
                <input
                  type="range"
                  min={param.min}
                  max={param.max}
                  step={param.step}
                  value={n.params[param.key] as number}
                  oninput={(e) =>
                    updateParam(
                      n.id,
                      param.key,
                      param.kind === 'int'
                        ? parseInt(e.currentTarget.value, 10)
                        : parseFloat(e.currentTarget.value),
                    )}
                />
                <span class="value">{n.params[param.key]}</span>
              {/if}
            </label>
          {/each}
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }

  .toolbar button,
  .toolbar select {
    padding: 6px 12px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 5px;
  }

  .toolbar button:not(:disabled):hover {
    border-color: var(--accent);
  }

  .toolbar button:disabled {
    opacity: 0.5;
  }

  .spacer {
    flex: 1;
  }

  .error {
    color: #f87171;
    font-size: 12px;
  }

  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }

  .preview {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    overflow: auto;
  }

  .preview img {
    max-width: 100%;
    max-height: 100%;
    image-rendering: pixelated;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4);
  }

  .hint {
    color: #71717a;
    font-size: 12px;
  }

  .stack {
    width: 280px;
    border-left: 1px solid var(--border);
    background: var(--panel);
    padding: 16px;
    overflow-y: auto;
  }

  .stack h2 {
    margin: 0 0 12px;
    font-size: 14px;
    color: var(--text-h);
  }

  .node {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 10px;
    margin-bottom: 10px;
    background: var(--bg);
  }

  .node.disabled {
    opacity: 0.5;
  }

  .node-header {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
  }

  .node-name {
    flex: 1;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-h);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .icon-btn {
    width: 22px;
    height: 22px;
    line-height: 1;
    padding: 0;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 4px;
  }

  .icon-btn:not(:disabled):hover {
    border-color: var(--accent);
  }

  .icon-btn:disabled {
    opacity: 0.3;
  }

  .param {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 10px;
    font-size: 12px;
  }

  .param:last-child {
    margin-bottom: 0;
  }

  .param input[type='range'] {
    width: 100%;
  }

  .value {
    align-self: flex-end;
    color: #a1a1aa;
  }
</style>
