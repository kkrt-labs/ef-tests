use chrono::{DateTime, Utc};
use dotenv::dotenv;
use reqwest::{self, header};
use serde::Deserialize;
use std::path::Path;
use std::{fs, io};
use zip::ZipArchive;

#[derive(Deserialize, Debug)]
struct Artifact {
    name: String,
    updated_at: DateTime<Utc>,
    workflow_run: WorkflowRun,
    archive_download_url: String,
}

#[derive(Deserialize, Debug)]
struct WorkflowRun {
    head_branch: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let token =
        std::env::var("GITHUB_TOKEN").map_err(|_| eyre::eyre!("Missing GITHUB_TOKEN in .env"))?;
    let url = "https://api.github.com/repos/sayajin-labs/kakarot-rpc/actions/artifacts";

    let client = reqwest::blocking::Client::builder();
    let response: serde_json::Value = client
        .user_agent("reqwest-rust")
        .build()?
        .get(url)
        .send()?
        .json()?;

    // Filter the artifacts to only include the ones we care about
    // and sort by updated_at
    let artifacts: Vec<Artifact> = serde_json::from_value(response["artifacts"].clone())?;
    let mut artifacts = artifacts
        .into_iter()
        .filter(|artifact| {
            artifact.name == "dump-katana" && artifact.workflow_run.head_branch == "main"
        })
        .collect::<Vec<_>>();
    artifacts.sort_by_key(|artifact| artifact.updated_at);
    artifacts.reverse();

    // Find the latest artifact
    let latest_artifact = artifacts
        .first()
        .ok_or_else(|| eyre::eyre!("Missing artifact value for Katana dump"))?;

    // Download the artifact
    let client = reqwest::blocking::Client::builder();
    let mut headers = header::HeaderMap::new();
    let mut auth = header::HeaderValue::from_str(&format!("Bearer {}", token))?;
    auth.set_sensitive(true);
    headers.insert(header::AUTHORIZATION, auth);
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("application/vnd.github+json"),
    );

    let mut response = client
        .user_agent("reqwest-rust")
        .gzip(true)
        .default_headers(headers)
        .build()?
        .get(&latest_artifact.archive_download_url)
        .send()?;

    let mut out = fs::File::create("temp.zip")?;
    io::copy(&mut response, &mut out)?;

    // Unzip the artifact
    unzip_file("temp.zip", ".katana")?;
    fs::remove_file("temp.zip")?;

    Ok(())
}

fn unzip_file(source: &str, destination: &str) -> io::Result<()> {
    // Open the ZIP archive
    let reader = fs::File::open(source)?;
    let mut archive = ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = Path::new(destination).join(file.name());

        if file.name().ends_with('/') {
            // It's a directory, create it
            fs::create_dir_all(&outpath)?;
        } else {
            // It's a file, extract it
            if let Some(parent) = outpath.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)?;
                }
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}
