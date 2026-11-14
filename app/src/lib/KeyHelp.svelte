<script lang="ts">
  // "How do I get a key?" – short steps per service with a button that opens the right page.
  import { api } from "./api";
  import { tr } from "./i18n.svelte";

  let { service }: { service: "deepl" | "giphy" | "klipy" | "anthropic" | "openai" | "ollama" } = $props();

  const info = $derived.by(() => {
    switch (service) {
      case "deepl":
        return {
          url: "https://www.deepl.com/pro-api",
          steps: [
            tr("Seite öffnen und „DeepL API Free“ wählen – kostenlos, 500.000 Zeichen pro Monat.", "Open the page and choose “DeepL API Free” – free, 500,000 characters per month."),
            tr("Konto anlegen. DeepL verlangt zur Prüfung eventuell eine Kreditkarte; der Free-Tarif kostet nichts.", "Create an account. DeepL may ask for a credit card for verification; the Free plan costs nothing."),
            tr("Im Konto unter „API-Schlüssel“ den Schlüssel kopieren (endet auf „:fx“) und hier einfügen.", "In your account under “API keys”, copy the key (ends in “:fx”) and paste it here."),
          ],
        };
      case "giphy":
        return {
          url: "https://developers.giphy.com/dashboard/",
          steps: [
            tr("Seite öffnen und anmelden oder ein kostenloses Konto anlegen.", "Open the page and sign in or create a free account."),
            tr("„Create an API Key“ klicken und „API“ wählen – nicht „SDK“.", "Click “Create an API Key” and choose “API” – not “SDK”."),
            tr("App-Namen und Beschreibung eingeben (z. B. „UltimatePaste privat“), Bedingungen akzeptieren.", "Enter an app name and description (e.g. “UltimatePaste personal”), accept the terms."),
            tr("Den Schlüssel unter „Your API Keys“ kopieren und hier einfügen. Neue Schlüssel erlauben 100 Abfragen pro Stunde.", "Copy the key under “Your API Keys” and paste it here. New keys allow 100 requests per hour."),
          ],
        };
      case "klipy":
        return {
          url: "https://partner.klipy.com/",
          steps: [
            tr("Seite öffnen und kostenlos registrieren.", "Open the page and register for free."),
            tr("Eine App anlegen (Name z. B. „UltimatePaste“).", "Create an app (name e.g. “UltimatePaste”)."),
            tr("Den API-Schlüssel der App kopieren und hier einfügen.", "Copy the app's API key and paste it here."),
          ],
        };
      case "anthropic":
        return {
          url: "https://platform.claude.com/settings/keys",
          steps: [
            tr("Seite öffnen und mit deinem Claude-Konto anmelden.", "Open the page and sign in with your Claude account."),
            tr("Unter „Billing“ Guthaben aufladen – die API wird nach Verbrauch abgerechnet, ein Claude-Abo gilt hier nicht.", "Add credit under “Billing” – the API is billed by usage; a Claude subscription does not apply here."),
            tr("„Create Key“ klicken, den Schlüssel (beginnt mit „sk-ant-“) kopieren und hier einfügen.", "Click “Create Key”, copy the key (starts with “sk-ant-”) and paste it here."),
          ],
        };
      case "ollama":
        return {
          url: "https://ollama.com/download",
          steps: [
            tr("Ollama installieren und ein Modell laden, z. B. „ollama pull llama3.2“.", "Install Ollama and pull a model, e.g. “ollama pull llama3.2”."),
            tr("Ein Schlüssel ist nicht nötig – alles läuft lokal auf deinem PC.", "No key needed – everything runs locally on your PC."),
          ],
        };
      default:
        return {
          url: "https://platform.openai.com/api-keys",
          steps: [
            tr("Für OpenAI: Seite öffnen, anmelden und unter „API keys“ einen Schlüssel erstellen.", "For OpenAI: open the page, sign in and create a key under “API keys”."),
            tr("Andere Anbieter mit OpenAI-kompatibler Schnittstelle: Schlüssel und URL stehen in deren Dokumentation.", "Other providers with an OpenAI-compatible API: key and URL are in their documentation."),
          ],
        };
    }
  });
</script>

<details class="keyhelp">
  <summary>{service === "ollama" ? tr("So richtest du es ein", "How to set it up") : tr("So bekommst du einen Schlüssel", "How to get a key")}</summary>
  <ol>
    {#each info.steps as step, i (i)}<li>{step}</li>{/each}
  </ol>
  <button type="button" onclick={() => api.openLink(info.url)}>{tr("Seite öffnen", "Open page")} ↗</button>
</details>

<style>
  .keyhelp {
    margin: 2px 0 6px;
    font-size: 12.5px;
    color: var(--muted);
  }
  summary {
    cursor: pointer;
    color: var(--accent);
    width: fit-content;
  }
  ol {
    margin: 6px 0 8px;
    padding-left: 20px;
    line-height: 1.5;
  }
  button {
    font-size: 12px;
    padding: 3px 10px;
  }
</style>
