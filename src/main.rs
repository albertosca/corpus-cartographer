//! corpus-cartographer: reads the export document and, when given, embeddings, writes links.json and
//! the two map specs into --out-dir. Each file is written to a temporary
//! name and renamed, so a reader never sees half a document.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use corpus_cartographer::embeddings::Embeddings;
use corpus_cartographer::links::{DEFAULT_FLOOR, DEFAULT_MAX_LINKS, LinkOptions};

#[derive(Parser)]
#[command(
    name = "corpus-cartographer",
    version,
    about = "Lays out a corpus map and finds links between items"
)]
struct Options {
    /// The engine-export document written by the host app.
    #[arg(long)]
    input: PathBuf,
    /// The directory holding vocab.json and embeddings.safetensors. Without
    /// it every term the export names is its own axis.
    #[arg(long, alias = corpus_cartographer::legacy_v2::EMBEDDINGS_FLAG_ALIAS)]
    embeddings_dir: Option<PathBuf>,
    /// Where links.json, map-spec.v1.json and map-spec.nested.json go.
    #[arg(long)]
    out_dir: PathBuf,
    #[arg(long, default_value_t = DEFAULT_MAX_LINKS)]
    max_links: usize,
    /// Cosine similarity below which no link is drawn, between -1 and 1.
    #[arg(long, default_value_t = DEFAULT_FLOOR, allow_negative_numbers = true)]
    floor: f64,
}

fn main() -> ExitCode {
    match execute(&Options::parse()) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn execute(options: &Options) -> Result<String, Box<dyn Error>> {
    if !(-1.0..=1.0).contains(&options.floor) {
        return Err(format!("--floor must be between -1 and 1, got {}", options.floor).into());
    }
    let input_text = fs::read_to_string(&options.input)
        .map_err(|error| format!("input: {}: {error}", options.input.display()))?;
    let link_options = LinkOptions {
        max_links: options.max_links,
        floor: options.floor,
    };
    let output = match &options.embeddings_dir {
        Some(directory) => {
            corpus_cartographer::run(&input_text, &Embeddings::load(directory)?, &link_options)?
        }
        None => corpus_cartographer::run_without_embeddings(&input_text, &link_options)?,
    };
    fs::create_dir_all(&options.out_dir)?;
    write_atomically(
        &options.out_dir.join("map-spec.v1.json"),
        &output.map_spec_v1,
    )?;
    write_atomically(
        &options.out_dir.join("map-spec.nested.json"),
        &output.map_spec_nested,
    )?;
    write_atomically(&options.out_dir.join("links.json"), &output.links_json)?;
    Ok(output.summary)
}

fn write_atomically(path: &Path, contents: &str) -> Result<(), Box<dyn Error>> {
    let temporary = path.with_extension("json.partial");
    fs::write(&temporary, contents)?;
    fs::rename(&temporary, path)?;
    Ok(())
}
