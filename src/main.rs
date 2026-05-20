mod config;
mod mixer;
mod state;
mod tui;

use clap::Parser;
use mixer::Mixer;
use nix::poll::{poll, PollFd, PollFlags};
use std::io::Read;
use std::os::fd::AsFd;

#[derive(Parser)]
#[command(name = "noize")]
struct Args {
    #[arg(long, default_value = "config.json")]
    config: String,
    #[arg(long)]
    fresh: bool,
}

fn main() -> anyhow::Result<()> {
    // mpv requires LC_NUMERIC to be "C" for proper operation
    unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()); }

    let args = Args::parse();
    let mut mixer = Mixer::new(&args.config, args.fresh)?;
    let _raw = tui::RawMode::enable()?;

    let stdin = std::io::stdin();
    let mut buffer = String::new();
    let mut last_digit_time = std::time::Instant::now();
    let digit_timeout = std::time::Duration::from_millis(300);

    tui::display(&mixer);
    mixer.clear_dirty();

    loop {
        let now = std::time::Instant::now();

        mixer.tick_retry();
        mixer.check_connection_states();
        mixer.drain_events();

        if !buffer.is_empty() && now - last_digit_time > digit_timeout {
            if let Ok(num) = buffer.parse::<i32>() && num <= mixer.max_volume {
                mixer.set_volume_direct(num);
            }
            buffer.clear();
        }

        if mixer.dirty {
            tui::display(&mixer);
            mixer.clear_dirty();
        }

        let fd = stdin.as_fd();
        let mut fds = [PollFd::new(fd, PollFlags::POLLIN)];

        if let Ok(n) = poll(&mut fds, 100u16) && n > 0 {
            let mut byte = [0u8; 1];
            if stdin.lock().read_exact(&mut byte).is_ok() {
                let key = byte[0] as char;
                match key {
                    'q' => break,
                    'j' => {
                        buffer.clear();
                        mixer.scroll_down();
                    }
                    'k' => {
                        buffer.clear();
                        mixer.scroll_up();
                    }
                    'h' => {
                        buffer.clear();
                        mixer.adjust_volume(-mixer.volume_step);
                    }
                    'l' => {
                        buffer.clear();
                        mixer.adjust_volume(mixer.volume_step);
                    }
                    'm' => {
                        buffer.clear();
                        mixer.toggle_mute();
                    }
                    ' ' => {
                        buffer.clear();
                        mixer.toggle_global_mute();
                    }
                    '0' if buffer.is_empty() => {
                        mixer.set_volume_direct(0);
                        buffer.clear();
                    }
                    d if d.is_ascii_digit() => {
                        buffer.push(d);
                        if buffer == "10" {
                            mixer.set_volume_direct(10);
                            buffer.clear();
                        } else if buffer.len() >= 2 {
                            if let Ok(num) = buffer.parse::<i32>() && num <= mixer.max_volume {
                                mixer.set_volume_direct(num);
                            }
                            buffer.clear();
                        }
                        last_digit_time = now;
                    }
                    _ => {}
                }
            }
        }
    }

    mixer.save_state();

    Ok(())
}
