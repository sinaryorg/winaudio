use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use winaudio_core::{
    list_sessions, set_mute_by_name, set_volume_by_name, AudioCapture, AudioFormat, SessionManager,
    SessionState, TargetProcess,
};

#[allow(clippy::upper_case_acronyms)]
type BOOL = i32;

static STOP_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> BOOL {
    if ctrl_type == 0 || ctrl_type == 1 {
        STOP_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
        1
    } else {
        0
    }
}

extern "system" {
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> BOOL>,
        add: BOOL,
    ) -> BOOL;
}

#[derive(Parser)]
#[command(
    name = "winaudio",
    about = "Modern per-process Windows audio capture, monitoring, and volume control",
    version = "0.1.0",
    after_help = "Run 'winaudio commands' or 'winaudio examples' to display all runnable command lines and real-world examples."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
#[allow(clippy::enum_variant_names)]
enum Commands {
    /// List all active Windows audio sessions and processes
    List {
        /// Show all sessions, including inactive/expired
        #[arg(short, long)]
        all: bool,
    },

    /// Display all runnable command lines, copy-pasteable examples, and cheatsheet
    #[command(alias = "examples", alias = "cheatsheet", alias = "guide")]
    Commands,

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

        /// Recording duration in seconds (optional: omit to record indefinitely until Ctrl+C)
        #[arg(short, long)]
        duration: Option<u64>,

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
        None | Some(Commands::Commands) => cmd_commands(),
        Some(Commands::List { all }) => cmd_list(all),
        Some(Commands::Volume { pid, name, set }) => cmd_volume(pid, name, set),
        Some(Commands::Mute { pid, name, unmute }) => cmd_mute(pid, name, unmute),
        Some(Commands::Record {
            all,
            pid,
            name,
            window,
            exclude,
            out,
            duration,
            sample_rate,
            channels,
        }) => cmd_record(
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

fn cmd_commands() -> Result<()> {
    println!(
        "\n{}",
        "🔊 WinAudio — Runnable Commands Cheatsheet"
            .bright_cyan()
            .bold()
    );
    println!("{}", "═".repeat(88).cyan());

    println!("\n{}", "1. DISCOVER ACTIVE AUDIO SESSIONS".yellow().bold());
    println!(
        "  {:<52} {}",
        "winaudio list".green().bold(),
        "List active applications playing audio with PID, volume & state".dimmed()
    );
    println!(
        "  {:<52} {}",
        "winaudio list --all".green().bold(),
        "Show all sessions (including inactive/closed apps)".dimmed()
    );

    println!(
        "\n{}",
        "2. SINGLE APPLICATION AUDIO RECORDING".yellow().bold()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --name Riff".green().bold(),
        "Record audio only from Riff (5s default, out: recorded_audio.wav)".dimmed()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --name chrome.exe --duration 10"
            .green()
            .bold(),
        "Record audio only from Chrome for 10 seconds".dimmed()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --pid 15012 --out app.wav".green().bold(),
        "Record specific process by its PID".dimmed()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --window \"YouTube\" --duration 15"
            .green()
            .bold(),
        "Record application matching window title substring".dimmed()
    );

    println!(
        "\n{}",
        "3. RECORD ALL AUDIO SOURCES (MASTER SYSTEM LOOPBACK)"
            .yellow()
            .bold()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --all --out system_audio.wav"
            .green()
            .bold(),
        "Record everything playing across your entire PC".dimmed()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --all --duration 30 --out all.wav"
            .green()
            .bold(),
        "Record all PC audio for 30 seconds".dimmed()
    );

    println!(
        "\n{}",
        "4. MULTI-APPLICATION MIXING (RECORD MULTIPLE APPS)"
            .yellow()
            .bold()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --name \"Riff,chrome\" --out mix.wav"
            .green()
            .bold(),
        "Mix Riff + Chrome into a single audio file".dimmed()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --name \"game.exe,discord.exe\""
            .green()
            .bold(),
        "Mix game audio and Discord voice chat".dimmed()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --pid 21632,15012 --duration 15"
            .green()
            .bold(),
        "Mix multiple applications by comma-separated PIDs".dimmed()
    );

    println!(
        "\n{}",
        "5. EXCLUDE AUDIO RECORDING (STREAMER / CLIPPING MODE)"
            .yellow()
            .bold()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --exclude spotify.exe".green().bold(),
        "Record all PC sound EXCEPT Spotify".dimmed()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --exclude \"Riff,steam\" --out clean.wav"
            .green()
            .bold(),
        "Record all PC sound EXCEPT Riff and Steam".dimmed()
    );

    println!(
        "\n{}",
        "6. AUDIO FORMAT & AI / SPEECH-TO-TEXT PRESETS"
            .yellow()
            .bold()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --name Riff -s 16000 -c 1".green().bold(),
        "16kHz mono (Whisper / Speech-to-Text AI preset)".dimmed()
    );
    println!(
        "  {:<52} {}",
        "winaudio record --name Riff -s 48000 -c 2".green().bold(),
        "48kHz stereo (CD / Studio quality default)".dimmed()
    );

    println!("\n{}", "7. VOLUME & MUTE CONTROL".yellow().bold());
    println!(
        "  {:<52} {}",
        "winaudio volume --name spotify.exe --set 0.50"
            .green()
            .bold(),
        "Set Spotify master volume to 50%".dimmed()
    );
    println!(
        "  {:<52} {}",
        "winaudio volume --pid 15012 --set 0.80".green().bold(),
        "Set volume by PID to 80%".dimmed()
    );
    println!(
        "  {:<52} {}",
        "winaudio mute --name zoom.exe".green().bold(),
        "Toggle mute state for Zoom".dimmed()
    );
    println!(
        "  {:<52} {}",
        "winaudio mute --name zoom.exe --unmute".green().bold(),
        "Unmute Zoom explicitly".dimmed()
    );

    println!("\n{}", "═".repeat(88).cyan());
    println!(
        "{}\n",
        "💡 Tip: You can run any command above directly in PowerShell or CMD.".bright_black()
    );

    Ok(())
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
    println!(
        "Total audio sessions displayed: {}\n",
        count.to_string().bold()
    );
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

#[allow(clippy::too_many_arguments)]
fn cmd_record(
    all: bool,
    pid_str: Option<String>,
    name: Option<String>,
    window: Option<String>,
    exclude: Option<String>,
    out: PathBuf,
    duration_secs: Option<u64>,
    sample_rate: u32,
    channels: u16,
) -> Result<()> {
    let (target, mode) = if all
        || (pid_str.is_none() && name.is_none() && window.is_none() && exclude.is_none())
    {
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
            (
                TargetProcess::ByPid(single_pid),
                winaudio_core::CaptureMode::Include,
            )
        } else {
            bail!("Invalid PID provided: {}", p);
        }
    } else if let Some(w) = window {
        (
            TargetProcess::ByWindowTitle(w),
            winaudio_core::CaptureMode::Include,
        )
    } else {
        (TargetProcess::All, winaudio_core::CaptureMode::Include)
    };

    println!("{} Initializing audio capture engine...", "●".cyan());

    let capture = AudioCapture::builder(target.clone())
        .mode(mode)
        .sample_rate(sample_rate)
        .channels(channels)
        .format(AudioFormat::Pcm16)
        .start()
        .context("Failed to start audio capture engine")?;

    let dur_display = match duration_secs {
        Some(s) if s > 0 => format!("for {} seconds [Ctrl+C to stop early]", s),
        _ => "indefinitely [Press Ctrl+C to stop recording]".to_string(),
    };

    match target {
        TargetProcess::All => {
            println!(
                "{} Capturing ALL system audio [Master Endpoint Loopback] ({} Hz, {}) {}...",
                "▶".green().bold(),
                sample_rate,
                if channels == 1 { "mono" } else { "stereo" },
                dur_display
            );
        }
        TargetProcess::Multiple(_) => {
            let pids = capture.resolved_pids();
            println!(
                "{} Capturing & MIXING {} audio streams [{:?}] ({} Hz, {}) {}...",
                "▶".cyan().bold(),
                pids.len(),
                pids,
                sample_rate,
                if channels == 1 { "mono" } else { "stereo" },
                dur_display
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
                    "{} Capturing all system audio EXCLUDING PID {} [{}] ({} Hz, {}) {}...",
                    "▶".yellow().bold(),
                    resolved_pid.to_string().yellow(),
                    app_info.cyan(),
                    sample_rate,
                    if channels == 1 { "mono" } else { "stereo" },
                    dur_display
                );
            } else {
                println!(
                    "{} Capturing audio from PID {} [{}] ({} Hz, {}) {}...",
                    "▶".green().bold(),
                    resolved_pid.to_string().yellow(),
                    app_info.cyan(),
                    sample_rate,
                    if channels == 1 { "mono" } else { "stereo" },
                    dur_display
                );
            }
        }
    }

    // Set Windows console Ctrl+C handler
    unsafe {
        SetConsoleCtrlHandler(Some(console_ctrl_handler), 1);
    }

    let pb = match duration_secs {
        Some(s) if s > 0 => {
            let p = ProgressBar::new(s);
            p.set_style(
                ProgressStyle::default_bar()
                    .template(
                        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}s ({msg})",
                    )
                    .unwrap()
                    .progress_chars("#>-"),
            );
            p.set_message("Recording... Ctrl+C to finish");
            p
        }
        _ => {
            let p = ProgressBar::new_spinner();
            p.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} [{elapsed_precise}] {msg}")
                    .unwrap(),
            );
            p.set_message("Recording live... Press Ctrl+C to stop");
            p
        }
    };

    let out_str = out.display().to_string();
    let target_duration = duration_secs.filter(|&s| s > 0).map(Duration::from_secs);

    let stop_signal = capture.stop_signal().clone();
    let pb_clone = pb.clone();
    let actual_dur = capture
        .record_to_wav_with_progress(&out_str, target_duration, move |sec| {
            if STOP_REQUESTED.load(std::sync::atomic::Ordering::Relaxed) {
                stop_signal.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            if let Some(target_sec) = duration_secs {
                if target_sec > 0 {
                    pb_clone.set_position(sec);
                } else {
                    pb_clone
                        .set_message(format!("Recording live ({}s)... Press Ctrl+C to stop", sec));
                    pb_clone.tick();
                }
            } else {
                pb_clone.set_message(format!("Recording live ({}s)... Press Ctrl+C to stop", sec));
                pb_clone.tick();
            }
        })
        .context("Recording to WAV failed")?;

    pb.finish_with_message("Complete");

    let total_secs = actual_dur.as_secs();
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    let time_str = if hours > 0 {
        format!("{}h {}m {}s", hours, mins, secs)
    } else if mins > 0 {
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", secs)
    };

    println!(
        "\n{} Successfully saved audio to: {} (Duration: {})",
        "✔".green().bold(),
        out_str.bright_cyan().underline(),
        time_str.yellow().bold()
    );

    Ok(())
}
