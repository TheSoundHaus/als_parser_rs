use std::{fs::File, io::BufReader};
use quick_xml::reader::Reader;
use quick_xml::events::Event;

fn main() -> std::io::Result<()> {
    let f = File::open("Dying")?;
    let mut reader = Reader::from_reader(BufReader::new(f));
    let mut buf = Vec::new();

    println!("Scanning project...");
    
    println!("{{");
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e) | Event::Empty(e)) => {
                match e.name().as_ref() {
                    b"AudioTrack" => println!("Found Audio Track,"),
                    b"MidiTrack" => println!("Found MIDI Track,"),
                    b"EffectiveName" => {
                        if let Ok(Some(attr)) = e.try_get_attribute("Value") {
                            println!("\tEffective Name: {},", String::from_utf8_lossy(&attr.value));
                        }
                    }
                    _ => (),
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
            _ => (),
        }
        buf.clear();
    }
    println!("}}");

    Ok(())
}