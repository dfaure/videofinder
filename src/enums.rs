/// Types of film
#[derive(Debug, PartialEq)]
pub enum FilmType {
    /*CINEMA = 0,*/ Television = 1,
}

/// Types of support (bitfield, for filters, but we don't use that in this code base, it's just how the values are in the DB)
/// If we need that one day: port to https://crates.io/crates/flagset
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SupportType {
    Tape = 1,
    Dvd = 2,
    ComputerFile = 4,
    Bluray = 8,
    //All = 15
}

pub fn letter_for_support_type(support_type: SupportType) -> &'static str {
    match support_type {
        SupportType::Tape => "C", // French ;)
        SupportType::Dvd => "D",
        SupportType::Bluray => "B",
        SupportType::ComputerFile => "O", // French
    }
}

pub fn color_for_support(support_type: SupportType, origin: String, on_loan: bool) -> slint::Color {
    let base_color = match support_type {
        SupportType::Tape => {
            // "enregistre" or "taped"
            let is_taped = origin.starts_with('E') || origin.starts_with('T');
            if is_taped {
                slint::Color::from_argb_encoded(0xFF1AE0FF) // light blue
            } else {
                slint::Color::from_argb_encoded(0xFFFF1DFF) // pink
            }

            // TODO add color legend somewhere :-)
        }
        SupportType::Dvd => slint::Color::from_argb_encoded(0xFF6DFF6B), // light green
        SupportType::Bluray => slint::Color::from_argb_encoded(0xFF000084), // dark blue
        SupportType::ComputerFile => slint::Color::from_argb_encoded(0xFFFFDCA8), // very light
                                                                          // orange
    };
    if on_loan {
        base_color.brighter(0.5)
    } else {
        base_color
    }
}
