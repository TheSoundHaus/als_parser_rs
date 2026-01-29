#![deny(clippy::all)]

use napi_derive::napi;
use std::{fs::File, io::BufReader, collections::HashMap};
use flate2::read::GzDecoder;
use quick_xml::reader::Reader;
use quick_xml::events::Event;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Project {
    #[serde(rename = "Tracks")]
    tracks: Vec<Track>,
}

impl Project {
    fn diff(&self, old: &Project) -> Vec<String> {
        let mut changes = Vec::new();

        let old_map: HashMap<_, _> = old.tracks.iter().map(|t| (&t.id, t)).collect();
        let new_map: HashMap<_, _> = self.tracks.iter().map(|t| (&t.id, t)).collect();

        // 1. Check for deleted tracks
        for (id, track) in &old_map {
            if !new_map.contains_key(id) {
                changes.push(format!("Removed track: {}", track.effective_name));
            }
        }

        // 2. Check for added or modified tracks
        for (id, track) in &new_map {
            if let Some(old_track) = old_map.get(id) {
                // If ID exists in both, check for deep inequality
                if track != *old_track {
                    track.diff_content(old_track, &mut changes);
                }
            } else {
                // This was misplaced in your snippet - fixed!
                changes.push(format!("Added new track: {}", track.effective_name));
            }
        }

        changes
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Track {
    #[serde(rename = "Type")]
    track_type: String,
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "EffectiveName")]
    effective_name: String,
    #[serde(rename = "UserName", skip_serializing_if = "Option::is_none")]
    user_name: Option<String>,
    #[serde(rename = "Branches", skip_serializing_if = "Option::is_none")]
    branches: Option<Vec<Branch>> 
}

impl Track {
    fn new(track_type: &[u8], id: &[u8]) -> Self {
        Self {
            track_type: String::from_utf8_lossy(track_type).into_owned(),
            id: String::from_utf8_lossy(id).into_owned(), 
            effective_name: String::new(),
            user_name: None,
            branches: None, 
        }
    }

    fn set_effective_name(&mut self, effective_name: &[u8]) {
        self.effective_name = String::from_utf8_lossy(effective_name).into_owned();
    }

    fn set_user_name(&mut self, user_name: &[u8]) {
        self.user_name = Some(String::from_utf8_lossy(user_name).into_owned());
    }

