use anyhow::Result;
use pitgun_core::{EventBatch, Sink};
use std::{collections::HashMap, fs::File, path::PathBuf};

pub struct CsvSink {
    dir: PathBuf,
    writers: HashMap<String, csv::Writer<File>>,
}

impl CsvSink {
    pub fn new(dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            writers: HashMap::new(),
        })
    }

    fn writer_for_channel(&self, channel: &str) -> Option<csv::Writer<File>> {
        let path = self.dir.join(format!("{}.csv", channel));
        match csv::Writer::from_path(path) {
            Ok(mut writer) => {
                if writer.write_record(&["Timestamp", "ChannelValue"]).is_err() {
                    eprintln!(
                        "pitgun-cli: failed to write CSV header for channel {}",
                        channel
                    );
                    None
                } else {
                    Some(writer)
                }
            }
            Err(err) => {
                eprintln!("pitgun-cli: failed to create CSV file: {err}");
                None
            }
        }
    }

    fn ensure_writer(&mut self, channel: &str) -> Option<&mut csv::Writer<File>> {
        if !self.writers.contains_key(channel) {
            if let Some(writer) = self.writer_for_channel(channel) {
                self.writers.insert(channel.to_string(), writer);
            } else {
                return None;
            }
        }
        self.writers.get_mut(channel)
    }
}

impl Sink for CsvSink {
    fn write(&mut self, batch: &EventBatch) {
        for event in &batch.events {
            let Some(writer) = self.ensure_writer(&event.channel) else {
                continue;
            };
            if writer
                .write_record(&[event.ts_ns.to_string(), event.value.to_string()])
                .is_err()
            {
                eprintln!(
                    "pitgun-cli: failed to write CSV record for {}",
                    event.channel
                );
            }
        }
    }
}
