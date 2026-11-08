// Rückfragen und Meldungen im Design der App, mit abgedunkeltem und unscharfem Hintergrund.
import { api, type BasicSettingsField, type BasicSettingsInput } from "./api";

const backdrop = document.querySelector<HTMLDivElement>("#backdrop")!;

/** Zeigt einen Dialog und gibt ihn wieder frei. Escape gilt als Abbrechen. */
function open(dialog: HTMLElement, onEscape: () => void): () => void {
  const previousFocus = document.activeElement as HTMLElement | null;
  backdrop.replaceChildren(dialog);
  backdrop.hidden = false;
  document.body.classList.add("dialog-open");
  document.querySelector("#content")?.setAttribute("inert", "");

  const onKey = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onEscape();
    }
  };
  document.addEventListener("keydown", onKey);

  return () => {
    document.removeEventListener("keydown", onKey);
    backdrop.hidden = true;
    backdrop.replaceChildren();
    document.body.classList.remove("dialog-open");
    document.querySelector("#content")?.removeAttribute("inert");
    previousFocus?.focus();
  };
}

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Record<string, unknown> = {},
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const node: HTMLElementTagNameMap[K] = document.createElement(tag);
  Object.assign(node, props);
  node.append(...children);
  return node;
}

interface ConfirmOptions {
  danger?: boolean;
  cancelText?: string;
}

/** Rückfrage. „Abbrechen“ hat den Fokus, damit ein versehentliches Enter nichts auslöst. */
export function confirmDialog(title: string, message: string, confirmText: string, options: ConfirmOptions = {}): Promise<boolean> {
  return new Promise((resolve) => {
    const cancel = element("button", { type: "button", textContent: options.cancelText ?? "Abbrechen" });
    const confirm = element("button", { type: "button", textContent: confirmText, className: options.danger ? "danger" : "attention" });
    const dialog = element(
      "div",
      { className: `dialog${options.danger ? " danger" : ""}`, role: "alertdialog" },
      element(
        "div",
        { className: "dialog-head" },
        element("span", { className: "dialog-icon", textContent: "⚠", ariaHidden: "true" }),
        element("div", {}, element("h2", { textContent: title }), element("p", { className: "dialog-text", textContent: message })),
      ),
      element("div", { className: "dialog-buttons" }, cancel, confirm),
    );
    const close = open(dialog, () => finish(false));
    function finish(result: boolean) {
      close();
      resolve(result);
    }
    cancel.addEventListener("click", () => finish(false));
    confirm.addEventListener("click", () => finish(true));
    cancel.focus();
  });
}

/** Reine Meldung mit nur einer Schaltfläche. */
export function informDialog(title: string, message: string): Promise<void> {
  return new Promise((resolve) => {
    const ok = element("button", { type: "button", textContent: "OK" });
    const dialog = element(
      "div",
      { className: "dialog", role: "alertdialog" },
      element(
        "div",
        { className: "dialog-head" },
        element("span", { className: "dialog-icon", textContent: "ℹ", ariaHidden: "true" }),
        element("div", {}, element("h2", { textContent: title }), element("p", { className: "dialog-text", textContent: message })),
      ),
      element("div", { className: "dialog-buttons" }, ok),
    );
    const close = open(dialog, () => finish());
    function finish() {
      close();
      resolve();
    }
    ok.addEventListener("click", finish);
    ok.focus();
  });
}

/** Nur Ziffern, auch beim Einfügen. */
function digitsOnly(input: HTMLInputElement) {
  input.addEventListener("beforeinput", (event) => {
    if (event.data && !/^\d+$/.test(event.data.trim())) {
      event.preventDefault();
    }
  });
}

/** Grundeinstellungen für alle Server: Spielclient und Zeitlimits. Liefert true, wenn gespeichert wurde. */
export async function basicSettingsDialog(): Promise<boolean> {
  const view = await api.basicSettings();
  return new Promise((resolve) => {
    const exe = element("input", { type: "text", id: "b-exe", className: "grow", spellcheck: false, value: view.input.reforgerExe });
    const total = element("input", { type: "text", id: "b-total", className: "number", maxLength: 4, inputMode: "numeric", value: view.input.totalMinutes });
    const idle = element("input", { type: "text", id: "b-idle", className: "number", maxLength: 4, inputMode: "numeric", value: view.input.idleMinutes });
    digitsOnly(total);
    digitsOnly(idle);

    const errors: Record<BasicSettingsField, HTMLParagraphElement> = {
      reforgerExe: element("p", { className: "field-error" }),
      totalMinutes: element("p", { className: "field-error" }),
      idleMinutes: element("p", { className: "field-error" }),
    };
    const saveError = element("p", { className: "field-error" });
    const browse = element("button", { type: "button", textContent: "Durchsuchen …" });
    const reset = element("button", { type: "button", textContent: "Standard wiederherstellen", className: "left" });
    const cancel = element("button", { type: "button", textContent: "Abbrechen" });
    const save = element("button", { type: "button", textContent: "Speichern", className: "accent" });
    const label = (text: string, target: string) => element("label", { className: "field-label", htmlFor: target, textContent: text });

    const dialog = element(
      "div",
      { className: "dialog wide", role: "dialog", ariaLabel: "Grundeinstellungen" },
      element("h2", { textContent: "Grundeinstellungen" }),
      element("p", { className: "hint", textContent: "Gelten für alle Server. Die Standardwerte passen in den meisten Fällen." }),
      element("h3", { className: "section-title", textContent: "Arma Reforger" }),
      label("Spielclient (ArmaReforgerSteam.exe)", "b-exe"),
      element("div", { className: "row" }, exe, browse),
      errors.reforgerExe,
      element("p", { className: "hint", textContent: view.detectedExeHint }),
      element("h3", { className: "section-title", textContent: "Zeitlimits" }),
      label("Höchstdauer eines Syncs in Minuten", "b-total"),
      total,
      errors.totalMinutes,
      label("Neustart ohne Fortschritt nach Minuten", "b-idle"),
      idle,
      errors.idleMinutes,
      element("p", {
        className: "hint",
        textContent: "Kommt so lange kein Fortschritt, startet der Spielclient einmal neu. Bereits geladene Mods bleiben erhalten.",
      }),
      saveError,
      element("div", { className: "dialog-buttons" }, reset, cancel, save),
    );

    const fill = (input: BasicSettingsInput) => {
      exe.value = input.reforgerExe;
      total.value = input.totalMinutes;
      idle.value = input.idleMinutes;
      showErrors({});
    };
    const showErrors = (found: Partial<Record<BasicSettingsField, string>>) => {
      for (const field of Object.keys(errors) as BasicSettingsField[]) {
        errors[field].textContent = found[field] ?? "";
      }
      saveError.textContent = "";
    };

    const close = open(dialog, () => finish(false));
    function finish(saved: boolean) {
      close();
      resolve(saved);
    }

    browse.addEventListener("click", async () => {
      const chosen = await api.browseExe(exe.value);
      if (chosen) {
        exe.value = chosen;
      }
    });
    reset.addEventListener("click", () => fill(view.defaults));
    cancel.addEventListener("click", () => finish(false));
    save.addEventListener("click", async () => {
      save.disabled = true;
      try {
        const result = await api.saveBasicSettings({ reforgerExe: exe.value, totalMinutes: total.value, idleMinutes: idle.value });
        showErrors(result.errors);
        if (result.ok) {
          finish(true);
        } else if (Object.keys(result.errors).length === 0) {
          saveError.textContent = result.message;
        }
      } finally {
        save.disabled = false;
      }
    });
    exe.focus();
  });
}
