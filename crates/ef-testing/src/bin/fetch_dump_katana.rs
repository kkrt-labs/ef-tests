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

/// Fetches the Katana dump from Kakarot-RPC artifacts
/// On every PR and merge, thanks to a CI job in Kakarot-RPC, the state of a Starknet devnet
/// with all of Kakarot Cairo smart contracts deployed is dumped in an artifact called 'dump-katana'
/// Starting a Starknet local chain from checkpoint speeds up all our processes.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let token =
        std::env::var("GITHUB_TOKEN").map_err(|_| eyre::eyre!("Missing GITHUB_TOKEN in .env"))?;
    let url = "https://api.github.com/repos/kkrt-labs/kakarot-rpc/actions/artifacts";

    let client = reqwest::blocking::Client::builder()
        .user_agent("reqwest-rust")
        .build()?;
    let response: serde_json::Value = client.get(url).send()?.json()?;

    // Filter the artifacts to only include dump-katana artifacts
    // and find the latest one by key `updated_at`
    let artifacts: Vec<Artifact> = serde_json::from_value(response["artifacts"].clone())?;
    let latest_artifact = artifacts
        .into_iter()
        .filter(|artifact| {
            artifact.name == "dump-katana" && artifact.workflow_run.head_branch == "main"
        })
        .max_by_key(|artifact| artifact.updated_at)
        .ok_or_else(|| eyre::eyre!("Missing artifact value for Katana dump"))?;

    // Download the artifact
    let mut headers = header::HeaderMap::new();
    let mut auth = header::HeaderValue::from_str(&format!("Bearer {}", token))?;
    auth.set_sensitive(true);
    headers.insert(header::AUTHORIZATION, auth);
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("application/vnd.github+json"),
    );

    let client_gzip = reqwest::blocking::Client::builder()
        .user_agent("reqwest-rust-with-gzip")
        .gzip(true)
        .default_headers(headers)
        .build()?;
    let mut response = client_gzip
        .get(latest_artifact.archive_download_url)
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
