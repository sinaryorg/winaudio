use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use winaudio_core::{
    list_sessions, set_mute_by_name, set_volume_by_name, AudioCapture, AudioFormat,
    SessionManager, SessionState, TargetProcess,
};

#[derive(Parser)]
#[command(
    name = "winaudio",
    about = "Modern per-process Windows audio capture, monitoring, and volume control",
    version = "0.1.0"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all active Windows audio sessions and processes
    List {
        /// Show all sessions, including inactive/expired
        #[arg(short, long)]
        all: bool,
    },

    /// Get or set volume for a specific process
    Volume {
        /// Target process ID
        #[arg(short, long)]
        pid: Option<u32>,

        /// Target application executable name (e.g., spotify.exe, discord.exe)
        #[arg(short, long)]
        name: Option<String>,

        /// Set master volume (0.0 to 1.0)
        #[arg(short, long)]
        set: Option<f32>,
    },

    /// Mute, unmute, or toggle mute state for a process
    Mute {
        /// Target process ID
        #[arg(short, long)]
        pid: Option<u32>,

        /// Target application executable name
        #[arg(short, long)]
        name: Option<String>,

        /// Explicitly unmute
        #[arg(short, long)]
        unmute: bool,
    },

    /// Record audio from a specific application without capturing system or microphone sound
    Record {
        /// Record ALL system audio sources (Master Endpoint Loopback)
        #[arg(short, long)]
        all: bool,

        /// Target process ID (can be comma-separated for multiple: e.g. 1234,5678)
        #[arg(short, long)]
        pid: Option<String>,

        /// Target application name (can be comma-separated for multiple: e.g. "game.exe,discord.exe")
        #[arg(short, long)]
        name: Option<String>,

        /// Target window title substring
        #[arg(short, long)]
        window: Option<String>,

        /// Application(s) to EXCLUDE (can be comma-separated: e.g. "spotify.exe,discord.exe")
        #[arg(short, long)]
        exclude: Option<String>,

        /// Output WAV file path
        #[arg(short, long, default_value = "recorded_audio.wav")]
        out: PathBuf,

        /// Recording duration in seconds
        #[arg(short, long, default_value = "5")]
        duration: u64,

        /// Target sample rate in Hz (e.g. 16000 for Whisper/speech, 48000 for high-fidelity)
        #[arg(short, long, default_value = "48000")]
        sample_rate: u32,

        /// Channels (1 for mono, 2 for stereo)
        #[arg(short, long, default_value = "2")]
        channels: u16,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::List { all } => cmd_list(all),
        Commands::Volume { pid, name, set } => cmd_volume(pid, name, set),
        Commands::Mute { pid, name, unmute } => cmd_mute(pid, name, unmute),
        Commands::Record {
            all,
            pid,
            name,
            window,
            exclude,
            out,
            duration,
            sample_rate,
            channels,
        } => cmd_record(
            all,
            pid,
            name,
            window,
            exclude,
            out,
            duration,
            sample_rate,
            channels,
        ),
    }
}

fn cmd_list(show_all: bool) -> Result<()> {
    let sessions = list_sessions().context("Failed to enumerate audio sessions")?;

    println!(
        "\n{:<8} {:<30} {:<30} {:<10} {:<8} {:<10}",
        "PID".bold().cyan(),
        "APPLICATION".bold().cyan(),
        "WINDOW TITLE".bold().cyan(),
        "VOLUME".bold().cyan(),
        "MUTED".bold().cyan(),
        "STATE".bold().cyan(),
    );
    println!("{}", "─".repeat(100).dimmed());

    let mut count = 0;
    for s in sessions {
        if !show_all && s.state != SessionState::Active && s.state != SessionState::Inactive {
            continue;
        }

        let state_str = match s.state {
            SessionState::Active => "Active".green(),
            SessionState::Inactive => "Inactive".yellow(),
            SessionState::Expired => "Expired".red(),
            SessionState::Unknown => "Unknown".normal(),
        };

        let mute_str = if s.is_muted {
            "YES".red()
        } else {
            "NO".green()
        };

        let win_title = s.window_title.as_deref().unwrap_or("-");
        let truncated_title = if win_title.len() > 28 {
            format!("{}...", &win_title[..25])
        } else {
            win_title.to_string()
        };

        println!(
            "{:<8} {:<30} {:<30} {:<10} {:<8} {:<10}",
            s.pid,
            s.display_name.bright_white(),
            truncated_title.dimmed(),
            format!("{:.0}%", s.volume * 100.0).bright_yellow(),
            mute_str,
            state_str,
        );
        count += 1;
    }

    println!("{}", "─".repeat(100).dimmed());
    println!("Total audio sessions displayed: {}\n", count.to_string().bold());
    Ok(())
}

fn cmd_volume(pid: Option<u32>, name: Option<String>, set_val: Option<f32>) -> Result<()> {
    if pid.is_none() && name.is_none() {
        bail!("You must specify either --pid <PID> or --name <NAME>");
    }

    if let Some(target_pid) = pid {
        if let Some(new_vol) = set_val {
            SessionManager::set_volume(target_pid, new_vol)
                .context("Failed to set process volume")?;
            println!(
                "{} Volume for PID {} set to {:.0}%",
                "✔".green(),
                target_pid,
                new_vol * 100.0
            );
        } else {
            let session = SessionManager::find_by_pid(target_pid)?
                .context("Audio session not found for specified PID")?;
            println!(
                "PID {}: {} - Volume: {:.0}%",
                session.pid,
                session.process_name,
                session.volume * 100.0
            );
        }
    } else if let Some(target_name) = name {
        if let Some(new_vol) = set_val {
            set_volume_by_name(&target_name, new_vol)?;
            println!(
                "{} Volume for '{}' set to {:.0}%",
                "✔".green(),
                target_name,
                new_vol * 100.0
            );
        } else {
            let sessions = SessionManager::find_by_name(&target_name)?;
            if sessions.is_empty() {
                bail!("No active audio sessions found for '{}'", target_name);
            }
            for s in sessions {
                println!(
                    "[{}] {} - Volume: {:.0}% (Muted: {})",
                    s.pid,
                    s.process_name,
                    s.volume * 100.0,
                    s.is_muted
                );
            }
        }
    }

    Ok(())
}

