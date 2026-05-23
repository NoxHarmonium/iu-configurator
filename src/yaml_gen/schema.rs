use serde::Serialize;

#[derive(Serialize)]
pub(super) struct IuConfig {
    pub(super) controllers: Vec<IuController>,
}

#[derive(Serialize)]
pub(super) struct IuController {
    pub(super) name: String,
    pub(super) preamble: String,
    pub(super) postamble: String,
    pub(super) zones: Vec<IuZone>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) sequences: Vec<IuSequence>,
}

#[derive(Serialize)]
pub(super) struct IuZone {
    pub(super) zone_id: String,
    pub(super) name: String,
    pub(super) entity_id: String,
}

#[derive(Serialize)]
pub(super) struct IuSequence {
    pub(super) name: String,
    pub(super) sequence_id: String,
    pub(super) delay: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) schedules: Vec<IuSchedule>,
    pub(super) zones: Vec<IuSeqZone>,
}

#[derive(Serialize)]
pub(super) struct IuSchedule {
    pub(super) name: String,
    pub(super) time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) weekday: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) day: Option<IuEveryNDays>,
}

#[derive(Serialize)]
pub(super) struct IuEveryNDays {
    pub(super) every_n_days: u32,
    pub(super) start_n_days: String,
}

#[derive(Serialize)]
pub(super) struct IuSeqZone {
    pub(super) zone_id: String,
    pub(super) duration: String,
}
