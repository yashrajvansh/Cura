use serde::{Deserialize, Serialize};

/// The three planes. Each D1, each R2 bucket, one per plane.
///
///   Intent  →  input  — DI  — R2 RI
///   Weave   →  logic  — DIP — R2 RIP
///   Pattern →  output — DP  — R2 RP
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Plane {
    Intent,
    Weave,
    Pattern,
}

impl Plane {
    pub fn all() -> [Plane; 3] {
        [Plane::Intent, Plane::Weave, Plane::Pattern]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Plane::Intent => "intent",
            Plane::Weave => "weave",
            Plane::Pattern => "pattern",
        }
    }

    pub fn r2_bucket(&self) -> &'static str {
        match self {
            Plane::Intent => "cura-ri",
            Plane::Weave => "cura-rip",
            Plane::Pattern => "cura-rp",
        }
    }

    pub fn d1_binding(&self) -> &'static str {
        match self {
            Plane::Intent => "DB_DI",
            Plane::Weave => "DB_DIP",
            Plane::Pattern => "DB_DP",
        }
    }

    pub fn d1_database(&self) -> &'static str {
        match self {
            Plane::Intent => "cura-di",
            Plane::Weave => "cura-dip",
            Plane::Pattern => "cura-dp",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    Lexical,
    Semantic,
    FirmwareSoftware,
    HardwareSoftware,
    Relay,
    Introspective,
    Service,
    Program,
    Sheath,
}

impl ObservationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ObservationKind::Lexical => "lexical",
            ObservationKind::Semantic => "semantic",
            ObservationKind::FirmwareSoftware => "firmware_software",
            ObservationKind::HardwareSoftware => "hardware_software",
            ObservationKind::Relay => "relay",
            ObservationKind::Introspective => "introspective",
            ObservationKind::Service => "service",
            ObservationKind::Program => "program",
            ObservationKind::Sheath => "sheath",
        }
    }
}
