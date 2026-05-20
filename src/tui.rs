use crate::mixer::Mixer;
use nix::sys::termios;
use std::os::fd::{AsFd, OwnedFd};

pub struct RawMode {
    orig: termios::Termios,
    fd: OwnedFd,
}

impl RawMode {
    pub fn enable() -> anyhow::Result<Self> {
        let stdin = std::io::stdin();
        let fd = stdin.as_fd().try_clone_to_owned()?;
        let orig = termios::tcgetattr(fd.as_fd())?;
        let mut raw = orig.clone();
        termios::cfmakeraw(&mut raw);
        termios::tcsetattr(fd.as_fd(), termios::SetArg::TCSANOW, &raw)?;
        Ok(RawMode { orig, fd })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        termios::tcsetattr(self.fd.as_fd(), termios::SetArg::TCSANOW, &self.orig).ok();
    }
}

pub fn display(mixer: &Mixer) {
    use std::io::Write;

    let mut out = std::io::stdout().lock();
    write!(out, "\x1b[2J\x1b[H").ok();
    writeln!(out, "=== noize ===").ok();

    for (i, src) in mixer.sources.iter().enumerate() {
        let marker = if i == mixer.current_index {
            ">"
        } else {
            " "
        };
        let vol = if src.muted {
            src.previous_volume
        } else {
            src.volume
        };
        let bar =
            "#".repeat(vol as usize) + &"-".repeat((mixer.max_volume - vol) as usize);
        let mute_indicator = if src.muted { " [MUTED]" } else { "" };
        let url_info = if src.urls.len() > 1 {
            format!(" [{}/{}]", src.url_index + 1, src.urls.len())
        } else {
            String::new()
        };
        let disconnected_indicator = if src.disconnected {
            " [DISCONNECTED]"
        } else if src.buffering {
            " [BUFFERING...]"
        } else {
            ""
        };

        let name = if src.name.len() > 30 {
            &src.name[..30]
        } else {
            &src.name
        };

        writeln!(
            out,
            "{} {:<30} [{}] {}/{}{}{}{}",
            marker,
            name,
            bar,
            vol,
            mixer.max_volume,
            mute_indicator,
            url_info,
            disconnected_indicator,
        )
        .ok();
    }

    writeln!(
        out,
        "\nControls: j/k=scroll, h/l=volume down/up, m=mute/unmute, space=global mute, 0-9=set volume, q=quit"
    )
    .ok();

    if mixer.global_mute {
        writeln!(out, "*** GLOBAL MUTE ACTIVE ***").ok();
    }

    out.flush().ok();
}
