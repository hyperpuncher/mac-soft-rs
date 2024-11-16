use clap::Parser;
use dialoguer::{theme::ColorfulTheme, MultiSelect};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use tokio::task;
use url::Url;

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    macos_version: String,
}

#[derive(Deserialize)]
struct CaskData {
    url: String,
    variations: Option<HashMap<String, Variation>>,
}

#[derive(Deserialize)]
struct Variation {
    url: String,
}

async fn download_app(
    app_name: &str,
    macos_version: &str,
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

    let output_dir = format!(
        "{}/Downloads/mac-soft-rs",
        dirs::home_dir().unwrap().display()
    );
    fs::create_dir_all(&output_dir).expect("Failed to create directory");

    // Extract the file name from the URL
    let url_obj = Url::parse(&download_url)?;
    let path_segments = url_obj
        .path_segments()
        .ok_or("Failed to extract path segments")?;
    let file_name_with_extension = path_segments.last().ok_or("Failed to extract file name")?;

    let file_path = format!("{}/{}", output_dir, file_name_with_extension);

    let mut response = reqwest::get(&download_url).await?;

    if let Some(total_size) = response.content_length() {
        pb.set_length(100);
        let mut downloaded_size = 0u64;

        let mut file = tokio::fs::File::create(&file_path).await?;

        // Download in chunks and update progress bar
        while let Some(chunk) = response.chunk().await? {
            let chunk_size = chunk.len() as u64;
            downloaded_size += chunk_size;
            tokio::io::copy(&mut chunk.as_ref(), &mut file).await?;

            // Calculate percentage completed and set progress
            let percentage = (downloaded_size as f64 / total_size as f64) * 100.0;
            pb.set_position(percentage as u64);
        }

        pb.finish_with_message(format!("Downloaded {}", app_name));
    } else {
        pb.finish_with_message(format!("Failed to download {}", app_name));
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let macos_version = args.macos_version;

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

    // Convert selections to actual app names
    let selected_apps: Vec<&str> = selections.into_iter().map(|i| apps[i]).collect();

    let mp = MultiProgress::new();

    let mut tasks = vec![];

    for app in &selected_apps {
        let app_name = app.to_string();
        let macos_version = macos_version.clone();

        // Create a new progress bar for this download
        let pb = mp.add(ProgressBar::new(0));
        pb.set_style(
            ProgressStyle::with_template(
                "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
            )
            .unwrap()
            .progress_chars("##-"),
        );
        pb.set_message(format!("Downloading {}", app_name));

        let task = task::spawn(async move {
            match download_app(&app_name, &macos_version, pb).await {
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
}
