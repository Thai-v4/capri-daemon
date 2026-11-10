//! Lire une ligne tapée au clavier sans l'afficher : pour donner une clé d'API dans un terminal.
//!
//! - macOS : `readpassphrase(3)`, de la bibliothèque C du système. Elle lit sur `/dev/tty` (le
//!   terminal de la commande), écho coupé, et le remet tel qu'il était, y compris quand un signal
//!   l'interrompt (Ctrl+C, Ctrl+Z, terminal fermé…) : elle rend l'écho, puis laisse le signal
//!   suivre son cours. Un Ctrl+C arrête donc la commande sans laisser le terminal muet ;
//! - Windows : l'écho de la console est coupé le temps de la saisie, puis remis comme il était,
//!   même si la lecture échoue. Ce qu'un Ctrl+C laisse derrière lui n'a pas été vérifié : le
//!   système termine alors le processus sans passer par ce module ;
//! - ailleurs (Linux, sans trousseau où ranger une clé) : une erreur, sans rien lire.

use std::io;

/// Lit une ligne sans l'afficher, et la rend sans son retour à la ligne. Le curseur passe à la
/// ligne suivante quand on valide.
pub fn read_hidden_line() -> io::Result<String> {
    imp::read_hidden_line().map(|line| line.trim_end_matches(['\r', '\n']).to_owned())
}

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::{CStr, c_char, c_int};
    use std::io;

    /// `RPP_REQUIRE_TTY`, de `<readpassphrase.h>` : échouer s'il n'y a pas de terminal, plutôt que
    /// de lire l'entrée standard en clair. Sans `RPP_ECHO_ON` (0x01), l'écho est coupé.
    const RPP_REQUIRE_TTY: c_int = 0x02;
    /// Bien plus qu'une clé (`capri_chat::key::MAX_CHARS` caractères) avec des blancs autour.
    /// `readpassphrase` jette ce qui dépasse : une clé aussi longue serait refusée de toute façon.
    const CAPACITY: usize = 4096;

    unsafe extern "C" {
        /// `<readpassphrase.h>` du SDK de macOS :
        /// `char *readpassphrase(const char *prompt, char *buf, size_t bufsiz, int flags);`
        fn readpassphrase(
            prompt: *const c_char,
            buf: *mut c_char,
            bufsiz: usize,
            flags: c_int,
        ) -> *mut c_char;
    }

    pub fn read_hidden_line() -> io::Result<String> {
        let mut buffer = vec![0u8; CAPACITY];
        // SAFETY: l'invite est une chaîne C vide et statique (l'app écrit la sienne avant) ;
        // `buffer` nous appartient, vit le temps de l'appel et fait `CAPACITY` octets : la fonction
        // y écrit au plus `CAPACITY - 1` caractères et un zéro final, et ne garde aucun pointeur
        // après son retour.
        let read = unsafe {
            readpassphrase(c"".as_ptr(), buffer.as_mut_ptr().cast(), buffer.len(), RPP_REQUIRE_TTY)
        };
        let line = if read.is_null() {
            Err(io::Error::last_os_error())
        } else {
            CStr::from_bytes_until_nul(&buffer)
                .ok()
                .and_then(|line| line.to_str().ok())
                .map(str::to_owned)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "la saisie n'est pas du texte"))
        };
        // la page de manuel demande d'effacer le tampon dès que possible
        for byte in &mut buffer {
            // SAFETY: `byte` désigne un octet de `buffer`, valide et aligné ; l'écriture volatile
            // empêche le compilateur de supprimer cet effacement.
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
        line
    }
}

#[cfg(windows)]
mod imp {
    use std::io::{self, BufRead};

    use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Console::{
        CONSOLE_MODE, ENABLE_ECHO_INPUT, GetConsoleMode, GetStdHandle, STD_INPUT_HANDLE, SetConsoleMode,
    };

    /// Rend à la console son mode d'avant la saisie.
    struct Restore {
        console: HANDLE,
        mode: CONSOLE_MODE,
    }

    impl Drop for Restore {
        fn drop(&mut self) {
            // SAFETY: la poignée est celle de l'entrée standard, qui reste ouverte, et le mode est
            // celui qu'elle avait.
            unsafe { SetConsoleMode(self.console, self.mode) };
        }
    }

    pub fn read_hidden_line() -> io::Result<String> {
        // SAFETY: aucune condition ; rend la poignée de l'entrée standard (que nous ne fermons pas).
        let console = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
        if console.is_null() || console == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        let mut mode: CONSOLE_MODE = 0;
        // SAFETY: `mode` est un entier vivant que la fonction remplit ; elle échoue si l'entrée
        // n'est pas une console.
        if unsafe { GetConsoleMode(console, &mut mode) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: même poignée ; le nouveau mode ne retire que l'écho.
        if unsafe { SetConsoleMode(console, mode & !ENABLE_ECHO_INPUT) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let restore = Restore { console, mode };
        let mut line = String::new();
        let read = io::stdin().lock().read_line(&mut line);
        drop(restore);
        // la touche Entrée n'a pas été affichée non plus
        eprintln!();
        read.map(|_| line)
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod imp {
    use std::io;

    pub fn read_hidden_line() -> io::Result<String> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "lecture sans écho impossible sur ce système"))
    }
}
