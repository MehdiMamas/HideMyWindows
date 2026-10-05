//! Localized titles of the Windows toast popup.
//!
//! Source: `Windows.UI.ShellCommon` resource `\ActionCenter\AC_ToastCenter_Title`,
//! as collected by the Windhawk notifications-placement mod. The class name is
//! always `Windows.UI.Core.CoreWindow`; the title is what separates a toast
//! from Start, Search, and Quick Settings in the same process.

const TOAST_CLASS: &str = "Windows.UI.Core.CoreWindow";

const TOAST_TITLES: &[&str] = &[
    "Nuwe kennisgewing",
    "አዲስ ማሳወቂያ",
    "নতুন জাননী",
    "Yeni bildiriş",
    "Новае апавяшчэнне",
    "নতুন বিজ্ঞপ্তি",
    "Novo obavještenje",
    "Notificació nova",
    "ᎢᏤᎢ ᎧᏃᎮᏓ",
    "Hysbysiad newydd",
    "اعلان جدید",
    "Bagong notification",
    "Fógra nua",
    "Brath ùr",
    "નવી સૂચના",
    "नई अधिसूचना",
    "Նոր ծանուցում",
    "Ný tilkynning",
    "ახალი შეტყობინება",
    "Жаңа хабарландыру",
    "ការ​ជូន​ដំណឹង​ថ្មី",
    "ಹೊಸ ಪ್ರಕಟಣೆ",
    "नवी अधिसुचोवणी",
    "Nei Notifikatioun",
    "ການແຈ້ງເຕືອນໃໝ່",
    "Whakamōhiotanga hōu",
    "Ново известување",
    "പുതിയ അറിയിപ്പ്",
    "नवीन सूचना",
    "Pemberitahuan baharu",
    "Notifika ġdida",
    "नयाँ सूचना",
    "Nytt varsel",
    "ନୂତନ ବିଜ୍ଞପ୍ତି",
    "ਨਵੀਂ ਸੂਚਨਾ",
    "Musuq willana",
    "Njoftim i ri",
    "Ново обавјештење",
    "Ново обавештење",
    "புதிய அறிவிப்பு",
    "కొత్త నోటిఫికేషన్",
    "Яңа белдерү",
    "يېڭى ئۇقتۇرۇش",
    "نئی اطلاع",
    "Yangi xabarnoma",
    "‏‏إعلام جديد",
    "Ново известие",
    "Nové oznámení",
    "Ny meddelelse",
    "Neue Benachrichtigung",
    "Νέα ειδοποίηση",
    "New notification",
    "Notificación nueva",
    "Nueva notificación",
    "Uus teatis",
    "Jakinarazpen berria",
    "Uusi ilmoitus",
    "Nouvelle notification",
    "Nova notificación",
    "הודעה חדשה",
    "Nova obavijesti",
    "Új értesítés",
    "Pemberitahuan baru",
    "Nuova notifica",
    "新しい通知",
    "새 알림",
    "Naujas pranešimas",
    "Jauns paziņojums",
    "Ny varsling",
    "Nieuwe melding",
    "Nowe powiadomienie",
    "Nova notificação",
    "Notificare nouă",
    "Новое уведомление",
    "Nové oznámenie",
    "Novo obvestilo",
    "Novo obaveštenje",
    "Nytt meddelande",
    "การแจ้งให้ทราบใหม่",
    "Yeni bildirim",
    "Нове сповіщення",
    "Thông báo mới",
    "新通知",
];

/// A toast popup, not another shell surface that shares the same process.
pub fn is_toast_window(class_name: &str, title: &str) -> bool {
    class_name.eq_ignore_ascii_case(TOAST_CLASS) && is_toast_title(title)
}

fn is_toast_title(title: &str) -> bool {
    TOAST_TITLES.contains(&title)
}

#[cfg(test)]
mod tests {
    use super::is_toast_window;

    #[test]
    fn matches_localized_toast_titles() {
        assert!(is_toast_window(
            "Windows.UI.Core.CoreWindow",
            "New notification"
        ));
        assert!(is_toast_window(
            "windows.ui.core.corewindow",
            "Nouvelle notification"
        ));
        assert!(is_toast_window(
            "Windows.UI.Core.CoreWindow",
            "Nuova notifica"
        ));
        assert!(is_toast_window(
            "Windows.UI.Core.CoreWindow",
            "Notificare nouă"
        ));
        assert!(is_toast_window(
            "Windows.UI.Core.CoreWindow",
            "Nowe powiadomienie"
        ));
    }

    #[test]
    fn ignores_other_shell_windows() {
        assert!(!is_toast_window("Windows.UI.Core.CoreWindow", "Start"));
        assert!(!is_toast_window("Windows.UI.Core.CoreWindow", ""));
        assert!(!is_toast_window("Shell_TrayWnd", "New notification"));
    }
}
