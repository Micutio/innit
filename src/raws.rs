mod spawn;

use spawn::Spawn;

rltk::embedded_resource!(RAW_FILE, "../raws/spawns.json");

pub fn load_raws() {
    rltk::link_resource!(RAW_FILE, "../raws/spawns.json");

    // Retrieve the raw data as an array of u8 (8-bit unsigned chars)
    let raw_data = rltk::embedding::EMBED
        .lock()
        // .unwrap()
        .get_resource("../../raws/spawns.json".to_string())
        .unwrap();
    let raw_string =
        std::str::from_utf8(&raw_data).expect("Unable to convert to a valid UTF-8 string.");
    let domain: Spawn = serde_json::from_str(&raw_string).expect("Unable to parse JSON");
}
