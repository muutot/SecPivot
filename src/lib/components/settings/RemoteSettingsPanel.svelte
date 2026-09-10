<script lang="ts">
  import {
    appSettings,
    findRemoteProfile,
    remoteMirrorPath,
    remoteProfilePath,
    remoteProfilesForKind,
  } from "$lib/services/settings";
  import type { RemoteKind, RemoteProfilePath } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import Select from "$lib/components/Select.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import SettingRangeCard from "$lib/components/settings/SettingRangeCard.svelte";
  import { t } from "$lib/i18n";

  interface Props {
    onclose: () => void;
    showHeader?: boolean;
    kind: RemoteKind;
  }

  let { onclose, showHeader = true, kind }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const profiles = $derived(remoteProfilesForKind(s.remoteProfiles, kind));
  const activeProfile = $derived.by(() => {
    const selected = findRemoteProfile(s.remoteProfiles, s.activeRemote);
    return selected?.settings.kind === kind ? selected : profiles[0];
  });
  const activePath = $derived(remoteProfilePath(activeProfile));
  const remote = $derived(activeProfile.settings);
  const activeName = $derived(activeProfile.name);
  const nameConflict = $derived(
    activeName.trim() !== "" &&
      profiles.some(
        (profile) =>
          remoteProfilePath(profile) !== activePath && profile.name.trim() === activeName.trim(),
      ),
  );
  const mirrorPath = $derived(remoteMirrorPath(activeProfile));
  const kindLabel = $derived(kind === "webdav" ? "WebDAV" : "S3");
  const lang = $derived(s.general.language);

  $effect(() => {
    if (s.activeRemote !== activePath) appSettings.setActiveRemote(activePath);
  });

  function change<K extends import("$lib/services/settings").RemoteUpdateKey>(
    key: K,
    value: import("$lib/services/settings").RemoteUpdateValue<K>,
  ): void {
    appSettings.updateRemote(activePath, key, value);
  }
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">{t(lang, "remote.header", { kind: kindLabel })}</span>
      <h2>{kindLabel}</h2>
      <p>{t(lang, "remote.headerDesc", { kind: kindLabel })}</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label={t(lang, "common.close")}>×</button>
  </header>
{/if}

