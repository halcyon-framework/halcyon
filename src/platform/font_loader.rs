use crate::platform::system_font_provider::load_legacy_embedded_inter;
use glyphon::fontdb::{Database, Source};

pub fn load_inter_font(db: &mut Database) -> glyphon::fontdb::ID {
    let data = load_legacy_embedded_inter().data;
    db.load_font_source(Source::Binary(data));
    db.faces().last().unwrap().id
}