    fn diff_content(&self, old: &Track, changes: &mut Vec<String>) {
        if self.user_name != old.user_name {
            let old_un = old.user_name.as_deref().unwrap_or("None");
            let new_un = self.user_name.as_deref().unwrap_or("None");
            changes.push(format!("Track {}: Renamed from '{}' to '{}'", self.effective_name, old_un, new_un));
        } 
        else if self.effective_name != old.effective_name {
            changes.push(format!("Track {}: Swapped instrument to {}", self.id, self.effective_name));
        }

        // Recursive call for internal racks
        diff_branch_lists(&self.branches, &old.branches, changes, &self.effective_name);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Branch {
    #[serde(rename = "Type")]
    branch_type: String,
    #[serde(rename = "EffectiveName")] 
    effective_name: String,
    #[serde(rename = "UserName", skip_serializing_if = "Option::is_none")]
    user_name: Option<String>,
    #[serde(rename = "Branches", skip_serializing_if = "Option::is_none")]
    branches: Option<Vec<Branch>>
}

impl Branch {
    fn new(branch_type: &[u8]) -> Self {
        Self {
            branch_type: String::from_utf8_lossy(branch_type).into_owned(),
            effective_name: String::new(),
            user_name: None,
            branches: None,
        }
    }

    fn set_effective_name(&mut self, effective_name: &[u8]) {
        self.effective_name = String::from_utf8_lossy(effective_name).into_owned();
    }

    fn set_user_name(&mut self, user_name: &[u8]) {
        self.user_name = Some(String::from_utf8_lossy(user_name).into_owned());
    }
}

/// Recursive helper to diff branches without IDs (comparing by name/index)
fn diff_branch_lists(new: &Option<Vec<Branch>>, old: &Option<Vec<Branch>>, changes: &mut Vec<String>, parent_name: &str) {
    match (new, old) {
        (Some(n_list), Some(o_list)) => {
            if n_list != o_list {
                changes.push(format!("Track {}: Modified internal Rack devices", parent_name));
            }
        },
        (Some(_), None) => changes.push(format!("Track {}: Added new Rack devices", parent_name)),
        (None, Some(_)) => changes.push(format!("Track {}: Removed all Rack devices", parent_name)),
        _ => {}
    }
}

// Internal parser function
fn get_project_from_als(path: &str) -> Project {
    let fin = File::open(path).expect("Failed to open ALS file");
    let decompressor = GzDecoder::new(BufReader::new(fin));
    let mut xml_reader = Reader::from_reader(BufReader::new(decompressor));

    let mut project = Project { tracks: Vec::new() };
    let mut cur_track: Option<Track> = None; 
    let mut branch_stack: Vec<Vec<Branch>> = Vec::new();
    let mut in_name_block = false;
    let mut buf = Vec::new();

    loop {
        match xml_reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.name();
                match name.as_ref() {
                    b"AudioTrack" | b"MidiTrack" | b"ReturnTrack" => {
                        if let Ok(Some(attr)) = e.try_get_attribute("Id") {
                            cur_track = Some(Track::new(name.as_ref(), attr.value.as_ref()));
                        }
                    },
                    b"Branches" => branch_stack.push(Vec::new()),
                    b"DrumBranch" | b"InstrumentBranch" | b"AudioEffectBranch" => {
                        if let Some(bucket) = branch_stack.last_mut() {
                            bucket.push(Branch::new(name.as_ref()));
                        }
                    },
                    b"Name" => in_name_block = true,
                    _ => (),
                }
            },
            Ok(Event::Empty(e)) => {
                let name = e.name();
                if in_name_block {
                    if let Ok(Some(attr)) = e.try_get_attribute("Value") {
                        if let Some(bucket) = branch_stack.last_mut() {
                            if let Some(branch) = bucket.last_mut() {
                                if name.as_ref() == b"EffectiveName" { branch.set_effective_name(&attr.value); }
                                else { branch.set_user_name(&attr.value); }
                            }
                        } else if let Some(ref mut track) = cur_track {
                            if name.as_ref() == b"EffectiveName" { track.set_effective_name(&attr.value); }
                            else { track.set_user_name(&attr.value); }
                        }
                    }
                }
            },
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"AudioTrack" | b"MidiTrack" | b"ReturnTrack" => {
                        if let Some(track) = cur_track.take() { project.tracks.push(track); }
                    },
                    b"Branches" => {
                        if let Some(deepest) = branch_stack.pop() {
                            if let Some(bucket) = branch_stack.last_mut() {
                                if let Some(parent) = bucket.last_mut() { parent.branches = Some(deepest); }
                            } else if let Some(ref mut t) = cur_track {
                                t.branches = Some(deepest);
                            }
                        }
                    },
                    b"Name" => in_name_block = false,
                    _ => (),
                }
            },
            Ok(Event::Eof) => break,
            _ => (),
        }
        buf.clear();
    }
    project
}

#[napi]
pub fn parse_xml(current_filepath: String, old_json_path: String) -> napi::Result<String> {
    // 1. Parse current project from .als
    let current_project = get_project_from_als(&current_filepath);

    // 2. Load old project from JSON file
    let old_json_file = File::open(&old_json_path)
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("Old JSON not found: {}", e)))?;
    
    let old_project: Project = serde_json::from_reader(BufReader::new(old_json_file))
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, format!("Failed to parse old JSON: {}", e)))?;

    // 3. Diff them
    let changes = current_project.diff(&old_project);

    // 4. Wrap it all up into a single JSON for Electron
    let response = serde_json::json!({
        "summary": changes.join("\n"),
        "project": current_project
    });

    Ok(response.to_string())
}