<div class="settings-scroll settings-scroll--stack-rows">
  <section class="setting-card">
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="cloud" size={17} /></span>
        <div>
          <strong>{t(lang, "remote.profilesTitle", { kind: kindLabel })}</strong>
          <p>{t(lang, "remote.profilesDesc")}</p>
        </div>
      </div>
      <Select
        id="remote-profile-select"
        className="setting-row-input"
        value={activePath}
        ariaLabel={t(lang, "remote.profilesTitle", { kind: kindLabel })}
        options={profiles.map((profile) => ({
          value: remoteProfilePath(profile),
          label: profile.name,
        }))}
        onchange={(path) => appSettings.setActiveRemote(path as RemoteProfilePath)}
      />
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="edit" size={17} /></span>
        <div>
          <strong>{t(lang, "remote.nameTitle")}</strong>
          <p>{t(lang, "remote.nameDesc")}</p>
        </div>
      </div>
      <div class="setting-row-input">
        <TextField
          id="remote-profile-name"
          size="control"
          invalid={nameConflict}
          type="text"
          value={activeName}
          spellcheck={false}
          oninput={(event) =>
            appSettings.renameRemoteProfile(activePath, event.currentTarget.value)}
        />
      </div>
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="folder" size={17} /></span>
        <div>
          <strong>{t(lang, "remote.pathTitle")}</strong>
          <p>{t(lang, "remote.pathDesc")}</p>
        </div>
      </div>
      <code class="mirror-dir">{activePath}</code>
    </div>
    {#if nameConflict}
      <p class="settings-note input-error">{t(lang, "remote.nameConflict")}</p>
    {/if}
    <div class="profile-actions">
      <Button variant="action" onclick={() => appSettings.addRemoteProfile(kind, "")} type="button">
        {t(lang, "remote.addProfile")}</Button
      >
      <Button
        variant="action"
        disabled={profiles.length <= 1}
        onclick={() => appSettings.removeRemoteProfile(activePath)}
        type="button"
      >
        {t(lang, "remote.deleteProfile")}</Button
      >
    </div>
  </section>

  <section class="setting-card">
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
        <div>
          <strong
            >{kind === "webdav"
              ? t(lang, "remote.endpointWebdav")
              : t(lang, "remote.endpointS3")}</strong
          >
          <p>
            {kind === "webdav"
              ? t(lang, "remote.endpointWebdavDesc")
              : t(lang, "remote.endpointS3Desc")}
          </p>
        </div>
      </div>
      <div class="setting-row-input">
        <TextField
          size="control"
          type="text"
          value={remote.endpoint}
          placeholder={kind === "webdav"
            ? "https://dav.example.com/dav"
            : "https://s3.amazonaws.com"}
          spellcheck={false}
          oninput={(event) => change("endpoint", event.currentTarget.value)}
        />
      </div>
    </div>

    {#if remote.kind === "s3"}
      <div class="setting-row">
        <div class="setting-heading">
          <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
          <div>
            <strong>{t(lang, "remote.region")}</strong>
            <p>{t(lang, "remote.regionDesc")}</p>
          </div>
        </div>
        <div class="setting-row-input">
          <TextField
            size="control"
            type="text"
            value={remote.region}
            placeholder="us-east-1"
            oninput={(event) => change("region", event.currentTarget.value)}
          />
        </div>
      </div>
      <div class="setting-row">
        <div class="setting-heading">
          <span class="setting-icon"><AppIcon name="folder" size={17} /></span>
          <div>
            <strong>{t(lang, "remote.bucket")}</strong>
            <p>{t(lang, "remote.bucketDesc")}</p>
          </div>
        </div>
        <div class="setting-row-input">
          <TextField
            size="control"
            type="text"
            value={remote.bucket}
            placeholder="my-bucket"
            oninput={(event) => change("bucket", event.currentTarget.value)}
          />
        </div>
      </div>
    {/if}

    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="key" size={17} /></span>
        <div>
          <strong
            >{kind === "webdav"
              ? t(lang, "remote.accessKeyWebdav")
              : t(lang, "remote.accessKeyS3")}</strong
          >
          <p>
            {kind === "webdav"
              ? t(lang, "remote.accessKeyWebdavDesc")
              : t(lang, "remote.accessKeyS3Desc")}
          </p>
        </div>
      </div>
      <div class="setting-row-input">
        <TextField
          size="control"
          type="text"
          autocomplete="off"
          spellcheck={false}
          value={remote.accessKey}
          placeholder={kind === "webdav" ? "user" : "AKIA..."}
          oninput={(event) => change("accessKey", event.currentTarget.value)}
        />
      </div>
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="lock" size={17} /></span>
        <div>
          <strong
            >{kind === "webdav"
              ? t(lang, "remote.secretWebdav")
              : t(lang, "remote.secretS3")}</strong
          >
          <p>
            {kind === "webdav"
              ? t(lang, "remote.secretWebdavDesc")
              : t(lang, "remote.secretS3Desc")}
          </p>
        </div>
      </div>
      <div class="setting-row-input">
        <TextField
          size="control"
          type="password"
          autocomplete="off"
          spellcheck={false}
          value={remote.secretKey}
          placeholder="••••••••"
          oninput={(event) => change("secretKey", event.currentTarget.value)}
        />
      </div>
    </div>
  </section>

  <p class="settings-note warn">
    {t(lang, "remote.dpapiNote")}
  </p>

  <section class="setting-card">
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="filter" size={17} /></span>
        <div>
          <strong>{t(lang, "remote.prefix")}</strong>
          <p>{t(lang, "remote.prefixDesc")}</p>
        </div>
      </div>
      <div class="setting-row-input">
        <TextField
          id="remote-prefix"
          size="control"
          type="text"
          value={remote.prefix}
          placeholder="vaults/"
          oninput={(event) => change("prefix", event.currentTarget.value)}
        />
      </div>
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="download" size={17} /></span>
        <div>
          <strong>{t(lang, "remote.mirrorDir")}</strong>
          <p>{t(lang, "remote.mirrorDirDesc")}</p>
        </div>
      </div>
      <code class="mirror-dir">{mirrorPath}</code>
    </div>
    <p class="settings-note">{t(lang, "remote.mirrorNote", { path: mirrorPath })}</p>
  </section>

  <SettingRangeCard
    icon="clock"
    label={t(lang, "remote.backupCount")}
    description={t(lang, "remote.backupCountDesc")}
    value={remote.backupCount}
    valueLabel={t(lang, "remote.backupUnit", { count: remote.backupCount })}
    min={0}
    max={10}
    onchange={(value) => change("backupCount", value)}
  />

  <section class="setting-card">
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="file" size={17} /></span>
        <div>
          <strong>{t(lang, "remote.templateTitle")}</strong>
          <p>{t(lang, "remote.templateDesc")}</p>
        </div>
      </div>
      <div class="setting-row-input">
        <TextField
          id="remote-backup-template"
          size="control"
          type="text"
          value={remote.backupTemplate}
          placeholder={"{name}.{timestamp}.{ext}.bak"}
          spellcheck={false}
          oninput={(event) => change("backupTemplate", event.currentTarget.value)}
        />
      </div>
    </div>
    <p class="settings-note">
      {t(lang, "remote.templatePlaceholders")}
    </p>
  </section>

  <p class="auto-save-note">{t(lang, "settings.autoSaveNote")}</p>
</div>

<style>
  .profile-actions {
    display: flex;
    gap: 8px;
    margin: 10px 0 2px;
  }
</style>
