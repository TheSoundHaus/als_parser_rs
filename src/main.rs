use std::{collections, fs::File, io::BufReader};
use flate2::bufread::GzDecoder;
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

        // hash both version by track
        let old_map: std::collections::HashMap<_, _> = old.tracks.iter().map(|t| (&t.id, t)).collect();
        let new_map: std::collections::HashMap<_, _> = self.tracks.iter().map(|t| (&t.id, t)).collect();

        // check for delted tracks
        for (id, track) in &old_map {
            if !new_map.contains_key(id) {
                changes.push(format!("Removed track: {}", track.effective_name));
            }
        }

        for (id, track) in &new_map {
            if let Some(old_track) = old_map.get(id) {
                if track != old_track {
                    track.diff_content(old_track, &mut changes);
                } else {
                    changes.push(format!("Added new track: {}", track.effective_name));
                }
            }
        }

        changes
    }
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Track {
    #[serde(rename = "Type")]
    track_type: String,         // Midi, Audio, Return

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

        // Check for username updates
        if self.user_name != old.user_name {
            let old_un = old.user_name.as_deref().unwrap_or("None");
            let new_un = self.user_name.as_deref().unwrap_or("None");
            changes.push(format!("Track {}: User name changed from '{}' to '{}'", self.id, old_un, new_un));
        } 

        // If effective name changed, it's probably an instrument change
        else if self.effective_name != old.effective_name {
            changes.push(format!("Track {}: Changed instrument {} to {}", self.id, old.effective_name, self.effective_name));
        }

        // Diff branches
        // diff_branches(old, changes);
    }

    // fn diff_branches(&self, old: &Track, changes: &mut Vec<String>) {
        
    // }
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

struct DiffSummary {
    changes: Vec<String>,
}

// Convert als project to project struct
fn get_project_from_als(path: &str) -> Project {
    // Input file
    let fin = File::open(path).unwrap();
    
    // Decompression / Parsing pipeline 
    // 2 buffers used here to minimize syscalls 
    let compressed_buffer = BufReader::new(fin);
    let decompressor = GzDecoder::new(compressed_buffer);
    let buffered_reader = BufReader::new(decompressor);
    let mut xml_reader = Reader::from_reader(buffered_reader);

    // Struct defining the Ableton project
    let mut project = Project {
        tracks: Vec::new()
    };

    // Track current track and branch stack
    let mut cur_track: Option<Track> = None; 
    let mut branch_stack: Vec<Vec<Branch>> = Vec::new();

    // Used to avoid inheriting usernames if username is undefined
    let mut in_name_block = false;

    let mut buf = Vec::new();
    loop {
        match xml_reader.read_event_into(&mut buf) {
            
            // <Opening Tags>
            Ok(Event::Start(e)) => {
                let name = e.name();
                
                match name.as_ref() {
                    // Track found
                    b"AudioTrack" | b"MidiTrack" | b"ReturnTrack" => {
                        if let Ok(Some(attr)) = e.try_get_attribute("Id") {
                            cur_track = Some(Track::new(name.as_ref(), attr.value.as_ref()));
                        }
                    },

                    // Branches group found 
                    b"Branches" => {
                        branch_stack.push(Vec::new());
                    },

                    // Branch found
                    b"DrumBranch" | b"InstrumentBranch" | b"AudioEffectBranch" => {
                        let cur_branch = branch_stack.last_mut();
                        if let Some(branch_vec) = cur_branch {
                            branch_vec.push(Branch::new(name.as_ref()));
                        }
                    },

                    // Entered name block (important for context locality)
                    b"Name" => {
                        in_name_block = true;
                    },

                   _ => (),
                }
            },

            // <Self Closing Tags />
            Ok(Event::Empty(e)) => {
                let name = e.name();

                match name.as_ref() {
                    b"EffectiveName" | b"UserName" => {

                        // Add attributes to to the latest branch/track if there is one
                        // Avoid context breaking by ensuring we are inside a <Name></Name> block
                        if let Ok(Some(attr)) = e.try_get_attribute("Value") && in_name_block {
                            if let Some(branch_bucket) = branch_stack.last_mut() {
                                if let Some(branch) = branch_bucket.last_mut() {
                                    if name.as_ref() == b"EffectiveName" { branch.set_effective_name(attr.value.as_ref()); }
                                    else if !attr.value.is_empty() { branch.set_user_name(attr.value.as_ref()); }
                                }
                            }

                            // Add attributes to the track if no branches active
                            else if let Some(ref mut track) = cur_track {
                                if name.as_ref() == b"EffectiveName" { track.set_effective_name(attr.value.as_ref());} 
                                else if !attr.value.is_empty() { track.set_user_name(attr.value.as_ref()); }
                            }            
                        } 
                    },

                    _ => ()
                }
            },

            // </ Closing Tags >
            Ok(Event::End(e)) => {
                let name = e.name();

                match name.as_ref() {
                    // Track found: ready to pop
                    b"AudioTrack" | b"MidiTrack" | b"ReturnTrack" => {
                        if let Some(track) = cur_track.take() {
                            project.tracks.push(track);
                        }
                    },

                    // Branches group found 
                    b"Branches" => {
                        if let Some(deepest_branch) = branch_stack.pop() {
                            // Still exists some branches above
                            if let Some(parent_bucket) = branch_stack.last_mut() {
                                if let Some(parent_branch) = parent_bucket.last_mut() {
                                    parent_branch.branches = Some(deepest_branch);
                                }
                            }
                            
                            // Track is direct parent
                            else if let Some(ref mut t) = cur_track {
                                t.branches = Some(deepest_branch);
                            }
                        }
                    },
                    
                    b"Name" => {
                        in_name_block = false;
                    },

                   _ => (),
                }
            },

            Ok(Event::Eof) => break,
            Err(e) => panic!("Error at position {}: {:?}", xml_reader.buffer_position(), e),
            _ => (),
        }
        buf.clear();
    }

    return project;
}

// Generate human readable diff from 2 project structs
fn diff_projects(project1: Project, project2: Project) -> Option<String> {
    
    let changes = project2.diff(&project1);

    for change in changes {
        println!("{}", change);
    }

    //TODO: Remove this. Just so Rust doesn't yell at me
    return Some(String::new());
}

fn main() -> std::io::Result<()> {
    let project1 = get_project_from_als("DiffTestPre.als");
    let project2 = get_project_from_als("DiffTestPost.als");

    // println!("{}", serde_json::to_string_pretty(&project1).unwrap());
    // println!("{}", serde_json::to_string_pretty(&project2).unwrap());
    
    if let Some(diff) = diff_projects(project1, project2) {
        println!("Gonna Fix Later. Done for now tho {}", diff);
    }

    Ok(())
}