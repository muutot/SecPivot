<script lang="ts">
  export type IconName =
    | "search"
    | "settings"
    | "sliders"
    | "lock"
    | "unlock"
    | "key"
    | "plus"
    | "copy"
    | "eye"
    | "eye-off"
    | "edit"
    | "trash"
    | "folder"
    | "folder-plus"
    | "save"
    | "open"
    | "star"
    | "globe"
    | "x"
    | "check"
    | "chevron-down"
    | "chevron-right"
    | "chevron-left"
    | "grid"
    | "keyboard"
    | "palette"
    | "shield"
    | "clock"
    | "info"
    | "filter"
    | "mail"
    | "phone"
    | "database"
    | "file"
    | "refresh"
    | "user"
    | "link"
    | "download"
    | "upload"
    | "widgets";

  interface Props {
    name: IconName;
    size?: number;
    strokeWidth?: number;
    filled?: boolean;
  }

  let { name, size = 18, strokeWidth = 1.8, filled = false }: Props = $props();

  const paths: Record<IconName, string> = {
    search: '<circle cx="11" cy="11" r="7"></circle><path d="m20 20-4-4"></path>',
    settings:
      '<circle cx="12" cy="12" r="3"></circle><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M19.1 4.9 17 7M7 17l-2.1 2.1"></path>',
    sliders:
      '<path d="M5 4v6M5 16v4M12 4v2M12 12v8M19 4v4M19 14v6"></path><path d="M3 10h4M10 12h4M17 14h4"></path>',
    lock: '<rect x="5" y="11" width="14" height="9" rx="2"></rect><path d="M8 11V7a4 4 0 0 1 8 0v4"></path>',
    unlock:
      '<rect x="5" y="11" width="14" height="9" rx="2"></rect><path d="M8 11V7a4 4 0 0 1 7.6-1.7"></path>',
    key: '<circle cx="7.5" cy="15.5" r="3.5"></circle><path d="m10 13 9-9M15 8l2 2M17 6l2 2"></path>',
    plus: '<path d="M12 5v14M5 12h14"></path>',
    copy: '<rect x="9" y="9" width="11" height="11" rx="2"></rect><path d="M5 15V5a2 2 0 0 1 2-2h10"></path>',
    eye: '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"></path><circle cx="12" cy="12" r="3"></circle>',
    "eye-off":
      '<path d="M4 4l16 16"></path><path d="M10.6 5.3A10.6 10.6 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-3 3.9M6.4 7.9A15.7 15.7 0 0 0 2 12s3.5 7 10 7a10 10 0 0 0 4.6-1"></path>',
    edit: '<path d="M4 20h4L20 8l-4-4L4 16v4z"></path>',
    trash:
      '<path d="M4 7h16"></path><path d="M9 7V4h6v3"></path><path d="M6 7l1 13h10l1-13"></path><path d="M10 11v6M14 11v6"></path>',
    folder:
      '<path d="M3 6a2 2 0 0 1 2-2h4l2 3h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6z"></path>',
    "folder-plus":
      '<path d="M3 6a2 2 0 0 1 2-2h4l2 3h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6z"></path><path d="M12 11v5M9.5 13.5h5"></path>',
    save: '<path d="M5 3h11l4 4v14H5z"></path><path d="M8 3v5h7V3"></path><rect x="7" y="13" width="10" height="8"></rect>',
    open: '<path d="M12 4v12"></path><path d="m6 11 6 6 6-6"></path><path d="M5 21h14"></path>',
    star: '<path d="m12 3 2.7 5.6 6.3.8-4.6 4.3 1.2 6.1L12 17l-5.6 2.8 1.2-6.1L3 9.4l6.3-.8z"></path>',
    globe:
      '<circle cx="12" cy="12" r="9"></circle><path d="M3 12h18"></path><path d="M12 3a14 14 0 0 1 0 18 14 14 0 0 1 0-18z"></path>',
    x: '<path d="M6 6l12 12M18 6L6 18"></path>',
    check: '<path d="m5 12 4 4L19 6"></path>',
    "chevron-down": '<path d="m6 9 6 6 6-6"></path>',
    "chevron-right": '<path d="m9 6 6 6-6 6"></path>',
    "chevron-left": '<path d="m15 6-6 6 6 6"></path>',
    grid: '<rect x="4" y="4" width="7" height="7" rx="1"></rect><rect x="13" y="4" width="7" height="7" rx="1"></rect><rect x="4" y="13" width="7" height="7" rx="1"></rect><rect x="13" y="13" width="7" height="7" rx="1"></rect>',
    keyboard:
      '<rect x="2" y="6" width="20" height="12" rx="2"></rect><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M6 14h.01M18 14h.01M9 14h6"></path>',
    palette:
      '<path d="M12 3a9 9 0 1 0 0 18h1.5a2 2 0 0 0 0-4H12a2 2 0 0 1 0-4h5.5A3.5 3.5 0 0 0 21 9.5C21 6 17 3 12 3z"></path><circle cx="7.5" cy="10.5" r="1"></circle><circle cx="10.5" cy="7.5" r="1"></circle><circle cx="15.5" cy="7.5" r="1"></circle>',
    shield: '<path d="M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z"></path>',
    clock: '<circle cx="12" cy="12" r="9"></circle><path d="M12 7v5l3 2"></path>',
    info: '<circle cx="12" cy="12" r="9"></circle><path d="M12 11v5"></path><path d="M12 7.5h.01"></path>',
    filter: '<path d="M3 5h18l-7 8v6l-4 2v-8z"></path>',
    mail: '<rect x="3" y="5" width="18" height="14" rx="2"></rect><path d="m3 7 9 6 9-6"></path>',
    phone:
      '<path d="M5 3h4l2 5-3 2a13 13 0 0 0 6 6l2-3 5 2v4a2 2 0 0 1-2 2A17 17 0 0 1 3 5a2 2 0 0 1 2-2z"></path>',
    database:
      '<ellipse cx="12" cy="5" rx="7" ry="3"></ellipse><path d="M5 5v14c0 1.7 3.1 3 7 3s7-1.3 7-3V5"></path><path d="M5 12c0 1.7 3.1 3 7 3s7-1.3 7-3"></path>',
    file: '<path d="M6 3h8l4 4v14H6z"></path><path d="M14 3v4h4"></path>',
    refresh: '<path d="M20 12a8 8 0 1 1-2.3-5.7"></path><path d="M20 3v5h-5"></path>',
    user: '<circle cx="12" cy="8" r="4"></circle><path d="M5 20c0-3.5 3-6 7-6s7 2.5 7 6"></path>',
    link: '<path d="M10 14a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1"></path><path d="M14 10a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1"></path>',
    download: '<path d="M12 4v12"></path><path d="m6 11 6 6 6-6"></path><path d="M5 21h14"></path>',
    upload: '<path d="M12 20V8"></path><path d="m6 14 6-6 6 6"></path><path d="M5 3h14"></path>',
    widgets:
      '<rect x="4" y="4" width="7" height="6" rx="1"></rect><rect x="13" y="4" width="7" height="6" rx="1"></rect><rect x="4" y="14" width="7" height="6" rx="1"></rect><rect x="13" y="14" width="7" height="6" rx="1"></rect>',
  };
</script>

<svg
  viewBox="0 0 24 24"
  width={size}
  height={size}
  fill={filled ? "currentColor" : "none"}
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  class="app-icon"
>
  {@html paths[name]}
</svg>
