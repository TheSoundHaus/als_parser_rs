use std::{fs::File, io::BufReader, io::BufWriter, io::Write as IoWrite};
use quick_xml::reader::Reader;
use quick_xml::events::Event;

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

fn main() -> std::io::Result<()> {
    let fin = File::open("Tutorial")?;
    let mut reader = Reader::from_reader(BufReader::new(fin));

    let fout = File::create("output.json")?;
    let mut writer = BufWriter::new(fout); 

    let mut buf = Vec::new();

    let mut level = 0; // current depth of the xml
    let mut is_first = false;

    writer.write_all(b"{\n")?;
    loop {
        match reader.read_event_into(&mut buf) {
            // Start tag: Parse and increment level by one
            Ok(Event::Start(e) ) => {
                match e.name().as_ref() {
                    b"AudioTrack" | b"MidiTrack" => {
                        // Format accounting for first item
                        if !is_first { writer.write_all(b",\n")?; }
                        else { is_first = false; }
                        
                        // Write tag found
                        indent!(writer, level);
                        let track_type = if e.name().as_ref() == b"AudioTrack" {"Audio Track"} else {"Midi Track"};
                        writer.write_all(b"Found ")?;
                        writer.write_all(track_type.as_bytes())?;
                    }
                    _ => (),
                }
                level += 1;
            }

            // Self closing tag: Parse and keep level the same
            Ok(Event::Empty(e)) => {
                match e.name().as_ref() {
                    b"EffectiveName" => {
                        // Format accounting for first item
                        if !is_first { writer.write_all(b",\n")?; }
                        else { is_first = false; }

                        indent!(writer, level);
                        if let Ok(Some(attr)) = e.try_get_attribute("Value") {
                            writer.write_all(b"Effective Name: ")?;
                            writer.write_all(&attr.value)?;
                        }
                    }
                    _ => (),
                }
            }

            // Closing tag: Don't parse and decrement level by one
            Ok(Event::End(_e)) => {
                level -= 1;
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