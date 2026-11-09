const ITEM_SELECTOR = 'button[role="menuitem"], button[role="menuitemcheckbox"]';

/** Only this menu level participates; an expanded cascade owns its own rows. */
export function menuItems(menu: HTMLElement): HTMLButtonElement[] {
  return Array.from(menu.querySelectorAll<HTMLButtonElement>(ITEM_SELECTOR)).filter(
    (item) => item.closest('[role="menu"]') === menu && !item.disabled,
  );
}

export function focusMenuItem(item: HTMLElement): void {
  item.focus({ preventScroll: true });
  item.scrollIntoView({ block: "nearest", inline: "nearest" });
}

/** Capture before page bubble shortcuts and release all listeners on unmount.
 * Tab restores the invocation point before the browser performs its normal
 * traversal. Outside clicks and newly focused dialogs retain their own focus. */
export function attachMenuKeyboard(
  root: HTMLElement,
  close: () => void,
  navigateSubmenu?: (event: KeyboardEvent) => boolean,
): () => void {
  const doc = root.ownerDocument;
  const invoker = doc.activeElement instanceof HTMLElement ? doc.activeElement : null;
  let restore = true;
  let restoringFocus = false;
  let closing = false;

  function requestClose(): void {
    if (closing) return;
    closing = true;
    close();
  }

  function restoreFocus(): void {
    if (
      restore &&
      invoker?.isConnected &&
      (root.contains(doc.activeElement) || doc.activeElement === doc.body)
    ) {
      restoringFocus = true;
      try {
        invoker.focus({ preventScroll: true });
      } finally {
        restoringFocus = false;
      }
    }
  }

  function keydown(event: KeyboardEvent): void {
    if (closing || event.defaultPrevented || !root.contains(event.target as Node)) return;
    if (event.key === "Tab") {
      event.stopPropagation();
      restoreFocus();
      requestClose();
      return;
    }
    if (event.altKey || event.ctrlKey || event.metaKey || event.isComposing) return;
    if (navigateSubmenu?.(event)) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      restoreFocus();
      requestClose();
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      // Native buttons still activate; page bubble shortcuts cannot also run.
      event.stopPropagation();
      return;
    }
    if (!["ArrowDown", "ArrowUp", "Home", "End", "ArrowLeft", "ArrowRight"].includes(event.key))
      return;
    event.preventDefault();
    event.stopPropagation();
    const target = event.target as HTMLElement;
    const menu = target.closest<HTMLElement>('[role="menu"]');
    if (!menu) return;
    const items = menuItems(menu);
    if (!items.length || event.key === "ArrowLeft" || event.key === "ArrowRight") return;
    const current = items.indexOf(target.closest<HTMLButtonElement>(ITEM_SELECTOR)!);
    const index =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? items.length - 1
          : event.key === "ArrowDown"
            ? (current + 1) % items.length
            : (current < 0 ? items.length - 1 : current - 1 + items.length) % items.length;
    focusMenuItem(items[index]);
  }

  function clickOutside(event: MouseEvent): void {
    if (root.contains(event.target as Node)) return;
    restore = false;
    requestClose();
  }

  function focusOutside(event: FocusEvent): void {
    if (restoringFocus || root.contains(event.target as Node)) return;
    restore = false;
    requestClose();
  }

  doc.addEventListener("keydown", keydown, true);
  doc.addEventListener("click", clickOutside);
  // Register after initial focus so no invocation-point event closes the menu.
  focusMenuItem(menuItems(root)[0] ?? root);
  doc.addEventListener("focusin", focusOutside);
  return () => {
    doc.removeEventListener("keydown", keydown, true);
    doc.removeEventListener("click", clickOutside);
    doc.removeEventListener("focusin", focusOutside);
    restoreFocus();
  };
}
