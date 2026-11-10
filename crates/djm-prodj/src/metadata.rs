use binrw::BinRead;
use rekordcrate::pdb::{Header, PageType, Row};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedTrackMetadata {
    pub rekordbox_id: u32,
    pub title: String,
    pub artist: String,
    pub bpm: Option<f32>,
    pub key: Option<String>,
    pub duration_secs: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredUsbMedia {
    pub media_id: String,
    pub media_name: String,
    pub player_number: u8,
    pub is_available: bool,
}

pub struct UsbMediaDatabase {
    pub media_name: String,
    pub tracks: HashMap<u32, ResolvedTrackMetadata>,
}

impl UsbMediaDatabase {
    pub fn empty(name: &str) -> Self {
        Self {
            media_name: name.to_string(),
            tracks: HashMap::new(),
        }
    }

    /// Parses a raw Pioneer `export.pdb` database from a byte buffer
    pub fn parse_export_pdb(data: &[u8], media_name: &str) -> Result<Self, String> {
        let mut reader = Cursor::new(data);
        let header = Header::read(&mut reader)
            .map_err(|e| format!("Failed to read PDB header: {}", e))?;

        // 1. Build Artist lookup table (ArtistId -> Artist Name)
        let mut artist_map: HashMap<u32, String> = HashMap::new();
        if let Some(artist_table) = header.tables.iter().find(|t| t.page_type == PageType::Artists) {
            if let Ok(pages) = header.read_pages(
                &mut reader,
                binrw::Endian::NATIVE,
                (&artist_table.first_page, &artist_table.last_page),
            ) {
                for page in pages {
                    for row_group in page.row_groups {
                        for row in row_group.present_rows() {
                            if let Row::Artist(ref art) = row {
                                let dbg = format!("{:?}", art);
                                if let Some((id, name)) = parse_artist_debug(&dbg) {
                                    artist_map.insert(id, name);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Read Tracks table
        let mut tracks = HashMap::new();
        if let Some(track_table) = header.tables.iter().find(|t| t.page_type == PageType::Tracks) {
            use std::io::{Seek, SeekFrom};
            reader.seek(SeekFrom::Start(0)).map_err(|e| format!("Seek failed: {}", e))?;
            let pages = header.read_pages(
                &mut reader,
                binrw::Endian::NATIVE,
                (&track_table.first_page, &track_table.last_page),
            ).map_err(|e| format!("Failed to read track pages: {}", e))?;

            for page in pages {
                for row_group in page.row_groups {
                    for row in row_group.present_rows() {
                        if let Row::Track(ref t) = row {
                            let dbg = format!("{:?}", t);
                            if let Some(parsed) = parse_track_debug(&dbg) {
                                let artist = artist_map
                                    .get(&parsed.artist_id)
                                    .cloned()
                                    .unwrap_or_else(|| "Unknown Artist".to_string());
                                tracks.insert(
                                    parsed.id,
                                    ResolvedTrackMetadata {
                                        rekordbox_id: parsed.id,
                                        title: parsed.title,
                                        artist,
                                        bpm: parsed.bpm,
                                        key: None,
                                        duration_secs: parsed.duration_secs,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }

        Ok(Self {
            media_name: media_name.to_string(),
            tracks,
        })
    }
}

struct ParsedTrackDebug {
    id: u32,
    artist_id: u32,
    title: String,
    bpm: Option<f32>,
    duration_secs: Option<u32>,
}

fn extract_device_sql_string(s: &str, field_name: &str) -> Option<String> {
    let pattern = format!("{}: DeviceSQLString(\"", field_name);
    let start_idx = s.find(&pattern)? + pattern.len();
    let rest = &s[start_idx..];
    let end_idx = rest.find("\")")?;
    Some(rest[..end_idx].to_string())
}

fn extract_id(s: &str, field_name: &str, wrapper: &str) -> Option<u32> {
    let pattern = format!("{}: {}(", field_name, wrapper);
    let start_idx = s.find(&pattern)? + pattern.len();
    let rest = &s[start_idx..];
    let end_idx = rest.find(')')?;
    rest[..end_idx].parse::<u32>().ok()
}

fn extract_u32(s: &str, field_name: &str) -> Option<u32> {
    let pattern = format!("{}: ", field_name);
    let start_idx = s.find(&pattern)? + pattern.len();
    let rest = &s[start_idx..];
    let end_idx = rest.find([',', '}']).unwrap_or(rest.len());
    rest[..end_idx].trim().parse::<u32>().ok()
}

fn parse_artist_debug(dbg: &str) -> Option<(u32, String)> {
    let id = extract_id(dbg, "id", "ArtistId")?;
    let name = extract_device_sql_string(dbg, "name").unwrap_or_default();
    Some((id, name))
}

fn parse_track_debug(dbg: &str) -> Option<ParsedTrackDebug> {
    let id = extract_id(dbg, "id", "TrackId")?;
    let artist_id = extract_id(dbg, "artist_id", "ArtistId").unwrap_or(0);
    let title = extract_device_sql_string(dbg, "title").unwrap_or_else(|| format!("Track {}", id));
    let bpm = extract_u32(dbg, "tempo").filter(|&t| t > 0).map(|t| t as f32 / 100.0);
    let duration_secs = extract_u32(dbg, "duration").filter(|&d| d > 0);

    Some(ParsedTrackDebug {
        id,
        artist_id,
        title,
        bpm,
        duration_secs,
    })
}

/// Metadata Resolver that manages databases per player and supports Degraded Mode
pub struct MetadataResolver {
    pub player_media: HashMap<u8, DiscoveredUsbMedia>,
    pub databases: HashMap<String, UsbMediaDatabase>,
}

impl Default for MetadataResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl MetadataResolver {
    pub fn new() -> Self {
        Self {
            player_media: HashMap::new(),
            databases: HashMap::new(),
        }
    }

    /// Registers a discovered USB stick for a player
    pub fn register_usb_media(
        &mut self,
        player_number: u8,
        media_id: &str,
        media_name: &str,
        is_available: bool,
    ) {
        self.player_media.insert(
            player_number,
            DiscoveredUsbMedia {
                media_id: media_id.to_string(),
                media_name: media_name.to_string(),
                player_number,
                is_available,
            },
        );
    }

    /// Adds a parsed database associated with a media_id
    pub fn add_database(&mut self, media_id: &str, db: UsbMediaDatabase) {
        self.databases.insert(media_id.to_string(), db);
    }

    /// Loads `export.pdb` directly from a file path
    pub fn load_database_from_file(
        &mut self,
        media_id: &str,
        media_name: &str,
        path: &Path,
    ) -> Result<(), String> {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("Failed to read database file '{}': {}", path.display(), e))?;
        let db = UsbMediaDatabase::parse_export_pdb(&bytes, media_name)?;
        self.add_database(media_id, db);
        Ok(())
    }

    /// Resolves metadata for a track played on a specific CDJ deck.
    /// Returns Some(metadata) if available, or None if in Degraded Mode (e.g. database not accessible).
    pub fn resolve_track(
        &self,
        player_number: u8,
        rekordbox_track_id: u32,
    ) -> Option<ResolvedTrackMetadata> {
        let media = self.player_media.get(&player_number)?;
        if !media.is_available {
            return None; // Degraded mode
        }

        let db = self.databases.get(&media.media_id)?;
        db.tracks.get(&rekordbox_track_id).cloned()
    }

    /// Whether a player is currently in Degraded Mode (track titles unavailable)
    pub fn is_degraded(&self, player_number: u8) -> bool {
        match self.player_media.get(&player_number) {
            Some(media) => !media.is_available || !self.databases.contains_key(&media.media_id),
            None => true,
        }
    }

    /// Returns the USB media name for a given player if known
    pub fn get_media_name(&self, player_number: u8) -> Option<String> {
        self.player_media
            .get(&player_number)
            .map(|m| m.media_name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rekordbox_fixture_database() {
        // Path to real rekordbox export.pdb fixture included in rekordcrate crate
        let fixture_path = Path::new(
            "/home/bram/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/rekordcrate-0.3.0/data/pdb/num_rows/export.pdb",
        );

        if fixture_path.exists() {
            let bytes = std::fs::read(fixture_path).expect("read fixture pdb");
            let db = UsbMediaDatabase::parse_export_pdb(&bytes, "USB_STICK_FIXTURE").expect("parse export.pdb");

            // Exactly 3886 tracks exist in this rekordbox fixture
            assert_eq!(db.tracks.len(), 3886);
            assert_eq!(db.media_name, "USB_STICK_FIXTURE");

            // Look up track ID 1
            if let Some(track) = db.tracks.get(&1) {
                assert!(!track.title.is_empty(), "Track 1 should have a non-empty title");
                assert!(!track.artist.is_empty(), "Track 1 should have an artist");
            }
        }
    }

    #[test]
    fn test_metadata_resolver_normal_and_degraded_mode() {
        let mut resolver = MetadataResolver::new();

        // 1. Initial state without database -> Degraded Mode
        assert!(resolver.is_degraded(1));
        assert_eq!(resolver.resolve_track(1, 1001), None);

        // 2. Register media with mock database
        let mut mock_db = UsbMediaDatabase::empty("NH_MAIN_STICK");
        mock_db.tracks.insert(
            1001,
            ResolvedTrackMetadata {
                rekordbox_id: 1001,
                title: "Original Vibe".to_string(),
                artist: "Next Horizon".to_string(),
                bpm: Some(174.0),
                key: Some("8A".to_string()),
                duration_secs: Some(215),
            },
        );

        resolver.register_usb_media(1, "STICK_UUID_ABC", "NH_MAIN_STICK", true);
        resolver.add_database("STICK_UUID_ABC", mock_db);

        // Active mode: Track resolved successfully!
        assert!(!resolver.is_degraded(1));
        let resolved = resolver.resolve_track(1, 1001).expect("resolved track");
        assert_eq!(resolved.title, "Original Vibe");
        assert_eq!(resolved.artist, "Next Horizon");
        assert_eq!(resolved.bpm, Some(174.0));
        assert_eq!(resolved.duration_secs, Some(215));

        // Unknown track ID returns None
        assert_eq!(resolver.resolve_track(1, 9999), None);

        // 3. Degraded mode triggered if media is flagged unavailable or removed
        resolver.register_usb_media(1, "STICK_UUID_ABC", "NH_MAIN_STICK", false);
        assert!(resolver.is_degraded(1));
        assert_eq!(resolver.resolve_track(1, 1001), None);
    }
}
