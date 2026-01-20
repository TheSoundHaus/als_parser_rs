use std::{fs::File, io::BufReader, io::BufWriter, io::Write as IoWrite};
use flate2::bufread::GzDecoder;
use quick_xml::reader::Reader;
use quick_xml::events::Event;
use phf::phf_set;

// Take slices of this large space array
//      and use them to indent the json. Much faster than a loop
const SPACES: &[u8; 256] = &[b' '; 256];

// Macro to slice above space array
macro_rules! indent {
    ($dst:expr, $lvl:expr) => {
        {
            // Bytes needed for 4 space indentation
            let byte_count = ($lvl * 4).min(256);

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

fn main() -> std::io::Result<()> {
    // Input file
    let fin = File::open("GoodMusic.als").unwrap();
    
    // Decompression / Parsing pipeline 
    // 2 buffers used here to minimize syscalls 
    let compressed_buffer = BufReader::new(fin);
    let decompressor = GzDecoder::new(compressed_buffer);
    let buffered_reader = BufReader::new(decompressor);
    let mut xml_reader = Reader::from_reader(buffered_reader);

    let fout = File::create("output.json")?;
    let mut writer = BufWriter::new(fout); 

    let mut buf = Vec::new();

    let mut depth = 0;
    let mut is_first_bitmask: u64 = !0;

    writer.write_all(b"{")?;
    is_first_bitmask &= !1;
    depth += 1;
    loop {
        match xml_reader.read_event_into(&mut buf) {
            // Start tag: Parse and increment level by one
            Ok(Event::Start(e) ) => {
                let name = e.name();
                if KEY_TAGS.contains(name.as_ref()) || MEMBER_TAGS.contains(name.as_ref()) {
                    // Format accounting for first item
                    let is_first = (is_first_bitmask >> depth) & 1;
                    if is_first == 0 {
                        writer.write_all(b",")?;
                    } else {
                        is_first_bitmask &= !(1 << depth);
                    }
                    writer.write_all(b"\n")?;

                    if KEY_TAGS.contains(name.as_ref()) {
                        indent!(writer, depth);
                        writer.write_all(b"\"")?;
                        writer.write_all(name.as_ref())?;
                        writer.write_all(b"\": [")?;    
                        depth += 1;
                    }
                    else if MEMBER_TAGS.contains(name.as_ref()) {
                        // Write tag found
                        indent!(writer, depth);
                        writer.write_all(b"{\n")?;
                        indent!(writer, depth);
                        writer.write_all(b"\"type\": \"")?;
                        writer.write_all(name.as_ref())?;
                        writer.write_all(b"\"")?;
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
                            writer.write_all(b",")?; 
                        }
                        else { 
                            is_first_bitmask &= !(1 << depth);
                        }
                        writer.write_all(b"\n")?;
                        indent!(writer, depth);
                        writer.write_all(b"\"")?;
                        writer.write_all(name.as_ref())?;
                        writer.write_all(b"\": \"")?;
                        writer.write_all(&attr.value)?;
                        writer.write_all(b"\"")?;
                    }
                }
            }

            // Closing tag
            Ok(Event::End(e)) => {
                let name = e.name();
                if KEY_TAGS.contains(name.as_ref()) {
                    writer.write_all(b"\n")?; 
                    is_first_bitmask |= 1 << depth;
                    depth -= 1;
                    indent!(writer, depth);
                    writer.write_all(b"]")?;
                }
                else if MEMBER_TAGS.contains(name.as_ref()) {
                    writer.write_all(b"\n")?;
                    indent!(writer, depth);
                    writer.write_all(b"}")?;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("Error at position {}: {:?}", xml_reader.buffer_position(), e),
            _ => (),
        }
        buf.clear();
    }
    writer.write_all(b"\n}")?;

    Ok(())
}