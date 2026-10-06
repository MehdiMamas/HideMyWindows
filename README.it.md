# 🪟 HideMyWindows

> 🌐 Disponibile in: [English](README.md) | [Français](README.fr.md) | [Italiano](README.it.md) | [Română](README.ro.md) | [Polski](README.pl.md)

![Banner](Assets/Banner.png)

**HideMyWindows** è un'app per Windows che **nasconde le finestre dalle catture dello schermo** — screenshot, registrazioni e software di streaming come OBS.
È pensata per **chi tiene alla privacy, streamer e studenti** che vogliono controllare ciò che gli altri vedono quando condividono lo schermo.

> ℹ️ **Informazioni su questa versione — ricostruita e migliorata da [Mehdi](https://github.com/mehdimamas).**
> L'HideMyWindows originale di [Cristian Gambino (@zCri)](https://github.com/zCri) non veniva più aggiornato. Questa versione **2.0** **conserva lo stesso nome e scopo** e ricostruisce l'app da zero su basi più leggere e manutenibili. Il merito dell'idea e dell'app originale va al suo autore — vedi [Crediti](#-crediti).

---

## ✨ Funzionalità

- 🔒 **Nascondi le finestre da screenshot e registrazioni** con la protezione Windows `SetWindowDisplayAffinity` (`WDA_EXCLUDEFROMCAPTURE`).
- 🎯 **Target per processo, finestra, titolo, classe o PID** — con `contiene`, `inizia/finisce con`, `uguale` e **regex**.
- ⚡ **Regole automatiche** — nascondi le app appena compaiono e, se vuoi, continua a riapplicare così le nuove finestre restano nascoste.
- 🚀 **Avvio rapido** — avvia un'app già nascosta, prima che venga disegnata sullo schermo.
- 🖥️ **Nascondi il pulsante della barra delle applicazioni** di qualsiasi app.
- 🪶 **Piccola e nativa** — un nucleo Tauri 2 (Rust) con interfaccia Svelte. L'installer pesa pochi MB, non centinaia.
- 🔄 **Nell'area di notifica**, **avvio con Windows** opzionale, temi scuro / chiaro / di sistema.
- 🌍 **Localizzata**: English, Français, Italiano, Română, Polski.

---

## 📥 Installazione

### Release GitHub
Scarica l'installer più recente per la tua architettura dalla [**pagina delle release**](../../releases):

| Il tuo PC | Download |
| --- | --- |
| Intel/AMD a 64 bit (la maggior parte dei PC) | `HideMyWindows_x64-setup.exe` |
| Windows a 32 bit | `HideMyWindows_x86-setup.exe` |
| ARM64 (es. Surface Pro X) | `HideMyWindows_arm64-setup.exe` |

Esegui l'installer e avvia HideMyWindows. Non servono diritti di amministratore.

> **Nota sull'architettura:** questa versione nasconde le app che girano sulla **stessa architettura** della build installata. Su un PC a 64 bit, installa la build x64 (copre la grande maggioranza delle app moderne). La build x86 esiste per Windows a 32 bit.

### Compilare dai sorgenti
Vedi [BUILDING.md](BUILDING.md).

---

## 🚀 Uso

1. **Home** — scegli un processo in esecuzione (o una finestra) e fai clic su **Nascondi tutte le finestre** / **Nascondi questa finestra**.
2. **Avvio rapido** — aggiungi le app che nascondi più spesso e avviale già nascoste con un clic.
3. **Regole finestre** — crea regole (es. *il nome del processo contiene "obs"* → *nascondi tutte le finestre*) per nascondere le app all'avvio. Attiva **Continua a riapplicare** per le app che aprono nuove finestre nel tempo.
4. **Impostazioni** — nascondi HideMyWindows stesso, esegui nell'area di notifica, avvia con Windows, scegli tema e lingua.

![Thumbnail](Assets/Thumbnail.png)

---

## ⚙️ Come funziona

- HideMyWindows applica il flag Windows **`SetWindowDisplayAffinity`** alle finestre scelte. Lo stesso meccanismo è usato dai password manager e dalle app protette da DRM per tenere le proprie finestre fuori dalle catture.
- Le **proprie** finestre sono protette con una chiamata diretta. Per proteggere le finestre di **un'altra** app, una piccola libreria (`hmw_payload.dll`) viene caricata in quell'app così il flag può essere impostato dall'interno — Windows consente solo al processo proprietario di una finestra di impostare questo flag.
- Le regole automatiche usano un **polling** leggero al posto di WMI, quindi **non servono diritti di amministratore**.

Vedi [ARCHITECTURE.md](ARCHITECTURE.md) per il quadro completo.

---

## 🧰 Tecnologie

| Parte | Tecnologia |
| --- | --- |
| Nucleo | [Tauri 2](https://tauri.app) + Rust (crate `windows`) |
| Interfaccia | [Svelte 5](https://svelte.dev) + Vite |
| Installer | NSIS (per utente e per macchina), exe portatile |
| Architetture | x64, x86, ARM64 |

---

## 📜 Licenza

Con licenza **MIT con Commons Clause** — puoi usarla, modificarla e condividerla liberamente, ma non puoi **venderla**. Vedi [LICENSE.txt](LICENSE.txt).

---

## 🙏 Crediti

- **App originale** di [Cristian Gambino (@zCri)](https://github.com/zCri) — l'idea, il nome e le prime versioni.
- Aiuto allo sviluppo iniziale di [@ad2017gd](https://github.com/ad2017gd) e contributi di [@minhprovjp](https://github.com/minhprovjp) e altri.
- Le correzioni della finestra bianca e dell'icona nell'area di notifica del progetto originale sono portate avanti nello spirito.
- **Ricostruito e migliorato da [Mehdi](https://github.com/mehdimamas)** con [Tauri](https://tauri.app) e [Svelte](https://svelte.dev).

Questo progetto esiste solo grazie al lavoro dell'autore originale. Grazie. 💙

---

## 🤝 Contribuire

Issue e pull request sono benvenute — vedi [CONTRIBUTING.md](CONTRIBUTING.md).
