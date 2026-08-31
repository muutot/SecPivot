<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { vault } from "$lib/services/vault";
  import { rememberCredential } from "$lib/services/security";
  import { isTauriRuntime } from "$lib/services/settings";
  import StandaloneVaultShell from "$lib/components/StandaloneVaultShell.svelte";
  import VaultCredentialFields from "$lib/components/VaultCredentialFields.svelte";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";

  interface Props {
    remembered: { path: string; fileName: string } | null;
    onopened: () => void;
    onswitch: () => void;
  }

  let { remembered, onopened, onswitch }: Props = $props();

  let password = $state("");
  let keyfilePath = $state("");
  let showPassword = $state(false);
  let busy = $state(false);
  let error = $state("");
  let helloAvailable = $state(false);

  $effect(() => {
    const path = remembered?.path;
    if (!path || !isTauriRuntime()) {
      helloAvailable = false;
      return;
    }
    void invoke<{ password?: string } | null>("get_saved_credential", { path })
      .then((result) => {
        helloAvailable = result != null;
      })
      .catch(() => {
        helloAvailable = false;
      });
  });

  async function pickKeyfile(): Promise<void> {
    const selected = await open({ multiple: false });
    if (selected) keyfilePath = String(selected);
  }

  async function unlock(): Promise<void> {
    if (!remembered) return;
    if (!password && !keyfilePath) {
      error = "请输入主密码或选择密钥文件";
      return;
    }
    busy = true;
    error = "";
    try {
      await vault.open(remembered.path, password, keyfilePath || undefined);
      void rememberCredential(remembered.path, password);
      onopened();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function unlockWithHello(): Promise<void> {
    if (!remembered) return;
    busy = true;
    error = "";
    try {
      const saved = await invoke<{ password?: string } | null>("get_saved_credential", {
        path: remembered.path,
      });
      if (!saved?.password) {
        error = "没有已保存的凭据,请先在设置中启用“记住密码(Windows Hello)”";
        helloAvailable = false;
        return;
      }
      await vault.open(remembered.path, saved.password);
      onopened();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<StandaloneVaultShell icon="lock" title="数据库已锁定" subtitle={remembered?.fileName ?? ""}>
  {#if remembered}
    <p class="lock-path" title={remembered.path}>{remembered.path}</p>
  {/if}

  <div class="lock-fields">
    <VaultCredentialFields
      bind:password
      bind:keyfilePath
      bind:showPassword
      {busy}
      {error}
      isCreate={false}
      onPickKeyfile={pickKeyfile}
    />
  </div>

  <div class="unlock-actions">
    <Button onclick={onswitch} disabled={busy}>使用其他数据库</Button>
    {#if helloAvailable}
      <Button onclick={() => void unlockWithHello()} disabled={busy}>
        <AppIcon name="unlock" size={15} />Windows Hello
      </Button>
    {/if}
    <Button
      variant="primary"
      onclick={() => void unlock()}
      disabled={busy || (!password && !keyfilePath)}
      {busy}
    >
      {busy ? "解锁中…" : "解锁"}
    </Button>
  </div>
</StandaloneVaultShell>

<style>
  .lock-path {
    margin: 3px 0 0;
    max-width: 100%;
    overflow: hidden;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lock-fields {
    width: 100%;
    margin-top: 20px;
  }

  .unlock-actions {
    display: flex;
    gap: 10px;
    margin-top: 18px;
  }
</style>
