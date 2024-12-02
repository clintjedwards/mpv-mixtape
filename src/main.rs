use rand::seq::SliceRandom;
use rand::Rng;
use std::env;
use std::fs::{self, File};
use std::io;
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn main() -> std::io::Result<()> {
    // Get the directory and playback duration from command-line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <video_directory> <clip_duration>", args[0]);
        return Ok(());
    }

    let video_dir = &args[1];
    let clip_duration: u64 = args[2]
        .parse()
        .expect("Second argument must be a positive integer representing clip duration in seconds");

    let video_extensions = vec!["mp4", "mkv", "avi", "mov"]; // Add more as needed

    // Get a list of all video files in the directory
    let mut videos: Vec<_> = fs::read_dir(video_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_file()
                && video_extensions
                    .iter()
                    .any(|ext| path.extension().unwrap().to_str().unwrap() == *ext)
            {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    if videos.is_empty() {
        println!("No videos found in the specified directory: {}", video_dir);
        return Ok(());
    }

    // Shuffle the videos
    let mut rng = rand::thread_rng();
    videos.shuffle(&mut rng);

    println!("Found {} videos in {}.", videos.len(), video_dir);
    println!(
        "Generating EDL file with {} seconds for each clip...",
        clip_duration
    );

    // Create an EDL file
    let edl_path = "/tmp/playlist.edl";
    let mut edl_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true) // Ensure the file is emptied
        .open(edl_path)?;

    writeln!(edl_file, "# mpv EDL v0")?;

    for video in &videos {
        // Get video duration
        let duration_secs = match get_video_duration(video) {
            Ok(secs) => secs,
            Err(e) => {
                println!("Could not get video duration for video: {:#?} {e}", video);
                15 // Default to 15 seconds if unavailable
            }
        };

        // Pick a random start time, clamping to ensure a valid segment
        let max_start_time = duration_secs.saturating_sub(clip_duration);
        let start_time = rng.gen_range(0..=max_start_time);

        println!("Adding {:?}: start={}s", video, start_time);

        // Write to the EDL file
        writeln!(
            edl_file,
            "{},{},{}",
            path_to_edl(video.to_string_lossy().as_ref()),
            start_time,
            clip_duration
        )?;
    }

    println!("EDL file generated: {}", edl_path);
    println!("Starting playback...");

    // Launch mpv with the EDL file
    let status = std::process::Command::new("mpv").arg(edl_path).status();

    if let Err(err) = status {
        eprintln!("Error starting playback: {}", err);
    }

    Ok(())
}

/// https://github.com/mpv-player/mpv/blob/master/DOCS/edl-mpv.rst#syntax-of-mpv-edl-files
/// Because EDL filenames need to be in a certain format, we use a length specifier.
fn path_to_edl(path: &str) -> String {
    format!("%{}%{}", path.len(), path)
}

fn get_video_duration(path: &Path) -> Result<u64, io::Error> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path)
        .output()?;

    // Check if the command succeeded
    if !output.status.success() {
        // Include stderr in the error message
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "ffprobe failed for file {:?} with status {:?}: {}",
                path,
                output.status,
                stderr.trim()
            ),
        ));
    }

    // Parse the duration from the stdout
    let duration_str = String::from_utf8_lossy(&output.stdout);
    duration_str
        .trim()
        .parse::<f64>()
        .map(|d| d as u64)
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Failed to parse duration for file {:?}: {}",
                    path,
                    duration_str.trim()
                ),
            )
        })
}
