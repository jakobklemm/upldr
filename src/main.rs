use anyhow::Result;
use rusqlite::Connection;
use rusqlite::Row;

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct Torrent {
    id: u64,
    hash: String,
    name: String,
    size: u64,
    seeders: u64,
    leechers: u64,
    num_files: u64,
    poster: String,
    url: String,
    uploaded: u64,
    files: Vec<File>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct File {
    name: String,
    size: usize,
}

impl File {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            name: row.get(1)?,
            size: row.get(2)?,
        })
    }
}

impl Torrent {
    fn from_row(row: &Row) -> Result<Self> {
        let num_files = row.get(7)?;

        let mut t = Self {
            id: row.get(0)?,
            hash: row.get(1)?,
            name: row.get(2)?,
            size: row.get(3)?,
            uploaded: row.get(4)?,
            seeders: row.get(5)?,
            leechers: row.get(6)?,
            // Maybe make image dependent on number of seeders => color for availability
            poster: String::from("https://s3.jeykey.net/public/images/torrent.png"),
            url: String::new(),
            num_files,
            files: Vec::with_capacity(num_files as usize),
        };

        let url = format!("magnet:?xt=urn:btih:{}&dn={}", t.hash, t.name);
        t.url = url;

        Ok(t)
    }

    fn files(&mut self, conn: &Connection) -> Result<()> {
        let query = format!("SELECT id, name, size FROM files WHERE id={};", self.id);
        let mut sm = conn.prepare(&query)?;
        let iters = sm.query_map([], |row| {
            if let Ok(file) = File::from_row(&row) {
                Ok(file)
            } else {
                Err(rusqlite::Error::InvalidQuery)
            }
        })?;

        for file in iters {
            if let Ok(f) = file {
                self.files.push(f);
            }
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let query = "SELECT ROWID, hex(infohash), name, size, uploaded, seeders, leechers, num_files FROM torrents;";

    let conn = rusqlite::Connection::open("data/torrents.sqlite")?;
    let mut stmt = conn.prepare(query)?;
    let iters = stmt.query_map([], |row| {
        if let Ok(mut torr) = Torrent::from_row(row) {
            // TODO: fetch files
            let _ = torr.files(&conn);
            Ok(torr)
        } else {
            Err(rusqlite::Error::InvalidQuery)
        }
    })?;

    let mut client = reqwest::Client::new();

    for torr in iters {
        if let Ok(torrent) = torr {
            println!("Processing: {}", torrent.id);
            let _ = client
                .post("https://meili.local.jeykey.net/indexes/torrents/documents")
                .json(&torrent)
                .send()
                .await?;
        }
    }

    Ok(())
}
