<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type {
    DatabaseSettings,
    DatabaseSettingsPatch,
    WritableDatabaseCipher,
    KdfParameters,
    KdfBenchmarkResult,
    DatabaseKdf,
  } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import { KeyedViewGuard, sessionResourceKey } from "$lib/utils/session-state";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import Toggle from "$lib/components/templates/form/Toggle.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import RadioGroup from "$lib/components/templates/form/RadioGroup.svelte";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  import { defaultKdfParameters, memoryMiB, readKdfParameters } from "$lib/utils/kdf-parameters";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  let settings = $state<DatabaseSettings | null>(null);
  let loading = $state(true);
  let error = $state("");
  let saving = $state(false);

  let kdf = $state<"Aes" | "Argon2" | "Argon2id">("Aes");
  let roundsInput = $state("600000");
  let iterationsInput = $state("3");
  let memoryInput = $state("64");
  let lanesInput = $state("4");
  let kdfVersion = $state(19);
  let targetInput = $state("1000");
  let benchmarking = $state(false);
  let benchmarkResult = $state<KdfBenchmarkResult | null>(null);
  let benchmarkSignature = $state("");
  const kdfParameters = $derived(
    readKdfParameters(kdf, roundsInput, iterationsInput, memoryInput, lanesInput, kdfVersion),
  );
  const signature = $derived(JSON.stringify({ parameters: kdfParameters, target: targetInput }));
  const canApplyBenchmark = $derived(benchmarkResult !== null && signature === benchmarkSignature);
  const showBenchmark = $derived(
    benchmarkResult !== null &&
      (canApplyBenchmark ||
        JSON.stringify(kdfParameters) === JSON.stringify(benchmarkResult.parameters)),
  );
  const validTarget = $derived(
    /^\d+$/.test(targetInput) && Number(targetInput) >= 100 && Number(targetInput) <= 5000,
  );

  function adoptParameters(parameters: KdfParameters): void {
    kdf = parameters.algorithm;
    if (parameters.algorithm === "Aes") roundsInput = parameters.rounds;
    else {
      iterationsInput = parameters.iterations;
      memoryInput = memoryMiB(parameters.memoryBytes);
      lanesInput = String(parameters.parallelism);
      kdfVersion = parameters.version;
    }
  }

  function changeKdf(algorithm: DatabaseKdf): void {
    if (settings) adoptParameters(defaultKdfParameters(algorithm, settings.kdfParameters));
  }

  async function benchmark(): Promise<void> {
    if (benchmarking || saving || !sessionId || !kdfParameters || !validTarget) return;
    const view = dialogView.capture();
    if (!view) return;
    const input = signature;
    const parameters = kdfParameters;
    benchmarking = true;
    benchmarkResult = null;
    error = "";
    try {
      const result = await vault.callInSession(sessionId, () =>
        vault.benchmarkDatabaseKdf(parameters, Number(targetInput)),
      );
      if (!dialogView.isCurrent(view) || signature !== input) return;
      benchmarkSignature = input;
      benchmarkResult = result;
    } catch (e) {
      if (dialogView.isCurrent(view)) error = String(e);
    } finally {
      if (dialogView.isCurrent(view)) benchmarking = false;
    }
  }
  let cipher = $state<WritableDatabaseCipher | null>(null);
  let compression = $state<"None" | "Gzip">("Gzip");
  let historyInput = $state("");
  let historySizeInput = $state("");
  let templateGroupInput = $state("");
  let recycleEnabled = $state(true);
  const sessionId = vault.getActiveSessionId();
  const dialogView = new KeyedViewGuard();
  dialogView.activate(sessionId ? sessionResourceKey(sessionId, "database-settings") : null);

  onDestroy(() => dialogView.activate(null));

  onMount(() => {
    if (!sessionId) {
      error = t(lang, "dbsettings.noVault");
      loading = false;
      return;
    }
    const view = dialogView.capture();
    if (!view) return;
    void vault
      .callInSession(sessionId, () => vault.getDatabaseSettings())
      .then((value) => {
        if (!dialogView.isCurrent(view)) return;
        if (!value) {
          error = t(lang, "dbsettings.browserUnsupported");
          return;
        }
        settings = value;
        adoptParameters(value.kdfParameters);
        cipher = value.cipher === "Twofish" ? null : value.cipher;
        compression = value.compression;
        historyInput = value.historyMaxItems === null ? "" : String(value.historyMaxItems);
        historySizeInput = value.historyMaxSize === null ? "" : String(value.historyMaxSize);
        templateGroupInput = value.entryTemplatesGroup ?? "";
        recycleEnabled = value.recycleBinEnabled;
      })
      .catch((e) => {
        if (!dialogView.isCurrent(view)) return;
        error = String(e);
      })
      .finally(() => {
        if (dialogView.isCurrent(view)) loading = false;
      });
  });

  const dirty = $derived(
    settings !== null &&
      (kdf !== settings.kdf ||
        JSON.stringify(kdfParameters) !== JSON.stringify(settings.kdfParameters) ||
        (settings.cipher === "Twofish" ? cipher !== null : cipher !== settings.cipher) ||
        compression !== settings.compression ||
        (historyInput === "" ? null : Number(historyInput)) !== settings.historyMaxItems ||
        (historySizeInput === "" ? null : Number(historySizeInput)) !== settings.historyMaxSize ||
        (templateGroupInput.trim() || null) !== settings.entryTemplatesGroup ||
        recycleEnabled !== settings.recycleBinEnabled),
  );

  async function save(): Promise<void> {
    if (saving || benchmarking || !settings || !kdfParameters) return;
    const view = dialogView.capture();
    if (!view) return;
    saving = true;
    error = "";
    try {
      const patch: DatabaseSettingsPatch = {};
      if (kdf !== settings.kdf) patch.kdf = kdf;
      if (JSON.stringify(kdfParameters) !== JSON.stringify(settings.kdfParameters))
        patch.kdfParameters = kdfParameters;
      if (cipher !== null && cipher !== settings.cipher) patch.cipher = cipher;
      if (compression !== settings.compression) patch.compression = compression;
      if ((historyInput === "" ? null : Number(historyInput)) !== settings.historyMaxItems) {
        patch.historyMaxItems = historyInput === "" ? null : Number(historyInput);
      }
      if ((historySizeInput === "" ? null : Number(historySizeInput)) !== settings.historyMaxSize) {
        patch.historyMaxSize = historySizeInput === "" ? null : Number(historySizeInput);
      }
      if ((templateGroupInput.trim() || null) !== settings.entryTemplatesGroup) {
        patch.entryTemplatesGroup = templateGroupInput.trim() || null;
      }
      if (recycleEnabled !== settings.recycleBinEnabled) {
        patch.recycleBinEnabled = recycleEnabled;
      }
      if (!sessionId) return;
      await vault.callInSession(sessionId, () => vault.updateDatabaseSettings(patch));
      if (!dialogView.isCurrent(view)) return;
      onclose();
    } catch (e) {
      if (dialogView.isCurrent(view)) error = String(e);
    } finally {
      if (dialogView.isCurrent(view)) saving = false;
    }
  }
