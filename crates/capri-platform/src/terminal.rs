//! Lire une ligne tapée au clavier sans l'afficher : pour donner une clé d'API dans un terminal.
//!
//! L'écho du terminal est coupé le temps de la saisie, puis remis exactement comme il était, même
//! si la lecture échoue. Unix (macOS, Linux) : `termios` ; Windows : le mode de la console.
//! Ailleurs, ou si l'entrée n'est pas un terminal : une erreur, sans rien lire.

use std::io;

/// Lit une ligne sans l'afficher, et la rend sans son retour à la ligne. Le curseur passe à la
/// ligne suivante quand on valide.
pub fn read_hidden_line() -> io::Result<String> {
    imp::read_hidden_line().map(|line| line.trim_end_matches(['\r', '\n']).to_owned())
}

#[cfg(unix)]
mod imp {
    use std::io::{self, BufRead};
    use std::mem::MaybeUninit;

    /// Rend au terminal ses réglages d'avant la saisie.
    struct Restore(libc::termios);

    impl Drop for Restore {
        fn drop(&mut self) {
            // SAFETY: `self.0` est une structure entière, lue par `tcgetattr` sur ce même
            // descripteur, qui vit le temps de l'appel.
            unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.0) };
        }
    }

    pub fn read_hidden_line() -> io::Result<String> {
        let mut original = MaybeUninit::<libc::termios>::uninit();
        // SAFETY: `tcgetattr` remplit la structure qu'on lui donne, ou échoue sans qu'on la lise.
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, original.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `tcgetattr` a réussi : la structure est entièrement remplie.
        let original = unsafe { original.assume_init() };
        let mut hidden = original;
        // plus d'écho, sauf celui du retour à la ligne
        hidden.c_lflag &= !libc::ECHO;
        hidden.c_lflag |= libc::ECHONL;
        // SAFETY: `hidden` est une structure entière, copie de celle du terminal ; TCSAFLUSH jette
        // ce qui aurait été tapé avant la question.
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &hidden) } != 0 {
            return Err(io::Error::last_os_error());
        }
        let _restore = Restore(original);
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        Ok(line)
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

#[cfg(not(any(unix, windows)))]
mod imp {
    use std::io;

    pub fn read_hidden_line() -> io::Result<String> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "lecture sans écho impossible sur ce système"))
    }
}
