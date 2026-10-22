use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RuxtTutorialChapter {
    #[default]
    MentalModel,
    Widgets,
    Layout,
    Windows,
    Graphics,
    Architecture,
}

impl RuxtTutorialChapter {
    pub const ALL: &'static [Self] = &[
        Self::MentalModel,
        Self::Widgets,
        Self::Layout,
        Self::Windows,
        Self::Graphics,
        Self::Architecture,
    ];

    pub fn number(&self) -> usize {
        match self {
            Self::MentalModel => 1,
            Self::Widgets => 2,
            Self::Layout => 3,
            Self::Windows => 4,
            Self::Graphics => 5,
            Self::Architecture => 6,
        }
    }

    pub fn pill_title(&self) -> &'static str {
        match self {
            Self::MentalModel => "1. Mental Model",
            Self::Widgets => "2. Widgets",
            Self::Layout => "3. Layouts",
            Self::Windows => "4. Windows",
            Self::Graphics => "5. Graphics",
            Self::Architecture => "6. Architecture",
        }
    }

    pub fn full_title(&self) -> &'static str {
        match self {
            Self::MentalModel => "1. The Immediate-Mode Mental Model",
            Self::Widgets => "2. Essential Widgets & Interactive Controls",
            Self::Layout => "3. Layout Containers, Grids & Responsiveness",
            Self::Windows => "4. Multi-Window Workspace & Touch Ergonomics",
            Self::Graphics => "5. 2D Vector Painting & 3D Graphics Pipelines",
            Self::Architecture => "6. Application Architecture, Persistence & Packaging",
        }
    }

    pub fn next(&self) -> Option<Self> {
        match self {
            Self::MentalModel => Some(Self::Widgets),
            Self::Widgets => Some(Self::Layout),
            Self::Layout => Some(Self::Windows),
            Self::Windows => Some(Self::Graphics),
            Self::Graphics => Some(Self::Architecture),
            Self::Architecture => None,
        }
    }

    pub fn prev(&self) -> Option<Self> {
        match self {
            Self::MentalModel => None,
            Self::Widgets => Some(Self::MentalModel),
            Self::Layout => Some(Self::Widgets),
            Self::Windows => Some(Self::Layout),
            Self::Graphics => Some(Self::Windows),
            Self::Architecture => Some(Self::Graphics),
        }
    }
}
