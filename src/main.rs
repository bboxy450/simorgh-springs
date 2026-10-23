use clap::{Parser, Subcommand};
use futures_util::TryStreamExt;
use simorgh_springs::{
    Bbox, Client, ClientConfig, Gnis, Noaa, Osm, Progress, Source, Wikidata, merge,
};
use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

#[derive(Parser)]
#[command(version, about = "Fetch and merge open spring datasets")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Fetch sources; atomically replace output only after every pass succeeds.
    Sync {
        #[arg(long)]
        out: PathBuf,
        #[arg(long,value_delimiter=',',default_value="osm,wikidata,noaa,gnis",value_parser=["osm","wikidata","noaa","gnis"])]
        source: Vec<String>,
        #[arg(long,allow_hyphen_values=true,value_parser=parse_bbox)]
        bbox: Option<Bbox>,
    },
    /// Run the embedded fixture checks, entirely offline.
    Verify,
}
fn parse_bbox(s: &str) -> Result<Bbox, String> {
    let values = s
        .split(',')
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if values.len() != 4 {
        return Err("expected S,W,N,E".into());
    }
    Bbox::new(values[0], values[1], values[2], values[3]).map_err(|e| e.to_string())
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Verify => {
            for (name, count) in simorgh_springs::verification::verify()? {
                println!("{name}: ok ({count})");
            }
        }
        Command::Sync { out, source, bbox } => {
            let client = Client::new(ClientConfig::default())?;
            let sources: Vec<(&str, Box<dyn Source>)> = vec![
                (
                    "osm",
                    Box::new(Osm {
                        bbox,
                        ..Osm::default()
                    }),
                ),
                (
                    "wikidata",
                    Box::new(Wikidata {
                        bbox,
                        ..Wikidata::default()
                    }),
                ),
                (
                    "noaa",
                    Box::new(Noaa {
                        bbox,
                        ..Noaa::default()
                    }),
                ),
                (
                    "gnis",
                    Box::new(Gnis {
                        bbox,
                        ..Gnis::default()
                    }),
                ),
            ];
            let mut records = Vec::new();
            for (name, fetcher) in sources {
                if !source.iter().any(|s| s == name) {
                    continue;
                }
                let progress = Progress::default();
                records.extend(
                    fetcher
                        .fetch(&client, &progress)
                        .try_collect::<Vec<_>>()
                        .await?,
                );
                eprintln!(
                    "{name}: {} records, {} rejected, {} requests",
                    progress.records(),
                    progress.rejected(),
                    progress.requests()
                );
            }
            let parent = out
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            let mut temp = tempfile::NamedTempFile::new_in(parent)?;
            {
                let mut writer = BufWriter::new(temp.as_file_mut());
                for spring in merge(records) {
                    serde_json::to_writer(&mut writer, &spring)?;
                    writer.write_all(b"\n")?;
                }
                writer.flush()?;
            }
            temp.persist(&out)?;
        }
    }
    Ok(())
}