fn cmd_mute(pid: Option<u32>, name: Option<String>, unmute: bool) -> Result<()> {
    if pid.is_none() && name.is_none() {
        bail!("You must specify either --pid <PID> or --name <NAME>");
    }

    let target_mute = !unmute;

    if let Some(target_pid) = pid {
        SessionManager::set_mute(target_pid, target_mute)?;
        println!(
            "{} Process PID {} {}",
            "✔".green(),
            target_pid,
            if target_mute { "muted" } else { "unmuted" }
        );
    } else if let Some(target_name) = name {
        set_mute_by_name(&target_name, target_mute)?;
        println!(
            "{} Application '{}' {}",
            "✔".green(),
            target_name,
            if target_mute { "muted" } else { "unmuted" }
        );
    }

    Ok(())
}

fn cmd_record(
    all: bool,
    pid_str: Option<String>,
    name: Option<String>,
    window: Option<String>,
    exclude: Option<String>,
    out: PathBuf,
    duration_secs: u64,
    sample_rate: u32,
    channels: u16,
) -> Result<()> {
    let (target, mode) = if all || (pid_str.is_none() && name.is_none() && window.is_none() && exclude.is_none()) {
        (TargetProcess::All, winaudio_core::CaptureMode::Include)
    } else if let Some(ex) = exclude {
        let ex_list: Vec<String> = ex
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        SessionManager::resolve_multi_exclude(&ex_list)?
    } else if let Some(n) = name {
        let name_list: Vec<String> = n
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        (
            SessionManager::resolve_multi_include(&name_list)?,
            winaudio_core::CaptureMode::Include,
        )
    } else if let Some(p) = pid_str {
        let pid_list: Vec<u32> = p
            .split(',')
            .filter_map(|s| s.trim().parse::<u32>().ok())
            .collect();
        if pid_list.len() > 1 {
            (
                TargetProcess::Multiple(pid_list.into_iter().map(TargetProcess::ByPid).collect()),
                winaudio_core::CaptureMode::Include,
            )
        } else if let Some(&single_pid) = pid_list.first() {
            (TargetProcess::ByPid(single_pid), winaudio_core::CaptureMode::Include)
        } else {
            bail!("Invalid PID provided: {}", p);
        }
    } else if let Some(w) = window {
        (TargetProcess::ByWindowTitle(w), winaudio_core::CaptureMode::Include)
    } else {
        (TargetProcess::All, winaudio_core::CaptureMode::Include)
    };

    println!(
        "{} Initializing audio capture engine...",
        "●".cyan()
    );

    let capture = AudioCapture::builder(target.clone())
        .mode(mode)
        .sample_rate(sample_rate)
        .channels(channels)
        .format(AudioFormat::Pcm16)
        .start()
        .context("Failed to start audio capture engine")?;

    match target {
        TargetProcess::All => {
            println!(
                "{} Capturing ALL system audio [Master Endpoint Loopback] ({} Hz, {}) for {} seconds...",
                "▶".green().bold(),
                sample_rate,
                if channels == 1 { "mono" } else { "stereo" },
                duration_secs
            );
        }
        TargetProcess::Multiple(_) => {
            let pids = capture.resolved_pids();
            println!(
                "{} Capturing & MIXING {} audio streams [{:?}] ({} Hz, {}) for {} seconds...",
                "▶".cyan().bold(),
                pids.len(),
                pids,
                sample_rate,
                if channels == 1 { "mono" } else { "stereo" },
                duration_secs
            );
        }
        _ => {
            let resolved_pid = capture.target_pid();
            let app_info = SessionManager::find_by_pid(resolved_pid)
                .ok()
                .flatten()
                .map(|s| s.display_name)
                .unwrap_or_else(|| format!("PID {}", resolved_pid));

            if mode == winaudio_core::CaptureMode::Exclude {
                println!(
                    "{} Capturing all system audio EXCLUDING PID {} [{}] ({} Hz, {}) for {} seconds...",
                    "▶".yellow().bold(),
                    resolved_pid.to_string().yellow(),
                    app_info.cyan(),
                    sample_rate,
                    if channels == 1 { "mono" } else { "stereo" },
                    duration_secs
                );
            } else {
                println!(
                    "{} Capturing audio from PID {} [{}] ({} Hz, {}) for {} seconds...",
                    "▶".green().bold(),
                    resolved_pid.to_string().yellow(),
                    app_info.cyan(),
                    sample_rate,
                    if channels == 1 { "mono" } else { "stereo" },
                    duration_secs
                );
            }
        }
    }

    let pb = ProgressBar::new(duration_secs);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}s ({msg})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let out_str = out.display().to_string();
    let duration = Duration::from_secs(duration_secs);

    let pb_clone = pb.clone();
    capture
        .record_to_wav_with_progress(&out_str, duration, move |sec| {
            pb_clone.set_position(sec);
        })
        .context("Recording to WAV failed")?;

    pb.finish_with_message("Complete");
    println!(
        "\n{} Successfully saved audio to: {}",
        "✔".green().bold(),
        out_str.bright_cyan().underline()
    );

    Ok(())
}
