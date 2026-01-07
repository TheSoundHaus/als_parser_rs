use std::{fs::File, io::BufReader, io::BufWriter, io::Write as IoWrite};
use quick_xml::reader::Reader;
use quick_xml::events::Event;
use phf::phf_set;

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

static TARGET_TAGS: phf::Set<&'static [u8]> = phf_set! {
    // Container Tags <tag></tag>
    b"AudioTrack",
    b"MidiTrack",
    b"Branches",
    b"DrumBranch",
    
    // Self closing tags <tag />
    b"EffectiveName",
    b"UserName",
};

fn main() -> std::io::Result<()> {
    let fin = File::open("Tutorial")?;
    let mut reader = Reader::from_reader(BufReader::new(fin));

    let fout = File::create("output.json")?;
    let mut writer = BufWriter::new(fout); 

    let mut buf = Vec::new();

    let mut depth = 0;
    let mut is_first = true;

    writer.write_all(b"{\n")?;
    loop {
        match reader.read_event_into(&mut buf) {
            // Start tag: Parse and increment level by one
            Ok(Event::Start(e) ) => {
                if TARGET_TAGS.contains(e.name().as_ref()) {
                    // Format accounting for first item
                    if !is_first { writer.write_all(b",\n")?; }
                    
                    // Write tag found
                    indent!(writer, depth);
                    writer.write_all(b"\"")?;
                    writer.write_all(e.name().as_ref())?;
                    writer.write_all(b"\" : {")?;
                    is_first = true; // reset for internal indentation
                }
                depth += 1;
            }

            // Self closing tag: Parse and keep level the same
            Ok(Event::Empty(e)) => {
                // Name of self closing tag
                let name = e.name();
                if TARGET_TAGS.contains(name.as_ref()) {
                    if !is_first { writer.write_all(b",\n")?; }
                    else { 
                        writer.write_all(b"\n")?;
                        is_first = false;
                    }

                    indent!(writer, depth);
                    if let Ok(Some(attr)) = e.try_get_attribute("Value") {
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
                depth -= 1;
                if TARGET_TAGS.contains(e.name().as_ref()) {
                    writer.write_all(b",\n")?;
                    
                    indent!(writer, depth);
                    writer.write_all(b"}")?;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
            _ => (),
        }
        buf.clear();
    }
    writer.write_all(b"}")?;

    Ok(())
}