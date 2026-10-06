# 🪟 HideMyWindows

> 🌐 Dostępne w językach: [English](README.md) | [Français](README.fr.md) | [Italiano](README.it.md) | [Română](README.ro.md) | [Polski](README.pl.md)

![Banner](Assets/Banner.png)

**HideMyWindows** to aplikacja dla Windows, która **ukrywa okna przed przechwytywaniem ekranu** — zrzutami, nagraniami i oprogramowaniem do streamingu, takim jak OBS.
Jest dla **osób dbających o prywatność, streamerów i studentów**, którzy chcą kontrolować, co widzą inni podczas udostępniania ekranu.

> ℹ️ **O tej wersji — odbudowana i ulepszona przez [Mehdiego](https://github.com/mehdimamas).**
> Oryginalny HideMyWindows autorstwa [Cristiana Gambino (@zCri)](https://github.com/zCri) nie był już aktualizowany. Ta wersja **2.0** **zachowuje tę samą nazwę i cel** i odbudowuje aplikację od zera na lżejszej, łatwiejszej w utrzymaniu podstawie. Uznanie za pomysł i oryginalną aplikację należy do jej autora — zobacz [Podziękowania](#-podziękowania).

---

## ✨ Funkcje

- 🔒 **Ukrywanie okien przed zrzutami i nagraniami** za pomocą ochrony Windows `SetWindowDisplayAffinity` (`WDA_EXCLUDEFROMCAPTURE`).
- 🎯 **Wybór według procesu, okna, tytułu, klasy lub PID** — z dopasowaniem `zawiera`, `zaczyna/kończy się na`, `równa się` i **wyrażeniami regularnymi**.
- ⚡ **Automatyczne reguły** — ukrywaj aplikacje w chwili pojawienia się i opcjonalnie stosuj je ponownie, żeby nowe okna zostawały ukryte.
- 🚀 **Szybkie uruchamianie** — uruchom aplikację już ukrytą, zanim pojawi się na ekranie.
- 🖥️ **Ukrywanie przycisku na pasku zadań** dowolnej aplikacji.
- 🪶 **Mała i natywna** — rdzeń Tauri 2 (Rust) i interfejs Svelte. Instalator waży kilka MB, nie setki.
- 🔄 **W zasobniku**, opcjonalny **start z Windows**, motywy ciemny / jasny / systemowy.
- 🌍 **Języki**: English, Français, Italiano, Română, Polski.

---

## 📥 Instalacja

### Wydania GitHub
Pobierz najnowszy instalator dla swojej architektury ze [**strony wydań**](../../releases):

| Twój komputer | Pobieranie |
| --- | --- |
| 64-bit Intel/AMD (większość komputerów) | `HideMyWindows_x64-setup.exe` |
| 32-bitowy Windows | `HideMyWindows_x86-setup.exe` |
| ARM64 (np. Surface Pro X) | `HideMyWindows_arm64-setup.exe` |

Uruchom instalator, a potem HideMyWindows. Uprawnienia administratora nie są wymagane.

> **Uwaga o architekturze:** ta wersja ukrywa aplikacje działające na **tej samej architekturze** co zainstalowana kompilacja. Na komputerze 64-bitowym zainstaluj kompilację x64 (obejmuje zdecydowaną większość współczesnych aplikacji). Kompilacja x86 jest dla 32-bitowego Windows.

### Kompilacja ze źródeł
Zobacz [BUILDING.md](BUILDING.md).

---

## 🚀 Użycie

1. **Start** — wybierz działający proces (albo konkretne okno) i kliknij **Ukryj wszystkie okna** / **Ukryj to okno**.
2. **Szybkie uruchamianie** — dodaj aplikacje, które ukrywasz najczęściej, i uruchamiaj je już ukryte jednym kliknięciem.
3. **Reguły okien** — twórz reguły (np. *nazwa procesu zawiera „obs”* → *ukryj wszystkie okna*), żeby ukrywać aplikacje przy starcie. Włącz **Stosuj ponownie**, gdy aplikacja otwiera nowe okna z czasem.
4. **Ustawienia** — ukryj samo HideMyWindows, działaj w zasobniku, uruchamiaj z Windows, wybierz motyw i język.

![Thumbnail](Assets/Thumbnail.png)

---

## ⚙️ Jak to działa

- HideMyWindows ustawia flagę Windows **`SetWindowDisplayAffinity`** na wybranych oknach. Tego samego mechanizmu używają menedżery haseł i aplikacje chronione DRM, żeby ich okna nie trafiały do przechwyceń.
- **Własne** okna są chronione bezpośrednim wywołaniem. Aby chronić okna **innej** aplikacji, do niej ładowana jest mała biblioteka (`hmw_payload.dll`), bo Windows pozwala ustawić tę flagę tylko procesowi, do którego należy okno.
- Reguły automatyczne używają lekkiego **odpytywania** zamiast WMI, więc **uprawnienia administratora nie są potrzebne**.

Pełny opis jest w [ARCHITECTURE.md](ARCHITECTURE.md).

---

## 🧰 Technologie

| Część | Technologia |
| --- | --- |
| Rdzeń | [Tauri 2](https://tauri.app) + Rust (crate `windows`) |
| Interfejs | [Svelte 5](https://svelte.dev) + Vite |
| Instalator | NSIS (dla użytkownika i dla komputera), przenośny exe |
| Architektury | x64, x86, ARM64 |

---

## 📜 Licencja

Na licencji **MIT z klauzulą Commons Clause** — możesz używać, zmieniać i udostępniać, ale nie możesz tego **sprzedawać**. Zobacz [LICENSE.txt](LICENSE.txt).

---

## 🙏 Podziękowania

- **Oryginalna aplikacja** [Cristiana Gambino (@zCri)](https://github.com/zCri) — pomysł, nazwa i pierwsze wersje.
- Pomoc przy wczesnym rozwoju od [@ad2017gd](https://github.com/ad2017gd) oraz wkład [@minhprovjp](https://github.com/minhprovjp) i innych.
- Poprawki białego okna i ikony w zasobniku z oryginalnego projektu są zachowane w duchu.
- **Odbudowane i ulepszone przez [Mehdiego](https://github.com/mehdimamas)** na [Tauri](https://tauri.app) i [Svelte](https://svelte.dev).

Ten projekt istnieje dzięki pracy oryginalnego autora. Dziękujemy. 💙

---

## 🤝 Współtworzenie

Zgłoszenia i pull requesty są mile widziane — zobacz [CONTRIBUTING.md](CONTRIBUTING.md).
