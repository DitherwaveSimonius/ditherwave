<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { open, save } from '@tauri-apps/plugin-dialog'

  interface ParamDescriptor {
    key: string
    displayName: string
    kind: 'bool' | 'int' | 'float'
    min: number
    max: number
    step: number
    default: number | boolean
  }
  interface EffectDescriptor {
    key: string
    displayName: string
    category: string
    params: ParamDescriptor[]
  }
  interface ImagePayload {
    width: number
    height: number
    pngBase64: string
  }

  let effects = $state<EffectDescriptor[]>([])
  let selectedKey = $state('')
  let paramValues = $state<Record<string, number | boolean>>({})
  let previewSrc = $state<string | null>(null)
  let hasImage = $state(false)
  let busy = $state(false)
  let error = $state<string | null>(null)

  const selectedEffect = $derived(effects.find((e) => e.key === selectedKey) ?? null)

  onMount(async () => {
    try {
      effects = await invoke<EffectDescriptor[]>('list_effects')
      if (effects.length > 0) selectEffect(effects[0].key)
    } catch (e) {
      error = String(e)
    }
  })

  function selectEffect(key: string) {
    selectedKey = key
    const effect = effects.find((e) => e.key === key)
    if (effect) {
      paramValues = Object.fromEntries(effect.params.map((p) => [p.key, p.default]))
    }
    if (hasImage) applyEffect()
  }

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

  async function runApplyEffect() {
    const payload = await invoke<ImagePayload>('apply_effect', {
      effectKey: selectedKey,
      params: paramValues,
    })
    previewSrc = `data:image/png;base64,${payload.pngBase64}`
  }

  function openImage() {
    withBusy(async () => {
      const path = await open({
        multiple: false,
        filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg'] }],
      })
      if (!path || Array.isArray(path)) return
      const payload = await invoke<ImagePayload>('open_image', { path })
      previewSrc = `data:image/png;base64,${payload.pngBase64}`
      hasImage = true
      if (selectedKey) await runApplyEffect()
    })
  }

  function applyEffect() {
    if (!selectedKey || !hasImage) return
    withBusy(runApplyEffect)
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

  function updateParam(key: string, value: number | boolean) {
    paramValues = { ...paramValues, [key]: value }
    applyEffect()
  }
</script>

<div class="toolbar">
  <button onclick={openImage} disabled={busy}>Open…</button>
  <select value={selectedKey} onchange={(e) => selectEffect(e.currentTarget.value)}>
    {#each effects as effect (effect.key)}
      <option value={effect.key}>{effect.displayName}</option>
    {/each}
  </select>
  <button onclick={exportImage} disabled={busy || !hasImage}>Export…</button>
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

  {#if selectedEffect}
    <div class="params">
      <h2>{selectedEffect.displayName}</h2>
      {#each selectedEffect.params as param (param.key)}
        <label class="param">
          <span>{param.displayName}</span>
          {#if param.kind === 'bool'}
            <input
              type="checkbox"
              checked={paramValues[param.key] as boolean}
              onchange={(e) => updateParam(param.key, e.currentTarget.checked)}
            />
          {:else}
            <input
              type="range"
              min={param.min}
              max={param.max}
              step={param.step}
              value={paramValues[param.key] as number}
              oninput={(e) =>
                updateParam(
                  param.key,
                  param.kind === 'int'
                    ? parseInt(e.currentTarget.value, 10)
                    : parseFloat(e.currentTarget.value),
                )}
            />
            <span class="value">{paramValues[param.key]}</span>
          {/if}
        </label>
      {/each}
    </div>
  {/if}
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

  .error {
    color: #f87171;
    margin-left: auto;
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
  }

  .params {
    width: 260px;
    border-left: 1px solid var(--border);
    background: var(--panel);
    padding: 16px;
    overflow-y: auto;
  }

  .params h2 {
    margin: 0 0 16px;
    font-size: 14px;
    color: var(--text-h);
  }

  .param {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 16px;
    font-size: 12px;
  }

  .param input[type='range'] {
    width: 100%;
  }

  .value {
    align-self: flex-end;
    color: #a1a1aa;
  }
</style>
