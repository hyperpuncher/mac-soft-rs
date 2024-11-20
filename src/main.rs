use dialoguer::{theme::ColorfulTheme, MultiSelect};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use tokio::fs;
use tokio::io;
use tokio::task;
use url::Url;

#[derive(Deserialize)]
struct CaskData {
    url: String,
    variations: Option<HashMap<String, Variation>>,
}

#[derive(Deserialize)]
struct Variation {
    url: String,
}

async fn copy_dir(src: &Path, dest: &Path) -> io::Result<()> {
    let mut stack = vec![src.to_path_buf()];

    while let Some(current_src) = stack.pop() {
        let current_dest = dest.join(current_src.strip_prefix(src).unwrap());

        if current_src.is_dir() {
            fs::create_dir_all(&current_dest).await?;
            let mut entries = fs::read_dir(&current_src).await?;
            while let Some(entry) = entries.next_entry().await? {
                stack.push(entry.path());
            }
        } else if current_src.is_file() {
            fs::copy(&current_src, &current_dest).await?;
        }
    }

    Ok(())
}

async fn download_app(
    app_name: &str,
    macos_version: &str,
    output_dir: &str,
    pb: ProgressBar,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("https://formulae.brew.sh/api/cask/{}.json", app_name);
    let response = reqwest::get(&url).await?.json::<CaskData>().await?;

    let download_url = match &response.variations {
        Some(variations) => variations
            .get(macos_version)
            .map(|v| v.url.clone())
            .unwrap_or(response.url),
        None => response.url,
    };

    // Extract the file name from the URL
    let url_obj = Url::parse(&download_url)?;
    let path_segments = url_obj
        .path_segments()
        .ok_or("Failed to extract path segments")?;
    let file_name_with_extension = path_segments.last().ok_or("Failed to extract file name")?;

    let file_path = format!("{}/{}", output_dir, file_name_with_extension);

    let mut response = reqwest::get(&download_url).await?;

    if let Some(total_size) = response.content_length() {
        let mut downloaded_size = 0u64;

        let mut file = tokio::fs::File::create(&file_path).await?;

        // Download in chunks and update progress bar
        while let Some(chunk) = response.chunk().await? {
            let chunk_size = chunk.len() as u64;
            downloaded_size += chunk_size;
            tokio::io::copy(&mut chunk.as_ref(), &mut file).await?;
            let percentage = (downloaded_size as f64 / total_size as f64) * 100.0;
            pb.set_position(percentage as u64);
        }

        pb.finish_with_message(format!("Downloaded {}", app_name));
    } else {
        pb.finish_with_message(format!("Failed to download {}", app_name));
    }

    Ok(())
}

async fn dmg_installer(dmg: &str, pb: ProgressBar) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("hdiutil").arg("attach").arg(dmg).output()?;
    if !output.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return Ok(());
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let volume_path = output_str
        .lines()
        .last()
        .and_then(|line| line.split("  ").last())
        .unwrap()
        .trim();

    let dest_dir = Path::new("/Applications");

    let mut entries = fs::read_dir(volume_path).await?;

    while let Some(entry) = entries.next_entry().await? {
        if let Some(extension) = entry.path().extension() {
            if extension == "app" {
                pb.set_message(format!(
                    "Installing {}",
                    entry
                        .file_name()
                        .to_string_lossy()
                        .split('.')
                        .next()
                        .unwrap()
                ));
                let dest_path = dest_dir.join(entry.file_name());
                copy_dir(&entry.path(), &dest_path).await?;
                pb.set_position(100);
                pb.finish_with_message(format!(
                    "Installed {}",
                    entry
                        .file_name()
                        .to_string_lossy()
                        .split('.')
                        .next()
                        .unwrap()
                ));
            }
        }
    }

    let output = Command::new("hdiutil")
        .arg("detach")
        .arg("-force")
        .arg(volume_path)
        .output()?;

    if !output.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return Ok(());
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let macos_version = std::env::args().nth(1).unwrap();

    let apps = vec![
        "anydesk",
        "brave-browser",
        "google-chrome",
        "iina",
        "keka",
        "microsoft-excel",
        "microsoft-powerpoint",
        "microsoft-word",
        "rustdesk",
        "skype",
        "telegram-desktop",
        "transmission",
        "viber",
        "whatsapp",
        "zoom",
    ];

    let selections: Option<Vec<usize>> = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select apps to install (Use SPACE to select, ENTER to confirm)")
        .items(&apps)
        .interact_opt()
        .unwrap();

    let selections = match selections {
        Some(selections) => selections,
        None => {
            println!("No apps selected");
            return;
        }
    };

    let output_dir = format!(
        "{}/Downloads/mac-soft-rs",
        dirs::home_dir().unwrap().display()
    );
    fs::create_dir_all(&output_dir)
        .await
        .expect("Failed to create output directory");

    // Convert selections to actual app names
    let selected_apps: Vec<&str> = selections.into_iter().map(|i| apps[i]).collect();

    let mp = MultiProgress::new();

    let mut tasks = vec![];

    for app in &selected_apps {
        let app_name = app.to_string();
        let macos_version = macos_version.clone();
        let output_dir = output_dir.clone();

        // Create a new progress bar for this download
        let pb = mp.add(ProgressBar::new(100));
        pb.set_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
            )
            .unwrap()
            .progress_chars("##-"),
        );
        pb.set_message(format!("Downloading {}", app_name));

        let task = task::spawn(async move {
            match download_app(&app_name, &macos_version, &output_dir, pb).await {
                Ok(_) => (),
                Err(err) => eprintln!("Failed to download {}: {}", app_name, err),
            }
        });
        tasks.push(task);
    }

    // Wait for all downloads to complete
    for task in tasks {
        task.await.unwrap();
    }

    let mut install_tasks = vec![];

    let mut entries = fs::read_dir(output_dir)
        .await
        .expect("Failed to read output directory");

    while let Some(entry) = entries.next_entry().await.expect("Failed to read entry") {
        let path = entry.path();

        if let Some(extension) = path.extension() {
            if extension == "dmg" {
                let pb = mp.add(ProgressBar::new(100));
                pb.set_style(
                    ProgressStyle::with_template(
                        "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
                    )
                    .unwrap()
                    .progress_chars("##-"),
                );

                let dmg_path = path.to_str().unwrap().to_string();

                let task = task::spawn(async move {
                    match dmg_installer(&dmg_path, pb).await {
                        Ok(_) => (),
                        Err(err) => eprintln!("Failed to install {}: {}", dmg_path, err),
                    }
                });
                install_tasks.push(task);
            }
        }
    }

    // Wait for all installations to complete
    for task in install_tasks {
        task.await.unwrap();
    }
}
