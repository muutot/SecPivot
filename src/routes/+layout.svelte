<script lang="ts">
  import { onMount } from "svelte";
  import "../app.css";
  import { appSettings } from "$lib/services/settings";
  import { applySettingsToDocument, syncCompactShellClass } from "$lib/services/settings-bootstrap";

  let { children } = $props();

  if (typeof document !== "undefined") {
    applySettingsToDocument();
  }

  onMount(() => {
    const unsubscribe = appSettings.subscribe((s) => {
      applySettingsToDocument();
      syncCompactShellClass(s.general.compactMode);
    });
    return unsubscribe;
  });
</script>

{@render children()}
