use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct IuConfig {
    pub(crate) controllers: Vec<IuController>,
}

#[derive(Serialize)]
pub(crate) struct IuController {
    pub(crate) name: String,
    pub(crate) preamble: String,
    pub(crate) postamble: String,
    pub(crate) zones: Vec<IuZone>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) sequences: Vec<IuSequence>,
}

#[derive(Serialize)]
pub(crate) struct IuZone {
    pub(crate) zone_id: String,
    pub(crate) name: String,
    pub(crate) entity_id: String,
}

#[derive(Serialize)]
pub(crate) struct IuSequence {
    pub(crate) name: String,
    pub(crate) sequence_id: String,
    pub(crate) delay: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) schedules: Vec<IuSchedule>,
    pub(crate) zones: Vec<IuSeqZone>,
}

#[derive(Serialize)]
pub(crate) struct IuSchedule {
    pub(crate) name: String,
    pub(crate) time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) weekday: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) day: Option<IuEveryNDays>,
}

#[derive(Serialize)]
pub(crate) struct IuEveryNDays {
    pub(crate) every_n_days: u32,
    pub(crate) start_n_days: String,
}

#[derive(Serialize)]
pub(crate) struct IuSeqZone {
    pub(crate) zone_id: String,
    pub(crate) duration: String,
}