</script>

<ModalShell
  title={t(lang, "dbsettings.title")}
  description={t(lang, "dbsettings.desc")}
  size="medium"
  scrollable
  closeOnEscape={!saving}
  {onclose}
>
  {#snippet children()}
    {#if loading}
      <p class="dialog-hint">{t(lang, "dbsettings.loading")}</p>
    {:else if settings}
      <div class="setting-block">
        <span class="setting-label">{t(lang, "database.kdf")}</span>
        <RadioGroup
          ariaLabel={t(lang, "dbsettings.kdfAria")}
          options={(["Aes", "Argon2", "Argon2id"] as const).map((value) => ({
            value,
            label: value,
          }))}
          value={kdf}
          onchange={changeKdf}
          disabled={benchmarking || saving}
        />
      </div>
      <div class="kdf-costs">
        {#if kdf === "Aes"}
          <label class="setting-block">
            <span class="setting-label">{t(lang, "dbsettings.rounds")}</span>
            <TextField
              numeric
              bind:value={roundsInput}
              disabled={benchmarking || saving}
              ariaLabel={t(lang, "dbsettings.rounds")}
            />
          </label>
        {:else}
          <label class="setting-block">
            <span class="setting-label">{t(lang, "dbsettings.iterations")}</span>
            <TextField
              numeric
              bind:value={iterationsInput}
              disabled={benchmarking || saving}
              ariaLabel={t(lang, "dbsettings.iterations")}
            />
          </label>
          <label class="setting-block">
            <span class="setting-label">{t(lang, "dbsettings.memory")}</span>
            <TextField
              numeric
              bind:value={memoryInput}
              disabled={benchmarking || saving}
              ariaLabel={t(lang, "dbsettings.memory")}
            />
          </label>
          <label class="setting-block">
            <span class="setting-label">{t(lang, "dbsettings.lanes")}</span>
            <TextField
              numeric
              bind:value={lanesInput}
              disabled={benchmarking || saving}
              ariaLabel={t(lang, "dbsettings.lanes")}
            />
          </label>
          <p class="dialog-hint">Argon2 v{kdfVersion === 16 ? "1.0" : "1.3"}</p>
        {/if}
      </div>
      {#if !kdfParameters}<p class="dialog-error">{t(lang, "dbsettings.invalidKdf")}</p>{/if}
      <div class="setting-block">
        <label>
          <span class="setting-label">{t(lang, "dbsettings.target")}</span>
          <TextField
            numeric
            bind:value={targetInput}
            disabled={benchmarking || saving}
            ariaLabel={t(lang, "dbsettings.target")}
          />
        </label>
        <div class="benchmark-actions">
          <Button
            variant="field"
            onclick={() => void benchmark()}
            disabled={benchmarking || saving || !kdfParameters || !validTarget}
            busy={benchmarking}
          >
            {t(lang, benchmarking ? "dbsettings.benchmarking" : "dbsettings.benchmark")}
          </Button>
          {#if canApplyBenchmark && benchmarkResult}
            <Button
              variant="field"
              onclick={() => benchmarkResult && adoptParameters(benchmarkResult.parameters)}
            >
              {t(lang, "dbsettings.applyBenchmark")}
            </Button>
          {/if}
        </div>
        <p class="dialog-hint">{t(lang, "dbsettings.benchmarkNote")}</p>
        {#if showBenchmark && benchmarkResult}
          <p class="benchmark-result" role="status">
            {t(lang, "dbsettings.benchmarkResult", {
              ms: benchmarkResult.elapsedMs.toFixed(1),
              cost:
                benchmarkResult.parameters.algorithm === "Aes"
                  ? benchmarkResult.parameters.rounds
                  : benchmarkResult.parameters.iterations,
            })}
            {#if !benchmarkResult.targetReached}
              {t(lang, "dbsettings.benchmarkApprox")}{/if}
          </p>
        {/if}
      </div>
      <div class="setting-block">
        <span class="setting-label">{t(lang, "database.cipher")}</span>
        {#if settings.cipher === "Twofish"}
          <span class="setting-label">
            {t(lang, "dbsettings.twofishNote")}
          </span>
        {/if}
        <RadioGroup
          ariaLabel={t(lang, "dbsettings.cipherAria")}
          options={[
            ...(settings.cipher === "Twofish"
              ? [{ value: "keep" as const, label: t(lang, "dbsettings.keepTwofish") }]
              : []),
            ...(["Aes256", "ChaCha20"] as const).map((value) => ({ value, label: value })),
          ]}
          value={cipher ?? "keep"}
          onchange={(value) => (cipher = value === "keep" ? null : value)}
        />
      </div>
      <div class="setting-block">
        <span class="setting-label">{t(lang, "database.compression")}</span>
        <RadioGroup
          ariaLabel={t(lang, "dbsettings.compressionAria")}
          options={(["None", "Gzip"] as const).map((value) => ({ value, label: value }))}
          bind:value={compression}
        />
      </div>
      <div class="setting-block">
        <span class="setting-label">{t(lang, "dbsettings.historyMax")}</span>
        <TextField
          numeric
          type="number"
          bind:value={historyInput}
          placeholder={t(lang, "dbsettings.default")}
        />
      </div>
      <div class="setting-block">
        <span class="setting-label">{t(lang, "dbsettings.historySize")}</span>
        <TextField
          numeric
          type="number"
          bind:value={historySizeInput}
          placeholder={t(lang, "dbsettings.default")}
        />
      </div>
      <div class="setting-block">
        <span class="setting-label">{t(lang, "dbsettings.templateGroup")}</span>
        <TextField
          mono
          bind:value={templateGroupInput}
          placeholder={t(lang, "dbsettings.templateGroupPh")}
        />
      </div>
      <div class="setting-block setting-row">
        <span class="setting-label">{t(lang, "dbsettings.recycleBin")}</span>
        <Toggle bind:checked={recycleEnabled} ariaLabel={t(lang, "dbsettings.recycleBin")} />
      </div>
    {:else}
      <p class="dialog-hint">{error || t(lang, "dbsettings.unreadable")}</p>
    {/if}
    {#if error && settings}<p class="dialog-error">{error}</p>{/if}
  {/snippet}
  {#snippet actions()}
    <Button onclick={onclose} disabled={saving}>{t(lang, "common.cancel")}</Button>
    <Button
      variant="primary"
      onclick={() => void save()}
      disabled={saving || benchmarking || !settings || !dirty || !kdfParameters}
    >
      {saving ? t(lang, "dbsettings.saving") : t(lang, "common.save")}
    </Button>
  {/snippet}
</ModalShell>

<style>
  .kdf-costs {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 100px), 1fr));
    gap: 0 10px;
  }

  .benchmark-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 8px 0;
  }

  .benchmark-result {
    margin: 8px 0 0;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
  }
  .setting-block {
    margin-bottom: 14px;
  }

  .setting-label {
    display: block;
    margin-bottom: 6px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .setting-row .setting-label {
    margin-bottom: 0;
  }

  .dialog-hint {
    margin: 0;
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
  }

  .dialog-error {
    margin: 10px 0 0;
    color: var(--danger-color);
    font-size: var(--font-size-secondary, 11px);
  }
</style>
