/// Types of film
#[derive(Debug, PartialEq)]
pub enum FilmType { /*CINEMA = 0,*/ TELEVISION = 1 }

/// Types of support (bitfield, for filters)
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SupportType {
    TAPE = 1,
    DVD = 2,
    COMPUTERFILE = 4,
    BLURAY = 8,
    //ALL = 15
}

pub fn letter_for_support_type(support_type: SupportType) -> &'static str {
    match support_type {
        SupportType::TAPE => "C", // French ;)
        SupportType::DVD => "D",
        SupportType::BLURAY => "B",
        SupportType::COMPUTERFILE => "O", // French
    }
}

pub fn color_for_support(support_type: SupportType, origin: String, on_loan: bool) -> slint::Color {
    let base_color = match support_type {
        SupportType::TAPE => {
            // "enregistre" or "taped"
            let is_taped = origin.starts_with('E') || origin.starts_with('T');
            if is_taped {
                slint::Color::from_argb_encoded(0xFF1AE0FF) // light blue
            } else {
                slint::Color::from_argb_encoded(0xFFFF1DFF) // pink
            }

            // TODO add color legend somewhere :-)
        },
        SupportType::DVD => slint::Color::from_argb_encoded(0xFF6DFF6B), // light green
        SupportType::BLURAY => slint::Color::from_argb_encoded(0xFF000084), // dark blue
        SupportType::COMPUTERFILE => slint::Color::from_argb_encoded(0xFFFFDCA8), // very light
                                                                                  // orange
    };
    if on_loan {
        base_color.brighter(0.5)
    } else {
        base_color
    }
}
