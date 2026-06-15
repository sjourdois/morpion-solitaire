use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineLen {
    Four = 4,
    Five = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TouchMode {
    /// Parallel lines may share exactly one endpoint.
    Touching,
    /// Parallel lines must be strictly disjoint (no shared point).
    Disjoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Variant {
    pub line_len: LineLen,
    pub touch_mode: TouchMode,
}

impl Variant {
    pub const T4: Variant = Variant {
        line_len: LineLen::Four,
        touch_mode: TouchMode::Touching,
    };
    pub const D4: Variant = Variant {
        line_len: LineLen::Four,
        touch_mode: TouchMode::Disjoint,
    };
    pub const T5: Variant = Variant {
        line_len: LineLen::Five,
        touch_mode: TouchMode::Touching,
    };
    pub const D5: Variant = Variant {
        line_len: LineLen::Five,
        touch_mode: TouchMode::Disjoint,
    };

    /// Line length of the variant (4 or 5). Not a collection length, hence no
    /// `is_empty` companion.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(self) -> u8 {
        self.line_len as u8
    }

    pub fn name(self) -> &'static str {
        match (self.line_len, self.touch_mode) {
            (LineLen::Four, TouchMode::Touching) => "4T",
            (LineLen::Four, TouchMode::Disjoint) => "4D",
            (LineLen::Five, TouchMode::Touching) => "5T",
            (LineLen::Five, TouchMode::Disjoint) => "5D",
        }
    }

    pub const ALL: [Variant; 4] = [Variant::T4, Variant::D4, Variant::T5, Variant::D5];

    /// Parse a variant name, case-insensitively and in either order
    /// (`5T`/`T5`/`5t` …). Returns `None` for anything else.
    pub fn from_name(s: &str) -> Option<Variant> {
        match s.to_ascii_uppercase().as_str() {
            "4T" | "T4" => Some(Variant::T4),
            "4D" | "D4" => Some(Variant::D4),
            "5T" | "T5" => Some(Variant::T5),
            "5D" | "D5" => Some(Variant::D5),
            _ => None,
        }
    }
}

impl Default for Variant {
    fn default() -> Self {
        Self::T5
    }
}
