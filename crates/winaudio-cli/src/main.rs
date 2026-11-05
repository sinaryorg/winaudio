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
        /// Target process ID
        #[arg(short, long)]
        pid: Option<u32>,

        /// Target process executable name (e.g., spotify.exe, chrome.exe)
        #[arg(short, long)]
        name: Option<String>,

        /// Target window title substring
        #[arg(short, long)]
        window: Option<String>,

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
            pid,
            name,
            window,
            out,
            duration,
            sample_rate,
            channels,
        } => cmd_record(pid, name, window, out, duration, sample_rate, channels),
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
    pid: Option<u32>,
    name: Option<String>,
    window: Option<String>,
    out: PathBuf,
    duration_secs: u64,
    sample_rate: u32,
    channels: u16,
) -> Result<()> {
    let target = if let Some(p) = pid {
        TargetProcess::ByPid(p)
    } else if let Some(n) = name {
        TargetProcess::ByName(n)
    } else if let Some(w) = window {
        TargetProcess::ByWindowTitle(w)
    } else {
        bail!("You must specify --pid <PID>, --name <NAME>, or --window <TITLE>");
    };

    println!(
        "{} Initializing process loopback audio capture...",
        "●".cyan()
    );

    let capture = AudioCapture::builder(target)
        .sample_rate(sample_rate)
        .channels(channels)
        .format(AudioFormat::Pcm16)
        .start()
        .context("Failed to start process loopback audio capture")?;

    let resolved_pid = capture.target_pid();
    let app_info = SessionManager::find_by_pid(resolved_pid)
        .ok()
        .flatten()
        .map(|s| s.display_name)
        .unwrap_or_else(|| format!("PID {}", resolved_pid));

    println!(
        "{} Capturing audio from PID {} [{}] ({} Hz, {}) for {} seconds...",
        "▶".green().bold(),
        resolved_pid.to_string().yellow(),
        app_info.cyan(),
        sample_rate,
        if channels == 1 { "mono" } else { "stereo" },
        duration_secs
    );

    let pb = ProgressBar::new(duration_secs);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}s ({msg})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let out_str = out.display().to_string();
    let duration = Duration::from_secs(duration_secs);

    capture
        .record_to_wav(&out_str, duration)
        .context("Recording to WAV failed")?;

    pb.finish_with_message("Complete");
    println!(
        "\n{} Successfully saved audio to: {}",
        "✔".green().bold(),
        out_str.bright_cyan().underline()
    );

    Ok(())
}
