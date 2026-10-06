# 🪟 HideMyWindows

> 🌐 Disponibil în: [English](README.md) | [Français](README.fr.md) | [Italiano](README.it.md) | [Română](README.ro.md) | [Polski](README.pl.md)

![Banner](Assets/Banner.png)

**HideMyWindows** este o aplicație Windows care **ascunde ferestrele de capturile de ecran** — capturi, înregistrări și programe de streaming precum OBS.
Este făcută pentru **utilizatori preocupați de confidențialitate, streameri și studenți** care vor să controleze ce văd ceilalți când își partajează ecranul.

> ℹ️ **Despre această versiune — reconstruită și îmbunătățită de [Mehdi](https://github.com/mehdimamas).**
> HideMyWindows original, de [Cristian Gambino (@zCri)](https://github.com/zCri), nu mai era actualizat. Această versiune **2.0** **păstrează același nume și scop** și reconstruiește aplicația de la zero pe o bază mai ușoară și mai ușor de întreținut. Meritul pentru idee și aplicația originală îi revine autorului — vezi [Credite](#-credite).

---

## ✨ Funcționalități

- 🔒 **Ascunde ferestrele de capturi și înregistrări** cu protecția Windows `SetWindowDisplayAffinity` (`WDA_EXCLUDEFROMCAPTURE`).
- 🎯 **Țintire după proces, fereastră, titlu, clasă sau PID** — cu `conține`, `începe/se termină cu`, `egal` și **regex**.
- ⚡ **Reguli automate** — ascunde aplicațiile imediat ce apar și, opțional, continuă să reaplice ca ferestrele noi să rămână ascunse.
- 🚀 **Lansare rapidă** — pornește o aplicație deja ascunsă, înainte să apară pe ecran.
- 🖥️ **Ascunde butonul din bara de activități** al oricărei aplicații.
- 🪶 **Mică și nativă** — un nucleu Tauri 2 (Rust) cu interfață Svelte. Instalatorul are câțiva MB, nu sute.
- 🔄 **În zona de notificare**, **pornire cu Windows** opțională, teme închis / deschis / sistem.
- 🌍 **Localizată**: English, Français, Italiano, Română, Polski.

---

## 📥 Instalare

### Versiuni GitHub
Descarcă cel mai recent instalator pentru arhitectura ta de pe [**pagina de versiuni**](../../releases):

| PC-ul tău | Descărcare |
| --- | --- |
| Intel/AMD pe 64 de biți (majoritatea PC-urilor) | `HideMyWindows_x64-setup.exe` |
| Windows pe 32 de biți | `HideMyWindows_x86-setup.exe` |
| ARM64 (ex. Surface Pro X) | `HideMyWindows_arm64-setup.exe` |

Rulează instalatorul și pornește HideMyWindows. Nu sunt necesare drepturi de administrator.

> **Notă despre arhitectură:** această versiune ascunde aplicațiile care rulează pe **aceeași arhitectură** ca build-ul instalat. Pe un PC pe 64 de biți, instalează build-ul x64 (acoperă marea majoritate a aplicațiilor moderne). Build-ul x86 există pentru Windows pe 32 de biți.

### Compilare din surse
Vezi [BUILDING.md](BUILDING.md).

---

## 🚀 Utilizare

1. **Acasă** — alege un proces pornit (sau o fereastră) și apasă **Ascunde toate ferestrele** / **Ascunde această fereastră**.
2. **Lansare rapidă** — adaugă aplicațiile pe care le ascunzi cel mai des și pornește-le deja ascunse dintr-un clic.
3. **Reguli pentru ferestre** — creează reguli (ex. *numele procesului conține „obs”* → *ascunde toate ferestrele*) ca să ascunzi aplicațiile la pornire. Activează **Continuă să reaplici** pentru aplicațiile care deschid ferestre noi.
4. **Setări** — ascunde HideMyWindows însuși, rulează în zona de notificare, pornește cu Windows, alege tema și limba.

![Thumbnail](Assets/Thumbnail.png)

---

## ⚙️ Cum funcționează

- HideMyWindows aplică indicatorul Windows **`SetWindowDisplayAffinity`** ferestrelor alese. Același mecanism este folosit de managerii de parole și de aplicațiile protejate DRM ca să-și țină ferestrele în afara capturilor.
- Ferestrele **proprii** sunt protejate printr-un apel direct. Pentru ferestrele **altei** aplicații, o bibliotecă mică (`hmw_payload.dll`) este încărcată în acea aplicație ca indicatorul să poată fi setat din interior — Windows permite doar procesului care deține fereastra să seteze acest indicator.
- Regulile automate folosesc **interogare** ușoară în loc de WMI, deci **nu sunt necesare drepturi de administrator**.

Vezi [ARCHITECTURE.md](ARCHITECTURE.md) pentru imaginea completă.

---

## 🧰 Tehnologii

| Parte | Tehnologie |
| --- | --- |
| Nucleu | [Tauri 2](https://tauri.app) + Rust (crate-ul `windows`) |
| Interfață | [Svelte 5](https://svelte.dev) + Vite |
| Instalator | NSIS (per utilizator și per calculator), exe portabil |
| Arhitecturi | x64, x86, ARM64 |

---

## 📜 Licență

Licențiat sub **MIT cu Commons Clause** — poți folosi, modifica și distribui liber, dar nu poți **vinde**. Vezi [LICENSE.txt](LICENSE.txt).

---

## 🙏 Credite

- **Aplicația originală** de [Cristian Gambino (@zCri)](https://github.com/zCri) — ideea, numele și primele versiuni.
- Ajutor la dezvoltarea inițială de la [@ad2017gd](https://github.com/ad2017gd) și contribuții de la [@minhprovjp](https://github.com/minhprovjp) și alții.
- Corecturile pentru fereastra albă și pictograma din zona de notificare din proiectul original sunt păstrate în spirit.
- **Reconstruit și îmbunătățit de [Mehdi](https://github.com/mehdimamas)** cu [Tauri](https://tauri.app) și [Svelte](https://svelte.dev).

Acest proiect există datorită muncii autorului original. Mulțumim. 💙

---

## 🤝 Contribuții

Issue-urile și pull request-urile sunt binevenite — vezi [CONTRIBUTING.md](CONTRIBUTING.md).
