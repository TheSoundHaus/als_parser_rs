#![deny(clippy::all)]

use napi_derive::napi;
use std::{fs::File, io::BufReader, io::BufWriter, io::Write as IoWrite};
use quick_xml::reader::Reader;
use quick_xml::events::Event;
use phf::phf_set;
use std::path;

// The program will take slices of this large space array
//      and use them to indent the json. Much faster than a loop
const SPACES: &[u8; 256] = &[b' '; 256];

// Macro to slice above space array
macro_rules! indent {
    ($dst:expr, $lvl:expr) => {
        {
            // Bytes needed for 2 space indentation
            let byte_count = ($lvl * 2).min(256);

            $dst.write_all(&SPACES[..byte_count])
                .expect("Failed to write indenations to file buffer\n");
        }
    };
}

// Only appear once per xml file
static KEY_TAGS: phf::Set<&'static [u8]> = phf_set! {
    b"Tracks",
    b"Branches",
    // b"MainTrack",
    // b"PreHearTrack", // PreHear Track is Master Track
};

// Appear repeatedly as members of the KEY_TAGS
static MEMBER_TAGS: phf::Set<&'static [u8]> = phf_set! {
    // Container Tags <tag></tag>
    b"AudioTrack",
    b"MidiTrack",
    b"ReturnTrack",
    b"DrumBranch",
    b"InstrumentBranch",
    b"AudioEffectBranch",
    
    // Self closing tags <tag />
    b"EffectiveName",
    b"UserName",
};

// Helper trait to convert io::Result to napi::Result
trait ToNapiResult<T> {
    fn to_napi(self) -> napi::Result<T>;
}

impl<T> ToNapiResult<T> for std::io::Result<T> {
    fn to_napi(self) -> napi::Result<T> {
        self.map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))
    }
}

#[napi]
pub fn parse_xml(filepath: String) -> napi::Result<String> {
    let fin = File::open(&filepath).to_napi()?;
    let mut reader = Reader::from_reader(BufReader::new(fin));

    let fout = File::create("output.json").to_napi()?;
    let mut writer = BufWriter::new(fout); 

    let mut buf = Vec::new();

    let mut depth = 0;
    let mut is_first_bitmask: u64 = !0;

    writer.write_all(b"{").to_napi()?;
    is_first_bitmask &= !1;
    depth += 1;
    loop {
        match reader.read_event_into(&mut buf) {
            // Start tag: Parse and increment level by one
            Ok(Event::Start(e) ) => {
                let name = e.name();
                if KEY_TAGS.contains(name.as_ref()) || MEMBER_TAGS.contains(name.as_ref()) {
                    // Format accounting for first item
                    let is_first = (is_first_bitmask >> depth) & 1;
                    if is_first == 0 {
                        writer.write_all(b",").to_napi()?;
                    } else {
                        is_first_bitmask &= !(1 << depth);
                    }
                    writer.write_all(b"\n").to_napi()?;

                    if KEY_TAGS.contains(name.as_ref()) {
                        indent!(writer, depth);
                        writer.write_all(b"\"").to_napi()?;
                        writer.write_all(name.as_ref()).to_napi()?;
                        writer.write_all(b"\": [").to_napi()?;    
                        depth += 1;
                    }
                    else if MEMBER_TAGS.contains(name.as_ref()) {
                        // Write tag found
                        indent!(writer, depth);
                        writer.write_all(b"{\n").to_napi()?;
                        indent!(writer, depth);
                        writer.write_all(b"\"type\": \"").to_napi()?;
                        writer.write_all(name.as_ref()).to_napi()?;
                        writer.write_all(b"\"").to_napi()?;
                    }
                }
            }

            // Self closing tag: Parse and keep level the same
            Ok(Event::Empty(e)) => {
                // Name of self closing tag
                let name = e.name();
                if MEMBER_TAGS.contains(name.as_ref()) {
                    if let Ok(Some(attr)) = e.try_get_attribute("Value") {
                        // Skip empty tags
                        if attr.value.is_empty() { continue; } 
                        
                        let is_first = (is_first_bitmask >> depth) & 1;
                        if is_first == 0 {
                            writer.write_all(b", ").to_napi()?;
                        }
                        else { 
                            is_first_bitmask &= !(1 << depth);
                        }
                        writer.write_all(b"\n").to_napi()?;
                        indent!(writer, depth);
                        writer.write_all(b"\"").to_napi()?;
                        writer.write_all(name.as_ref()).to_napi()?;
                        writer.write_all(b"\": \"").to_napi()?;
                        writer.write_all(&attr.value).to_napi()?;
                        writer.write_all(b"\"").to_napi()?;
                    }
                }
            }

            // Closing tag
            Ok(Event::End(e)) => {
                let name = e.name();
                if KEY_TAGS.contains(name.as_ref()) {
                    writer.write_all(b"\n").to_napi()?;
                    is_first_bitmask |= 1 << depth;
                    depth -= 1;
                    indent!(writer, depth);
                    writer.write_all(b"]").to_napi()?;
                }
                else if MEMBER_TAGS.contains(name.as_ref()) {
                    writer.write_all(b"\n").to_napi()?;
                    indent!(writer, depth);
                    writer.write_all(b"}").to_napi()?;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
            _ => (),
        }
        buf.clear();
    }
    writer.write_all(b"\n}").to_napi()?;

    let path_string = path::absolute("output.json").to_napi()?;

    Ok(path_string.to_string_lossy().to_string())
}
