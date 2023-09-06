use serde::Deserialize;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[derive(Deserialize)]
    struct Blob {
        path: String,
        sha: String,
    }

    // Fetch Kakarot RPC tree
    let url = "https://api.github.com/repos/sayajin-labs/kakarot-rpc/git/trees/main?recursive=1";

    let client = reqwest::blocking::Client::builder();
    let response: serde_json::Value = client
        .user_agent("reqwest-rust")
        .build()?
        .get(url)
        .send()?
        .json()?;

    // Filter the blobs to only include kakarot submodule
    let blobs: Vec<Blob> = serde_json::from_value(response["tree"].clone())?;
    let blobs: Vec<Blob> = blobs
        .into_iter()
        .filter(|b| b.path == "lib/kakarot")
        .collect();

    if blobs.len() != 1 {
        return Err(eyre::eyre!("Expected 1 blob, got {}", blobs.len()).into());
    }

    // Write remote sha to file
    let remote_sha = &blobs[0].sha;

    std::fs::create_dir_all(".katana/").expect("Failed to create Kakata dump dir");
    std::fs::write(".katana/remote_kakarot_sha", remote_sha)?;

    Ok(())
}
