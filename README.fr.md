# 🪟 HideMyWindows

> 🌐 Disponible en : [English](README.md) | [Français](README.fr.md) | [Italiano](README.it.md) | [Română](README.ro.md) | [Polski](README.pl.md)

![Banner](Assets/Banner.png)

**HideMyWindows** est une application Windows qui **cache vos fenêtres des captures d’écran** — captures, enregistrements et logiciels de streaming comme OBS.
Elle s’adresse aux **utilisateurs soucieux de leur vie privée, aux streamers et aux étudiants** qui veulent contrôler ce que les autres voient lorsqu’ils partagent leur écran.

> ℹ️ **À propos de cette version — reconstruite et améliorée par [Mehdi](https://github.com/mehdimamas).**
> Le HideMyWindows original de [Cristian Gambino (@zCri)](https://github.com/zCri) n’était plus mis à jour. Cette version **2.0** **garde le même nom et le même objectif** et reconstruit l’application de zéro sur une base plus légère et plus facile à maintenir. Le mérite de l’idée et de l’application originale revient à son auteur — voir [Crédits](#-crédits).

---

## ✨ Fonctionnalités

- 🔒 **Masquer les fenêtres des captures et enregistrements** grâce à la protection Windows `SetWindowDisplayAffinity` (`WDA_EXCLUDEFROMCAPTURE`).
- 🎯 **Ciblage par processus, fenêtre, titre, classe ou PID** — avec `contient`, `commence/finit par`, `égal` et les **expressions régulières**.
- ⚡ **Règles automatiques** — masquer les applications dès qu’elles apparaissent, et éventuellement réappliquer pour que les nouvelles fenêtres restent cachées.
- 🚀 **Lancement rapide** — démarrer une application déjà masquée, avant qu’elle ne s’affiche.
- 🖥️ **Masquer le bouton de la barre des tâches** de n’importe quelle application.
- 🪶 **Légère et native** — un cœur Tauri 2 (Rust) et une interface Svelte. L’installateur fait quelques Mo, pas des centaines.
- 🔄 **Dans la zone de notification**, **démarrage avec Windows** en option, thèmes sombre / clair / système.
- 🌍 **Localisée** : English, Français, Italiano, Română, Polski.

---

## 📥 Installation

### Versions GitHub
Téléchargez le dernier installateur pour votre architecture depuis la [**page des versions**](../../releases) :

| Votre PC | Téléchargement |
| --- | --- |
| Intel/AMD 64 bits (la plupart des PC) | `HideMyWindows_x64-setup.exe` |
| Windows 32 bits | `HideMyWindows_x86-setup.exe` |
| ARM64 (ex. Surface Pro X) | `HideMyWindows_arm64-setup.exe` |

Lancez l’installateur, puis HideMyWindows. Aucun droit administrateur n’est requis.

> **Note sur l’architecture :** cette version masque les applications qui tournent sur la **même architecture** que la version installée. Sur un PC 64 bits, installez la version x64 (elle couvre la grande majorité des applications modernes). La version x86 existe pour Windows 32 bits.

### Compiler depuis les sources
Voir [BUILDING.md](BUILDING.md).

---

## 🚀 Utilisation

1. **Accueil** — choisissez un processus en cours (ou une fenêtre) et cliquez sur **Masquer toutes les fenêtres** / **Masquer cette fenêtre**.
2. **Lancement rapide** — ajoutez les applications que vous masquez le plus souvent et démarrez-les déjà masquées en un clic.
3. **Règles de fenêtres** — créez des règles (ex. *le nom du processus contient « obs »* → *masquer toutes les fenêtres*) pour masquer automatiquement les applications à leur lancement. Activez **Continuer à réappliquer** pour les applications qui ouvrent de nouvelles fenêtres.
4. **Paramètres** — masquer HideMyWindows lui-même, rester dans la zone de notification, démarrer avec Windows, choisir un thème et une langue.

![Thumbnail](Assets/Thumbnail.png)

---

## ⚙️ Fonctionnement

- HideMyWindows applique le drapeau Windows **`SetWindowDisplayAffinity`** aux fenêtres choisies. Le même mécanisme est utilisé par les gestionnaires de mots de passe et les applications protégées par DRM pour exclure leurs fenêtres des captures.
- Ses **propres** fenêtres sont protégées par un appel direct. Pour protéger les fenêtres d’**une autre** application, une petite bibliothèque (`hmw_payload.dll`) est chargée dans cette application afin de poser le drapeau depuis l’intérieur — Windows ne laisse que le processus propriétaire d’une fenêtre poser ce drapeau.
- Les règles automatiques utilisent un **sondage** léger à la place de WMI, donc **aucun droit administrateur n’est nécessaire**.

Voir [ARCHITECTURE.md](ARCHITECTURE.md) pour le détail.

---

## 🧰 Technologies

| Partie | Technologie |
| --- | --- |
| Cœur | [Tauri 2](https://tauri.app) + Rust (crate `windows`) |
| Interface | [Svelte 5](https://svelte.dev) + Vite |
| Installateur | NSIS (par utilisateur et par machine), exe portable |
| Architectures | x64, x86, ARM64 |

---

## 📜 Licence

Sous licence **MIT avec la Commons Clause** — vous pouvez l’utiliser, la modifier et la partager librement, mais vous ne pouvez pas la **vendre**. Voir [LICENSE.txt](LICENSE.txt).

---

## 🙏 Crédits

- **Application originale** de [Cristian Gambino (@zCri)](https://github.com/zCri) — l’idée, le nom et les premières versions.
- Aide au développement initial de [@ad2017gd](https://github.com/ad2017gd), et contributions de [@minhprovjp](https://github.com/minhprovjp) et d’autres.
- Les correctifs de la fenêtre blanche et de l’icône de zone de notification du projet original sont repris dans l’esprit.
- **Reconstruit et amélioré par [Mehdi](https://github.com/mehdimamas)** avec [Tauri](https://tauri.app) et [Svelte](https://svelte.dev).

Ce projet n’existe que grâce au travail de l’auteur original. Merci. 💙

---

## 🤝 Contribuer

Les tickets et les pull requests sont les bienvenus — voir [CONTRIBUTING.md](CONTRIBUTING.md).
